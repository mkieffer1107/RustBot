use crate::sets::PySet;
use crate::{PyExc, PyResult};
use num_bigint::BigInt;
use num_traits::{FromPrimitive, ToPrimitive, Zero};
use serde_json::{Map, Value as Json, json};
use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// Strings share storage and build their code-point index only when needed.
#[derive(Clone, Debug)]
pub struct PyStr(Rc<StrData>);

#[derive(Debug)]
struct StrData {
    text: String,
    offsets: OnceCell<Vec<usize>>,
}

impl PyStr {
    pub fn new(text: impl Into<String>) -> Self {
        Self(Rc::new(StrData {
            text: text.into(),
            offsets: OnceCell::new(),
        }))
    }

    pub fn as_str(&self) -> &str {
        &self.0.text
    }

    fn offsets(&self) -> &[usize] {
        self.0.offsets.get_or_init(|| {
            self.as_str()
                .char_indices()
                .map(|(i, _)| i)
                .chain([self.as_str().len()])
                .collect()
        })
    }

    pub fn len(&self) -> usize {
        self.offsets().len() - 1
    }
    pub fn is_empty(&self) -> bool {
        self.as_str().is_empty()
    }

    pub fn get_index(&self, index: i64) -> PyResult<Self> {
        let n = self.len() as i128;
        let i = if index < 0 {
            n + index as i128
        } else {
            index as i128
        };
        if i < 0 || i >= n {
            return Err(PyExc::index_error("string index out of range"));
        }
        let i = i as usize;
        Ok(Self::new(
            &self.as_str()[self.offsets()[i]..self.offsets()[i + 1]],
        ))
    }

    pub fn slice(&self, start: Option<i64>, stop: Option<i64>, step: i64) -> PyResult<Self> {
        if step == 0 {
            return Err(PyExc::value_error("slice step cannot be zero"));
        }
        let n = self.len() as i128;
        let normalize = |i: i64| {
            let i = if i < 0 { n + i as i128 } else { i as i128 };
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
        let mut out = String::new();
        while if step < 0 { i > end } else { i < end } {
            let j = i as usize;
            out.push_str(&self.as_str()[self.offsets()[j]..self.offsets()[j + 1]]);
            i += step as i128;
        }
        Ok(Self::new(out))
    }
}

/// Mutable containers retain Python aliasing; integers never overflow.
#[derive(Clone, Debug)]
pub enum Value {
    None,
    Bool(bool),
    Int(BigInt),
    Float(f64),
    Str(PyStr),
    List(Rc<RefCell<Vec<Value>>>),
    Tuple(Rc<[Value]>),
    Dict(Rc<RefCell<Vec<(Value, Value)>>>),
    Set(PySet),
    FrozenSet(PySet),
    Date(String),
    DateTime(String),
    Opaque { text: String, type_name: String },
}

impl Value {
    pub fn list(values: Vec<Self>) -> Self {
        Self::List(Rc::new(RefCell::new(values)))
    }
    pub fn tuple(values: Vec<Self>) -> Self {
        Self::Tuple(values.into())
    }
    pub fn dict() -> Self {
        Self::Dict(Rc::new(RefCell::new(Vec::new())))
    }

    pub fn type_name(&self) -> &str {
        match self {
            Self::None => "NoneType",
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::Str(_) => "str",
            Self::List(_) => "list",
            Self::Tuple(_) => "tuple",
            Self::Dict(_) => "dict",
            Self::Date(_) => "date",
            Self::Set(_) => "set",
            Self::FrozenSet(_) => "frozenset",
            Self::DateTime(_) => "datetime",
            Self::Opaque { type_name, .. } => type_name,
        }
    }

    pub fn as_str(&self) -> PyResult<&str> {
        match self {
            Self::Str(s) => Ok(s.as_str()),
            _ => Err(PyExc::type_error("expected str")),
        }
    }

    pub fn as_int(&self) -> PyResult<BigInt> {
        match self {
            Self::Int(n) => Ok(n.clone()),
            Self::Bool(b) => Ok(BigInt::from(*b)),
            _ => Err(PyExc::type_error(
                "object cannot be interpreted as an integer",
            )),
        }
    }

