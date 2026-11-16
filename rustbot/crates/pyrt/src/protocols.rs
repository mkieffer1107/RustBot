//! Lazy builtin iteration, arbitrary-precision ranges and live dictionary views.
//!
//! Dictionary/set iterators detect size changes. Same-size key substitutions
//! cannot be fully detected without a mutation version in the core containers.
use crate::{
    PyExc, PyResult, Value, formatting,
    object::{CallArgs, IteratorObject, NativeFunction, PyObject},
    runtime::Runtime,
};
use num_bigint::BigInt;
use num_traits::{FromPrimitive, One, Signed, ToPrimitive, Zero};
use std::{any::Any, cell::RefCell, collections::HashSet, rc::Rc};

fn runtime_error(message: &str) -> PyExc {
    PyExc {
        kind: "RuntimeError",
        message: message.into(),
    }
}
fn equal(a: &Value, b: &Value) -> PyResult<bool> {
    Ok(a.is_identical(b) || a.equals(b)?)
}

#[derive(Clone, Debug)]
pub struct RangeObject {
    start: BigInt,
    stop: BigInt,
    step: BigInt,
    length: BigInt,
}

pub fn range(start: BigInt, stop: BigInt, step: BigInt) -> PyResult<Value> {
    if step.is_zero() {
        return Err(PyExc::value_error("range() arg 3 must not be zero"));
    }
    let distance = if step.is_positive() {
        &stop - &start
    } else {
        &start - &stop
    };
    let length = if distance.is_positive() {
        (distance - BigInt::one()) / step.abs() + BigInt::one()
    } else {
        BigInt::zero()
    };
    Ok(Value::Object(Rc::new(RangeObject {
        start,
        stop,
        step,
        length,
    })))
}

impl RangeObject {
    fn integer_candidate(value: &Value) -> Option<BigInt> {
        match value {
            Value::Int(n) => Some(n.clone()),
            Value::Bool(b) => Some(BigInt::from(*b)),
            Value::Float(f) if f.is_finite() && f.fract() == 0.0 => BigInt::from_f64(*f),
            _ => None,
        }
    }
    fn position(&self, value: &Value) -> Option<BigInt> {
        let value = Self::integer_candidate(value)?;
        if self.length.is_zero() {
            return None;
        }
        let distance = value - &self.start;
        if !(&distance % &self.step).is_zero() {
            return None;
        }
        let index = distance / &self.step;
        if index.is_negative() || index >= self.length {
            None
        } else {
            Some(index)
        }
    }
    fn iterator(&self, reverse: bool) -> Value {
        let mut remaining = self.length.clone();
        let mut current = if reverse && !remaining.is_zero() {
            &self.start + (&remaining - BigInt::one()) * &self.step
        } else {
            self.start.clone()
        };
        let step = if reverse {
            -&self.step
        } else {
            self.step.clone()
        };
        IteratorObject::value(move |_| {
            if remaining.is_zero() {
                return Ok(None);
            }
            let value = Value::Int(current.clone());
            current += &step;
            remaining -= 1;
            Ok(Some(value))
        })
    }
    pub fn slice(
        &self,
        start: Option<BigInt>,
        stop: Option<BigInt>,
        step: BigInt,
    ) -> PyResult<Value> {
        if step.is_zero() {
            return Err(PyExc::value_error("slice step cannot be zero"));
        }
        let backwards = step.is_negative();
        let minimum = if backwards {
            -BigInt::one()
        } else {
            BigInt::zero()
        };
        let maximum = if backwards {
            &self.length - BigInt::one()
        } else {
            self.length.clone()
        };
        let normalize = |mut n: BigInt| {
            if n.is_negative() {
                n += &self.length;
            }
            n.max(minimum.clone()).min(maximum.clone())
        };
        let start = start.map(normalize).unwrap_or_else(|| {
            if backwards {
                &self.length - BigInt::one()
            } else {
                BigInt::zero()
            }
        });
        let stop = stop.map(normalize).unwrap_or_else(|| {
            if backwards {
                -BigInt::one()
            } else {
                self.length.clone()
            }
        });
        range(
            &self.start + start * &self.step,
            &self.start + stop * &self.step,
            &self.step * step,
        )
    }
}

