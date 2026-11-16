use num_bigint::BigInt;
use pyrt::{Value, numeric_objects as numbers, object::CallArgs, runtime::Runtime};
use std::cmp::Ordering;
fn runtime() -> Runtime {
    let mut rt = Runtime::new(BigInt::from(0), "2026-06-15T12:00:00").unwrap();
    numbers::register(&mut rt);
    rt
}
fn construct(rt: &mut Runtime, module: &str, name: &str, args: Vec<Value>) -> Value {
    let module = rt.import(module).unwrap();
    let ty = rt.getattr(&module, name).unwrap();
    rt.call(&ty, CallArgs::new(args)).unwrap()
}
fn method(rt: &mut Runtime, value: &Value, name: &str, args: CallArgs) -> Value {
    let f = rt.getattr(value, name).unwrap();
    rt.call(&f, args).unwrap()
}
fn str_value(v: &Value) -> String {
    pyrt::formatting::str(v).unwrap()
}
#[test]
fn fractions_normalize_parse_and_convert_binary_float() {
    let mut rt = runtime();
    let f = construct(
        &mut rt,
        "fractions",
        "Fraction",
        vec![Value::from(-6i64), Value::from(-8i64)],
    );
    assert_eq!(str_value(&f), "3/4");
    assert_eq!(
        rt.getattr(&f, "numerator").unwrap().as_int().unwrap(),
        3.into()
    );
    let decimal = construct(
        &mut rt,
        "fractions",
        "Fraction",
        vec![Value::from("1.25e-1")],
    );
    assert_eq!(str_value(&decimal), "1/8");
    let exact = construct(&mut rt, "fractions", "Fraction", vec![Value::Float(0.1)]);
    assert_eq!(str_value(&exact), "3602879701896397/36028797018963968");
    let limited = method(
        &mut rt,
        &exact,
        "limit_denominator",
        CallArgs::new(vec![Value::from(1000i64)]),
    );
    assert_eq!(str_value(&limited), "1/10");
    assert_eq!(numbers::to_float(&exact).unwrap().unwrap(), 0.1);
}
#[test]
fn fraction_arithmetic_comparison_and_hash_match_python() {
    let mut rt = runtime();
    let a = construct(
        &mut rt,
        "fractions",
        "Fraction",
        vec![Value::from(-3i64), Value::from(4i64)],
    );
    let b = construct(
        &mut rt,
        "fractions",
        "Fraction",
        vec![Value::from(1i64), Value::from(2i64)],
    );
    for (op, expected) in [
        ("add", "-1/4"),
        ("sub", "-5/4"),
        ("mul", "-3/8"),
        ("truediv", "-3/2"),
        ("mod", "1/4"),
    ] {
        assert_eq!(
            str_value(&numbers::binary(op, &a, &b).unwrap().unwrap()),
            expected
        );
    }
    assert_eq!(
        numbers::binary("floordiv", &a, &b)
            .unwrap()
            .unwrap()
            .as_int()
            .unwrap(),
        BigInt::from(-2)
    );
    assert_eq!(numbers::compare(&a, &b).unwrap().unwrap(), Ordering::Less);
    assert_eq!(
        pyrt::sets::hash(&b).unwrap(),
        pyrt::sets::hash(&Value::Float(0.5)).unwrap()
    );
    for (text, expected) in [
        ("1/3", 1537228672809129301i64),
        ("-1/3", -1537228672809129301),
        ("1/2305843009213693951", 314159),
    ] {
        let f = construct(&mut rt, "fractions", "Fraction", vec![Value::from(text)]);
        assert_eq!(pyrt::sets::hash(&f).unwrap(), expected);
    }
}
#[test]
fn decimal_half_up_quantize_fixed_format_and_signed_zero() {
    let mut rt = runtime();
    for (text, places, expected) in [
        ("2.675", 2, "2.68"),
        ("-2.675", 2, "-2.68"),
        ("2.5", 0, "3"),
        ("-0.004", 2, "-0.00"),
        ("1499", -2, "1500"),
        ("-0.004", -3, "-0"),
    ] {
        let d = construct(&mut rt, "decimal", "Decimal", vec![Value::from(text)]);
        let unit = construct(&mut rt, "decimal", "Decimal", vec![Value::from(1i64)]);
        let quantum = method(
            &mut rt,
            &unit,
            "scaleb",
            CallArgs::new(vec![Value::from(-(places as i64))]),
        );
        let mut args = CallArgs::new(vec![quantum]);
        args.keyword("rounding".into(), Value::from("ROUND_HALF_UP"))
            .unwrap();
        let rounded = method(&mut rt, &d, "quantize", args);
        assert_eq!(
            pyrt::formatting::format(&rounded, "f").unwrap(),
            expected,
            "{text}"
        );
    }
}

