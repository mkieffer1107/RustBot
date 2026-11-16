//! The math and string-module subset found by the static source inventory.
use crate::{PyExc, PyResult, Value, float_repr, ops};
use num_bigint::BigInt;
use num_traits::{FromPrimitive, One, Signed, ToPrimitive, Zero};

pub const PUNCTUATION: &str = "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";
pub const ASCII_LOWERCASE: &str = "abcdefghijklmnopqrstuvwxyz";
pub const ASCII_UPPERCASE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const ASCII_LETTERS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const DIGITS: &str = "0123456789";
pub const WHITESPACE: &str = " \t\n\r\x0b\x0c";

/// CPython's _PyLong_Frexp: a nearest-even 53-bit significand in [0.5, 1)
/// and an exponent, including the carry from rounding an all-ones significand.
pub fn int_frexp(integer: &BigInt) -> (f64, u64) {
    if integer.is_zero() {
        return (0.0, 0);
    }
    let magnitude = integer.abs();
    let mut bits = magnitude.bits();
    let significand = if bits <= 53 {
        magnitude.to_u64().expect("small integer") << (53 - bits)
    } else {
        let shift = bits - 53;
        let mut high = &magnitude >> shift as usize;
        let remainder = &magnitude - (&high << shift as usize);
        let halfway = BigInt::one() << (shift - 1) as usize;
        if remainder > halfway || (remainder == halfway && (&high & BigInt::one()).is_one()) {
            high += 1;
        }
        let mut high = high.to_u64().expect("53-bit rounded significand");
        if high == 1_u64 << 53 {
            high >>= 1;
            bits += 1;
        }
        high
    };
    let fraction = significand as f64 * (1.0 / 9_007_199_254_740_992.0);
    (
        if integer.is_negative() {
            -fraction
        } else {
            fraction
        },
        bits,
    )
}

fn logarithm(value: &Value, decimal: bool) -> PyResult<f64> {
    let function = |x: f64| if decimal { x.log10() } else { x.ln() };
    if matches!(value, Value::Int(_) | Value::Bool(_)) {
        let integer = value.as_int()?;
        if !integer.is_positive() {
            return Err(PyExc::value_error("expected a positive input"));
        }
        if let Some(x) = integer.to_f64().filter(|x| x.is_finite()) {
            return Ok(function(x));
        }
        let (fraction, exponent) = int_frexp(&integer);
        // Keep CPython's order of operations; changing this to log(n)'s leading
        // integer bits or using a fused multiply-add can change its last bit.
        return Ok(function(fraction) + function(2.0) * exponent as f64);
    }
    let x = value.as_float()?;
    if x <= 0.0 {
        return Err(PyExc::value_error(format!(
            "expected a positive input, got {}",
            float_repr(x)
        )));
    }
    Ok(function(x))
}

/// Built-in round, distinct from math's integer-valued ceil and floor.
pub fn round(value: &Value, ndigits: Option<&Value>) -> PyResult<Value> {
    if let Some(result) = crate::numeric_objects::round(value, ndigits) {
        return result;
    }
    let ndigits = ndigits.filter(|v| !matches!(v, Value::None));
    let digits = ndigits.map(Value::as_int).transpose()?;
    match value {
        Value::Int(_) | Value::Bool(_) => {
            let integer = value.as_int()?;
            let Some(digits) = digits.filter(BigInt::is_negative) else {
                return Ok(Value::Int(integer));
            };
            let places = -digits;
            if places > BigInt::from(integer.bits()) {
                return Ok(Value::Int(BigInt::zero()));
            }
            let places = places
                .to_u32()
                .ok_or_else(|| PyExc::overflow_error("too many digits in integer"))?;
            let scale = BigInt::from(10_u8).pow(places);
            let magnitude = ops::round_ratio(&integer.abs(), &scale) * scale;
            Ok(Value::Int(if integer.is_negative() {
                -magnitude
            } else {
                magnitude
            }))
        }
        Value::Float(x) => {
            let Some(digits) = digits else {
                if x.is_nan() {
                    return Err(PyExc::value_error("cannot convert float NaN to integer"));
                }
                if x.is_infinite() {
                    return Err(PyExc::overflow_error(
                        "cannot convert float infinity to integer",
                    ));
                }
                return Ok(Value::Int(
                    BigInt::from_f64(x.round_ties_even()).expect("finite rounded float"),
                ));
            };
            if !x.is_finite() || digits > BigInt::from(323) {
                return Ok(value.clone());
            }
            if digits < BigInt::from(-308) {
                return Ok(Value::Float(0.0_f64.copysign(*x)));
            }
            let digits = digits.to_i32().expect("bounded decimal precision");
            let scale = BigInt::from(10_u8).pow(digits.unsigned_abs());
            let bits = x.abs().to_bits();
            let exponent = ((bits >> 52) & 0x7ff) as i32;
            let fraction = bits & ((1_u64 << 52) - 1);
            let (mantissa, exponent) = if exponent == 0 {
                (fraction, -1074)
            } else {
                (fraction | (1_u64 << 52), exponent - 1075)
            };
            let (mut numerator, mut denominator) = (BigInt::from(mantissa), BigInt::one());
            if exponent >= 0 {
                numerator <<= exponent as usize;
            } else {
                denominator <<= (-exponent) as usize;
            }
            if digits >= 0 {
                numerator *= &scale;
            } else {
                denominator *= &scale;
            }
            let integer = ops::round_ratio(&numerator, &denominator);
            let rounded = if digits >= 0 {
                ops::int_ratio_to_float(&integer, &scale)
            } else {
                ops::int_ratio_to_float(&(integer * scale), &BigInt::one())
            }
            .map_err(|e| {
                if e.kind == "OverflowError" {
                    PyExc::overflow_error("rounded value too large to represent")
                } else {
                    e
                }
            })?;
            Ok(Value::Float(rounded.copysign(*x)))
        }
        _ => Err(PyExc::type_error(format!(
            "type {} doesn't define __round__ method",
            value.type_name()
        ))),
    }
}

