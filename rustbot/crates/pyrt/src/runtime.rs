//! Shared execution context for statically translated Rust functions.
use crate::{
    PyExc, PyResult, Value,
    context::RandomContext,
    datetime::Clock,
    object::{CallArgs, Module, NativeFunction},
};
use num_bigint::BigInt;
use std::collections::HashMap;
use std::rc::Rc;

pub type ModuleInit = fn(&mut Runtime, &Value) -> PyResult<()>;

pub struct Runtime {
    pub random: RandomContext,
    pub clock: Clock,
    modules: HashMap<String, Value>,
    initializers: HashMap<String, ModuleInit>,
    builtin_values: HashMap<String, Value>,
    depth: usize,
}

impl Runtime {
    pub fn new(seed: BigInt, clock: &str) -> PyResult<Self> {
        let mut runtime = Self {
            random: RandomContext::new(seed),
            clock: Clock::parse(clock)?,
            modules: HashMap::new(),
            initializers: HashMap::new(),
            builtin_values: HashMap::new(),
            depth: 0,
        };
        crate::builtins::install(&mut runtime);
        crate::stdlib::register(&mut runtime);
        Ok(runtime)
    }
    pub fn register_module(&mut self, name: impl Into<String>, init: ModuleInit) {
        self.initializers.insert(name.into(), init);
    }
    pub fn register_builtin(&mut self, name: impl Into<String>, value: Value) {
        self.builtin_values.insert(name.into(), value);
    }
    pub fn builtin(&mut self, name: &str) -> PyResult<Value> {
        self.builtin_values
            .get(name)
            .cloned()
            .ok_or_else(|| PyExc::name_error(format!("name '{name}' is not defined")))
    }
    pub fn import(&mut self, name: &str) -> PyResult<Value> {
        if let Some(module) = self.modules.get(name) {
            return Ok(module.clone());
        }
        if let Some((parent, _)) = name.rsplit_once('.') {
            self.import(parent)?;
        }
        let init = self
            .initializers
            .get(name)
            .copied()
            .ok_or_else(|| PyExc::import_error(format!("No module named '{name}'")))?;
        let module = Value::Object(Rc::new(Module::new(name)?));
        self.modules.insert(name.into(), module.clone());
        if let Err(e) = init(self, &module) {
            self.modules.remove(name);
            return Err(e);
        }
        if let Some((parent, child)) = name.rsplit_once('.') {
            let parent = self
                .modules
                .get(parent)
                .cloned()
                .ok_or_else(|| PyExc::import_error("parent module not loaded"))?;
            self.setattr(&parent, child, module.clone())?;
        }
        Ok(module)
    }
    pub fn call(&mut self, callable: &Value, args: CallArgs) -> PyResult<Value> {
        if self.depth >= 1000 {
            return Err(PyExc::recursion_error("maximum recursion depth exceeded"));
        }
        self.depth += 1;
        let result = match callable {
            Value::Object(o) => o.call(self, args),
            _ => Err(PyExc::type_error(format!(
                "'{}' object is not callable",
                callable.type_name()
            ))),
        };
        self.depth -= 1;
        result
    }
    pub fn getattr(&mut self, value: &Value, name: &str) -> PyResult<Value> {
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
