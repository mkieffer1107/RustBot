//! Python container methods and sequence mutations.
//!
//! Method iterable arguments currently materialize snapshots. Callable sort keys
//! and live iterator/view objects belong to the interpreter's object layer.
use crate::{
    PyExc, PyResult, Value, formatting,
    sets::{self, PySet},
};
use num_traits::ToPrimitive;
use std::cell::RefCell;
use std::rc::Rc;

fn same(a: &Value, b: &Value) -> PyResult<bool> {
    Ok(a.is_identical(b) || a.equals(b)?)
}
fn missing(receiver: &Value, method: &str) -> PyExc {
    PyExc::attribute_error(format!(
        "'{}' object has no attribute '{method}'",
        receiver.type_name()
    ))
}
fn arity(method: &str, args: &[Value], min: usize, max: usize) -> PyResult<()> {
    let n = args.len();
    if min == max && n != min {
        return Err(PyExc::type_error(format!(
            "{method} expected {min} arguments, got {n}"
        )));
    }
    if n < min {
        return Err(PyExc::type_error(format!(
            "{method} expected at least {min} argument{}, got {n}",
            if min == 1 { "" } else { "s" }
        )));
    }
    if n > max {
        return Err(PyExc::type_error(format!(
            "{method} expected at most {max} argument{}, got {n}",
            if max == 1 { "" } else { "s" }
        )));
    }
    Ok(())
}
fn one(receiver: &Value, method: &str, args: &[Value]) -> PyResult<()> {
    if args.len() != 1 {
        return Err(PyExc::type_error(format!(
            "{}.{}() takes exactly one argument ({} given)",
            receiver.type_name(),
            method,
            args.len()
        )));
    }
    Ok(())
}
fn none(receiver: &Value, method: &str, args: &[Value]) -> PyResult<()> {
    if !args.is_empty() {
        return Err(PyExc::type_error(format!(
            "{}.{}() takes no arguments ({} given)",
            receiver.type_name(),
            method,
            args.len()
        )));
    }
    Ok(())
}
fn clipped_index(value: &Value) -> PyResult<i64> {
    let n = value.as_int().map_err(|_| {
        PyExc::type_error("slice indices must be integers or have an __index__ method")
    })?;
    Ok(n.to_i64().unwrap_or_else(|| {
        if n.sign() == num_bigint::Sign::Minus {
            i64::MIN
        } else {
            i64::MAX
        }
    }))
}
fn item_index(index: &Value, len: usize, message: &str) -> PyResult<usize> {
    let n = index
        .as_int()
        .map_err(|_| {
            PyExc::type_error(format!(
                "list indices must be integers or slices, not {}",
                index.type_name()
            ))
        })?
        .to_i64()
        .ok_or_else(|| PyExc::index_error("cannot fit int into an index-sized integer"))?;
    let n = if n < 0 {
        len as i128 + i128::from(n)
    } else {
        i128::from(n)
    };
    if n < 0 || n >= len as i128 {
        return Err(PyExc::index_error(message));
    }
    Ok(n as usize)
}
fn bound(index: i64, n: usize) -> usize {
    (if index < 0 {
        n as i128 + i128::from(index)
    } else {
        i128::from(index)
    })
    .clamp(0, n as i128) as usize
}
fn key_error(key: &Value) -> PyExc {
    PyExc::key_error(formatting::repr(key).unwrap_or_else(|_| "key not found".into()))
}

pub fn call_method(receiver: &Value, method: &str, args: &[Value]) -> PyResult<Value> {
    match receiver {
        Value::List(_) | Value::Tuple(_) => sequence_method(receiver, method, args),
        Value::Dict(_) => dict_method(receiver, method, args),
        Value::Set(_) | Value::FrozenSet(_) => set_method(receiver, method, args),
        _ => Err(missing(receiver, method)),
    }
}

