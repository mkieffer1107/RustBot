//! Control-flow values cross native closures implementing Python try/finally.
use crate::Value;

#[derive(Clone, Debug, Default)]
pub enum Flow {
    #[default]
    Next,
    Return(Value),
    Break,
    Continue,
}
