//! Small CPython 3.14 boundary vectors; no bot corpus or oracle process needed.
use num_bigint::BigInt;
use num_traits::{One, Zero};
use pyrt::{Value, math, ops};
use std::cmp::Ordering;

fn bits(value: Value) -> u64 {
    let Value::Float(value) = value else {
        panic!("expected float")
    };
    value.to_bits()
}

fn integer(value: Value) -> BigInt {
    let Value::Int(value) = value else {
        panic!("expected integer")
    };
    value
}

#[test]
fn integer_true_division_rounds_once_after_huge_cancellation() {
    let huge = BigInt::from(10_u8).pow(400);
    assert_eq!(ops::int_ratio_to_float(&huge, &huge).unwrap(), 1.0);
    assert_eq!(
        ops::int_ratio_to_float(&(&huge + BigInt::from(10_u8).pow(384)), &huge).unwrap(),
        1.0
    );
    let unit = BigInt::one() << 53;
    assert_eq!(
        ops::int_ratio_to_float(&(&unit + 1), &unit)
            .unwrap()
            .to_bits(),
        0x3ff0000000000000
    );
    assert_eq!(
        ops::int_ratio_to_float(&(&unit + 3), &unit)
            .unwrap()
            .to_bits(),
        0x3ff0000000000002
    );
    let underflow_unit = BigInt::one() << 1075;
    for (numerator, expected) in [(1, 0), (3, 2), (5, 2)] {
        assert_eq!(
            ops::int_ratio_to_float(&numerator.into(), &underflow_unit)
                .unwrap()
                .to_bits(),
            expected
        );
        assert_eq!(
            ops::int_ratio_to_float(&(-BigInt::from(numerator)), &underflow_unit)
                .unwrap()
                .to_bits(),
            expected | (1 << 63)
        );
    }
    assert_eq!(
        ops::int_ratio_to_float(&BigInt::zero(), &(-huge))
            .unwrap()
            .to_bits(),
        (-0.0_f64).to_bits()
    );
}

#[test]
fn division_overflow_and_float_specials_match_python() {
    let boundary = (BigInt::one() << 1024) - (BigInt::one() << 970);
    assert_eq!(
        ops::int_ratio_to_float(&(&boundary - 1), &BigInt::one()).unwrap(),
        f64::MAX
    );
    assert_eq!(
        ops::int_ratio_to_float(&boundary, &BigInt::one())
            .unwrap_err()
            .kind,
        "OverflowError"
    );
    assert_eq!(
        ops::true_div(&Value::from(1_i64), &Value::from(0_i64))
            .unwrap_err()
            .kind,
        "ZeroDivisionError"
    );
    assert_eq!(
        ops::true_div(&Value::Float(f64::NAN), &Value::Float(0.0))
            .unwrap_err()
            .kind,
        "ZeroDivisionError"
    );
    assert_eq!(
        bits(ops::true_div(&Value::Float(0.0), &Value::Float(-1.0)).unwrap()),
        (-0.0_f64).to_bits()
    );
    assert!(
        matches!(ops::true_div(&Value::Float(f64::INFINITY), &Value::Float(f64::INFINITY)).unwrap(), Value::Float(x) if x.is_nan())
    );
    assert_eq!(
        bits(ops::true_div(&Value::Float(f64::MAX), &Value::Float(0.5)).unwrap()),
        f64::INFINITY.to_bits()
    );
}

