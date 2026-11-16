//! Shared execution context for statically translated Rust functions.
use crate::{
    PyExc, PyResult, Value,
    context::RandomContext,
    datetime::Clock,
    object::{CallArgs, Module, NativeFunction},
};
use num_bigint::BigInt;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

pub type ModuleInit = fn(&mut Runtime, &Value) -> PyResult<()>;
type Namespace = RefCell<Vec<(Value, Value)>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceSpan {
    pub file: &'static str,
    pub first: u32,
    pub last: u32,
    pub qualname: &'static str,
}

struct RuntimeState {
    modules: RefCell<HashMap<String, Value>>,
    initializers: RefCell<HashMap<String, ModuleInit>>,
    builtin_values: RefCell<HashMap<String, Value>>,
    depth: Cell<usize>,
    exceptions: RefCell<Vec<PyExc>>,
    tracing: Cell<bool>,
    calls: RefCell<Vec<SourceSpan>>,
    namespaces: RefCell<Vec<Weak<Namespace>>>,
}

impl Drop for RuntimeState {
    fn drop(&mut self) {
        let ids = self
            .namespaces
            .get_mut()
            .iter()
            .map(|ns| ns.as_ptr() as usize)
            .collect();
        crate::frame::release_namespaces(&ids);
        for namespace in self.namespaces.get_mut().iter().filter_map(Weak::upgrade) {
            let contents = std::mem::take(&mut *namespace.borrow_mut());
            drop(contents);
        }
        self.modules.get_mut().clear();
        self.builtin_values.get_mut().clear();
    }
}

#[derive(Clone)]
pub struct Runtime {
    pub random: RandomContext,
    pub clock: Clock,
    state: Rc<RuntimeState>,
}

#[derive(Clone)]
pub struct RuntimeHandle {
    random: RandomContext,
    clock: Clock,
    state: Weak<RuntimeState>,
}

impl std::fmt::Debug for RuntimeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RuntimeHandle")
    }
}

impl RuntimeHandle {
    pub fn upgrade(&self) -> PyResult<Runtime> {
        Ok(Runtime {
            random: self.random.clone(),
            clock: self.clock,
            state: self
                .state
                .upgrade()
                .ok_or_else(|| PyExc::runtime_error("execution context no longer exists"))?,
        })
    }
}

