//! Python operators: integer promotion, floor division, sequence operations and errors.
use crate::{PyExc, PyResult, Value};
use num_bigint::BigInt;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::cmp::Ordering;

fn integers(a: &Value, b: &Value) -> bool {
    matches!(a, Value::Int(_) | Value::Bool(_)) && matches!(b, Value::Int(_) | Value::Bool(_))
}

fn numeric(a: &Value, b: &Value) -> bool {
    matches!(a, Value::Int(_) | Value::Bool(_) | Value::Float(_))
        && matches!(b, Value::Int(_) | Value::Bool(_) | Value::Float(_))
}

fn unsupported(op: &str, a: &Value, b: &Value) -> PyExc {
    PyExc::type_error(format!(
        "unsupported operand type(s) for {op}: '{}' and '{}'",
        a.type_name(),
        b.type_name()
    ))
}

pub fn add(a: &Value, b: &Value) -> PyResult<Value> {
    if integers(a, b) {
        return Ok(Value::Int(a.as_int()? + b.as_int()?));
    }
    if numeric(a, b) {
        return Ok(Value::Float(a.as_float()? + b.as_float()?));
    }
    match (a, b) {
        (Value::Str(a), Value::Str(b)) => Ok(Value::from(format!("{}{}", a.as_str(), b.as_str()))),
        (Value::List(a), Value::List(b)) => Ok(Value::list(
            a.borrow()
                .iter()
                .chain(b.borrow().iter())
                .cloned()
                .collect(),
        )),
        (Value::Tuple(a), Value::Tuple(b)) => {
            Ok(Value::tuple(a.iter().chain(b.iter()).cloned().collect()))
        }
        _ => Err(unsupported("+", a, b)),
    }
}

pub fn sub(a: &Value, b: &Value) -> PyResult<Value> {
    if integers(a, b) {
        return Ok(Value::Int(a.as_int()? - b.as_int()?));
    }
    if numeric(a, b) {
        return Ok(Value::Float(a.as_float()? - b.as_float()?));
    }
    Err(unsupported("-", a, b))
}

fn repetition(sequence: &Value, count: &Value) -> PyResult<Value> {
    let count = count.as_int()?;
    let count = count
        .to_isize()
        .ok_or_else(|| PyExc::overflow_error("cannot fit int into an index-sized integer"))?
        .max(0) as usize;
    match sequence {
        Value::Str(s) => {
            if s.is_empty() {
                return Ok(Value::from(""));
            }
            let size = s
                .as_str()
                .len()
                .checked_mul(count)
                .filter(|n| *n <= isize::MAX as usize)
                .ok_or_else(|| PyExc::overflow_error("repeated string is too long"))?;
            let mut out = String::new();
            out.try_reserve(size)
                .map_err(|_| PyExc::memory_error("cannot allocate repeated string"))?;
            for _ in 0..count {
                out.push_str(s.as_str());
            }
            Ok(Value::from(out))
        }
        Value::List(_) | Value::Tuple(_) => {
            let values = sequence.sequence()?;
            if values.is_empty() {
                return Ok(if matches!(sequence, Value::Tuple(_)) {
                    sequence.clone()
                } else {
                    Value::list(Vec::new())
                });
            }
            let size = values
                .len()
                .checked_mul(count)
                .filter(|n| *n <= isize::MAX as usize)
                .ok_or_else(|| PyExc::overflow_error("repeated sequence is too long"))?;
            let mut out = Vec::new();
            out.try_reserve(size)
                .map_err(|_| PyExc::memory_error("cannot allocate repeated sequence"))?;
            for _ in 0..count {
                out.extend(values.iter().cloned());
            }
            Ok(if matches!(sequence, Value::Tuple(_)) {
                Value::tuple(out)
            } else {
                Value::list(out)
            })
        }
        _ => Err(unsupported("*", sequence, &Value::from(count))),
    }
}

pub fn mul(a: &Value, b: &Value) -> PyResult<Value> {
    if integers(a, b) {
        return Ok(Value::Int(a.as_int()? * b.as_int()?));
    }
    if numeric(a, b) {
        return Ok(Value::Float(a.as_float()? * b.as_float()?));
    }
    if matches!(a, Value::Int(_) | Value::Bool(_)) {
        return repetition(b, a);
    }
    if matches!(b, Value::Int(_) | Value::Bool(_)) {
        return repetition(a, b);
    }
    Err(unsupported("*", a, b))
}

fn int_divmod(a: BigInt, b: BigInt) -> PyResult<(BigInt, BigInt)> {
    if b.is_zero() {
        return Err(PyExc::zero_division_error(
            "integer division or modulo by zero",
        ));
    }
    let (mut quotient, mut remainder) = (&a / &b, &a % &b);
    if !remainder.is_zero() && remainder.is_negative() != b.is_negative() {
        quotient -= 1;
        remainder += b;
    }
    Ok((quotient, remainder))
}

