//! Extensible Python objects and native Rust callables used by translated code.
use crate::{PyExc, PyResult, Value, runtime::Runtime};
use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub struct CallArgs {
    pub positional: Vec<Value>,
    pub keywords: Vec<(String, Value)>,
}

impl CallArgs {
    pub fn new(positional: Vec<Value>) -> Self {
        Self {
            positional,
            keywords: Vec::new(),
        }
    }
    pub fn keyword(&mut self, name: String, value: Value) -> PyResult<()> {
        if self.keywords.iter().any(|(n, _)| *n == name) {
            return Err(PyExc::type_error(format!(
                "multiple values for keyword argument '{name}'"
            )));
        }
        self.keywords.push((name, value));
        Ok(())
    }
    pub fn extend_keywords(&mut self, mapping: &Value) -> PyResult<()> {
        let Value::Dict(items) = mapping else {
            return Err(PyExc::type_error("argument after ** must be a mapping"));
        };
        for (name, value) in items.borrow().iter() {
            self.keyword(name.as_str()?.to_owned(), value.clone())?;
        }
        Ok(())
    }
    pub fn reject_keywords(&self) -> PyResult<()> {
        if self.keywords.is_empty() {
            Ok(())
        } else {
            Err(PyExc::type_error("function takes no keyword arguments"))
        }
    }
    pub fn arity(&self, min: usize, max: usize) -> PyResult<()> {
        if (min..=max).contains(&self.positional.len()) {
            Ok(())
        } else {
            Err(PyExc::type_error("incorrect number of arguments"))
        }
    }
}

pub trait PyObject: fmt::Debug {
    fn as_any(&self) -> &dyn Any;
    fn type_name(&self) -> &str;
    fn str(&self) -> PyResult<String> {
        self.repr()
    }
    fn format(&self, spec: &str) -> PyResult<String> {
        if spec.is_empty() {
            self.str()
        } else {
            Err(PyExc::type_error(format!(
                "unsupported format string passed to {}.__format__",
                self.type_name()
            )))
        }
    }
    fn canonical(&self) -> PyResult<Option<serde_json::Value>> {
        Ok(None)
    }
    fn repr(&self) -> PyResult<String> {
        Ok(format!("<{} object at 0x0>", self.type_name()))
    }
    fn is_truthy(&self) -> bool {
        true
    }
    fn equals(&self, _other: &Value) -> PyResult<bool> {
        Ok(false)
    }
    fn hash(&self) -> PyResult<i64> {
        Err(PyExc::type_error(format!(
            "unhashable type: '{}'",
            self.type_name()
        )))
    }
    fn sequence(&self) -> PyResult<Vec<Value>> {
        Err(PyExc::type_error(format!(
            "'{}' object is not iterable",
            self.type_name()
        )))
    }
    fn len(&self) -> PyResult<usize> {
        Err(PyExc::type_error(format!(
            "object of type '{}' has no len()",
            self.type_name()
        )))
    }
    fn is_empty(&self) -> PyResult<bool> {
        Ok(self.len()? == 0)
    }
    fn get_item(&self, _index: &Value) -> PyResult<Value> {
        Err(PyExc::type_error("object is not subscriptable"))
    }
    fn contains(&self, item: &Value) -> PyResult<bool> {
        for value in self.sequence()? {
            if value.is_identical(item) || value.equals(item)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn iter(&self, _runtime: &mut Runtime) -> PyResult<Value> {
        Ok(IteratorObject::from_values(self.sequence()?))
    }
    fn get_attr(&self, _runtime: &mut Runtime, name: &str) -> PyResult<Value> {
        Err(PyExc::attribute_error(format!(
            "'{}' object has no attribute '{name}'",
            self.type_name()
        )))
    }
    fn set_attr(&self, _name: &str, _value: Value) -> PyResult<()> {
        Err(PyExc::attribute_error("object has no writable attributes"))
    }
    fn call(&self, _runtime: &mut Runtime, _args: CallArgs) -> PyResult<Value> {
        Err(PyExc::type_error(format!(
            "'{}' object is not callable",
            self.type_name()
        )))
    }
    fn is_callable(&self) -> bool {
        false
    }
    fn instance_check(&self, _value: &Value) -> Option<bool> {
        None
    }
    fn next(&self, _runtime: &mut Runtime) -> PyResult<Option<Value>> {
        Err(PyExc::type_error("object is not an iterator"))
    }
    fn is_iterator(&self) -> bool {
        false
    }
}

type NativeBody = dyn Fn(&mut Runtime, CallArgs) -> PyResult<Value>;

#[derive(Debug)]
pub struct PlainObject;
impl PyObject for PlainObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "object"
    }
    fn hash(&self) -> PyResult<i64> {
        let h = ((self as *const Self as usize).rotate_right(4)) as i64;
        Ok(if h == -1 { -2 } else { h })
    }
}

