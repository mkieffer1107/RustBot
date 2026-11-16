//! Python operators: integer promotion, floor division, sequence operations and errors.
use crate::{PyExc, PyResult, Value};
use num_bigint::BigInt;
use num_traits::{FromPrimitive, One, Signed, ToPrimitive, Zero};
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
    if let Some(value) = crate::numeric_objects::binary("add", a, b) {
        return value;
    }
    if let Some(value) = crate::stdlib::temporal_binary("+", a, b) {
        return value;
    }
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
    if let Some(value) = crate::numeric_objects::binary("sub", a, b) {
        return value;
    }
    if let Some(value) = crate::stdlib::temporal_binary("-", a, b) {
        return value;
    }
    if matches!(a, Value::Set(_) | Value::FrozenSet(_))
        && matches!(b, Value::Set(_) | Value::FrozenSet(_))
    {
        return crate::containers::call_method(a, "difference", std::slice::from_ref(b));
    }
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
    if let Some(value) = crate::numeric_objects::binary("mul", a, b) {
        return value;
    }
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

/// Round a nonnegative rational to an integer, resolving ties toward even.
pub(crate) fn round_ratio(numerator: &BigInt, denominator: &BigInt) -> BigInt {
    debug_assert!(!numerator.is_negative() && denominator.is_positive());
    let mut quotient = numerator / denominator;
    let twice_remainder = (numerator % denominator) << 1;
    if twice_remainder > *denominator
        || (twice_remainder == *denominator && (&quotient & BigInt::one()).is_one())
    {
        quotient += 1;
    }
    quotient
}

/// Correctly round an integer ratio directly to binary64. Converting the two
/// integers independently would overflow for ratios such as 10**400 / 10**400.
pub fn int_ratio_to_float(numerator: &BigInt, denominator: &BigInt) -> PyResult<f64> {
    if denominator.is_zero() {
        return Err(PyExc::zero_division_error("division by zero"));
    }
    let negative = numerator.is_negative() != denominator.is_negative();
    let zero = if negative { -0.0 } else { 0.0 };
    if numerator.is_zero() {
        return Ok(zero);
    }
    let (a, b) = (numerator.abs(), denominator.abs());
    let difference = i128::from(a.bits()) - i128::from(b.bits());
    if difference > 1024 {
        return Err(PyExc::overflow_error(
            "integer division result too large for a float",
        ));
    }
    if difference < -1075 {
        return Ok(zero);
    }
    let mut exponent = difference as i32;
    let below = if exponent >= 0 {
        a < (&b << exponent as usize)
    } else {
        (&a << (-exponent) as usize) < b
    };
    if below {
        exponent -= 1;
    }
    if exponent > 1023 {
        return Err(PyExc::overflow_error(
            "integer division result too large for a float",
        ));
    }
    if exponent < -1075 {
        return Ok(zero);
    }
    let shift = if exponent >= -1022 {
        52 - exponent
    } else {
        1074
    };
    let mantissa = if shift >= 0 {
        round_ratio(&(&a << shift as usize), &b)
    } else {
        round_ratio(&a, &(&b << (-shift) as usize))
    };
    let mut mantissa = mantissa.to_u64().expect("rounded binary64 significand");
    let bits = if exponent < -1022 {
        // A rounded subnormal may become the smallest normal number; its bit
        // representation is still the integer count of subnormal units.
        mantissa
    } else {
        if mantissa == 1_u64 << 53 {
            mantissa >>= 1;
            exponent += 1;
        }
        if exponent > 1023 {
            return Err(PyExc::overflow_error(
                "integer division result too large for a float",
            ));
        }
        (((exponent + 1023) as u64) << 52) | (mantissa - (1_u64 << 52))
    };
    Ok(f64::from_bits(bits | (u64::from(negative) << 63)))
}

