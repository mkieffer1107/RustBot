//! Builtin exception classes and values for translated raise/except statements.
//!
//! PyExc stores only a kind and display message. Rewrapping a PyExc preserves
//! that message, but cannot recover its original args, traceback or cause.
use crate::{
    PyExc, PyResult, Value, formatting,
    object::{NativeType, PyObject},
    runtime::Runtime,
};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    rc::Rc,
};

// Parents precede children; PatternError is cached privately for the re module.
const TYPES: &[(&str, Option<&str>)] = &[
    ("BaseException", None),
    ("Exception", Some("BaseException")),
    ("SystemExit", Some("BaseException")),
    ("KeyboardInterrupt", Some("BaseException")),
    ("GeneratorExit", Some("BaseException")),
    ("ArithmeticError", Some("Exception")),
    ("FloatingPointError", Some("ArithmeticError")),
    ("OverflowError", Some("ArithmeticError")),
    ("ZeroDivisionError", Some("ArithmeticError")),
    ("AssertionError", Some("Exception")),
    ("AttributeError", Some("Exception")),
    ("BufferError", Some("Exception")),
    ("EOFError", Some("Exception")),
    ("ImportError", Some("Exception")),
    ("ModuleNotFoundError", Some("ImportError")),
    ("LookupError", Some("Exception")),
    ("IndexError", Some("LookupError")),
    ("KeyError", Some("LookupError")),
    ("MemoryError", Some("Exception")),
    ("NameError", Some("Exception")),
    ("UnboundLocalError", Some("NameError")),
    ("OSError", Some("Exception")),
    ("BlockingIOError", Some("OSError")),
    ("ChildProcessError", Some("OSError")),
    ("ConnectionError", Some("OSError")),
    ("BrokenPipeError", Some("ConnectionError")),
    ("ConnectionAbortedError", Some("ConnectionError")),
    ("ConnectionRefusedError", Some("ConnectionError")),
    ("ConnectionResetError", Some("ConnectionError")),
    ("FileExistsError", Some("OSError")),
    ("FileNotFoundError", Some("OSError")),
    ("InterruptedError", Some("OSError")),
    ("IsADirectoryError", Some("OSError")),
    ("NotADirectoryError", Some("OSError")),
    ("PermissionError", Some("OSError")),
    ("ProcessLookupError", Some("OSError")),
    ("TimeoutError", Some("OSError")),
    ("ReferenceError", Some("Exception")),
    ("RuntimeError", Some("Exception")),
    ("NotImplementedError", Some("RuntimeError")),
    ("RecursionError", Some("RuntimeError")),
    ("StopIteration", Some("Exception")),
    ("StopAsyncIteration", Some("Exception")),
    ("SyntaxError", Some("Exception")),
    ("IndentationError", Some("SyntaxError")),
    ("TabError", Some("IndentationError")),
    ("SystemError", Some("Exception")),
    ("TypeError", Some("Exception")),
    ("ValueError", Some("Exception")),
    ("UnicodeError", Some("ValueError")),
    ("UnicodeEncodeError", Some("UnicodeError")),
    ("UnicodeDecodeError", Some("UnicodeError")),
    ("UnicodeTranslateError", Some("UnicodeError")),
    ("Warning", Some("Exception")),
    ("UserWarning", Some("Warning")),
    ("DeprecationWarning", Some("Warning")),
    ("PendingDeprecationWarning", Some("Warning")),
    ("SyntaxWarning", Some("Warning")),
    ("RuntimeWarning", Some("Warning")),
    ("FutureWarning", Some("Warning")),
    ("ImportWarning", Some("Warning")),
    ("UnicodeWarning", Some("Warning")),
    ("BytesWarning", Some("Warning")),
    ("ResourceWarning", Some("Warning")),
    ("EncodingWarning", Some("Warning")),
    ("PatternError", Some("Exception")),
    ("DecimalException", Some("ArithmeticError")),
    ("InvalidOperation", Some("DecimalException")),
    ("ConversionSyntax", Some("InvalidOperation")),
    ("DivisionImpossible", Some("InvalidOperation")),
    ("DivisionUndefined", Some("InvalidOperation")),
    ("InvalidContext", Some("InvalidOperation")),
    ("DivisionByZero", Some("DecimalException")),
    ("FloatOperation", Some("DecimalException")),
    ("Clamped", Some("DecimalException")),
    ("Inexact", Some("DecimalException")),
    ("Rounded", Some("DecimalException")),
    ("Subnormal", Some("DecimalException")),
    ("Overflow", Some("Inexact")),
    ("Underflow", Some("Inexact")),
];
pub(crate) const DECIMAL_EXCEPTIONS: &[&str] = &[
    "DecimalException",
    "InvalidOperation",
    "ConversionSyntax",
    "DivisionImpossible",
    "DivisionUndefined",
    "InvalidContext",
    "DivisionByZero",
    "FloatOperation",
    "Clamped",
    "Inexact",
    "Rounded",
    "Subnormal",
    "Overflow",
    "Underflow",
];
fn module_for(name: &str) -> &'static str {
    if name == "PatternError" {
        "re"
    } else if DECIMAL_EXCEPTIONS.contains(&name) {
        "decimal"
    } else {
        "builtins"
    }
}
fn secondary_parents(name: &str) -> &'static [&'static str] {
    match name {
        "DivisionByZero" | "DivisionUndefined" => &["ZeroDivisionError"],
        "FloatOperation" => &["TypeError"],
        "Overflow" => &["Rounded"],
        "Underflow" => &["Rounded", "Subnormal"],
        _ => &[],
    }
}
fn kind(name: &str) -> Option<&'static str> {
    let name = if matches!(name, "IOError" | "EnvironmentError") {
        "OSError"
    } else {
        name
    };
    TYPES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(name, _)| *name)
}
fn parent(name: &str) -> Option<&'static str> {
    TYPES.iter().find(|(n, _)| *n == name).and_then(|(_, p)| *p)
}
fn is_subclass(mut child: &str, base: &str) -> bool {
    let base = kind(base).unwrap_or(base);
    child = kind(child).unwrap_or(child);
    loop {
        if child == base
            || secondary_parents(child)
                .iter()
                .any(|parent| is_subclass(parent, base))
        {
            return true;
        }
        let Some(next) = parent(child) else {
            return false;
        };
        child = next;
    }
}