impl PyObject for RangeObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "range"
    }
    fn repr(&self) -> PyResult<String> {
        Ok(if self.step == BigInt::one() {
            format!("range({}, {})", self.start, self.stop)
        } else {
            format!("range({}, {}, {})", self.start, self.stop, self.step)
        })
    }
    fn is_truthy(&self) -> bool {
        !self.length.is_zero()
    }
    fn len(&self) -> PyResult<usize> {
        self.length
            .to_usize()
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or_else(|| PyExc::overflow_error("Python int too large to convert to C ssize_t"))
    }
    fn get_item(&self, index: &Value) -> PyResult<Value> {
        let mut index = index.as_int().map_err(|_| {
            PyExc::type_error(format!(
                "range indices must be integers or slices, not {}",
                index.type_name()
            ))
        })?;
        if index.is_negative() {
            index += &self.length;
        }
        if index.is_negative() || index >= self.length {
            return Err(PyExc::index_error("range object index out of range"));
        }
        Ok(Value::Int(&self.start + index * &self.step))
    }
    fn contains(&self, item: &Value) -> PyResult<bool> {
        if matches!(item, Value::Object(_)) {
            // User-defined equality is supported without converting the range
            // length to a machine integer. Builtin numeric keys take O(1).
            let mut current = self.start.clone();
            let mut remaining = self.length.clone();
            while !remaining.is_zero() {
                if equal(&Value::Int(current.clone()), item)? {
                    return Ok(true);
                }
                current += &self.step;
                remaining -= 1;
            }
            Ok(false)
        } else {
            Ok(self.position(item).is_some())
        }
    }
    fn equals(&self, other: &Value) -> PyResult<bool> {
        let Value::Object(other) = other else {
            return Ok(false);
        };
        let Some(other) = other.as_any().downcast_ref::<Self>() else {
            return Ok(false);
        };
        Ok(self.length == other.length
            && (self.length.is_zero()
                || (self.start == other.start
                    && (self.length == BigInt::one() || self.step == other.step))))
    }
    fn hash(&self) -> PyResult<i64> {
        let start = if self.length.is_zero() {
            Value::None
        } else {
            Value::Int(self.start.clone())
        };
        let step = if self.length <= BigInt::one() {
            Value::None
        } else {
            Value::Int(self.step.clone())
        };
        crate::sets::hash(&Value::tuple(vec![
            Value::Int(self.length.clone()),
            start,
            step,
        ]))
    }
    fn sequence(&self) -> PyResult<Vec<Value>> {
        let len = self.len()?;
        let mut out = Vec::new();
        out.try_reserve(len)
            .map_err(|_| PyExc::memory_error("cannot allocate range sequence"))?;
        let mut current = self.start.clone();
        for _ in 0..len {
            out.push(Value::Int(current.clone()));
            current += &self.step;
        }
        Ok(out)
    }
    fn iter(&self, _rt: &mut Runtime) -> PyResult<Value> {
        Ok(self.iterator(false))
    }
    fn get_attr(&self, _rt: &mut Runtime, name: &str) -> PyResult<Value> {
        match name {
            "start" => Ok(Value::Int(self.start.clone())),
            "stop" => Ok(Value::Int(self.stop.clone())),
            "step" => Ok(Value::Int(self.step.clone())),
            "__reversed__" => {
                let range = self.clone();
                Ok(NativeFunction::value(name, move |_, args| {
                    args.reject_keywords()?;
                    args.arity(0, 0)?;
                    Ok(range.iterator(true))
                }))
            }
            "count" | "index" => {
                let range = self.clone();
                let method = name.to_owned();
                Ok(NativeFunction::value(name, move |_, args: CallArgs| {
                    args.reject_keywords()?;
                    args.arity(1, 1)?;
                    let position = range.position(&args.positional[0]);
                    if method == "count" {
                        Ok(Value::from(i64::from(position.is_some())))
                    } else {
                        position.map(Value::Int).ok_or_else(|| {
                            PyExc::value_error(format!(
                                "{} is not in range",
                                formatting::repr(&args.positional[0]).unwrap_or_default()
                            ))
                        })
                    }
                }))
            }
            _ => Err(PyExc::attribute_error(format!(
                "'range' object has no attribute '{name}'"
            ))),
        }
    }
}