fn float_divmod(a: f64, b: f64) -> PyResult<(f64, f64)> {
    if b == 0.0 {
        return Err(PyExc::zero_division_error("float divmod()"));
    }
    // CPython floatobject.c: derive quotient from the remainder to keep the
    // integer result stable across roundoff near an exact multiple.
    let mut remainder = a % b;
    let mut quotient = (a - remainder) / b;
    if remainder != 0.0 {
        if remainder.is_sign_negative() != b.is_sign_negative() {
            remainder += b;
            quotient -= 1.0;
        }
    } else {
        remainder = 0.0_f64.copysign(b);
    }
    let floor = if quotient != 0.0 {
        let floor = quotient.floor();
        if quotient - floor > 0.5 {
            floor + 1.0
        } else {
            floor
        }
    } else {
        0.0_f64.copysign(a / b)
    };
    Ok((floor, remainder))
}

pub fn floor_div(a: &Value, b: &Value) -> PyResult<Value> {
    if integers(a, b) {
        return Ok(Value::Int(int_divmod(a.as_int()?, b.as_int()?)?.0));
    }
    if numeric(a, b) {
        return Ok(Value::Float(float_divmod(a.as_float()?, b.as_float()?)?.0));
    }
    Err(unsupported("//", a, b))
}

pub fn modulo(a: &Value, b: &Value) -> PyResult<Value> {
    if integers(a, b) {
        return Ok(Value::Int(int_divmod(a.as_int()?, b.as_int()?)?.1));
    }
    if numeric(a, b) {
        return Ok(Value::Float(float_divmod(a.as_float()?, b.as_float()?)?.1));
    }
    Err(unsupported("%", a, b))
}

pub fn neg(v: &Value) -> PyResult<Value> {
    match v {
        Value::Bool(_) | Value::Int(_) => Ok(Value::Int(-v.as_int()?)),
        Value::Float(x) => Ok(Value::Float(-x)),
        _ => Err(PyExc::type_error("bad operand type for unary -")),
    }
}

pub fn bitwise(op: &str, a: &Value, b: &Value) -> PyResult<Value> {
    let (left, right) = (a.as_int()?, b.as_int()?);
    let value = match op {
        "&" => left & right,
        "|" => left | right,
        "^" => left ^ right,
        "<<" | ">>" => {
            if right.is_negative() {
                return Err(PyExc::value_error("negative shift count"));
            }
            if op == ">>" && right > BigInt::from(left.bits()) {
                return Ok(Value::Int(if left.is_negative() {
                    -BigInt::one()
                } else {
                    BigInt::zero()
                }));
            }
            let right = right
                .to_usize()
                .ok_or_else(|| PyExc::overflow_error("too many digits in integer"))?;
            if op == "<<" {
                left << right
            } else {
                left >> right
            }
        }
        _ => return Err(PyExc::value_error("unknown bitwise operator")),
    };
    if matches!((a, b), (Value::Bool(_), Value::Bool(_))) && matches!(op, "&" | "|" | "^") {
        Ok(Value::Bool(!value.is_zero()))
    } else {
        Ok(Value::Int(value))
    }
}

/// Partial ordering keeps NaNs unordered and compares large integers exactly.
pub fn compare(a: &Value, b: &Value) -> PyResult<Option<Ordering>> {
    if integers(a, b) {
        return Ok(Some(a.as_int()?.cmp(&b.as_int()?)));
    }
    match (a, b) {
        (Value::Float(a), Value::Float(b)) => Ok(a.partial_cmp(b)),
        (Value::Int(_) | Value::Bool(_), Value::Float(f)) => compare_int_float(&a.as_int()?, *f),
        (Value::Float(f), Value::Int(_) | Value::Bool(_)) => {
            Ok(compare_int_float(&b.as_int()?, *f)?.map(Ordering::reverse))
        }
        (Value::Str(a), Value::Str(b)) => Ok(Some(a.as_str().cmp(b.as_str()))),
        (Value::List(_), Value::List(_)) | (Value::Tuple(_), Value::Tuple(_)) => {
            let (a, b) = (a.sequence()?, b.sequence()?);
            for (a, b) in a.iter().zip(&b) {
                if !a.equals(b)? {
                    return compare(a, b);
                }
            }
            Ok(Some(a.len().cmp(&b.len())))
        }
        _ => Err(PyExc::type_error(format!(
            "'<' not supported between instances of '{}' and '{}'",
            a.type_name(),
            b.type_name()
        ))),
    }
}