    pub fn as_float(&self) -> PyResult<f64> {
        match self {
            Self::Float(x) => Ok(*x),
            Self::Bool(b) => Ok(u8::from(*b) as f64),
            Self::Int(n) => n
                .to_f64()
                .filter(|v| v.is_finite())
                .ok_or_else(|| PyExc::overflow_error("int too large to convert to float")),
            _ => Err(PyExc::type_error("must be real number")),
        }
    }

    pub fn sequence(&self) -> PyResult<Vec<Self>> {
        match self {
            Self::List(v) => Ok(v.borrow().clone()),
            Self::Tuple(v) => Ok(v.to_vec()),
            Self::Str(s) => Ok(s
                .as_str()
                .chars()
                .map(|c| Self::from(c.to_string()))
                .collect()),
            Self::Dict(v) => Ok(v.borrow().iter().map(|(k, _)| k.clone()).collect()),
            Self::Set(s) | Self::FrozenSet(s) => Ok(s.values()),
            _ => Err(PyExc::type_error("object is not iterable")),
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Self::None => false,
            Self::Bool(b) => *b,
            Self::Int(n) => !n.is_zero(),
            Self::Float(x) => *x != 0.0,
            Self::Str(s) => !s.is_empty(),
            Self::List(v) => !v.borrow().is_empty(),
            Self::Tuple(v) => !v.is_empty(),
            Self::Dict(v) => !v.borrow().is_empty(),
            Self::Set(s) | Self::FrozenSet(s) => !s.is_empty(),
            _ => true,
        }
    }