#[test]
fn powers_modular_inverses_and_unary_operators() {
    assert_eq!(
        integer(ops::power(&Value::from(2_i64), &Value::from(200_i64)).unwrap()),
        BigInt::one() << 200
    );
    let huge_exponent = Value::Int(BigInt::one() << 200);
    assert_eq!(
        integer(ops::power(&Value::from(-1_i64), &huge_exponent).unwrap()),
        BigInt::one()
    );
    assert_eq!(
        bits(ops::power(&Value::from(2_i64), &Value::from(-3_i64)).unwrap()),
        0.125_f64.to_bits()
    );
    assert_eq!(
        bits(ops::power(&Value::Float(-0.0), &Value::Float(3.0)).unwrap()),
        (-0.0_f64).to_bits()
    );
    assert_eq!(
        bits(ops::power(&Value::Float(1.0), &Value::Float(f64::NAN)).unwrap()),
        1.0_f64.to_bits()
    );
    assert_eq!(
        bits(ops::power(&Value::Float(0.0), &Value::Float(f64::NEG_INFINITY)).unwrap()),
        f64::INFINITY.to_bits()
    );
    assert_eq!(
        ops::power(&Value::Float(1e308), &Value::Float(2.0))
            .unwrap_err()
            .kind,
        "OverflowError"
    );
    assert_eq!(
        ops::power(&Value::from(0_i64), &Value::from(-1_i64))
            .unwrap_err()
            .kind,
        "ZeroDivisionError"
    );
    for (a, b, modulus, expected) in [
        (2, -1, 5, 3),
        (2, -1, -5, -2),
        (-2, 3, -5, -3),
        (2, 0, -1, 0),
    ] {
        assert_eq!(
            integer(
                ops::pow_mod(
                    &Value::from(a as i64),
                    &Value::from(b as i64),
                    &Value::from(modulus as i64)
                )
                .unwrap()
            ),
            BigInt::from(expected)
        );
    }
    assert_eq!(
        ops::pow_mod(
            &Value::from(2_i64),
            &Value::from(-1_i64),
            &Value::from(4_i64)
        )
        .unwrap_err()
        .kind,
        "ValueError"
    );
    assert_eq!(
        ops::pow_mod(
            &Value::from(2_i64),
            &Value::from(3_i64),
            &Value::from(0_i64)
        )
        .unwrap_err()
        .kind,
        "ValueError"
    );
    assert_eq!(
        integer(ops::pos(&Value::Bool(true)).unwrap()),
        BigInt::one()
    );
    assert_eq!(
        integer(ops::invert(&Value::Bool(true)).unwrap()),
        BigInt::from(-2)
    );
    assert_eq!(
        ops::invert(&Value::Float(1.0)).unwrap_err().kind,
        "TypeError"
    );
}

#[test]
fn huge_integer_logs_use_cpython_frexp_and_evaluation_order() {
    let cases = [
        (
            BigInt::from(10_u8).pow(400),
            0x408cc845b54b54f1,
            0x4079000000000000,
        ),
        (
            BigInt::from(10_u8).pow(1000),
            0x40a1fd2b914f1516,
            0x408f400000000000,
        ),
        (
            (BigInt::one() << 4096) - 1,
            0x40a62e42fefa39ef,
            0x40934413509f79ff,
        ),
        (
            (BigInt::one() << 1024) - (BigInt::one() << 970),
            0x40862e42fefa39ef,
            0x40734413509f79ff,
        ),
    ];
    for (n, ln, log10) in cases {
        assert_eq!(
            bits(math::call("log", &[Value::Int(n.clone())]).unwrap()),
            ln
        );
        assert_eq!(
            bits(math::call("log10", &[Value::Int(n.clone())]).unwrap()),
            log10
        );
        assert_eq!(
            bits(math::call("log", &[Value::Int(n.clone()), Value::Int(n)]).unwrap()),
            1.0_f64.to_bits()
        );
    }
    let (mantissa, exponent) = math::int_frexp(&((BigInt::one() << 200) - 1));
    assert_eq!((mantissa, exponent), (0.5, 201));
}

