//! A shared native/WASM session around the translated `bot.runtime` module.
use super::trace::{self, SourceIndex};
use num_bigint::BigInt;
use pyrt::{
    PyExc, PyResult, Value,
    object::{CallArgs, NativeFunction},
    random::RandomState,
    runtime::{Runtime, SourceSpan},
};
use serde_json::{Value as Json, json};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

/// Own respond spans keyed by module name, obtained from generated metadata or
/// an exact handler/closure lookup. Absent names remain unresolved.
pub type RespondSpans = BTreeMap<String, (String, u32, u32)>;
type Captured = Rc<RefCell<BTreeMap<String, Vec<SourceSpan>>>>;
struct Tracing {
    responds: RespondSpans,
    captured: Captured,
}

/// A loaded bot runtime and its current conversation memory.
pub struct Session {
    pub runtime: Runtime,
    pub memory: Value,
    module: Value,
    loaded_random: RandomState,
    tracing: Option<Tracing>,
}

impl Session {
    /// Load the generated runtime once with the supplied seed and clock.
    pub fn new(seed: BigInt, clock: &str) -> PyResult<Self> {
        let runtime = super::new_runtime(seed, clock)?;
        Self::from_runtime(runtime)
    }

    /// Build a session around an already loaded runtime module. This is useful
    /// to embedders and permits a small fixture without a complete bot registry.
    pub fn from_runtime(mut runtime: Runtime) -> PyResult<Self> {
        let module = runtime.import("bot.runtime")?;
        let _ = runtime.getattr(&module, "turn")?;
        let loaded_random = runtime.random.global().borrow().getstate();
        Ok(Self {
            runtime,
            memory: Value::dict(),
            module,
            loaded_random,
            tracing: None,
        })
    }

    /// Replace conversation memory and restore the post-import global random
    /// state, exactly as the Space's reset does. Other Random instances remain.
    pub fn reset(&mut self) -> PyResult<()> {
        self.runtime
            .random
            .global()
            .borrow_mut()
            .setstate(&self.loaded_random)?;
        self.memory = Value::dict();
        if let Some(tracing) = &self.tracing {
            tracing.captured.borrow_mut().clear();
        }
        Ok(())
    }

    /// Count the loaded modules and handlers. Timing is left to the caller;
    /// responds_found is null until response span metadata has been supplied.
    pub fn load_info(&mut self) -> PyResult<Json> {
        let modules = self
            .runtime
            .getattr(&self.module, "_MODULES")?
            .sequence()?
            .len();
        let handlers = self
            .runtime
            .getattr(&self.module, "_HANDLERS")?
            .sequence()?
            .len();
        Ok(json!({"modules":modules,"handlers":handlers,
            "responds_found":self.tracing.as_ref().map(|trace| trace.responds.len())}))
    }

    /// Instrument each respond handler once; replacing trace resources later
    /// does not wrap the handlers again. Source tables are passed at use time.
    pub fn enable_trace(&mut self, responds: RespondSpans) -> PyResult<()> {
        let handlers = self
            .runtime
            .getattr(&self.module, "_HANDLERS")?
            .sequence()?;
        let mut handler_names = Vec::with_capacity(handlers.len());
        for entry in &handlers {
            let parts = entry.sequence()?;
            if parts.len() != 3 {
                return Err(PyExc::value_error("invalid runtime handler entry"));
            }
            handler_names.push(parts[0].as_str()?.to_owned());
        }
        for (name, (file, first, last)) in &responds {
            if !handler_names.contains(name)
                || file != &format!("bot/modules/{name}.py")
                || *first == 0
                || last < first
            {
                return Err(PyExc::value_error("invalid response span metadata"));
            }
        }
        let captured = if let Some(existing) = &self.tracing {
            existing.captured.clone()
        } else {
            let captured: Captured = Rc::new(RefCell::new(BTreeMap::new()));
            let mut wrapped = Vec::with_capacity(handlers.len());
            for entry in handlers {
                let mut parts = entry.sequence()?;
                if parts.len() != 3 {
                    return Err(PyExc::value_error("invalid runtime handler entry"));
                }
                let name = parts[0].as_str()?.to_owned();
                let original = parts[2].clone();
                let sink = captured.clone();
                let label = name.clone();
                parts[2] = NativeFunction::value(format!("trace_{name}"), move |rt, args| {
                    rt.begin_trace();
                    let result = rt.call(&original, args);
                    let spans = rt.end_trace();
                    sink.borrow_mut().insert(label.clone(), spans);
                    result
                });
                wrapped.push(Value::tuple(parts));
            }
            self.runtime
                .setattr(&self.module, "_HANDLERS", Value::list(wrapped))?;
            captured
        };
        captured.borrow_mut().clear();
        self.tracing = Some(Tracing { responds, captured });
        Ok(())
    }