    pub fn is_identical(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::None, Self::None) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::List(a), Self::List(b)) => Rc::ptr_eq(a, b),
            (Self::Tuple(a), Self::Tuple(b)) => Rc::ptr_eq(a, b),
            (Self::Dict(a), Self::Dict(b)) => Rc::ptr_eq(a, b),
            (Self::Set(a), Self::Set(b)) | (Self::FrozenSet(a), Self::FrozenSet(b)) => {
                a.is_identical(b)
            }
            _ => false,
        }
    }

    pub fn equals(&self, other: &Self) -> PyResult<bool> {
        self.equals_at(other, 0)
    }

    fn equals_at(&self, other: &Self, depth: usize) -> PyResult<bool> {
        if depth > 200 {
            return Err(PyExc::recursion_error(
                "maximum recursion depth exceeded in comparison",
            ));
        }
        if self.is_identical(other) {
            return Ok(true);
        }
        match (self, other) {
            (Self::Bool(_) | Self::Int(_), Self::Bool(_) | Self::Int(_)) => {
                Ok(self.as_int()? == other.as_int()?)
            }
            (Self::Float(a), Self::Float(b)) => Ok(a == b),
            (Self::Float(f), Self::Int(_) | Self::Bool(_))
            | (Self::Int(_) | Self::Bool(_), Self::Float(f)) => {
                let n = if matches!(self, Self::Float(_)) {
                    other.as_int()?
                } else {
                    self.as_int()?
                };
                Ok(f.is_finite()
                    && f.fract() == 0.0
                    && BigInt::from_f64(*f).is_some_and(|i| i == n))
            }
            (Self::Str(a), Self::Str(b)) => Ok(a.as_str() == b.as_str()),
            (Self::List(a), Self::List(b)) => equal_sequences(&a.borrow(), &b.borrow(), depth),
            (Self::Tuple(a), Self::Tuple(b)) => equal_sequences(a, b, depth),
            (Self::Dict(a), Self::Dict(b)) => {
                let (a, b) = (a.borrow(), b.borrow());
                if a.len() != b.len() {
                    return Ok(false);
                }
                for (ak, av) in a.iter() {
                    let mut found = false;
                    for (bk, bv) in b.iter() {
                        if ak.equals_at(bk, depth + 1)? {
                            found = av.equals_at(bv, depth + 1)?;
                            break;
                        }
                    }
                    if !found {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Self::Date(a), Self::Date(b)) | (Self::DateTime(a), Self::DateTime(b)) => Ok(a == b),
            (Self::Set(a) | Self::FrozenSet(a), Self::Set(b) | Self::FrozenSet(b)) => {
                if a.len() != b.len() {
                    return Ok(false);
                }
                for v in a.values() {
                    if !b.contains(&v)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (
                Self::Opaque {
                    text: a,
                    type_name: at,
                },
                Self::Opaque {
                    text: b,
                    type_name: bt,
                },
            ) => Ok(a == b && at == bt),
            _ => Ok(false),
        }
    }

    fn check_hashable(&self) -> PyResult<()> {
        match self {
            Self::List(_) | Self::Dict(_) | Self::Set(_) => Err(PyExc::type_error(format!(
                "unhashable type: '{}'",
                self.type_name()
            ))),
            Self::Tuple(v) => {
                for item in v.iter() {
                    item.check_hashable()?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn dict_set(&self, key: Self, value: Self) -> PyResult<()> {
        key.check_hashable()?;
        let Self::Dict(d) = self else {
            return Err(PyExc::type_error("expected dict"));
        };
        let mut d = d.borrow_mut();
        for (k, v) in d.iter_mut() {
            if key.is_identical(k) || key.equals(k)? {
                *v = value;
                return Ok(());
            }
        }
        d.push((key, value));
        Ok(())
    }

    pub fn dict_get(&self, key: &Self) -> PyResult<Self> {
        key.check_hashable()?;
        let Self::Dict(d) = self else {
            return Err(PyExc::type_error("expected dict"));
        };
        for (k, v) in d.borrow().iter() {
            if key.is_identical(k) || key.equals(k)? {
                return Ok(v.clone());
            }
        }
        Err(PyExc::key_error("key not found"))
    }

    /// A memo keeps repeated references shared and handles mutable cycles.
    pub fn deepcopy(&self) -> Self {
        self.copy_with_memo(&mut HashMap::new())
    }

    fn copy_with_memo(&self, memo: &mut HashMap<(u8, usize), Self>) -> Self {
        match self {
            Self::List(v) => {
                let id = (0, Rc::as_ptr(v) as usize);
                if let Some(v) = memo.get(&id) {
                    return v.clone();
                }
                let out = Self::list(Vec::new());
                memo.insert(id, out.clone());
                if let Self::List(items) = &out {
                    *items.borrow_mut() =
                        v.borrow().iter().map(|v| v.copy_with_memo(memo)).collect();
                }
                out
            }
            Self::Dict(v) => {
                let id = (1, Rc::as_ptr(v) as usize);
                if let Some(v) = memo.get(&id) {
                    return v.clone();
                }
                let out = Self::dict();
                memo.insert(id, out.clone());
                if let Self::Dict(items) = &out {
                    *items.borrow_mut() = v
                        .borrow()
                        .iter()
                        .map(|(k, v)| (k.copy_with_memo(memo), v.copy_with_memo(memo)))
                        .collect();
                }
                out
            }
            Self::Tuple(v) => {
                let id = (2, v.as_ptr() as usize);
                if let Some(existing) = memo.get(&id) {
                    return existing.clone();
                }
                let copied: Vec<_> = v.iter().map(|v| v.copy_with_memo(memo)).collect();
                // Tuples may occur inside mutable cycles. Reuse any tuple made by
                // recursion while its children were being copied, as Python does.
                if let Some(existing) = memo.get(&id) {
                    return existing.clone();
                }
                let unchanged = v.iter().zip(&copied).all(|(original, copied)| {
                    matches!(
                        original,
                        Self::None | Self::Bool(_) | Self::Int(_) | Self::Float(_) | Self::Str(_)
                    ) || original.is_identical(copied)
                });
                let out = if unchanged {
                    self.clone()
                } else {
                    Self::tuple(copied)
                };
                memo.insert(id, out.clone());
                out
            }
            Self::Set(v) | Self::FrozenSet(v) => {
                let id = (3, v.identity());
                if let Some(existing) = memo.get(&id) {
                    return existing.clone();
                }
                let copied = v.values().into_iter().map(|v| v.copy_with_memo(memo));
                let out = PySet::from_values(copied).expect("deepcopy preserves hashability");
                let out = if matches!(self, Self::Set(_)) {
                    Self::Set(out)
                } else {
                    Self::FrozenSet(out)
                };
                memo.insert(id, out.clone());
                out
            }
            _ => self.clone(),
        }
    }
}

fn equal_sequences(a: &[Value], b: &[Value], depth: usize) -> PyResult<bool> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.iter().zip(b) {
        if !a.equals_at(b, depth + 1)? {
            return Ok(false);
        }
    }
    Ok(true)
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::Str(PyStr::new(s))
    }
}
impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::Str(PyStr::new(s))
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}
impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Self::Int(n.into())
    }
}
impl From<usize> for Value {
    fn from(n: usize) -> Self {
        Self::Int(n.into())
    }
}

pub fn float_repr(value: f64) -> String {
    if value.is_nan() {
        return "nan".into();
    }
    if value == f64::INFINITY {
        return "inf".into();
    }
    if value == f64::NEG_INFINITY {
        return "-inf".into();
    }
    let text = format!("{value:?}");
    if let Some((mantissa, exponent)) = text.split_once('e') {
        let exponent: i32 = exponent.parse().expect("Rust float exponent");
        format!("{mantissa}e{exponent:+03}")
    } else {
        text
    }
}

/// Canonical encoding has the same depth cutoff as the oracle.
pub fn canon_json(value: &Value) -> PyResult<Json> {
    encode_at(value, 0)
}

fn encode_at(value: &Value, depth: usize) -> PyResult<Json> {
    if depth > 60 {
        return Ok(json!({"$repr": "<too deep>", "$type": value.type_name()}));
    }
    Ok(match value {
        Value::None => Json::Null,
        Value::Bool(v) => Json::Bool(*v),
        Value::Int(v) => {
            serde_json::from_str(&v.to_string()).map_err(|e| PyExc::value_error(e.to_string()))?
        }
        Value::Float(v) => json!({"$float": float_repr(*v)}),
        Value::Str(s) => Json::String(s.as_str().into()),
        Value::List(v) => Json::Array(
            v.borrow()
                .iter()
                .map(|v| encode_at(v, depth + 1))
                .collect::<PyResult<_>>()?,
        ),
        Value::Tuple(v) => {
            json!({"$tuple": v.iter().map(|v| encode_at(v, depth + 1)).collect::<PyResult<Vec<_>>>()?})
        }
        Value::Dict(d) => {
            let d = d.borrow();
            if d.iter()
                .all(|(k, _)| matches!(k, Value::Str(s) if !s.as_str().starts_with('$')))
            {
                let mut out = Map::new();
                for (k, v) in d.iter() {
                    out.insert(k.as_str()?.into(), encode_at(v, depth + 1)?);
                }
                Json::Object(out)
            } else {
                json!({"$dict": d.iter().map(|(k, v)| Ok(vec![encode_at(k, depth + 1)?, encode_at(v, depth + 1)?])).collect::<PyResult<Vec<_>>>()?})
            }
        }
        Value::Date(s) => json!({"$date": s}),
        Value::DateTime(s) => json!({"$datetime": s}),
        Value::Set(s) => {
            json!({"$set": s.values().iter().map(|v| encode_at(v, depth + 1)).collect::<PyResult<Vec<_>>>()?})
        }
        Value::FrozenSet(s) => {
            json!({"$frozenset": s.values().iter().map(|v| encode_at(v, depth + 1)).collect::<PyResult<Vec<_>>>()?})
        }
        Value::Opaque { text, type_name } => json!({"$repr": text, "$type": type_name}),
    })
}

pub fn from_canon_json(json: &Json) -> PyResult<Value> {
    Ok(match json {
        Json::Null => Value::None,
        Json::Bool(v) => Value::Bool(*v),
        Json::String(s) => Value::from(s.clone()),
        Json::Number(n) => {
            if n.is_f64() {
                Value::Float(
                    n.as_f64()
                        .ok_or_else(|| PyExc::value_error("invalid float"))?,
                )
            } else {
                Value::Int(
                    n.to_string()
                        .parse()
                        .map_err(|_| PyExc::value_error("invalid integer"))?,
                )
            }
        }
        Json::Array(v) => Value::list(v.iter().map(from_canon_json).collect::<PyResult<_>>()?),
        Json::Object(o) => {
            if o.len() <= 2 {
                let text = |key: &str| {
                    o[key]
                        .as_str()
                        .ok_or_else(|| PyExc::value_error(format!("invalid {key}")))
                };
                if o.contains_key("$float") {
                    return Ok(Value::Float(
                        text("$float")?
                            .parse()
                            .map_err(|_| PyExc::value_error("invalid float"))?,
                    ));
                }
                if let Some(v) = o.get("$tuple") {
                    return Ok(Value::tuple(
                        v.as_array()
                            .ok_or_else(|| PyExc::type_error("invalid tuple"))?
                            .iter()
                            .map(from_canon_json)
                            .collect::<PyResult<_>>()?,
                    ));
                }
                if let Some(v) = o.get("$dict") {
                    let d = Value::dict();
                    for pair in v
                        .as_array()
                        .ok_or_else(|| PyExc::type_error("invalid dict"))?
                    {
                        let pair = pair
                            .as_array()
                            .filter(|p| p.len() == 2)
                            .ok_or_else(|| PyExc::value_error("invalid dictionary entry"))?;
                        d.dict_set(from_canon_json(&pair[0])?, from_canon_json(&pair[1])?)?;
                    }
                    return Ok(d);
                }
                if o.contains_key("$date") {
                    return Ok(Value::Date(text("$date")?.into()));
                }
                if o.contains_key("$datetime") {
                    return Ok(Value::DateTime(text("$datetime")?.into()));
                }
                if o.contains_key("$repr") {
                    return Ok(Value::Opaque {
                        text: text("$repr")?.into(),
                        type_name: o.get("$type").and_then(Json::as_str).unwrap_or("").into(),
                    });
                }
                for tag in ["$set", "$frozenset"] {
                    if let Some(values) = o.get(tag) {
                        let values = values
                            .as_array()
                            .ok_or_else(|| PyExc::type_error("invalid set"))?
                            .iter()
                            .map(from_canon_json)
                            .collect::<PyResult<Vec<_>>>()?;
                        let set = PySet::from_values(values)?;
                        return Ok(if tag == "$set" {
                            Value::Set(set)
                        } else {
                            Value::FrozenSet(set)
                        });
                    }
                }
            }
            let d = Value::dict();
            for (k, v) in o {
                d.dict_set(Value::from(k.clone()), from_canon_json(v)?)?;
            }
            d
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codepoints_and_reverse_slice() {
        let s = PyStr::new("aé😀z");
        assert_eq!(s.len(), 4);
        assert_eq!(s.get_index(-2).unwrap().as_str(), "😀");
        assert_eq!(s.slice(None, None, -1).unwrap().as_str(), "z😀éa");
        assert_eq!(s.slice(None, Some(-1), -1).unwrap().as_str(), "");
        assert_eq!(s.slice(Some(-99), Some(99), 2).unwrap().as_str(), "a😀");
        assert_eq!(s.get_index(4).unwrap_err().kind, "IndexError");
        assert_eq!(s.slice(None, None, 0).unwrap_err().kind, "ValueError");
    }

    #[test]
    fn numeric_dictionary_keys_keep_first_key_and_insertion_position() {
        let d = Value::dict();
        d.dict_set(Value::Bool(true), Value::from("first")).unwrap();
        d.dict_set(Value::from("x"), Value::None).unwrap();
        d.dict_set(Value::Float(1.0), Value::from("updated"))
            .unwrap();
        assert_eq!(
            canon_json(&d).unwrap(),
            json!({"$dict": [[true,"updated"],["x",null]]})
        );
        assert!(
            Value::from(9_007_199_254_740_993_i64)
                .equals(&Value::Float(9_007_199_254_740_992.0))
                .is_ok_and(|v| !v)
        );
        assert_eq!(
            d.dict_set(Value::list(vec![]), Value::None)
                .unwrap_err()
                .kind,
            "TypeError"
        );
    }

    #[test]
    fn canonical_values_preserve_huge_ints_dollar_keys_and_negative_zero() {
        let input: Json = serde_json::from_str(r#"{"$dict":[["$float","literal"],[99999999999999999999999999999,{"$tuple":[{"$float":"-0.0"},"é"]}]]}"#).unwrap();
        assert_eq!(
            canon_json(&from_canon_json(&input).unwrap()).unwrap(),
            input
        );
    }

    #[test]
    fn deepcopy_preserves_aliases_and_mutable_cycles() {
        let child = Value::list(vec![Value::from(7_i64)]);
        let original = Value::list(vec![child.clone(), child]);
        let copied = original.deepcopy().sequence().unwrap();
        assert!(copied[0].is_identical(&copied[1]));
        assert!(!copied[0].is_identical(&original.sequence().unwrap()[0]));
        let cyclic = Value::list(vec![]);
        if let Value::List(v) = &cyclic {
            v.borrow_mut().push(cyclic.clone());
        }
        let clone = cyclic.deepcopy();
        assert!(clone.is_identical(&clone.sequence().unwrap()[0]));
        // Break reference cycles explicitly in this test.
        for v in [clone, cyclic] {
            if let Value::List(v) = v {
                v.borrow_mut().clear();
            }
        }
    }
}