pub fn slice_range(
    value: &Value,
    start: Option<BigInt>,
    stop: Option<BigInt>,
    step: BigInt,
) -> PyResult<Value> {
    if let Value::Object(object) = value
        && let Some(range) = object.as_any().downcast_ref::<RangeObject>()
    {
        return range.slice(start, stop, step);
    }
    Err(PyExc::type_error("expected range"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewKind {
    Keys,
    Values,
    Items,
}

#[derive(Clone, Debug)]
pub struct ViewObject {
    dict: Value,
    kind: ViewKind,
}

pub fn dict_view(dict: &Value, kind: ViewKind) -> PyResult<Value> {
    if !matches!(dict, Value::Dict(_)) {
        return Err(PyExc::type_error("expected dict"));
    }
    Ok(Value::Object(Rc::new(ViewObject {
        dict: dict.clone(),
        kind,
    })))
}

thread_local! {static ACTIVE_REPRS:RefCell<HashSet<usize>>=RefCell::new(HashSet::new());}

impl ViewObject {
    fn snapshot(&self) -> Vec<Value> {
        let Value::Dict(dict) = &self.dict else {
            unreachable!()
        };
        dict.borrow()
            .iter()
            .map(|(k, v)| match self.kind {
                ViewKind::Keys => k.clone(),
                ViewKind::Values => v.clone(),
                ViewKind::Items => Value::tuple(vec![k.clone(), v.clone()]),
            })
            .collect()
    }
    fn iterator(&self, reverse: bool) -> PyResult<Value> {
        dict_iterator(&self.dict, self.kind, reverse)
    }
}

impl PyObject for ViewObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        match self.kind {
            ViewKind::Keys => "dict_keys",
            ViewKind::Values => "dict_values",
            ViewKind::Items => "dict_items",
        }
    }
    fn len(&self) -> PyResult<usize> {
        let Value::Dict(dict) = &self.dict else {
            unreachable!()
        };
        Ok(dict.borrow().len())
    }
    fn is_truthy(&self) -> bool {
        self.len().is_ok_and(|n| n > 0)
    }
    fn repr(&self) -> PyResult<String> {
        let identity = self as *const Self as usize;
        if !ACTIVE_REPRS.with(|active| active.borrow_mut().insert(identity)) {
            return Ok("...".into());
        }
        let result = formatting::repr(&Value::list(self.snapshot()))
            .map(|inner| format!("{}({inner})", self.type_name()));
        ACTIVE_REPRS.with(|active| active.borrow_mut().remove(&identity));
        result
    }
    fn sequence(&self) -> PyResult<Vec<Value>> {
        Ok(self.snapshot())
    }
    fn iter(&self, _rt: &mut Runtime) -> PyResult<Value> {
        self.iterator(false)
    }
    fn contains(&self, item: &Value) -> PyResult<bool> {
        match self.kind {
            ViewKind::Keys => match self.dict.dict_get(item) {
                Ok(_) => Ok(true),
                Err(e) if e.kind == "KeyError" => Ok(false),
                Err(e) => Err(e),
            },
            ViewKind::Values => {
                for value in self.snapshot() {
                    if equal(&value, item)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            ViewKind::Items => {
                let Value::Tuple(pair) = item else {
                    return Ok(false);
                };
                if pair.len() != 2 {
                    return Ok(false);
                }
                match self.dict.dict_get(&pair[0]) {
                    Ok(value) => equal(&value, &pair[1]),
                    Err(e) if e.kind == "KeyError" => Ok(false),
                    Err(e) => Err(e),
                }
            }
        }
    }
    fn equals(&self, other: &Value) -> PyResult<bool> {
        if self.kind == ViewKind::Values {
            return Ok(false);
        }
        let other_values = match other {
            Value::Set(s) | Value::FrozenSet(s) => s.values(),
            Value::Object(o) => {
                let Some(view) = o.as_any().downcast_ref::<Self>() else {
                    return Ok(false);
                };
                if view.kind == ViewKind::Values {
                    return Ok(false);
                }
                view.snapshot()
            }
            _ => return Ok(false),
        };
        if self.len()? != other_values.len() {
            return Ok(false);
        }
        for item in other_values {
            if !self.contains(&item)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn get_attr(&self, _rt: &mut Runtime, name: &str) -> PyResult<Value> {
        if name == "__reversed__" {
            let view = self.clone();
            return Ok(NativeFunction::value(name, move |_, args| {
                args.reject_keywords()?;
                args.arity(0, 0)?;
                view.iterator(true)
            }));
        }
        if name == "isdisjoint" && self.kind != ViewKind::Values {
            let view = self.clone();
            return Ok(NativeFunction::value(name, move |rt, args| {
                args.reject_keywords()?;
                args.arity(1, 1)?;
                let iterator = rt.iter(&args.positional[0])?;
                while let Some(item) = rt.next(&iterator)? {
                    if view.contains(&item)? {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            }));
        }
        Err(PyExc::attribute_error(format!(
            "'{}' object has no attribute '{name}'",
            self.type_name()
        )))
    }
}

fn dict_iterator(dict: &Value, kind: ViewKind, reverse: bool) -> PyResult<Value> {
    let Value::Dict(values) = dict else {
        return Err(PyExc::type_error("expected dict"));
    };
    let values = values.clone();
    let expected = values.borrow().len();
    let mut index = if reverse { expected as i128 - 1 } else { 0 };
    let mut invalid = false;
    Ok(IteratorObject::value(move |_| {
        let values = values.borrow();
        if invalid || values.len() != expected {
            invalid = true;
            return Err(runtime_error("dictionary changed size during iteration"));
        }
        if index < 0 || index >= values.len() as i128 {
            return Ok(None);
        }
        let (key, value) = &values[index as usize];
        index += if reverse { -1 } else { 1 };
        Ok(Some(match kind {
            ViewKind::Keys => key.clone(),
            ViewKind::Values => value.clone(),
            ViewKind::Items => Value::tuple(vec![key.clone(), value.clone()]),
        }))
    }))
}

/// Builtin iterators retain source storage rather than snapshotting mutable lists
/// or dictionaries. Reverse-list iteration starts at the original final index.
pub fn iter(value: &Value, reverse: bool) -> PyResult<Value> {
    match value {
        Value::List(list) => {
            let list = list.clone();
            let mut index = if reverse {
                list.borrow().len() as i128 - 1
            } else {
                0
            };
            Ok(IteratorObject::value(move |_| {
                let list = list.borrow();
                if index < 0 || index >= list.len() as i128 {
                    return Ok(None);
                }
                let value = list[index as usize].clone();
                index += if reverse { -1 } else { 1 };
                Ok(Some(value))
            }))
        }
        Value::Tuple(values) => {
            let values = values.clone();
            let mut index = if reverse { values.len() as i128 - 1 } else { 0 };
            Ok(IteratorObject::value(move |_| {
                if index < 0 || index >= values.len() as i128 {
                    return Ok(None);
                }
                let value = values[index as usize].clone();
                index += if reverse { -1 } else { 1 };
                Ok(Some(value))
            }))
        }
        Value::Str(text) => {
            let text = text.clone();
            let mut index = if reverse { text.len() as i128 - 1 } else { 0 };
            Ok(IteratorObject::value(move |_| {
                if index < 0 || index >= text.len() as i128 {
                    return Ok(None);
                }
                let value = Value::Str(text.get_index(index as i64)?);
                index += if reverse { -1 } else { 1 };
                Ok(Some(value))
            }))
        }
        Value::Dict(_) => dict_iterator(value, ViewKind::Keys, reverse),
        Value::Set(set) | Value::FrozenSet(set) => {
            if reverse {
                return Err(PyExc::type_error(format!(
                    "'{}' object is not reversible",
                    value.type_name()
                )));
            }
            let source = set.clone();
            let expected = set.len();
            let mut values = set.values().into_iter();
            let mut invalid = false;
            Ok(IteratorObject::value(move |_| {
                if invalid || source.len() != expected {
                    invalid = true;
                    return Err(runtime_error("Set changed size during iteration"));
                }
                Ok(values.next())
            }))
        }
        Value::Object(object) => {
            if let Some(range) = object.as_any().downcast_ref::<RangeObject>() {
                return Ok(range.iterator(reverse));
            }
            if let Some(view) = object.as_any().downcast_ref::<ViewObject>() {
                return view.iterator(reverse);
            }
            if object.is_iterator() && !reverse {
                return Ok(value.clone());
            }
            if reverse {
                let object = object.clone();
                let mut index = object.len()? as i128 - 1;
                Ok(IteratorObject::value(move |_| {
                    if index < 0 {
                        return Ok(None);
                    }
                    let result = object.get_item(&Value::from(index as i64));
                    index -= 1;
                    match result {
                        Ok(value) => Ok(Some(value)),
                        Err(e) if e.kind == "IndexError" || e.kind == "StopIteration" => Ok(None),
                        Err(e) => Err(e),
                    }
                }))
            } else {
                Ok(IteratorObject::from_values(object.sequence()?))
            }
        }
        _ => Err(PyExc::type_error(format!(
            "'{}' object is not {}",
            value.type_name(),
            if reverse { "reversible" } else { "iterable" }
        ))),
    }
}