    /// Discover respond spans through the same wrapper/closure walk as the
    /// Space, then instrument the handlers. Repeated calls do not stack wrappers.
    pub fn enable_trace_auto(&mut self) -> PyResult<()> {
        if self.tracing.is_some() {
            return Ok(());
        }
        let handlers = self
            .runtime
            .getattr(&self.module, "_HANDLERS")?
            .sequence()?;
        let mut responds = RespondSpans::new();
        for entry in handlers {
            let fields = entry.sequence()?;
            if fields.len() != 3 {
                return Err(PyExc::value_error("invalid runtime handler entry"));
            }
            let name = fields[0].as_str()?;
            if let Some(span) = trace::own_respond(&mut self.runtime, name, &fields[2])? {
                responds.insert(name.to_owned(), span);
            }
        }
        self.enable_trace(responds)
    }

    /// Execute one raw runtime turn. The generated bot owns its Python logic.
    pub fn turn(&mut self, text: &str) -> PyResult<Value> {
        if let Some(tracing) = &self.tracing {
            tracing.captured.borrow_mut().clear();
        }
        let turn = self.runtime.getattr(&self.module, "turn")?;
        self.runtime.call(
            &turn,
            CallArgs::new(vec![Value::from(text), self.memory.clone()]),
        )
    }