pub struct NativeFunction {
    pub name: String,
    pub qualname: String,
    pub module: String,
    pub attrs: Value,
    origin: Option<FunctionOrigin>,
    body: Rc<NativeBody>,
}

/// Private-to-the-runtime Python code and closure provenance. These fields do
/// not become Python-visible attributes and never include module globals as
/// closure cells. Free-variable order is CPython's co_freevars tuple order.
#[derive(Clone, Debug)]
pub struct FunctionOrigin {
    pub span: crate::runtime::SourceSpan,
    pub closure: Rc<crate::frame::Frame>,
    pub freevars: Vec<String>,
}

impl fmt::Debug for NativeFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("NativeFunction")
            .field(&self.qualname)
            .finish()
    }
}

impl NativeFunction {
    #[inline(never)]
    fn create(
        name: String,
        qualname: String,
        module: String,
        attrs: Value,
        origin: Option<FunctionOrigin>,
        body: Rc<NativeBody>,
    ) -> Value {
        Value::Object(Rc::new(Self {
            name,
            qualname,
            module,
            attrs,
            origin,
            body,
        }))
    }

    pub fn value(
        name: impl Into<String>,
        body: impl Fn(&mut Runtime, CallArgs) -> PyResult<Value> + 'static,
    ) -> Value {
        let name = name.into();
        Self::qualified(name.clone(), name, "builtins".into(), body)
    }
    pub fn qualified(
        name: String,
        qualname: String,
        module: String,
        body: impl Fn(&mut Runtime, CallArgs) -> PyResult<Value> + 'static,
    ) -> Value {
        Self::create(name, qualname, module, Value::dict(), None, Rc::new(body))
    }
    pub fn qualified_with_attrs(
        name: String,
        qualname: String,
        module: String,
        attrs: Value,
        body: impl Fn(&mut Runtime, CallArgs) -> PyResult<Value> + 'static,
    ) -> Value {
        Self::create(name, qualname, module, attrs, None, Rc::new(body))
    }

    pub fn qualified_with_origin(
        name: String,
        qualname: String,
        module: String,
        attrs: Value,
        origin: FunctionOrigin,
        body: impl Fn(&mut Runtime, CallArgs) -> PyResult<Value> + 'static,
    ) -> Value {
        Self::create(name, qualname, module, attrs, Some(origin), Rc::new(body))
    }

    pub fn source_span(&self) -> Option<crate::runtime::SourceSpan> {
        self.origin.as_ref().map(|origin| origin.span)
    }

    /// Read live cells rather than taking a definition-time snapshot: rebinding
    /// a captured callable after decoration must affect the respond search.
    /// Empty cells behave like cell_contents raising ValueError and are skipped.
    pub fn closure_values(&self, runtime: &mut Runtime) -> Vec<Value> {
        self.origin.as_ref().map_or_else(Vec::new, |origin| {
            origin
                .freevars
                .iter()
                .filter_map(|name| origin.closure.load(runtime, name).ok())
                .collect()
        })
    }
}

