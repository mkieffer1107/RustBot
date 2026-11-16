//! Python-compatible primitives shared by the native and WebAssembly ports.

pub mod cases;
mod exception;
pub mod formatting;
pub mod ops;
pub mod random;
pub mod regex;
pub mod sets;
pub mod strings;
mod unicode_data;
mod value;

pub use exception::{PyExc, PyResult};
pub use value::{PyStr, Value, canon_json, float_repr, from_canon_json};