impl Runtime {
    pub fn new(seed: BigInt, clock: &str) -> PyResult<Self> {
        let mut runtime = Self {
            random: RandomContext::new(seed),
            clock: Clock::parse(clock)?,
            state: Rc::new(RuntimeState {
                modules: RefCell::new(HashMap::new()),
                initializers: RefCell::new(HashMap::new()),
                builtin_values: RefCell::new(HashMap::new()),
                depth: Cell::new(0),
                exceptions: RefCell::new(Vec::new()),
                tracing: Cell::new(false),
                calls: RefCell::new(Vec::new()),
                namespaces: RefCell::new(Vec::new()),
            }),
        };
        crate::builtins::install(&mut runtime);
        crate::exceptions::install(&mut runtime);
        crate::stdlib::register(&mut runtime);
        crate::stdlib_extra::register(&mut runtime);
        crate::numeric_objects::register(&mut runtime);
        Ok(runtime)
    }
    pub fn register_module(&mut self, name: impl Into<String>, init: ModuleInit) {
        self.state
            .initializers
            .borrow_mut()
            .insert(name.into(), init);
    }
    pub fn register_builtin(&mut self, name: impl Into<String>, value: Value) {
        self.state
            .builtin_values
            .borrow_mut()
            .insert(name.into(), value);
    }
    pub fn builtin(&mut self, name: &str) -> PyResult<Value> {
        self.state
            .builtin_values
            .borrow()
            .get(name)
            .cloned()
            .ok_or_else(|| PyExc::name_error(format!("name '{name}' is not defined")))
    }
    pub fn import(&mut self, name: &str) -> PyResult<Value> {
        if let Some(module) = self.state.modules.borrow().get(name).cloned() {
            return Ok(module);
        }
        if let Some((parent, _)) = name.rsplit_once('.') {
            self.import(parent)?;
            // The package initializer may have imported this requested child.
            // Reuse that module instead of executing its initializer twice.
            if let Some(module) = self.state.modules.borrow().get(name).cloned() {
                return Ok(module);
            }
        }
        let init = self
            .state
            .initializers
            .borrow()
            .get(name)
            .copied()
            .ok_or_else(|| PyExc::import_error(format!("No module named '{name}'")))?;
        let descriptor = Module::new(name)?;
        if let Value::Dict(namespace) = &descriptor.globals {
            self.state
                .namespaces
                .borrow_mut()
                .push(Rc::downgrade(namespace));
        }
        let module = Value::Object(Rc::new(descriptor));
        let is_package = self
            .state
            .initializers
            .borrow()
            .keys()
            .any(|key| key.starts_with(&format!("{name}.")));
        if is_package {
            self.setattr(&module, "__package__", Value::from(name))?;
            self.setattr(&module, "__path__", Value::list(vec![Value::from(name)]))?;
        }
        self.state
            .modules
            .borrow_mut()
            .insert(name.into(), module.clone());
        if let Err(e) = init(self, &module) {
            self.state.modules.borrow_mut().remove(name);
            return Err(e);
        }
        if let Some((parent, child)) = name.rsplit_once('.') {
            let parent = self
                .state
                .modules
                .borrow()
                .get(parent)
                .cloned()
                .ok_or_else(|| PyExc::import_error("parent module not loaded"))?;
            self.setattr(&parent, child, module.clone())?;
        }
        Ok(module)
    }
    pub fn module_names(&self, prefix: &str) -> Vec<(String, bool)> {
        let modules = self.state.initializers.borrow();
        let prefix = if prefix.is_empty() {
            String::new()
        } else {
            format!("{prefix}.")
        };
        let mut names: Vec<_> = modules
            .keys()
            .filter_map(|name| {
                name.strip_prefix(&prefix)
                    .filter(|name| !name.contains('.'))
                    .map(|child| {
                        (
                            child.to_owned(),
                            modules.keys().any(|m| m.starts_with(&format!("{name}."))),
                        )
                    })
            })
            .collect();
        names.sort();
        names
    }
    pub fn call(&mut self, callable: &Value, args: CallArgs) -> PyResult<Value> {
        let depth = self.state.depth.get();
        if depth >= 1000 {
            return Err(PyExc::recursion_error("maximum recursion depth exceeded"));
        }
        self.state.depth.set(depth + 1);
        let result = match callable {
            Value::Object(o) => o.call(self, args),
            _ => Err(PyExc::type_error(format!(
                "'{}' object is not callable",
                callable.type_name()
            ))),
        };
        self.state.depth.set(depth);
        result
    }
    pub fn handle(&self) -> RuntimeHandle {
        RuntimeHandle {
            random: self.random.clone(),
            clock: self.clock,
            state: Rc::downgrade(&self.state),
        }
    }
    pub fn import_from(&mut self, module: &str, name: &str) -> PyResult<Value> {
        let parent = self.import(module)?;
        match self.getattr(&parent, name) {
            Ok(value) => Ok(value),
            Err(e) if e.kind == "AttributeError" => self.import(&format!("{module}.{name}")),
            Err(e) => Err(e),
        }
    }
    pub fn active_exception(&self) -> PyResult<PyExc> {
        self.state
            .exceptions
            .borrow()
            .last()
            .cloned()
            .ok_or_else(|| PyExc::runtime_error("No active exception to reraise"))
    }
    pub fn with_exception<T>(
        &mut self,
        exception: PyExc,
        body: impl FnOnce(&mut Runtime) -> PyResult<T>,
    ) -> PyResult<T> {
        struct Guard(Rc<RuntimeState>);
        impl Drop for Guard {
            fn drop(&mut self) {
                self.0.exceptions.borrow_mut().pop();
            }
        }
        self.state.exceptions.borrow_mut().push(exception);
        let _guard = Guard(self.state.clone());
        body(self)
    }
    pub fn trace_call(
        &mut self,
        file: &'static str,
        first: u32,
        last: u32,
        qualname: &'static str,
    ) {
        if self.state.tracing.get() {
            self.state.calls.borrow_mut().push(SourceSpan {
                file,
                first,
                last,
                qualname,
            });
        }
    }
    pub fn begin_trace(&mut self) {
        self.state.calls.borrow_mut().clear();
        self.state.tracing.set(true);
    }
    pub fn end_trace(&mut self) -> Vec<SourceSpan> {
        self.state.tracing.set(false);
        std::mem::take(&mut *self.state.calls.borrow_mut())
    }
    pub fn getattr(&mut self, value: &Value, name: &str) -> PyResult<Value> {
        if matches!(value, Value::Date(_) | Value::DateTime(_))
            && let Some(value) = crate::stdlib::temporal(value)
        {
            return self.getattr(&crate::stdlib::temporal_value(value), name);
        }
        if let Value::Object(o) = value {
            return o.get_attr(self, name);
        }
        if name == "__class__" {
            return self.builtin(value.type_name());
        }
        if matches!(value, Value::Int(_) | Value::Bool(_)) {
            return match name {
                "numerator" => Ok(Value::Int(value.as_int()?)),
                "denominator" => Ok(Value::from(1_i64)),
                "real" => Ok(value.clone()),
                "imag" => Ok(Value::from(0_i64)),
                _ => Err(PyExc::attribute_error(format!(
                    "int has no attribute '{name}'"
                ))),
            };
        }
        if !has_method(value, name) {
            return Err(PyExc::attribute_error(format!(
                "'{}' object has no attribute '{name}'",
                value.type_name()
            )));
        }
        let receiver = value.clone();
        let method = name.to_owned();
        Ok(NativeFunction::value(name, move |rt, mut args| {
            if matches!(receiver, Value::Str(_))
                && matches!(method.as_str(), "format" | "format_map")
            {
                let kwargs = Value::dict();
                if method == "format_map" {
                    args.reject_keywords()?;
                    args.arity(1, 1)?;
                    return Ok(Value::from(crate::formatting::template(
                        receiver.as_str()?,
                        &[],
                        &args.positional[0],
                    )?));
                }
                for (name, value) in args.keywords {
                    kwargs.dict_set(Value::from(name), value)?;
                }
                return Ok(Value::from(crate::formatting::template(
                    receiver.as_str()?,
                    &args.positional,
                    &kwargs,
                )?));
            }
            if let Value::List(items) = &receiver
                && method == "sort"
            {
                args.arity(0, 0)?;
                let mut key = None;
                let mut reverse = false;
                for (name, value) in &args.keywords {
                    match name.as_str() {
                        "key" => key = Some(value.clone()),
                        "reverse" => reverse = value.is_truthy(),
                        _ => return Err(PyExc::type_error("invalid keyword argument for sort()")),
                    }
                }
                let original = std::mem::take(&mut *items.borrow_mut());
                let backup = original.clone();
                match crate::builtins::sort(rt, original, key, reverse) {
                    Ok(sorted) => {
                        let modified = !items.borrow().is_empty();
                        *items.borrow_mut() = sorted;
                        if modified {
                            return Err(PyExc::value_error("list modified during sort"));
                        }
                        return Ok(Value::None);
                    }
                    Err(e) => {
                        *items.borrow_mut() = backup;
                        return Err(e);
                    }
                }
            }
            if let Value::Dict(_) = &receiver
                && method == "update"
            {
                args.arity(0, 1)?;
                crate::containers::call_method(&receiver, "update", &args.positional)?;
                for (name, value) in args.keywords {
                    receiver.dict_set(Value::from(name), value)?;
                }
                return Ok(Value::None);
            }
            if matches!(receiver, Value::Str(_)) {
                bind_string_keywords(&method, &mut args)?;
            }
            args.reject_keywords()?;
            if matches!(
                method.as_str(),
                "extend"
                    | "join"
                    | "update"
                    | "union"
                    | "intersection"
                    | "difference"
                    | "symmetric_difference"
                    | "difference_update"
                    | "intersection_update"
                    | "symmetric_difference_update"
                    | "isdisjoint"
                    | "issubset"
                    | "issuperset"
            ) {
                for value in &mut args.positional {
                    if matches!(value, Value::Object(_)) {
                        *value = Value::list(rt.collect(value)?);
                    }
                }
            }
            match &receiver {
                Value::Str(s) => crate::strings::call_method(s.as_str(), &method, &args.positional),
                Value::List(_)
                | Value::Tuple(_)
                | Value::Dict(_)
                | Value::Set(_)
                | Value::FrozenSet(_) => {
                    crate::containers::call_method(&receiver, &method, &args.positional)
                }
                Value::Float(x) if method == "is_integer" => {
                    args.arity(0, 0)?;
                    Ok(Value::Bool(x.is_finite() && x.fract() == 0.0))
                }
                _ => Err(PyExc::attribute_error(format!(
                    "'{}' object has no attribute '{method}'",
                    receiver.type_name()
                ))),
            }
        }))
    }
    pub fn setattr(&mut self, value: &Value, name: &str, assigned: Value) -> PyResult<()> {
        if let Value::Object(o) = value {
            o.set_attr(name, assigned)
        } else {
            Err(PyExc::attribute_error("object has no writable attributes"))
        }
    }
    pub fn iter(&mut self, value: &Value) -> PyResult<Value> {
        if let Value::Object(o) = value {
            if o.is_iterator() {
                return Ok(value.clone());
            } else {
                return o.iter(self);
            }
        }
        crate::protocols::iter(value, false)
    }
    pub fn next(&mut self, iterator: &Value) -> PyResult<Option<Value>> {
        match iterator {
            Value::Object(o) => o.next(self),
            _ => Err(PyExc::type_error("object is not an iterator")),
        }
    }
    pub fn collect(&mut self, value: &Value) -> PyResult<Vec<Value>> {
        let iter = self.iter(value)?;
        let mut result = Vec::new();
        while let Some(value) = self.next(&iter)? {
            result.push(value);
        }
        Ok(result)
    }
}

