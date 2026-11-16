use pyrt::{
    PyExc, Value, classes, exceptions, formatting,
    object::{CallArgs, NativeFunction},
    runtime::Runtime,
};
use std::{cell::Cell, rc::Rc};
fn namespace(entries: Vec<(&str, Value)>) -> Value {
    let out = Value::dict();
    for (k, v) in entries {
        out.dict_set(Value::from(k), v).unwrap();
    }
    out
}
fn constructor(rt: &mut Runtime, module: &str, name: &str, args: Vec<Value>) -> Value {
    let module = rt.import(module).unwrap();
    let class = rt.getattr(&module, name).unwrap();
    rt.call(&class, CallArgs::new(args)).unwrap()
}

#[test]
fn deepcopy_slotted_class_preserves_mixed_cycles_without_running_init_again() {
    let mut rt = Runtime::default();
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let init = NativeFunction::value("__init__", move |rt, args| {
        count.set(count.get() + 1);
        rt.setattr(&args.positional[0], "child", args.positional[1].clone())?;
        Ok(Value::None)
    });
    let class = classes::create(
        "Cell",
        "fixture",
        vec![],
        namespace(vec![
            (
                "__slots__",
                Value::tuple(vec![Value::from("child"), Value::from("peer")]),
            ),
            ("__init__", init),
        ]),
        &rt,
    )
    .unwrap();
    let child = Value::list(vec![]);
    let original = rt.call(&class, CallArgs::new(vec![child.clone()])).unwrap();
    rt.setattr(&original, "peer", original.clone()).unwrap();
    if let Value::List(values) = &child {
        values.borrow_mut().push(original.clone());
    }
    let copied = Value::list(vec![original.clone(), original.clone(), child.clone()])
        .deepcopy()
        .sequence()
        .unwrap();
    assert!(!copied[0].is_identical(&original));
    assert!(copied[0].is_identical(&copied[1]));
    assert!(
        rt.getattr(&copied[0], "peer")
            .unwrap()
            .is_identical(&copied[0])
    );
    let copied_child = rt.getattr(&copied[0], "child").unwrap();
    assert!(!copied_child.is_identical(&child));
    assert!(copied_child.is_identical(&copied[2]));
    assert!(copied_child.sequence().unwrap()[0].is_identical(&copied[0]));
    assert!(
        rt.getattr(&copied[0], "__class__")
            .unwrap()
            .is_identical(&class)
    );
    assert_eq!(calls.get(), 1);
    for value in [&original, &copied[0]] {
        rt.setattr(value, "peer", Value::None).unwrap();
    }
    for value in [child, copied_child] {
        if let Value::List(values) = value {
            values.borrow_mut().clear();
        }
    }
}

#[test]
fn deepcopy_reuses_public_instance_dictionary_even_when_dictionary_is_copied_first() {
    let mut rt = Runtime::default();
    let class = classes::create("Record", "fixture", vec![], Value::dict(), &rt).unwrap();
    let original = rt.call(&class, CallArgs::default()).unwrap();
    let attrs = rt.getattr(&original, "__dict__").unwrap();
    rt.setattr(&original, "self", original.clone()).unwrap();
    let copied = Value::list(vec![attrs, original.clone()])
        .deepcopy()
        .sequence()
        .unwrap();
    let copied_attrs = rt.getattr(&copied[1], "__dict__").unwrap();
    assert!(copied[0].is_identical(&copied_attrs));
    assert!(
        copied_attrs
            .dict_get(&Value::from("self"))
            .unwrap()
            .is_identical(&copied[1])
    );
    assert!(!copied[1].is_identical(&original));
    rt.setattr(&original, "self", Value::None).unwrap();
    rt.setattr(&copied[1], "self", Value::None).unwrap();
}

#[test]
fn deepcopy_exception_copies_args_and_attributes_with_the_surrounding_memo() {
    let mut rt = Runtime::default();
    let class = rt.builtin("ValueError").unwrap();
    let payload = Value::list(vec![Value::from(1i64)]);
    let original = rt
        .call(&class, CallArgs::new(vec![payload.clone()]))
        .unwrap();
    rt.setattr(&original, "payload", payload.clone()).unwrap();
    let copied = Value::list(vec![original.clone(), original, payload.clone()])
        .deepcopy()
        .sequence()
        .unwrap();
    assert!(copied[0].is_identical(&copied[1]));
    assert!(!copied[2].is_identical(&payload));
    assert!(
        rt.getattr(&copied[0], "args").unwrap().sequence().unwrap()[0].is_identical(&copied[2])
    );
    assert!(
        rt.getattr(&copied[0], "payload")
            .unwrap()
            .is_identical(&copied[2])
    );
}

