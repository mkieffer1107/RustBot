//! Decode compiler-emitted literal data without evaluating code or an AST.
use crate::{PyExc, PyResult, Value};
use serde_json::Value as Json;

fn validate(root: &Json) -> PyResult<()> {
    // Iterative validation avoids adding another recursion limit to literal data.
    let mut pending = vec![root];
    while let Some(value) = pending.pop() {
        match value {
            Json::Null | Json::Bool(_) | Json::Number(_) | Json::String(_) => {}
            Json::Array(values) => pending.extend(values),
            Json::Object(object) => {
                if object.len() != 1 {
                    return Err(PyExc::value_error("invalid literal object"));
                }
                let (tag, payload) = object.iter().next().expect("one literal tag");
                match tag.as_str() {
                    "$float" => {
                        let value = payload
                            .as_str()
                            .ok_or_else(|| PyExc::value_error("invalid literal float"))?;
                        value
                            .parse::<f64>()
                            .map_err(|_| PyExc::value_error("invalid literal float"))?;
                    }
                    "$tuple" => {
                        let values = payload
                            .as_array()
                            .ok_or_else(|| PyExc::value_error("invalid literal tuple"))?;
                        pending.extend(values);
                    }
                    "$dict" => {
                        let pairs = payload
                            .as_array()
                            .ok_or_else(|| PyExc::value_error("invalid literal dictionary"))?;
                        for pair in pairs {
                            let pair = pair.as_array().filter(|pair| pair.len() == 2).ok_or_else(
                                || PyExc::value_error("invalid literal dictionary entry"),
                            )?;
                            pending.extend(pair);
                        }
                    }
                    _ => return Err(PyExc::value_error("unsupported literal tag")),
                }
            }
        }
    }
    Ok(())
}

/// Decode canonical literal JSON into a fresh Python value graph.
/// Dictionary literals use `$dict` even for string keys, so a user's key such
/// as `$tuple` cannot be mistaken for metadata. Sets are intentionally excluded.
pub fn decode(encoded: &str) -> PyResult<Value> {
    let data: Json = serde_json::from_str(encoded)
        .map_err(|error| PyExc::value_error(format!("invalid literal JSON: {error}")))?;
    validate(&data)?;
    crate::from_canon_json(&data)
}
