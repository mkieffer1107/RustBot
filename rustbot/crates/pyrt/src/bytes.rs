//! Immutable Python bytes used by text codecs and digest interfaces.
use crate::{
    PyExc, PyResult, Value,
    object::{IteratorObject, NativeFunction, PyObject},
    runtime::Runtime,
};
use num_traits::ToPrimitive;
use std::{any::Any, rc::Rc};

#[derive(Clone, Debug)]
pub struct BytesObject(Rc<Vec<u8>>);
impl BytesObject {
    pub fn value(bytes: Vec<u8>) -> Value {
        Value::Object(Rc::new(Self(Rc::new(bytes))))
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
/// Extract concrete bytes without invoking arbitrary iterable conversion.
pub fn bytes(value: &Value) -> PyResult<Vec<u8>> {
    if let Value::Object(object) = value
        && let Some(bytes) = object.as_any().downcast_ref::<BytesObject>()
    {
        return Ok(bytes.0.as_ref().clone());
    }
    Err(PyExc::type_error(format!(
        "a bytes-like object is required, not '{}'",
        value.type_name()
    )))
}
pub fn repr(data: &[u8]) -> String {
    let quote = if data.contains(&b'\'') && !data.contains(&b'"') {
        b'"'
    } else {
        b'\''
    };
    let mut out = String::from("b");
    out.push(quote as char);
    for &b in data {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            b if b == quote => {
                out.push('\\');
                out.push(b as char);
            }
            32..=126 => out.push(b as char),
            _ => out.push_str(&format!("\\x{b:02x}")),
        }
    }
    out.push(quote as char);
    out
}
impl PyObject for BytesObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "bytes"
    }
    fn repr(&self) -> PyResult<String> {
        Ok(repr(&self.0))
    }
    fn is_truthy(&self) -> bool {
        !self.0.is_empty()
    }
    fn len(&self) -> PyResult<usize> {
        Ok(self.0.len())
    }
    fn equals(&self, other: &Value) -> PyResult<bool> {
        Ok(if let Value::Object(o) = other {
            o.as_any()
                .downcast_ref::<Self>()
                .is_some_and(|b| b.0 == self.0)
        } else {
            false
        })
    }
    fn hash(&self) -> PyResult<i64> {
        Ok(crate::sets::buffer_hash(&self.0))
    }
    fn sequence(&self) -> PyResult<Vec<Value>> {
        Ok(self.0.iter().map(|b| Value::from(i64::from(*b))).collect())
    }
    fn get_item(&self, index: &Value) -> PyResult<Value> {
        let n = index.as_int().map_err(|_| {
            PyExc::type_error(format!(
                "byte indices must be integers or slices, not {}",
                index.type_name()
            ))
        })?;
        let n = n
            .to_i64()
            .ok_or_else(|| PyExc::index_error("cannot fit int into an index-sized integer"))?;
        let n = if n < 0 {
            self.0.len() as i128 + i128::from(n)
        } else {
            i128::from(n)
        };
        if n < 0 || n >= self.0.len() as i128 {
            return Err(PyExc::index_error("index out of range"));
        }
        Ok(Value::from(i64::from(self.0[n as usize])))
    }
    fn contains(&self, item: &Value) -> PyResult<bool> {
        if matches!(item, Value::Int(_) | Value::Bool(_)) {
            let byte = item
                .as_int()?
                .to_u8()
                .ok_or_else(|| PyExc::value_error("byte must be in range(0, 256)"))?;
            Ok(self.0.contains(&byte))
        } else {
            let needle = bytes(item)?;
            Ok(needle.is_empty() || self.0.windows(needle.len()).any(|w| w == needle))
        }
    }
    fn iter(&self, _rt: &mut Runtime) -> PyResult<Value> {
        let data = self.0.clone();
        let mut index = 0;
        Ok(IteratorObject::value(move |_| {
            let value = data.get(index).map(|b| Value::from(i64::from(*b)));
            index += usize::from(value.is_some());
            Ok(value)
        }))
    }
    fn get_attr(&self, _rt: &mut Runtime, name: &str) -> PyResult<Value> {
        let data = self.0.clone();
        match name {
            "decode" => Ok(NativeFunction::value(name, move |_, args| {
                args.reject_keywords()?;
                args.arity(0, 2)?;
                let encoding = args
                    .positional
                    .first()
                    .map(Value::as_str)
                    .transpose()?
                    .unwrap_or("utf-8");
                let errors = args
                    .positional
                    .get(1)
                    .map(Value::as_str)
                    .transpose()?
                    .unwrap_or("strict");
                Ok(Value::from(decode(&data, encoding, errors)?))
            })),
            "hex" => Ok(NativeFunction::value(name, move |_, args| {
                args.reject_keywords()?;
                args.arity(0, 0)?;
                Ok(Value::from(
                    data.iter().map(|b| format!("{b:02x}")).collect::<String>(),
                ))
            })),
            _ => Err(PyExc::attribute_error(format!(
                "'bytes' object has no attribute '{name}'"
            ))),
        }
    }
}