fn sequence_method(receiver: &Value, method: &str, args: &[Value]) -> PyResult<Value> {
    if matches!(method, "count" | "index") {
        if method == "count" {
            one(receiver, method, args)?;
        } else {
            arity(method, args, 1, 3)?;
        }
        let values = receiver.sequence()?;
        let start = args
            .get(1)
            .map(clipped_index)
            .transpose()?
            .map_or(0, |n| bound(n, values.len()));
        let stop = args
            .get(2)
            .map(clipped_index)
            .transpose()?
            .map_or(values.len(), |n| bound(n, values.len()));
        let mut count = 0usize;
        for (i, v) in values.iter().enumerate().take(stop).skip(start) {
            if same(v, &args[0])? {
                if method == "index" {
                    return Ok(Value::from(i));
                }
                count += 1;
            }
        }
        if method == "index" {
            return Err(PyExc::value_error(if matches!(receiver, Value::List(_)) {
                format!("{} is not in list", formatting::repr(&args[0])?)
            } else {
                "tuple.index(x): x not in tuple".into()
            }));
        }
        return Ok(Value::from(count));
    }
    let Value::List(list) = receiver else {
        return Err(missing(receiver, method));
    };
    match method {
        "append" => {
            one(receiver, method, args)?;
            list.borrow_mut().push(args[0].clone());
        }
        "extend" => {
            one(receiver, method, args)?;
            let values = args[0].sequence()?;
            list.borrow_mut().extend(values);
        }
        "insert" => {
            arity(method, args, 2, 2)?;
            let n = args[0].as_int()?.to_i64().ok_or_else(|| {
                PyExc::overflow_error("Python int too large to convert to C ssize_t")
            })?;
            let mut values = list.borrow_mut();
            let i = bound(n, values.len());
            values.insert(i, args[1].clone());
        }
        "remove" => {
            one(receiver, method, args)?;
            let values = list.borrow().clone();
            let mut found = None;
            for (i, v) in values.iter().enumerate() {
                if same(v, &args[0])? {
                    found = Some(i);
                    break;
                }
            }
            let i = found.ok_or_else(|| PyExc::value_error("list.remove(x): x not in list"))?;
            list.borrow_mut().remove(i);
        }
        "pop" => {
            arity(method, args, 0, 1)?;
            // Python validates index type before checking emptiness.
            let index = args.first().cloned().unwrap_or_else(|| Value::from(-1i64));
            let len = list.borrow().len();
            let _ = index
                .as_int()
                .map_err(|_| {
                    PyExc::type_error(format!(
                        "'{}' object cannot be interpreted as an integer",
                        index.type_name()
                    ))
                })?
                .to_i64()
                .ok_or_else(|| {
                    PyExc::overflow_error("Python int too large to convert to C ssize_t")
                })?;
            if len == 0 {
                return Err(PyExc::index_error("pop from empty list"));
            }
            let i = item_index(&index, len, "pop index out of range")?;
            return Ok(list.borrow_mut().remove(i));
        }
        "clear" => {
            none(receiver, method, args)?;
            list.borrow_mut().clear();
        }
        "copy" => {
            none(receiver, method, args)?;
            return Ok(Value::list(list.borrow().clone()));
        }
        "reverse" => {
            none(receiver, method, args)?;
            list.borrow_mut().reverse();
        }
        _ => return Err(missing(receiver, method)),
    }
    Ok(Value::None)
}

fn dict_position(dict: &Value, key: &Value) -> PyResult<Option<usize>> {
    // Validate hashability through the core dictionary API, which also accepts
    // typed date keys even when a set hash implementation is unavailable.
    match dict.dict_get(key) {
        Ok(_) => (),
        Err(e) if e.kind == "KeyError" => return Ok(None),
        Err(e) => return Err(e),
    }
    let Value::Dict(values) = dict else {
        unreachable!()
    };
    let keys: Vec<Value> = values.borrow().iter().map(|(k, _)| k.clone()).collect();
    for (i, k) in keys.iter().enumerate() {
        if same(k, key)? {
            return Ok(Some(i));
        }
    }
    Ok(None)
}