pub fn install(rt: &mut Runtime) {
    for &(name, _) in TYPES {
        if module_for(name) == "builtins" {
            exception_type(rt, name).expect("static exception hierarchy");
        }
    }
    let os = rt.builtin("OSError").expect("installed OSError");
    rt.register_builtin("IOError", os.clone());
    rt.register_builtin("EnvironmentError", os);
}

/// Cached builtin exception types; PatternError is exported by the re module.
pub fn exception_type(rt: &mut Runtime, name: &'static str) -> PyResult<Value> {
    let kind =
        kind(name).ok_or_else(|| PyExc::type_error(format!("unknown exception type '{name}'")))?;
    let cache = if module_for(kind) == "builtins" {
        kind.to_owned()
    } else {
        format!("__pyrt_exception_{kind}")
    };
    if let Ok(value) = rt.builtin(&cache) {
        return Ok(value);
    }
    let attrs = Value::dict();
    let mut bases = Vec::new();
    if let Some(parent) = parent(kind) {
        let parent_type = exception_type(rt, parent)?;
        bases.push(parent_type.clone());
    } else {
        let object = rt.builtin("object")?;
        bases.push(object.clone());
    }
    for parent in secondary_parents(kind) {
        bases.push(exception_type(rt, parent)?);
    }
    attrs.dict_set(Value::from("__bases__"), Value::tuple(bases))?;
    let value = NativeType::with_attrs(
        kind,
        module_for(kind),
        attrs.clone(),
        move |value| matches!(value,Value::Object(object) if object.as_any().downcast_ref::<ExceptionObject>().is_some_and(|e|is_subclass(e.kind,kind))),
        move |rt, args| {
            args.reject_keywords()?;
            let class = exception_type(rt, kind)?;
            Ok(Value::Object(Rc::new(ExceptionObject::new(
                kind,
                args.positional,
                Some(class),
                None,
            ))))
        },
    );
    rt.register_builtin(cache, value.clone());
    Ok(value)
}

fn exception_class(value: &Value) -> Option<&'static str> {
    let Value::Object(object) = value else {
        return None;
    };
    let class = object.as_any().downcast_ref::<NativeType>()?;
    kind(&class.name).filter(|kind| class.module == module_for(kind))
}

/// CPython validates every catch tuple entry before attempting the match.
pub fn matches(exc: &PyExc, types: &Value) -> PyResult<bool> {
    let values = if let Value::Tuple(values) = types {
        values.to_vec()
    } else {
        vec![types.clone()]
    };
    let kinds = values
        .iter()
        .map(|v| {
            exception_class(v).ok_or_else(|| {
                PyExc::type_error(
                    "catching classes that do not inherit from BaseException is not allowed",
                )
            })
        })
        .collect::<PyResult<Vec<_>>>()?;
    Ok(kinds.into_iter().any(|base| is_subclass(exc.kind, base)))
}