#[test]
fn round_uses_decimal_half_even_without_double_rounding() {
    for (x, digits, expected) in [
        (2.675, 2, 0x40055c28f5c28f5c),
        (0.045, 2, 0x3fa47ae147ae147b),
        (0.15, 1, 0x3fb999999999999a),
        (1.25, 1, 0x3ff3333333333333),
        (1.35, 1, 0x3ff6666666666666),
        (125.0, -1, 0x405e000000000000),
        (135.0, -1, 0x4061800000000000),
        (-0.1, 0, 0x8000000000000000),
        (f64::from_bits(1), 323, 0),
        (f64::from_bits(1), 324, 1),
    ] {
        assert_eq!(
            bits(math::round(&Value::Float(x), Some(&Value::from(digits as i64))).unwrap()),
            expected
        );
    }
    for (value, expected) in [(2.5, 2), (3.5, 4), (-2.5, -2), (-3.5, -4)] {
        assert_eq!(
            integer(math::round(&Value::Float(value), None).unwrap()),
            BigInt::from(expected)
        );
    }
    for (value, expected) in [(125, 120), (135, 140), (-125, -120), (-135, -140)] {
        assert_eq!(
            integer(math::round(&Value::from(value as i64), Some(&Value::from(-1_i64))).unwrap()),
            BigInt::from(expected)
        );
    }
    assert_eq!(
        math::round(&Value::Float(f64::MAX), Some(&Value::from(-307_i64)))
            .unwrap_err()
            .kind,
        "OverflowError"
    );
    assert_eq!(
        math::round(&Value::Float(f64::NAN), None).unwrap_err().kind,
        "ValueError"
    );
    assert!(
        matches!(math::round(&Value::Float(f64::NAN), Some(&Value::from(0_i64))).unwrap(), Value::Float(x) if x.is_nan())
    );
    assert_eq!(
        integer(math::round(&Value::Float(1.25), Some(&Value::None)).unwrap()),
        BigInt::one()
    );
}

#[test]
fn inventory_math_errors_and_signed_zero() {
    for name in ["sin", "cos", "asin", "sqrt", "log", "log10", "radians"] {
        assert!(
            matches!(math::call(name, &[Value::Float(f64::NAN)]).unwrap(), Value::Float(x) if x.is_nan())
        );
    }
    for (name, argument, kind) in [
        ("sqrt", -1.0, "ValueError"),
        ("asin", 2.0, "ValueError"),
        ("sin", f64::INFINITY, "ValueError"),
        ("cos", f64::NEG_INFINITY, "ValueError"),
        ("log", 0.0, "ValueError"),
        ("log10", -1.0, "ValueError"),
        ("ceil", f64::INFINITY, "OverflowError"),
        ("floor", f64::NAN, "ValueError"),
    ] {
        assert_eq!(
            math::call(name, &[Value::Float(argument)])
                .unwrap_err()
                .kind,
            kind
        );
    }
    assert_eq!(
        bits(math::call("sqrt", &[Value::Float(-0.0)]).unwrap()),
        (-0.0_f64).to_bits()
    );
    assert_eq!(
        math::call("log", &[Value::from(2_i64), Value::from(1_i64)])
            .unwrap_err()
            .kind,
        "ZeroDivisionError"
    );
    assert_eq!(
        math::call("sqrt", &[Value::Int(BigInt::one() << 2000)])
            .unwrap_err()
            .kind,
        "OverflowError"
    );
}

#[test]
fn mixed_numeric_equality_and_ordering_remain_exact() {
    for (n, f, expected) in [
        (
            BigInt::from(9_007_199_254_740_993_i64),
            9_007_199_254_740_992.0,
            Ordering::Greater,
        ),
        (BigInt::from(-1), -1.5, Ordering::Greater),
        (BigInt::from(0), f64::from_bits(1), Ordering::Less),
        (BigInt::one() << 2000, f64::MAX, Ordering::Greater),
    ] {
        let integer = Value::Int(n);
        let float = Value::Float(f);
        assert_eq!(ops::compare(&integer, &float).unwrap(), Some(expected));
        assert_eq!(
            ops::compare(&float, &integer).unwrap(),
            Some(expected.reverse())
        );
        assert!(!integer.equals(&float).unwrap());
    }
    assert!(Value::Bool(true).equals(&Value::Float(1.0)).unwrap());
    assert_eq!(
        ops::compare(&Value::from(0_i64), &Value::Float(f64::NAN)).unwrap(),
        None
    );
}