pub fn true_div(a: &Value, b: &Value) -> PyResult<Value> {
    if let Some(value) = crate::numeric_objects::binary("truediv", a, b) {
        return value;
    }
    if integers(a, b) {
        return Ok(Value::Float(int_ratio_to_float(
            &a.as_int()?,
            &b.as_int()?,
        )?));
    }
    if numeric(a, b) {
        let (left, right) = (a.as_float()?, b.as_float()?);
        if right == 0.0 {
            return Err(PyExc::zero_division_error("float division by zero"));
        }
        return Ok(Value::Float(left / right));
    }
    Err(unsupported("/", a, b))
}

pub fn divmod(a: &Value, b: &Value) -> PyResult<Value> {
    if let Some(value)=crate::numeric_objects::binary("divmod",a,b){return value;}
    if integers(a, b) {
        let (q, r) = int_divmod(a.as_int()?, b.as_int()?)?;
        return Ok(Value::tuple(vec![Value::Int(q), Value::Int(r)]));
    }
    if numeric(a, b) {
        let (q, r) = float_divmod(a.as_float()?, b.as_float()?)?;
        return Ok(Value::tuple(vec![Value::Float(q), Value::Float(r)]));
    }
    Err(unsupported("divmod()", a, b))
}

pub fn power(a: &Value, b: &Value) -> PyResult<Value> {
    if let Some(value) = crate::numeric_objects::binary("pow", a, b) {
        return value;
    }
    if integers(a, b) {
        let (base, exponent) = (a.as_int()?, b.as_int()?);
        if !exponent.is_negative() {
            if exponent.is_zero() {
                return Ok(Value::Int(BigInt::one()));
            }
            if base.is_zero() || base.is_one() {
                return Ok(Value::Int(base));
            }
            if base == -BigInt::one() {
                return Ok(Value::Int(if (&exponent & BigInt::one()).is_zero() {
                    BigInt::one()
                } else {
                    base
                }));
            }
            let mut exponent = exponent
                .to_u64()
                .ok_or_else(|| PyExc::overflow_error("too many digits in integer"))?;
            if u128::from(base.bits()).saturating_mul(u128::from(exponent)) > isize::MAX as u128 {
                return Err(PyExc::overflow_error("too many digits in integer"));
            }
            let mut factor = base;
            let mut result = BigInt::one();
            while exponent != 0 {
                if exponent & 1 != 0 {
                    result *= &factor;
                }
                exponent >>= 1;
                if exponent != 0 {
                    factor = &factor * &factor;
                }
            }
            return Ok(Value::Int(result));
        }
    }
    if !numeric(a, b) {
        return Err(unsupported("** or pow()", a, b));
    }
    float_power(a.as_float()?, b.as_float()?).map(Value::Float)
}

fn float_power(base: f64, exponent: f64) -> PyResult<f64> {
    if exponent == 0.0 {
        return Ok(1.0);
    }
    if base.is_nan() {
        return Ok(base);
    }
    if exponent.is_nan() {
        return Ok(if base == 1.0 { 1.0 } else { exponent });
    }
    let magnitude = base.abs();
    if exponent.is_infinite() {
        if magnitude == 1.0 {
            return Ok(1.0);
        }
        return Ok(if (magnitude > 1.0) == exponent.is_sign_positive() {
            f64::INFINITY
        } else {
            0.0
        });
    }
    let negative =
        base.is_sign_negative() && exponent.fract() == 0.0 && exponent.abs() % 2.0 == 1.0;
    if base.is_infinite() {
        let result = if exponent > 0.0 { f64::INFINITY } else { 0.0 };
        return Ok(if negative { -result } else { result });
    }
    if base == 0.0 {
        if exponent < 0.0 {
            return Err(PyExc::zero_division_error("zero to a negative power"));
        }
        return Ok(if negative { -0.0 } else { 0.0 });
    }
    if base < 0.0 && exponent.fract() != 0.0 {
        return Err(PyExc::not_implemented_error(
            "complex-valued powers require a complex runtime value",
        ));
    }
    let result = magnitude.powf(exponent);
    if result.is_infinite() {
        return Err(PyExc::overflow_error(
            "(34, 'Numerical result out of range')",
        ));
    }
    Ok(if negative { -result } else { result })
}

