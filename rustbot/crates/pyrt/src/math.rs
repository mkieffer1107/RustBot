//! The math and string-module subset found by the static source inventory.
use crate::{PyExc, PyResult, Value};
use num_bigint::BigInt;
use num_traits::FromPrimitive;

pub const PUNCTUATION: &str = "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";
pub const ASCII_LOWERCASE: &str = "abcdefghijklmnopqrstuvwxyz";
pub const ASCII_UPPERCASE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const ASCII_LETTERS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const DIGITS: &str = "0123456789";
pub const WHITESPACE: &str = " \t\n\r\x0b\x0c";

pub fn call(name: &str, args: &[Value]) -> PyResult<Value> {
    if args.is_empty() || args.len() > if name == "log" { 2 } else { 1 } {
        return Err(PyExc::type_error("incorrect number of math arguments"));
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
    let domain = || PyExc::value_error("math domain error");
    let value = match name {
        "sin" | "cos" if x.is_infinite() => return Err(domain()),
        "sin" => x.sin(),
        "cos" => x.cos(),
        "asin" => {
            if x.abs() > 1.0 {
                return Err(domain());
            }
            x.asin()
        }
        "sqrt" => {
            if x < 0.0 {
                return Err(domain());
            }
            x.sqrt()
        }
        "radians" => x * (std::f64::consts::PI / 180.0),
        "log10" => {
            if x <= 0.0 {
                return Err(domain());
            }
            x.log10()
        }
        "log" => {
            if x <= 0.0 {
                return Err(domain());
            }
            let y = x.ln();
            if let Some(base) = args.get(1) {
                let base = base.as_float()?;
                if base <= 0.0 {
                    return Err(domain());
                }
                if base == 1.0 {
                    return Err(PyExc::zero_division_error("float division by zero"));
                }
                y / base.ln()
            } else {
                y
            }
        }
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