impl PyObject for NativeFunction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "function"
    }
    fn hash(&self) -> PyResult<i64> {
        let h = ((self as *const Self as usize).rotate_right(4)) as i64;
        Ok(if h == -1 { -2 } else { h })
    }
    fn repr(&self) -> PyResult<String> {
        Ok(format!("<function {} at 0x0>", self.qualname))
    }
    fn call(&self, runtime: &mut Runtime, args: CallArgs) -> PyResult<Value> {
        (self.body)(runtime, args)
    }
    fn is_callable(&self) -> bool {
        true
    }
    fn get_attr(&self, _runtime: &mut Runtime, name: &str) -> PyResult<Value> {
        if name == "__dict__" {
            return Ok(self.attrs.clone());
        }
        if let Ok(value) = self.attrs.dict_get(&Value::from(name)) {
            return Ok(value);
        }
        match name {
            "__name__" => Ok(Value::from(self.name.clone())),
            "__qualname__" => Ok(Value::from(self.qualname.clone())),
            "__module__" => Ok(Value::from(self.module.clone())),
            "__doc__" | "__defaults__" | "__kwdefaults__" => Ok(Value::None),
            "__annotations__" => {
                let value = Value::dict();
                self.attrs.dict_set(Value::from(name), value.clone())?;
                Ok(value)
            }
            _ => Err(PyExc::attribute_error("function attribute not found")),
        }
    }
    fn set_attr(&self, name: &str, value: Value) -> PyResult<()> {
        if matches!(name, "__name__" | "__qualname__") && !matches!(value, Value::Str(_)) {
            return Err(PyExc::type_error(
                "function name must be set to a string object",
            ));
        }
        if name == "__defaults__" && !matches!(value, Value::Tuple(_) | Value::None) {
            return Err(PyExc::type_error(
                "__defaults__ must be set to a tuple object",
            ));
        }
        if matches!(name, "__kwdefaults__" | "__annotations__")
            && !matches!(value, Value::Dict(_) | Value::None)
        {
            return Err(PyExc::type_error("function attribute must be a dict"));
        }
        self.attrs.dict_set(Value::from(name), value)
    }
}

type InstanceCheck = dyn Fn(&Value) -> bool;

pub struct NativeType {
    pub name: String,
    pub module: String,
    pub attrs: Value,
    body: Rc<NativeBody>,
    check: Rc<InstanceCheck>,
}

impl fmt::Debug for NativeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("NativeType").field(&self.name).finish()
    }
}

impl NativeType {
    pub fn value(
        name: impl Into<String>,
        module: impl Into<String>,
        check: impl Fn(&Value) -> bool + 'static,
        body: impl Fn(&mut Runtime, CallArgs) -> PyResult<Value> + 'static,
    ) -> Value {
        Self::with_attrs(name, module, Value::dict(), check, body)
    }
    pub fn with_attrs(
        name: impl Into<String>,
        module: impl Into<String>,
        attrs: Value,
        check: impl Fn(&Value) -> bool + 'static,
        body: impl Fn(&mut Runtime, CallArgs) -> PyResult<Value> + 'static,
    ) -> Value {
        Value::Object(Rc::new(Self {
            name: name.into(),
            module: module.into(),
            attrs,
            body: Rc::new(body),
            check: Rc::new(check),
        }))
    }
}

impl PyObject for NativeType {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "type"
    }
    fn hash(&self) -> PyResult<i64> {
        let h = ((self as *const Self as usize).rotate_right(4)) as i64;
        Ok(if h == -1 { -2 } else { h })
    }
    fn repr(&self) -> PyResult<String> {
        Ok(if self.module == "builtins" {
            format!("<class '{}'>", self.name)
        } else {
            format!("<class '{}.{}'>", self.module, self.name)
        })
    }
    fn is_callable(&self) -> bool {
        true
    }
    fn instance_check(&self, value: &Value) -> Option<bool> {
        Some((self.check)(value))
    }
    fn call(&self, rt: &mut Runtime, args: CallArgs) -> PyResult<Value> {
        (self.body)(rt, args)
    }
    fn get_attr(&self, _rt: &mut Runtime, name: &str) -> PyResult<Value> {
        match name {
            "__name__" | "__qualname__" => Ok(Value::from(self.name.clone())),
            "__module__" => Ok(Value::from(self.module.clone())),
            "__dict__" => Ok(self.attrs.clone()),
            _ => self.attrs.dict_get(&Value::from(name)).map_err(|_| {
                PyExc::attribute_error(format!("type '{}' has no attribute '{name}'", self.name))
            }),
        }
    }
    fn set_attr(&self, name: &str, value: Value) -> PyResult<()> {
        self.attrs.dict_set(Value::from(name), value)
    }
}

#[derive(Debug)]
pub struct Module {
    pub name: String,
    pub globals: Value,
}