fn compare_int_float(integer: &BigInt, float: f64) -> PyResult<Option<Ordering>> {
    use num_traits::FromPrimitive;
    if float.is_nan() {
        return Ok(None);
    }
    if float == f64::INFINITY {
        return Ok(Some(Ordering::Less));
    }
    if float == f64::NEG_INFINITY {
        return Ok(Some(Ordering::Greater));
    }
    let whole = BigInt::from_f64(float).expect("finite float has an integer part");
    let order = integer.cmp(&whole);
    Ok(Some(if order == Ordering::Equal {
        0.0_f64
            .partial_cmp(&float.fract())
            .expect("finite fractional part")
    } else {
        order
    }))
}

pub fn getitem(sequence: &Value, index: &Value) -> PyResult<Value> {
    if matches!(sequence, Value::Dict(_)) {
        return sequence.dict_get(index);
    }
    let index = index
        .as_int()?
        .to_i64()
        .ok_or_else(|| PyExc::index_error("cannot fit int into an index-sized integer"))?;
    match sequence {
        Value::Str(s) => Ok(Value::Str(s.get_index(index)?)),
        Value::List(_) | Value::Tuple(_) => {
            let values = sequence.sequence()?;
            let i = if index < 0 {
                values.len() as i128 + index as i128
            } else {
                index as i128
            };
            if i < 0 || i >= values.len() as i128 {
                return Err(PyExc::index_error("sequence index out of range"));
            }
            Ok(values[i as usize].clone())
        }
        _ => Err(PyExc::type_error("object is not subscriptable")),
    }
}

pub fn contains(container: &Value, item: &Value) -> PyResult<bool> {
    match container {
        Value::Str(s) => Ok(s.as_str().contains(item.as_str()?)),
        Value::Set(s) | Value::FrozenSet(s) => s.contains(item),
        Value::Dict(_) => match container.dict_get(item) {
            Ok(_) => Ok(true),
            Err(e) if e.kind == "KeyError" => Ok(false),
            Err(e) => Err(e),
        },
        _ => {
            for v in container.sequence()? {
                if v.is_identical(item) || v.equals(item)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn big_int_boolean_and_signed_division() {
        let max = Value::Int(BigInt::from(i64::MAX));
        assert!(
            matches!(add(&max, &Value::Bool(true)).unwrap(), Value::Int(n) if n == BigInt::from(i64::MAX) + 1)
        );
        let cases: [(i64, i64, i64, i64); 3] = [(-7, 3, -3, 2), (7, -3, -3, -2), (-7, -3, 2, -1)];
        for (a, b, q, r) in cases {
            assert!(
                floor_div(&Value::from(a), &Value::from(b))
                    .unwrap()
                    .equals(&Value::from(q))
                    .unwrap()
            );
            assert!(
                modulo(&Value::from(a), &Value::from(b))
                    .unwrap()
                    .equals(&Value::from(r))
                    .unwrap()
            );
        }
        assert_eq!(
            floor_div(&max, &Value::Bool(false)).unwrap_err().kind,
            "ZeroDivisionError"
        );
    }

    #[test]
    fn float_floor_rounding_and_negative_zero() {
        assert!(
            floor_div(&Value::Float(1.0), &Value::Float(0.1))
                .unwrap()
                .equals(&Value::Float(9.0))
                .unwrap()
        );
        let Value::Float(r) = modulo(&Value::Float(4.0), &Value::Float(-2.0)).unwrap() else {
            panic!()
        };
        assert_eq!(r.to_bits(), (-0.0_f64).to_bits());
        assert_eq!(
            compare(
                &Value::from(9_007_199_254_740_993_i64),
                &Value::Float(9_007_199_254_740_992.0)
            )
            .unwrap(),
            Some(Ordering::Greater)
        );
        assert_eq!(
            compare(&Value::from(-1_i64), &Value::Float(-1.5)).unwrap(),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn list_repetition_keeps_references_and_getitem_uses_codepoints() {
        let child = Value::list(vec![]);
        let repeated = mul(&Value::list(vec![child.clone()]), &Value::from(2_i64))
            .unwrap()
            .sequence()
            .unwrap();
        assert!(repeated[0].is_identical(&child) && repeated[1].is_identical(&child));
        assert_eq!(
            getitem(&Value::from("é😀x"), &Value::from(-2_i64))
                .unwrap()
                .as_str()
                .unwrap(),
            "😀"
        );
        assert!(matches!(
            bitwise("&", &Value::Bool(true), &Value::Bool(false)).unwrap(),
            Value::Bool(false)
        ));
        let count = Value::from(i64::MAX);
        assert_eq!(mul(&Value::from(""), &count).unwrap().as_str().unwrap(), "");
        assert!(
            mul(&Value::list(vec![]), &count)
                .unwrap()
                .sequence()
                .unwrap()
                .is_empty()
        );
    }
}