fn dict_method(receiver: &Value, method: &str, args: &[Value]) -> PyResult<Value> {
    let Value::Dict(dict) = receiver else {
        unreachable!()
    };
    match method {
        "get" | "setdefault" | "pop" => {
            arity(method, args, 1, 2)?;
            if method == "pop" && dict.borrow().is_empty() {
                return args.get(1).cloned().ok_or_else(|| key_error(&args[0]));
            }
            let position = dict_position(receiver, &args[0])?;
            if let Some(i) = position {
                return Ok(if method == "pop" {
                    dict.borrow_mut().remove(i).1
                } else {
                    dict.borrow()[i].1.clone()
                });
            }
            if method == "pop" && args.len() == 1 {
                return Err(key_error(&args[0]));
            }
            let default = args.get(1).cloned().unwrap_or(Value::None);
            if method == "setdefault" {
                receiver.dict_set(args[0].clone(), default.clone())?;
            }
            Ok(default)
        }
        "update" => {
            arity(method, args, 0, 1)?;
            if let Some(other) = args.first() {
                if let Value::Dict(other) = other {
                    let pairs = other.borrow().clone();
                    for (key, value) in pairs {
                        receiver.dict_set(key, value)?;
                    }
                } else {
                    for (i, pair) in other.sequence()?.into_iter().enumerate() {
                        let pair = pair.sequence().map_err(|_| PyExc::type_error(format!("cannot convert dictionary update sequence element #{i} to a sequence")))?;
                        if pair.len() != 2 {
                            return Err(PyExc::value_error(format!(
                                "dictionary update sequence element #{i} has length {}; 2 is required",
                                pair.len()
                            )));
                        }
                        receiver.dict_set(pair[0].clone(), pair[1].clone())?;
                    }
                }
            }
            Ok(Value::None)
        }
        "popitem" => {
            none(receiver, method, args)?;
            let (k, v) = dict
                .borrow_mut()
                .pop()
                .ok_or_else(|| PyExc::key_error("popitem(): dictionary is empty"))?;
            Ok(Value::tuple(vec![k, v]))
        }
        "keys" | "values" | "items" => {
            none(receiver, method, args)?;
            crate::protocols::dict_view(
                receiver,
                match method {
                    "keys" => crate::protocols::ViewKind::Keys,
                    "values" => crate::protocols::ViewKind::Values,
                    _ => crate::protocols::ViewKind::Items,
                },
            )
        }
        "clear" => {
            none(receiver, method, args)?;
            dict.borrow_mut().clear();
            Ok(Value::None)
        }
        "copy" => {
            none(receiver, method, args)?;
            Ok(Value::Dict(Rc::new(RefCell::new(dict.borrow().clone()))))
        }
        _ => Err(missing(receiver, method)),
    }
}

fn as_set(v: &Value) -> Option<&PySet> {
    if let Value::Set(s) | Value::FrozenSet(s) = v {
        Some(s)
    } else {
        None
    }
}
fn wrap_set(receiver: &Value, s: PySet) -> Value {
    if matches!(receiver, Value::FrozenSet(_)) {
        Value::FrozenSet(s)
    } else {
        Value::Set(s)
    }
}
fn update_set(target: &PySet, other: &Value) -> PyResult<()> {
    if let Some(other) = as_set(other) {
        target.update_set(other)
    } else {
        let values = other.sequence()?;
        if matches!(other, Value::Dict(_)) {
            target.reserve_for_update(values.len());
        }
        for v in values {
            target.insert(v)?;
        }
        Ok(())
    }
}
fn intersection(target: &PySet, other: &Value) -> PyResult<PySet> {
    let result = PySet::new();
    if let Some(other) = as_set(other) {
        if target.is_identical(other) {
            return target.python_copy();
        }
        let (larger, smaller) = if other.len() > target.len() {
            (other, target)
        } else {
            (target, other)
        };
        for v in smaller.values() {
            if larger.contains(&v)? {
                result.insert(v)?;
            }
        }
    } else {
        for v in other.sequence()? {
            sets::hash(&v)?;
            if target.contains(&v)? {
                result.insert(v)?;
                if result.len() >= target.len() {
                    break;
                }
            }
        }
    }
    Ok(result)
}
fn difference_update(target: &PySet, other: &Value) -> PyResult<()> {
    if let Some(other_set) = as_set(other) {
        if target.is_identical(other_set) {
            target.clear();
            return Ok(());
        }
        let values = if other_set.len() >> 3 > target.len() {
            intersection(target, other)?.values()
        } else {
            other_set.values()
        };
        for v in values {
            target.discard(&v)?;
        }
    } else {
        for v in other.sequence()? {
            sets::element_hash(&v)?;
            target.discard(&v)?;
        }
    }
    target.compact_after_difference();
    Ok(())
}
fn difference(target: &PySet, other: &Value) -> PyResult<PySet> {
    let size = match other {
        Value::Set(s) | Value::FrozenSet(s) => Some(s.len()),
        Value::Dict(d) => Some(d.borrow().len()),
        _ => None,
    };
    if size.is_none_or(|size| (target.len() >> 2) > size) {
        let result = target.python_copy()?;
        difference_update(&result, other)?;
        return Ok(result);
    }
    let result = PySet::new();
    for v in target.values() {
        if !crate::ops::contains(other, &v)? {
            result.insert(v)?;
        }
    }
    Ok(result)
}
fn symmetric_difference_update(target: &PySet, other: &Value) -> PyResult<()> {
    if as_set(other).is_some_and(|s| s.is_identical(target)) {
        target.clear();
        return Ok(());
    }
    let values = if matches!(other, Value::Dict(_)) {
        other.sequence()?
    } else if let Some(s) = as_set(other) {
        s.values()
    } else {
        PySet::from_values(other.sequence()?)?.values()
    };
    for v in values {
        if !target.discard(&v)? {
            target.insert(v)?;
        }
    }
    Ok(())
}