pub fn raised(rt: &mut Runtime, value: &Value) -> PyResult<PyExc> {
    if let Value::Object(object) = value
        && let Some(exc) = object.as_any().downcast_ref::<ExceptionObject>()
    {
        return Ok(PyExc {
            kind: exc.kind,
            message: exc.str()?,
        });
    }
    if exception_class(value).is_some() {
        let instance = rt.call(value, crate::object::CallArgs::default())?;
        return raised(rt, &instance);
    }
    Err(PyExc::type_error(
        "exceptions must derive from BaseException",
    ))
}

/// Convert the runtime error for `except ... as name`, preserving its display.
pub fn value(exc: &PyExc) -> Value {
    let args = if exc.message.is_empty() {
        vec![]
    } else {
        vec![Value::from(exc.message.clone())]
    };
    Value::Object(Rc::new(ExceptionObject::new(
        exc.kind,
        args,
        None,
        Some(exc.message.clone()),
    )))
}

pub struct ExceptionObject {
    kind: &'static str,
    args: RefCell<Vec<Value>>,
    class: Option<Value>,
    message: RefCell<Option<String>>,
    attrs: Value,
    text_depth: Cell<u16>,
}
impl std::fmt::Debug for ExceptionObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ExceptionObject").field(&self.kind).finish()
    }
}
impl ExceptionObject {
    fn new(
        kind: &'static str,
        args: Vec<Value>,
        class: Option<Value>,
        message: Option<String>,
    ) -> Self {
        Self {
            kind,
            args: RefCell::new(args),
            class,
            message: RefCell::new(message),
            attrs: Value::dict(),
            text_depth: Cell::new(0),
        }
    }
}
struct TextGuard<'a>(&'a Cell<u16>);
impl<'a> TextGuard<'a> {
    fn enter(depth: &'a Cell<u16>) -> PyResult<Self> {
        if depth.get() >= 64 {
            return Err(PyExc::recursion_error(
                "maximum recursion depth exceeded while getting exception text",
            ));
        }
        depth.set(depth.get() + 1);
        Ok(Self(depth))
    }
}
impl Drop for TextGuard<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

impl PyObject for ExceptionObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        self.kind
    }
    fn repr(&self) -> PyResult<String> {
        let _guard = TextGuard::enter(&self.text_depth)?;
        let text = self
            .args
            .borrow()
            .iter()
            .map(formatting::repr)
            .collect::<PyResult<Vec<_>>>()?
            .join(", ");
        Ok(format!("{}({text})", self.kind))
    }
    fn str(&self) -> PyResult<String> {
        let _guard = TextGuard::enter(&self.text_depth)?;
        if let Some(message) = self.message.borrow().as_ref() {
            return Ok(message.clone());
        }
        let args = self.args.borrow();
        match args.len() {
            0 => Ok(String::new()),
            1 => {
                if self.kind == "KeyError" {
                    formatting::repr(&args[0])
                } else {
                    formatting::str(&args[0])
                }
            }
            _ => formatting::str(&Value::tuple(args.clone())),
        }
    }
    fn hash(&self) -> PyResult<i64> {
        let h = (self as *const Self as usize).rotate_right(4) as i64;
        Ok(if h == -1 { -2 } else { h })
    }
    fn get_attr(&self, rt: &mut Runtime, name: &str) -> PyResult<Value> {
        match name {
            "args" => Ok(Value::tuple(self.args.borrow().clone())),
            "__class__" => self
                .class
                .clone()
                .map_or_else(|| exception_type(rt, self.kind), Ok),
            "__dict__" => Ok(self.attrs.clone()),
            "value" if self.kind == "StopIteration" => {
                Ok(self.args.borrow().first().cloned().unwrap_or(Value::None))
            }
            "code" if self.kind == "SystemExit" => {
                let args = self.args.borrow();
                Ok(match args.len() {
                    0 => Value::None,
                    1 => args[0].clone(),
                    _ => Value::tuple(args.clone()),
                })
            }
            _ => self.attrs.dict_get(&Value::from(name)).map_err(|_| {
                PyExc::attribute_error(format!("'{}' object has no attribute '{name}'", self.kind))
            }),
        }
    }
    fn set_attr(&self, name: &str, value: Value) -> PyResult<()> {
        if name == "args" {
            *self.args.borrow_mut() = value.sequence()?;
            *self.message.borrow_mut() = None;
            Ok(())
        } else {
            self.attrs.dict_set(Value::from(name), value)
        }
    }
}
