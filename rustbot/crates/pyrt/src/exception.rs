use std::fmt;

pub type PyResult<T> = Result<T, PyExc>;

/// Python exceptions are values, so translated hooks can catch or propagate them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyExc {
    pub kind: &'static str,
    pub message: String,
}

macro_rules! constructors {
    ($($name:ident => $kind:literal),* $(,)?) => {
        impl PyExc {
            $(pub fn $name(message: impl Into<String>) -> Self {
                Self { kind: $kind, message: message.into() }
            })*
        }
    };
}

constructors! {
    type_error => "TypeError",
    value_error => "ValueError",
    index_error => "IndexError",
    key_error => "KeyError",
    attribute_error => "AttributeError",
    overflow_error => "OverflowError",
    zero_division_error => "ZeroDivisionError",
    recursion_error => "RecursionError",
    memory_error => "MemoryError",
    not_implemented_error => "NotImplementedError",
    name_error => "NameError",
    unbound_local_error => "UnboundLocalError",
    import_error => "ImportError",
    stop_iteration => "StopIteration",
}

impl fmt::Display for PyExc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for PyExc {}