#[test]
fn deepcopy_keeps_immutable_native_objects_but_allocates_plain_objects() {
    let mut rt = Runtime::default();
    let fraction = constructor(
        &mut rt,
        "fractions",
        "Fraction",
        vec![Value::from(1i64), Value::from(3i64)],
    );
    let decimal = constructor(&mut rt, "decimal", "Decimal", vec![Value::from("1.25")]);
    let bytes = pyrt::bytes::BytesObject::value(vec![1, 2]);
    let re = rt.import("re").unwrap();
    let compile = rt.getattr(&re, "compile").unwrap();
    let pattern = rt
        .call(&compile, CallArgs::new(vec![Value::from("a+")]))
        .unwrap();
    for value in [fraction, decimal, bytes, pattern] {
        assert!(value.is_identical(&value.deepcopy()));
    }
    let plain = rt.builtin("object").unwrap();
    let plain = rt.call(&plain, CallArgs::default()).unwrap();
    assert!(!plain.is_identical(&plain.deepcopy()));
}

#[test]
fn decimal_signals_match_broad_handlers_and_secondary_exception_bases() {
    let mut rt = Runtime::default();
    let module = rt.import("decimal").unwrap();
    let invalid = rt.getattr(&module, "InvalidOperation").unwrap();
    let decimal = rt.getattr(&module, "Decimal").unwrap();
    let error = rt
        .call(&decimal, CallArgs::new(vec![Value::from("bad")]))
        .unwrap_err();
    assert_eq!(error.kind, "InvalidOperation");
    assert_eq!(error.message, "[<class 'decimal.ConversionSyntax'>]");
    for name in ["Exception", "ArithmeticError"] {
        let class = rt.builtin(name).unwrap();
        assert!(exceptions::matches(&error, &class).unwrap());
    }
    assert!(exceptions::matches(&error, &invalid).unwrap());
    assert_eq!(
        rt.getattr(&invalid, "__module__")
            .unwrap()
            .as_str()
            .unwrap(),
        "decimal"
    );
    for (kind, base) in [
        ("DivisionByZero", "ZeroDivisionError"),
        ("DivisionUndefined", "ZeroDivisionError"),
        ("FloatOperation", "TypeError"),
    ] {
        assert!(
            exceptions::matches(
                &PyExc {
                    kind,
                    message: String::new()
                },
                &rt.builtin(base).unwrap()
            )
            .unwrap()
        );
    }
    let underflow = rt.getattr(&module, "Underflow").unwrap();
    let bases = rt
        .getattr(&underflow, "__bases__")
        .unwrap()
        .sequence()
        .unwrap();
    let names: Vec<String> = bases
        .iter()
        .map(|b| {
            rt.getattr(b, "__name__")
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(names, ["Inexact", "Rounded", "Subnormal"]);
    let value = constructor(
        &mut rt,
        "decimal",
        "Decimal",
        vec![Value::from("12345678901234567890123456789")],
    );
    let quantize = rt.getattr(&value, "quantize").unwrap();
    let unit = constructor(&mut rt, "decimal", "Decimal", vec![Value::from(1i64)]);
    let error = rt.call(&quantize, CallArgs::new(vec![unit])).unwrap_err();
    assert_eq!(error.kind, "InvalidOperation");
    assert_eq!(error.message, "[<class 'decimal.InvalidOperation'>]");
    assert_eq!(
        formatting::str(&exceptions::value(&error)).unwrap(),
        error.message
    );
}

#[test]
fn decimal_default_quantize_uses_half_even_and_explicit_half_up_still_works() {
    let mut rt = Runtime::default();
    let unit = constructor(&mut rt, "decimal", "Decimal", vec![Value::from(1i64)]);
    for (text, expected) in [("2.5", "2"), ("3.5", "4"), ("-0.5", "-0")] {
        let value = constructor(&mut rt, "decimal", "Decimal", vec![Value::from(text)]);
        let method = rt.getattr(&value, "quantize").unwrap();
        let value = rt.call(&method, CallArgs::new(vec![unit.clone()])).unwrap();
        assert_eq!(formatting::str(&value).unwrap(), expected);
    }
    let value = constructor(&mut rt, "decimal", "Decimal", vec![Value::from("2.5")]);
    let method = rt.getattr(&value, "quantize").unwrap();
    let value = rt
        .call(
            &method,
            CallArgs {
                positional: vec![unit],
                keywords: vec![("rounding".into(), Value::from("ROUND_HALF_UP"))],
            },
        )
        .unwrap();
    assert_eq!(formatting::str(&value).unwrap(), "3");
}