    /// Execute one turn and return its decision and shaped winner trace.
    /// Returns an explicit error when trace resources have not been configured.
    pub fn turn_with_trace(
        &mut self,
        text: &str,
        sources: &SourceIndex,
    ) -> PyResult<(Value, Json)> {
        if self.tracing.is_none() {
            return Err(PyExc::value_error("trace handlers are not configured"));
        }
        if sources.is_empty() {
            return Err(PyExc::value_error("trace source resources are empty"));
        }
        let decision = self.turn(text)?;
        let reply = decision.dict_get(&Value::from("reply"))?;
        let reply = reply.as_str()?;
        let winner_value = decision.dict_get(&Value::from("module"))?;
        let winner = match &winner_value {
            Value::None => None,
            value => Some(value.as_str()?),
        };
        let tracing = self
            .tracing
            .as_ref()
            .expect("trace configuration checked above");
        let captured = tracing.captured.borrow();
        let spans = winner
            .and_then(|name| captured.get(name))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let respond = winner
            .and_then(|name| tracing.responds.get(name))
            .map(|(file, first, last)| (file.as_str(), *first, *last));
        let shaped = trace::build_trace(reply, winner, spans, respond, sources)?;
        Ok((decision, shaped))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn package(_: &mut Runtime, _: &Value) -> PyResult<()> {
        Ok(())
    }
    fn fixture(rt: &mut Runtime, module: &Value) -> PyResult<()> {
        // Simulate a module import that consumes the global RNG before load().
        let _ = rt.random.global().borrow_mut().random();
        let mut handlers = Vec::new();
        for (name, line) in [("alpha", 10), ("beta", 20)] {
            let handler = NativeFunction::value(name, move |rt, args| {
                args.arity(2, 2)?;
                let file = if name == "alpha" {
                    "bot/modules/alpha.py"
                } else {
                    "bot/modules/beta.py"
                };
                rt.trace_call(file, line, line + 1, "respond");
                rt.trace_call(file, line, line + 1, "respond");
                Ok(Value::from(if name == "alpha" {
                    "  Alpha response with enough letters for source lookup  "
                } else {
                    "Beta response with enough letters for source lookup"
                }))
            });
            handlers.push(Value::tuple(vec![
                Value::from(name),
                Value::from(1i64),
                handler,
            ]));
        }
        rt.setattr(
            module,
            "_MODULES",
            Value::list(vec![Value::None, Value::None]),
        )?;
        rt.setattr(module, "_HANDLERS", Value::list(handlers))?;
        let turn = NativeFunction::value("turn", |rt, args| {
            args.arity(2, 2)?;
            let text = args.positional[0].as_str()?;
            let memory = &args.positional[1];
            let module = rt.import("bot.runtime")?;
            let handlers = rt.getattr(&module, "_HANDLERS")?.sequence()?;
            let mut choices = Vec::new();
            for item in handlers {
                let fields = item.sequence()?;
                let result = rt.call(
                    &fields[2],
                    CallArgs::new(vec![Value::from(text), memory.clone()]),
                )?;
                choices.push((fields[0].clone(), result));
            }
            let choice = if text == "beta" { 1 } else { 0 };
            memory.dict_set(Value::from("last"), Value::from(text))?;
            let draw = rt.random.global().borrow_mut().random();
            let result = Value::dict();
            result.dict_set(Value::from("module"), choices[choice].0.clone())?;
            result.dict_set(Value::from("reply"), choices[choice].1.clone())?;
            result.dict_set(Value::from("draw"), Value::Float(draw))?;
            Ok(result)
        });
        rt.setattr(module, "turn", turn)
    }
    fn session() -> Session {
        let mut rt = Runtime::new(BigInt::from(42), "2026-06-15T12:00:00").unwrap();
        rt.register_module("bot", package);
        rt.register_module("bot.runtime", fixture);
        Session::from_runtime(rt).unwrap()
    }
    fn resources() -> SourceIndex {
        let text = "alpharesponsewithenoughlettersforsourcelookup";
        SourceIndex::from_json(
            format!(r#"{{"bot/modules/alpha.py":"header{text}"}}"#).as_bytes(),
            format!(r#"{{"bot/modules/alpha.py":[6,{}]}}"#, text.len()).as_bytes(),
        )
        .unwrap()
    }
    #[test]
    fn raw_turn_and_reset_restore_post_import_random_state() {
        let mut session = session();
        let unseeded = session.runtime.random.new_unseeded();
        let old_memory = session.memory.clone();
        let first = session.turn("raw  text").unwrap();
        assert_eq!(
            session
                .memory
                .dict_get(&Value::from("last"))
                .unwrap()
                .as_str()
                .unwrap(),
            "raw  text"
        );
        let draw = first
            .dict_get(&Value::from("draw"))
            .unwrap()
            .as_float()
            .unwrap();
        unseeded.borrow_mut().random();
        let count = unseeded.borrow().words_consumed();
        session.reset().unwrap();
        assert!(!session.memory.is_identical(&old_memory));
        assert!(session.memory.dict_get(&Value::from("last")).is_err());
        assert_eq!(unseeded.borrow().words_consumed(), count);
        let next = session
            .turn("x")
            .unwrap()
            .dict_get(&Value::from("draw"))
            .unwrap()
            .as_float()
            .unwrap();
        assert_eq!(draw.to_bits(), next.to_bits());
    }
    #[test]
    fn trace_instruments_each_handler_and_selects_only_the_winner() {
        let mut session = session();
        let resources = resources();
        assert!(session.turn_with_trace("alpha", &resources).is_err());
        let mut responds = RespondSpans::new();
        responds.insert("alpha".into(), ("bot/modules/alpha.py".into(), 10, 11));
        responds.insert("beta".into(), ("bot/modules/beta.py".into(), 20, 21));
        session.enable_trace(responds.clone()).unwrap();
        let empty = SourceIndex::from_json(b"{}", b"{}").unwrap();
        assert!(session.turn_with_trace("no-change", &empty).is_err());
        assert!(session.memory.dict_get(&Value::from("last")).is_err());
        assert_eq!(
            session.load_info().unwrap(),
            json!({"modules":2,"handlers":2,"responds_found":2})
        );
        let (decision, trace) = session.turn_with_trace("alpha", &resources).unwrap();
        assert_eq!(
            decision
                .dict_get(&Value::from("module"))
                .unwrap()
                .as_str()
                .unwrap(),
            "alpha"
        );
        assert_eq!(trace["files"], json!(["bot/modules/alpha.py"]));
        assert_eq!(
            trace["calls"]["bot/modules/alpha.py"],
            json!([[10, 11, "respond"]])
        );
        assert_eq!(trace["source"], "bot/modules/alpha.py");
        assert_eq!(trace["source_line"], 2);
        assert_eq!(trace["respond"], json!(["bot/modules/alpha.py", 10, 11]));
        // Reconfiguration updates resources without stacking wrappers.
        session.enable_trace(responds).unwrap();
        let (_, trace) = session.turn_with_trace("beta", &resources).unwrap();
        assert_eq!(trace["files"], json!(["bot/modules/beta.py"]));
        assert_eq!(
            trace["calls"]["bot/modules/beta.py"],
            json!([[20, 21, "respond"]])
        );
    }
}