fn codec(name: &str) -> PyResult<(&'static str, u32)> {
    let original = name;
    let name = name.to_ascii_lowercase().replace(['-', '_', ' '], "");
    match name.as_str() {
        "utf8" | "utf" | "u8" => Ok(("utf-8", 0x10ffff)),
        "ascii" | "usascii" | "646" => Ok(("ascii", 127)),
        "latin1" | "latin" | "iso88591" | "l1" | "cp819" => Ok(("latin-1", 255)),
        _ => Err(PyExc {
            kind: "LookupError",
            message: format!("unknown encoding: {original}"),
        }),
    }
}
fn error_handler(errors: &str) -> PyResult<()> {
    if matches!(errors, "strict" | "ignore" | "replace") {
        Ok(())
    } else {
        Err(PyExc {
            kind: "LookupError",
            message: format!("unknown error handler name '{errors}'"),
        })
    }
}
fn unicode_error(kind: &'static str, message: String) -> PyExc {
    PyExc { kind, message }
}

pub fn encode(text: &str, encoding: &str, errors: &str) -> PyResult<Value> {
    let (encoding, max) = codec(encoding)?;
    if encoding == "utf-8" {
        return Ok(BytesObject::value(text.as_bytes().to_vec()));
    }
    let chars: Vec<char> = text.chars().collect();
    let mut result = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] as u32 <= max {
            result.push(chars[i] as u8);
            i += 1;
            continue;
        }
        error_handler(errors)?;
        if errors == "strict" {
            let mut end = i + 1;
            while end < chars.len() && chars[end] as u32 > max {
                end += 1;
            }
            let cp = chars[i] as u32;
            let escaped = if cp <= 255 {
                format!("\\x{cp:02x}")
            } else if cp <= 65535 {
                format!("\\u{cp:04x}")
            } else {
                format!("\\U{cp:08x}")
            };
            let positions = if end == i + 1 {
                format!("character '{escaped}' in position {i}")
            } else {
                format!("characters in position {i}-{}", end - 1)
            };
            return Err(unicode_error(
                "UnicodeEncodeError",
                format!(
                    "'{encoding}' codec can't encode {positions}: ordinal not in range({})",
                    max + 1
                ),
            ));
        }
        if errors == "replace" {
            result.push(b'?');
        }
        i += 1;
    }
    Ok(BytesObject::value(result))
}

pub fn decode(data: &[u8], encoding: &str, errors: &str) -> PyResult<String> {
    let (encoding, max) = codec(encoding)?;
    if encoding == "latin-1" {
        return Ok(data.iter().map(|b| char::from(*b)).collect());
    }
    if encoding == "ascii" {
        let mut result = String::new();
        for (i, &byte) in data.iter().enumerate() {
            if u32::from(byte) <= max {
                result.push(byte as char);
                continue;
            }
            error_handler(errors)?;
            if errors == "strict" {
                return Err(unicode_error(
                    "UnicodeDecodeError",
                    format!(
                        "'ascii' codec can't decode byte 0x{byte:02x} in position {i}: ordinal not in range(128)"
                    ),
                ));
            }
            if errors == "replace" {
                result.push('\u{fffd}');
            }
        }
        return Ok(result);
    }
    let mut result = String::new();
    let mut position = 0;
    while position < data.len() {
        match std::str::from_utf8(&data[position..]) {
            Ok(text) => {
                result.push_str(text);
                break;
            }
            Err(error) => {
                result.push_str(
                    std::str::from_utf8(&data[position..position + error.valid_up_to()])
                        .expect("valid UTF8 prefix"),
                );
                position += error.valid_up_to();
                error_handler(errors)?;
                if errors == "strict" {
                    let reason = if error.error_len().is_none() {
                        "unexpected end of data"
                    } else if data[position] < 0xc2 || data[position] > 0xf4 {
                        "invalid start byte"
                    } else {
                        "invalid continuation byte"
                    };
                    let length = error.error_len().unwrap_or(data.len() - position);
                    let details = if length == 1 {
                        format!("byte 0x{:02x} in position {position}", data[position])
                    } else {
                        format!("bytes in position {position}-{}", position + length - 1)
                    };
                    return Err(unicode_error(
                        "UnicodeDecodeError",
                        format!("'utf-8' codec can't decode {details}: {reason}"),
                    ));
                }
                if errors == "replace" {
                    result.push('\u{fffd}');
                }
                position += error.error_len().unwrap_or(data.len() - position);
            }
        }
    }
    Ok(result)
}

pub fn slice(value: &Value, start: Option<i64>, stop: Option<i64>, step: i64) -> PyResult<Value> {
    if step == 0 {
        return Err(PyExc::value_error("slice step cannot be zero"));
    }
    let data = bytes(value)?;
    let n = data.len() as i128;
    let normalize = |i: i64| {
        let i = if i < 0 {
            n + i128::from(i)
        } else {
            i128::from(i)
        };
        if step < 0 {
            i.clamp(-1, n - 1)
        } else {
            i.clamp(0, n)
        }
    };
    let mut i = start
        .map(normalize)
        .unwrap_or(if step < 0 { n - 1 } else { 0 });
    let end = stop.map(normalize).unwrap_or(if step < 0 { -1 } else { n });
    let mut out = Vec::new();
    while if step < 0 { i > end } else { i < end } {
        out.push(data[i as usize]);
        i += i128::from(step);
    }
    Ok(BytesObject::value(out))
}