fn set_method(receiver: &Value, method: &str, args: &[Value]) -> PyResult<Value> {
    let s = as_set(receiver).expect("set receiver");
    if matches!(receiver, Value::FrozenSet(_))
        && matches!(
            method,
            "add"
                | "discard"
                | "remove"
                | "pop"
                | "clear"
                | "update"
                | "intersection_update"
                | "difference_update"
                | "symmetric_difference_update"
        )
    {
        return Err(missing(receiver, method));
    }
    match method {
        "add" => {
            one(receiver, method, args)?;
            s.insert(args[0].clone())?;
        }
        "discard" => {
            one(receiver, method, args)?;
            s.discard(&args[0])?;
        }
        "remove" => {
            one(receiver, method, args)?;
            if !s.discard(&args[0])? {
                return Err(key_error(&args[0]));
            }
        }
        "pop" => {
            none(receiver, method, args)?;
            return s.pop();
        }
        "clear" => {
            none(receiver, method, args)?;
            s.clear();
        }
        "copy" => {
            none(receiver, method, args)?;
            return if matches!(receiver, Value::FrozenSet(_)) {
                Ok(receiver.clone())
            } else {
                Ok(Value::Set(s.python_copy()?))
            };
        }
        "update" => {
            for other in args {
                update_set(s, other)?;
            }
        }
        "union" => {
            let result = s.python_copy()?;
            for other in args {
                if !as_set(other).is_some_and(|other| other.is_identical(s)) {
                    update_set(&result, other)?;
                }
            }
            return Ok(wrap_set(receiver, result));
        }
        "intersection" | "intersection_update" => {
            let mut result = s.python_copy()?;
            for (i, other) in args.iter().enumerate() {
                result = intersection(if i == 0 { s } else { &result }, other)?;
            }
            if method == "intersection" {
                return Ok(wrap_set(receiver, result));
            }
            s.replace_contents(&result);
        }
        "difference" => {
            let result = if let Some(first) = args.first() {
                difference(s, first)?
            } else {
                s.python_copy()?
            };
            for other in args.iter().skip(1) {
                difference_update(&result, other)?;
            }
            return Ok(wrap_set(receiver, result));
        }
        "difference_update" => {
            for other in args {
                difference_update(s, other)?;
            }
        }
        "symmetric_difference" => {
            one(receiver, method, args)?;
            let result = PySet::new();
            update_set(&result, &args[0])?;
            symmetric_difference_update(&result, receiver)?;
            return Ok(wrap_set(receiver, result));
        }
        "symmetric_difference_update" => {
            one(receiver, method, args)?;
            symmetric_difference_update(s, &args[0])?;
        }
        "isdisjoint" => {
            one(receiver, method, args)?;
            if let Some(other) = as_set(&args[0]) {
                let (larger, smaller) = if other.len() > s.len() {
                    (other, s)
                } else {
                    (s, other)
                };
                for v in smaller.values() {
                    if larger.contains(&v)? {
                        return Ok(Value::Bool(false));
                    }
                }
            } else {
                for v in args[0].sequence()? {
                    sets::element_hash(&v)?;
                    if s.contains(&v)? {
                        return Ok(Value::Bool(false));
                    }
                }
            }
            return Ok(Value::Bool(true));
        }
        "issubset" | "issuperset" => {
            one(receiver, method, args)?;
            if method == "issuperset" && as_set(&args[0]).is_none() {
                for value in args[0].sequence()? {
                    sets::element_hash(&value)?;
                    if !s.contains(&value)? {
                        return Ok(Value::Bool(false));
                    }
                }
                return Ok(Value::Bool(true));
            }
            let built;
            let other = if let Some(other) = as_set(&args[0]) {
                other
            } else {
                built = PySet::from_values(args[0].sequence()?)?;
                &built
            };
            let (small, large) = if method == "issubset" {
                (s, other)
            } else {
                (other, s)
            };
            if small.len() > large.len() {
                return Ok(Value::Bool(false));
            }
            for v in small.values() {
                if !large.contains(&v)? {
                    return Ok(Value::Bool(false));
                }
            }
            return Ok(Value::Bool(true));
        }
        _ => return Err(missing(receiver, method)),
    }
    Ok(Value::None)
}