/// Python's three-argument pow, including modular inverses and negative moduli.
pub fn pow_mod(a: &Value, b: &Value, modulus: &Value) -> PyResult<Value> {
    if !integers(a, b) || !matches!(modulus, Value::Int(_) | Value::Bool(_)) {
        return Err(PyExc::type_error(
            "pow() 3rd argument not allowed unless all arguments are integers",
        ));
    }
    let (mut base, exponent, modulus) = (a.as_int()?, b.as_int()?, modulus.as_int()?);
    if modulus.is_zero() {
        return Err(PyExc::value_error("pow() 3rd argument cannot be 0"));
    }
    let positive_modulus = modulus.abs();
    if positive_modulus.is_one() {
        return Ok(Value::Int(BigInt::zero()));
    }
    if exponent.is_negative() {
        let mut r = positive_modulus.clone();
        let mut next_r = int_divmod(base, positive_modulus.clone())?.1;
        let (mut coefficient, mut next_coefficient) = (BigInt::zero(), BigInt::one());
        while !next_r.is_zero() {
            let quotient = &r / &next_r;
            (r, next_r) = (next_r.clone(), r - &quotient * next_r);
            (coefficient, next_coefficient) = (
                next_coefficient.clone(),
                coefficient - quotient * next_coefficient,
            );
        }
        if !r.is_one() {
            return Err(PyExc::value_error(
                "base is not invertible for the given modulus",
            ));
        }
        base = coefficient;
    }
    let mut result = base.modpow(&exponent.abs(), &positive_modulus);
    if modulus.is_negative() && !result.is_zero() {
        result -= positive_modulus;
    }
    Ok(Value::Int(result))
}

pub fn pos(value: &Value) -> PyResult<Value> {
    if let Some(value) = crate::numeric_objects::unary("pos", value) {
        return value;
    }
    match value {
        Value::Int(_) | Value::Bool(_) => Ok(Value::Int(value.as_int()?)),
        Value::Float(_) => Ok(value.clone()),
        _ => Err(PyExc::type_error(format!(
            "bad operand type for unary +: '{}'",
            value.type_name()
        ))),
    }
}

pub fn invert(value: &Value) -> PyResult<Value> {
    match value {
        Value::Int(_) | Value::Bool(_) => Ok(Value::Int(!value.as_int()?)),
        _ => Err(PyExc::type_error(format!(
            "bad operand type for unary ~: '{}'",
            value.type_name()
        ))),
    }
}

pub fn floor_div(a: &Value, b: &Value) -> PyResult<Value> {
    if let Some(value) = crate::numeric_objects::binary("floordiv", a, b) {
        return value;
    }
    if integers(a, b) {
        return Ok(Value::Int(int_divmod(a.as_int()?, b.as_int()?)?.0));
    }
    if numeric(a, b) {
        return Ok(Value::Float(float_divmod(a.as_float()?, b.as_float()?)?.0));
    }
    Err(unsupported("//", a, b))
}

pub fn modulo(a: &Value, b: &Value) -> PyResult<Value> {
    if let Some(value) = crate::numeric_objects::binary("mod", a, b) {
        return value;
    }
    if let Value::Str(template) = a {
        return crate::formatting::percent(template.as_str(), b).map(Value::from);
    }
    if integers(a, b) {
        return Ok(Value::Int(int_divmod(a.as_int()?, b.as_int()?)?.1));
    }
    if numeric(a, b) {
        return Ok(Value::Float(float_divmod(a.as_float()?, b.as_float()?)?.1));
    }
    Err(unsupported("%", a, b))
}