pub fn call(name: &str, args: &[Value]) -> PyResult<Value> {
    if args.is_empty() || args.len() > if name == "log" { 2 } else { 1 } {
        return Err(PyExc::type_error("incorrect number of math arguments"));
    }
    if matches!(name, "log" | "log10") {
        let numerator = logarithm(&args[0], name == "log10")?;
        let value = if let Some(base) = args.get(1) {
            let denominator = logarithm(base, false)?;
            if denominator == 0.0 {
                return Err(PyExc::zero_division_error("float division by zero"));
            }
            numerator / denominator
        } else {
            numerator
        };
        return Ok(Value::Float(value));
    }
    if matches!(name, "ceil" | "floor") && matches!(args[0], Value::Int(_) | Value::Bool(_)) {
        return Ok(Value::Int(args[0].as_int()?));
    }
    let x = args[0].as_float()?;
    if matches!(name, "ceil" | "floor") {
        if x.is_nan() {
            return Err(PyExc::value_error("cannot convert float NaN to integer"));
        }
        if x.is_infinite() {
            return Err(PyExc::overflow_error(
                "cannot convert float infinity to integer",
            ));
        }
        return Ok(Value::Int(
            BigInt::from_f64(if name == "ceil" { x.ceil() } else { x.floor() })
                .expect("finite rounded float"),
        ));
    }
    let value = match name {
        "sin" | "cos" if x.is_infinite() => {
            return Err(PyExc::value_error(format!(
                "expected a finite input, got {}",
                float_repr(x)
            )));
        }
        "sin" => x.sin(),
        "cos" => x.cos(),
        "asin" => {
            if x.abs() > 1.0 {
                return Err(PyExc::value_error(format!(
                    "expected a number in range from -1 up to 1, got {}",
                    float_repr(x)
                )));
            }
            x.asin()
        }
        "sqrt" => {
            if x < 0.0 {
                return Err(PyExc::value_error(format!(
                    "expected a nonnegative input, got {}",
                    float_repr(x)
                )));
            }
            x.sqrt()
        }
        "radians" => x * (std::f64::consts::PI / 180.0),
        _ => {
            return Err(PyExc::attribute_error(format!(
                "math has no attribute {name}"
            )));
        }
    };
    Ok(Value::Float(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn domain_and_integer_conversion_match_python() {
        assert_eq!(
            call("sqrt", &[Value::Float(-1.0)]).unwrap_err().kind,
            "ValueError"
        );
        assert_eq!(
            call("sin", &[Value::Float(f64::INFINITY)])
                .unwrap_err()
                .kind,
            "ValueError"
        );
        assert_eq!(
            call("ceil", &[Value::Float(f64::NAN)]).unwrap_err().kind,
            "ValueError"
        );
        assert_eq!(
            call("floor", &[Value::Float(f64::INFINITY)])
                .unwrap_err()
                .kind,
            "OverflowError"
        );
        let huge = Value::Int(BigInt::from(1_u8) << 200_u32);
        assert!(
            call("floor", std::slice::from_ref(&huge))
                .unwrap()
                .equals(&huge)
                .unwrap()
        );
        assert!(
            call("floor", &[Value::Float(-1.25)])
                .unwrap()
                .equals(&Value::from(-2_i64))
                .unwrap()
        );
        assert_eq!(
            call("log", &[Value::Float(2.0), Value::Float(1.0)])
                .unwrap_err()
                .kind,
            "ZeroDivisionError"
        );
    }
}