fn has_method(value: &Value, name: &str) -> bool {
    method_names(value).contains(&name)
}

pub fn method_names(value: &Value) -> &'static [&'static str] {
    match value {
        Value::Str(_) => &[
            "lower",
            "upper",
            "title",
            "capitalize",
            "casefold",
            "swapcase",
            "strip",
            "lstrip",
            "rstrip",
            "isalpha",
            "isdigit",
            "isdecimal",
            "isnumeric",
            "isalnum",
            "isspace",
            "isupper",
            "islower",
            "istitle",
            "isprintable",
            "isidentifier",
            "isascii",
            "splitlines",
            "split",
            "rsplit",
            "partition",
            "rpartition",
            "find",
            "rfind",
            "index",
            "rindex",
            "count",
            "startswith",
            "endswith",
            "removeprefix",
            "removesuffix",
            "zfill",
            "center",
            "ljust",
            "rjust",
            "expandtabs",
            "join",
            "replace",
            "format",
            "format_map",
            "encode",
            "translate",
            "__contains__",
            "__getitem__",
            "__mul__",
        ],
        Value::List(_) => &[
            "append", "extend", "insert", "remove", "pop", "clear", "copy", "count", "index",
            "reverse", "sort",
        ],
        Value::Tuple(_) => &["count", "index"],
        Value::Dict(_) => &[
            "get",
            "setdefault",
            "update",
            "pop",
            "popitem",
            "keys",
            "values",
            "items",
            "clear",
            "copy",
            "fromkeys",
        ],
        Value::Set(_) => &[
            "add",
            "discard",
            "remove",
            "pop",
            "clear",
            "copy",
            "update",
            "union",
            "intersection",
            "difference",
            "symmetric_difference",
            "difference_update",
            "intersection_update",
            "symmetric_difference_update",
            "isdisjoint",
            "issubset",
            "issuperset",
        ],
        Value::FrozenSet(_) => &[
            "copy",
            "union",
            "intersection",
            "difference",
            "symmetric_difference",
            "isdisjoint",
            "issubset",
            "issuperset",
        ],
        Value::Float(_) => &["is_integer"],
        _ => &[],
    }
}