#[test]
fn fraction_rounding_matches_ties_to_even() {
    let mut rt = runtime();
    for (n, d, expected) in [
        (1i64, 2i64, 0i64),
        (3, 2, 2),
        (5, 2, 2),
        (-1, 2, 0),
        (-3, 2, -2),
        (-5, 2, -2),
    ] {
        let value = construct(
            &mut rt,
            "fractions",
            "Fraction",
            vec![Value::from(n), Value::from(d)],
        );
        assert_eq!(
            numbers::round(&value, None)
                .unwrap()
                .unwrap()
                .as_int()
                .unwrap(),
            BigInt::from(expected)
        );
    }
    let value = construct(&mut rt, "fractions", "Fraction", vec![Value::from("2.675")]);
    let rounded = numbers::round(&value, Some(&Value::from(2i64)))
        .unwrap()
        .unwrap();
    assert_eq!(str_value(&rounded), "67/25");
}

#[test]
fn decimal_quantize_enforces_default_precision() {
    let mut rt = runtime();
    let value = construct(
        &mut rt,
        "decimal",
        "Decimal",
        vec![Value::from("12345678901234567890123456789")],
    );
    let unit = construct(&mut rt, "decimal", "Decimal", vec![Value::from(1i64)]);
    let quantize = rt.getattr(&value, "quantize").unwrap();
    let mut args = CallArgs::new(vec![unit]);
    args.keyword("rounding".into(), Value::from("ROUND_HALF_UP"))
        .unwrap();
    assert_eq!(
        rt.call(&quantize, args).unwrap_err().kind,
        "InvalidOperation"
    );
}

#[test]
fn decimal_float_conversion_handles_overflow_and_signed_underflow() {
    let mut rt = runtime();
    for (text, expected) in [
        ("1e400", f64::INFINITY),
        ("-1e400", f64::NEG_INFINITY),
        ("-1e-400", -0.0),
        ("-0", -0.0),
    ] {
        let value = construct(&mut rt, "decimal", "Decimal", vec![Value::from(text)]);
        assert_eq!(
            numbers::to_float(&value).unwrap().unwrap().to_bits(),
            expected.to_bits()
        );
    }
}

#[test]
fn fraction_keywords_power_and_divmod() {
    let mut rt = runtime();
    let ty = rt.import_from("fractions", "Fraction").unwrap();
    let mut args = CallArgs::default();
    args.keyword("numerator".into(), Value::from(-3i64))
        .unwrap();
    args.keyword("denominator".into(), Value::from(4i64))
        .unwrap();
    let value = rt.call(&ty, args).unwrap();
    let squared = numbers::binary("pow", &value, &Value::from(2i64))
        .unwrap()
        .unwrap();
    assert_eq!(str_value(&squared), "9/16");
    let inverse = numbers::binary("pow", &value, &Value::from(-1i64))
        .unwrap()
        .unwrap();
    assert_eq!(str_value(&inverse), "-4/3");
    let half = construct(
        &mut rt,
        "fractions",
        "Fraction",
        vec![Value::from(1i64), Value::from(2i64)],
    );
    let result = numbers::binary("divmod", &value, &half)
        .unwrap()
        .unwrap()
        .sequence()
        .unwrap();
    assert_eq!(result[0].as_int().unwrap(), BigInt::from(-2));
    assert_eq!(str_value(&result[1]), "1/4");
}