impl Module {
    pub fn new(name: impl Into<String>) -> PyResult<Self> {
        let name = name.into();
        let globals = Value::dict();
        globals.dict_set(Value::from("__name__"), Value::from(name.clone()))?;
        globals.dict_set(
            Value::from("__package__"),
            Value::from(name.rsplit_once('.').map_or("", |(p, _)| p)),
        )?;
        Ok(Self { name, globals })
    }
}

impl PyObject for Module {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "module"
    }
    fn repr(&self) -> PyResult<String> {
        Ok(format!("<module '{}'>", self.name))
    }
    fn get_attr(&self, _runtime: &mut Runtime, name: &str) -> PyResult<Value> {
        if name == "__dict__" {
            return Ok(self.globals.clone());
        }
        self.globals.dict_get(&Value::from(name)).map_err(|_| {
            PyExc::attribute_error(format!("module '{}' has no attribute '{name}'", self.name))
        })
    }
    fn set_attr(&self, name: &str, value: Value) -> PyResult<()> {
        self.globals.dict_set(Value::from(name), value)
    }
}

type IteratorStep = dyn FnMut(&mut Runtime) -> PyResult<Option<Value>>;

pub struct IteratorObject {
    step: RefCell<Box<IteratorStep>>,
    exhausted: RefCell<bool>,
}

impl fmt::Debug for IteratorObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IteratorObject")
    }
}

impl IteratorObject {
    pub fn value(step: impl FnMut(&mut Runtime) -> PyResult<Option<Value>> + 'static) -> Value {
        Value::Object(Rc::new(Self {
            step: RefCell::new(Box::new(step)),
            exhausted: RefCell::new(false),
        }))
    }
    pub fn from_values(values: Vec<Value>) -> Value {
        let mut iter = values.into_iter();
        Self::value(move |_| Ok(iter.next()))
    }
}

impl PyObject for IteratorObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "iterator"
    }
    fn is_iterator(&self) -> bool {
        true
    }
    fn next(&self, runtime: &mut Runtime) -> PyResult<Option<Value>> {
        if *self.exhausted.borrow() {
            return Ok(None);
        }
        let result = self
            .step
            .try_borrow_mut()
            .map_err(|_| PyExc::value_error("generator already executing"))?
            .as_mut()(runtime);
        match result {
            Ok(None) => {
                *self.exhausted.borrow_mut() = true;
                Ok(None)
            }
            Err(e) if e.kind == "StopIteration" => {
                *self.exhausted.borrow_mut() = true;
                Ok(None)
            }
            other => other,
        }
    }
}

/// Runtime defaults are values, preserving mutable default-argument aliases.
#[derive(Clone, Debug)]
pub struct Signature {
    pub name: String,
    pub positional: Vec<String>,
    pub positional_only: usize,
    pub defaults: Vec<Value>,
    pub keyword_only: Vec<(String, Option<Value>)>,
    pub varargs: Option<String>,
    pub varkw: Option<String>,
}