/// Augmented assignment returns the original mutable container when Python
/// mutates in place; scalar and immutable sequence operands use binary operators.
pub fn inplace(
    rt: &mut crate::runtime::Runtime,
    op: &str,
    a: &Value,
    b: &Value,
) -> PyResult<Value> {
    let op = op.strip_suffix('=').unwrap_or(op);
    if let Value::List(list) = a {
        if op == "+" {
            if a.is_identical(b) {
                let copied = list.borrow().clone();
                list.borrow_mut().extend(copied);
            } else {
                let iterator = rt.iter(b)?;
                loop {
                    let value = match rt.next(&iterator) {
                        Err(error) if error.kind == "StopIteration" => None,
                        value => value?,
                    };
                    let Some(value) = value else {
                        break;
                    };
                    list.borrow_mut().push(value);
                }
            }
            return Ok(a.clone());
        }
        if op == "*" {
            let Value::List(repeated) = mul(a, b)? else {
                unreachable!("list repetition returns list");
            };
            *list.borrow_mut() = repeated.borrow().clone();
            return Ok(a.clone());
        }
    }
    if matches!(a, Value::Set(_)) && matches!(op, "|" | "&" | "^" | "-") {
        if !matches!(b, Value::Set(_) | Value::FrozenSet(_)) {
            return Err(unsupported(&format!("{op}="), a, b));
        }
        let method = match op {
            "|" => "update",
            "&" => "intersection_update",
            "^" => "symmetric_difference_update",
            "-" => "difference_update",
            _ => unreachable!(),
        };
        crate::containers::call_method(a, method, std::slice::from_ref(b))?;
        return Ok(a.clone());
    }
    match op {
        "+" => add(a, b),
        "-" => sub(a, b),
        "*" => mul(a, b),
        "/" => true_div(a, b),
        "//" => floor_div(a, b),
        "%" => modulo(a, b),
        "**" => power(a, b),
        "|" | "&" | "^" | "<<" | ">>" => bitwise(op, a, b),
        _ => Err(PyExc::value_error(format!(
            "unknown augmented assignment operator '{op}'"
        ))),
    }
}

pub fn neg(v: &Value) -> PyResult<Value> {
    if let Some(value) = crate::numeric_objects::unary("neg", v) {
        return value;
    }
    match v {
        Value::Bool(_) | Value::Int(_) => Ok(Value::Int(-v.as_int()?)),
        Value::Float(x) => Ok(Value::Float(-x)),
        _ => Err(PyExc::type_error("bad operand type for unary -")),
    }
}

pub fn bitwise(op: &str, a: &Value, b: &Value) -> PyResult<Value> {
    if matches!(a, Value::Set(_) | Value::FrozenSet(_))
        && matches!(b, Value::Set(_) | Value::FrozenSet(_))
    {
        let method = match op {
            "&" => "intersection",
            "|" => "union",
            "^" => "symmetric_difference",
            _ => return Err(unsupported(op, a, b)),
        };
        return crate::containers::call_method(a, method, std::slice::from_ref(b));
    }
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
    if let Some(order) = crate::numeric_objects::compare(a, b) {
        return order.map(Some);
    }
    if let Some(order) = crate::stdlib::temporal_compare(a, b) {
        return order.map(Some);
    }
    if matches!(a, Value::Set(_) | Value::FrozenSet(_))
        && matches!(b, Value::Set(_) | Value::FrozenSet(_))
    {
        if a.equals(b)? {
            return Ok(Some(Ordering::Equal));
        }
        if crate::containers::call_method(a, "issubset", std::slice::from_ref(b))?.is_truthy() {
            return Ok(Some(Ordering::Less));
        }
        if crate::containers::call_method(a, "issuperset", std::slice::from_ref(b))?.is_truthy() {
            return Ok(Some(Ordering::Greater));
        }
        return Ok(None);
    }
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
    if let Value::Object(o) = sequence {
        return o.get_item(index);
    }
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
        Value::Object(o) => o.contains(item),
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