fn bind_string_keywords(method: &str, args: &mut CallArgs) -> PyResult<()> {
    let (names, defaults): (&[&str], Vec<Value>) = match method {
        "split" | "rsplit" => (&["sep", "maxsplit"], vec![Value::None, Value::from(-1_i64)]),
        "splitlines" => (&["keepends"], vec![Value::Bool(false)]),
        "expandtabs" => (&["tabsize"], vec![Value::from(8_i64)]),
        "replace" => (
            &["", "", "count"],
            vec![Value::None, Value::None, Value::from(-1_i64)],
        ),
        _ => return Ok(()),
    };
    let supplied = args.positional.len();
    let mut seen = std::collections::HashSet::new();
    for (name, value) in std::mem::take(&mut args.keywords) {
        let i = names
            .iter()
            .position(|n| !n.is_empty() && *n == name)
            .ok_or_else(|| PyExc::type_error("invalid string method keyword"))?;
        if i < supplied || !seen.insert(i) {
            return Err(PyExc::type_error("multiple values for argument"));
        }
        while args.positional.len() <= i {
            args.positional
                .push(defaults[args.positional.len()].clone());
        }
        args.positional[i] = value;
    }
    Ok(())
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new(0.into(), "2026-06-15T12:00:00").expect("default runtime")
    }
}
