//! Native adapters around generated modules; Python logic stays in its mirrors.
pub mod engine;
mod generated;
pub mod trace;
use num_bigint::BigInt;
use pyrt::{PyExc, PyResult, Value, object::CallArgs, runtime::Runtime};

pub fn new_runtime(seed: BigInt, clock: &str) -> PyResult<Runtime> {
    let mut runtime = Runtime::new(seed, clock)?;
    generated::register(&mut runtime);
    runtime.import("bot")?;
    Ok(runtime)
}

pub fn call(runtime: &mut Runtime, name: &str, args: CallArgs) -> PyResult<Value> {
    let function = resolve(runtime, name)?;
    runtime.call(&function, args)
}

pub fn resolve(runtime: &mut Runtime, name: &str) -> PyResult<Value> {
    let (module, attribute) = name
        .rsplit_once('.')
        .ok_or_else(|| PyExc::value_error("expected a dotted function name"))?;
    let module = runtime.import(module)?;
    runtime.getattr(&module, attribute)
}

pub fn resolve_hook(runtime: &mut Runtime, name: &str, hook: &str) -> PyResult<Value> {
    let module = runtime.import("bot.runtime")?;
    if name == "arbiter" && hook == "choose" {
        return runtime.getattr(&module, "_ARBITER");
    }
    let attribute = match hook {
        "observe" => "_OBSERVERS",
        "respond" => "_HANDLERS",
        "after" => "_AFTER",
        _ => return Err(PyExc::value_error("unknown hook")),
    };
    for entry in runtime.getattr(&module, attribute)?.sequence()? {
        let fields = entry.sequence()?;
        if fields
            .first()
            .is_some_and(|field| field.as_str().ok() == Some(name))
        {
            return fields
                .get(2)
                .cloned()
                .ok_or_else(|| PyExc::index_error("invalid hook entry"));
        }
    }
    Err(PyExc::key_error(format!("{name}.{hook}")))
}