fn slice_bounds(
    n: usize,
    start: Option<i64>,
    stop: Option<i64>,
    step: i64,
) -> PyResult<(i128, i128)> {
    if step == 0 {
        return Err(PyExc::value_error("slice step cannot be zero"));
    }
    let n = n as i128;
    let normalize = |v: i64| {
        let v = if v < 0 {
            n + i128::from(v)
        } else {
            i128::from(v)
        };
        if step < 0 {
            v.clamp(-1, n - 1)
        } else {
            v.clamp(0, n)
        }
    };
    Ok((
        start
            .map(normalize)
            .unwrap_or(if step < 0 { n - 1 } else { 0 }),
        stop.map(normalize).unwrap_or(if step < 0 { -1 } else { n }),
    ))
}
fn slice_indices(
    n: usize,
    start: Option<i64>,
    stop: Option<i64>,
    step: i64,
) -> PyResult<Vec<usize>> {
    let (mut i, end) = slice_bounds(n, start, stop, step)?;
    let mut out = Vec::new();
    while if step < 0 { i > end } else { i < end } {
        out.push(i as usize);
        i += i128::from(step);
    }
    Ok(out)
}
pub fn slice(value: &Value, start: Option<i64>, stop: Option<i64>, step: i64) -> PyResult<Value> {
    match value {
        Value::Str(s) => Ok(Value::Str(s.slice(start, stop, step)?)),
        Value::List(_) | Value::Tuple(_) => {
            let values = value.sequence()?;
            let result = slice_indices(values.len(), start, stop, step)?
                .into_iter()
                .map(|i| values[i].clone())
                .collect();
            Ok(if matches!(value, Value::Tuple(_)) {
                Value::tuple(result)
            } else {
                Value::list(result)
            })
        }
        _ => Err(PyExc::type_error(format!(
            "'{}' object is not subscriptable",
            value.type_name()
        ))),
    }
}
pub fn setitem(receiver: &Value, index: &Value, value: Value) -> PyResult<()> {
    match receiver {
        Value::Dict(_) => receiver.dict_set(index.clone(), value),
        Value::List(list) => {
            let len = list.borrow().len();
            let i = item_index(index, len, "list assignment index out of range")?;
            list.borrow_mut()[i] = value;
            Ok(())
        }
        _ => Err(PyExc::type_error(format!(
            "'{}' object does not support item assignment",
            receiver.type_name()
        ))),
    }
}
pub fn delitem(receiver: &Value, index: &Value) -> PyResult<()> {
    match receiver {
        Value::Dict(dict) => {
            let i = dict_position(receiver, index)?.ok_or_else(|| key_error(index))?;
            dict.borrow_mut().remove(i);
            Ok(())
        }
        Value::List(list) => {
            let len = list.borrow().len();
            let i = item_index(index, len, "list assignment index out of range")?;
            list.borrow_mut().remove(i);
            Ok(())
        }
        _ => Err(PyExc::type_error(format!(
            "'{}' object doesn't support item deletion",
            receiver.type_name()
        ))),
    }
}
pub fn setslice(
    receiver: &Value,
    start: Option<i64>,
    stop: Option<i64>,
    step: i64,
    replacement: &Value,
) -> PyResult<()> {
    let Value::List(list) = receiver else {
        return Err(PyExc::type_error(format!(
            "'{}' object does not support item assignment",
            receiver.type_name()
        )));
    };
    let len = list.borrow().len();
    let (start_bound, end_bound) = slice_bounds(len, start, stop, step)?;
    let values = replacement.sequence().map_err(|_| {
        PyExc::type_error(if step == 1 {
            "can only assign an iterable"
        } else {
            "must assign iterable to extended slice"
        })
    })?;
    if step == 1 {
        list.borrow_mut().splice(
            start_bound as usize..end_bound.max(start_bound) as usize,
            values,
        );
    } else {
        let indices = slice_indices(len, start, stop, step)?;
        if indices.len() != values.len() {
            return Err(PyExc::value_error(format!(
                "attempt to assign sequence of size {} to extended slice of size {}",
                values.len(),
                indices.len()
            )));
        }
        let mut target = list.borrow_mut();
        for (i, v) in indices.into_iter().zip(values) {
            target[i] = v;
        }
    }
    Ok(())
}
pub fn delslice(
    receiver: &Value,
    start: Option<i64>,
    stop: Option<i64>,
    step: i64,
) -> PyResult<()> {
    let Value::List(list) = receiver else {
        return Err(PyExc::type_error(format!(
            "'{}' object does not support item deletion",
            receiver.type_name()
        )));
    };
    let mut indices = slice_indices(list.borrow().len(), start, stop, step)?;
    indices.sort_unstable_by(|a, b| b.cmp(a));
    let mut target = list.borrow_mut();
    for i in indices {
        target.remove(i);
    }
    Ok(())
}