impl Signature {
    pub fn bind(&self, args: CallArgs) -> PyResult<Value> {
        if self.positional_only > self.positional.len()
            || self.defaults.len() > self.positional.len()
        {
            return Err(PyExc::value_error("invalid function signature"));
        }
        let bound = Value::dict();
        let extra = Value::dict();
        let mut seen = std::collections::HashSet::new();
        let supplied = args.positional.len().min(self.positional.len());
        for (name, value) in self.positional.iter().zip(&args.positional) {
            bound.dict_set(Value::from(name.clone()), value.clone())?;
        }
        if args.positional.len() > self.positional.len() && self.varargs.is_none() {
            return Err(PyExc::type_error(format!(
                "{}() received too many positional arguments",
                self.name
            )));
        }
        if let Some(name) = &self.varargs {
            bound.dict_set(
                Value::from(name.clone()),
                Value::tuple(
                    args.positional
                        .iter()
                        .skip(self.positional.len())
                        .cloned()
                        .collect(),
                ),
            )?;
        }
        for (name, value) in args.keywords {
            if !seen.insert(name.clone()) {
                return Err(PyExc::type_error("duplicate keyword argument"));
            }
            if let Some(index) = self
                .positional
                .iter()
                .position(|n| *n == name)
                .filter(|i| *i >= self.positional_only)
            {
                if index < supplied {
                    return Err(PyExc::type_error(format!(
                        "{}() got multiple values for argument '{name}'",
                        self.name
                    )));
                }
                if bound.dict_get(&Value::from(name.clone())).is_ok() {
                    return Err(PyExc::type_error("duplicate keyword argument"));
                }
                bound.dict_set(Value::from(name), value)?;
            } else if self.keyword_only.iter().any(|(n, _)| *n == name) {
                if bound.dict_get(&Value::from(name.clone())).is_ok() {
                    return Err(PyExc::type_error("duplicate keyword argument"));
                }
                bound.dict_set(Value::from(name), value)?;
            } else if self.varkw.is_some() {
                extra.dict_set(Value::from(name), value)?;
            } else {
                return Err(PyExc::type_error(format!(
                    "{}() got an unexpected keyword argument '{name}'",
                    self.name
                )));
            }
        }
        let default_start = self.positional.len() - self.defaults.len();
        for (i, name) in self.positional.iter().enumerate() {
            let key = Value::from(name.clone());
            if bound.dict_get(&key).is_err() {
                if i < default_start {
                    return Err(PyExc::type_error(format!(
                        "{}() missing argument '{name}'",
                        self.name
                    )));
                }
                bound.dict_set(key, self.defaults[i - default_start].clone())?;
            }
        }
        for (name, default) in &self.keyword_only {
            let key = Value::from(name.clone());
            if bound.dict_get(&key).is_err() {
                bound.dict_set(
                    key,
                    default.clone().ok_or_else(|| {
                        PyExc::type_error(format!(
                            "{}() missing keyword-only argument '{name}'",
                            self.name
                        ))
                    })?,
                )?;
            }
        }
        if let Some(name) = &self.varkw {
            bound.dict_set(Value::from(name.clone()), extra)?;
        }
        Ok(bound)
    }
}

#[cfg(test)]
mod origin_tests {
    use super::*;
    use crate::{frame::Frame, runtime::SourceSpan};

    #[test]
    fn source_origin_reads_live_free_cells_and_skips_empty_cells() {
        let mut runtime = Runtime::default();
        let module = Frame::module(Value::dict());
        let global = NativeFunction::value("global", |_, _| Ok(Value::None));
        module.store("global", global).unwrap();
        let closure = Frame::function(module, &["target", "empty"], &[], &[], Value::dict());
        let first = NativeFunction::value("first", |_, _| Ok(Value::None));
        let second = NativeFunction::value("second", |_, _| Ok(Value::None));
        closure.store("target", first.clone()).unwrap();
        let span = SourceSpan {
            file: "bot/modules/example.py",
            first: 7,
            last: 12,
            qualname: "respond",
        };
        let value = NativeFunction::qualified_with_origin(
            "wrapper".into(),
            "wrapper".into(),
            "example".into(),
            Value::dict(),
            FunctionOrigin {
                span,
                closure: closure.clone(),
                freevars: vec!["empty".into(), "target".into()],
            },
            |_, _| Ok(Value::None),
        );
        let Value::Object(object) = value else {
            panic!()
        };
        let function = object.as_any().downcast_ref::<NativeFunction>().unwrap();
        assert_eq!(function.source_span(), Some(span));
        let captured = function.closure_values(&mut runtime);
        assert_eq!(captured.len(), 1);
        assert!(captured[0].is_identical(&first));
        closure.store("target", second.clone()).unwrap();
        let captured = function.closure_values(&mut runtime);
        assert_eq!(captured.len(), 1);
        assert!(captured[0].is_identical(&second));
        assert!(function.get_attr(&mut runtime, "origin").is_err());
        assert!(function.get_attr(&mut runtime, "__closure__").is_err());
    }

    #[test]
    fn native_functions_without_python_origins_have_no_cells() {
        let mut runtime = Runtime::default();
        let value = NativeFunction::value("builtin", |_, _| Ok(Value::None));
        let Value::Object(object) = value else {
            panic!()
        };
        let function = object.as_any().downcast_ref::<NativeFunction>().unwrap();
        assert_eq!(function.source_span(), None);
        assert!(function.closure_values(&mut runtime).is_empty());
    }
}
