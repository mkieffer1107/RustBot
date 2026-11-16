use pyrt::{PyExc, Value, exceptions, formatting, object::CallArgs, runtime::Runtime};
fn exception(rt: &mut Runtime, name: &str, args: Vec<Value>) -> Value {
    let class = rt.builtin(name).unwrap();
    rt.call(&class, CallArgs::new(args)).unwrap()
}

#[test]
fn constructors_preserve_args_python_repr_and_str() {
    let mut rt = Runtime::default();
    for (name, args, repr, text) in [
        ("Exception", vec![], "Exception()", ""),
        (
            "ValueError",
            vec![Value::from("bad")],
            "ValueError('bad')",
            "bad",
        ),
        ("Exception", vec![Value::from(1i64)], "Exception(1)", "1"),
        (
            "Exception",
            vec![Value::from("a"), Value::from(2i64)],
            "Exception('a', 2)",
            "('a', 2)",
        ),
        ("KeyError", vec![Value::from("x")], "KeyError('x')", "'x'"),
        ("KeyError", vec![Value::from(3i64)], "KeyError(3)", "3"),
    ] {
        let value = exception(&mut rt, name, args.clone());
        assert_eq!(formatting::repr(&value).unwrap(), repr);
        assert_eq!(formatting::str(&value).unwrap(), text);
        assert!(
            rt.getattr(&value, "args")
                .unwrap()
                .equals(&Value::tuple(args))
                .unwrap()
        );
        assert!(
            rt.getattr(&value, "__class__")
                .unwrap()
                .is_identical(&rt.builtin(name).unwrap())
        );
    }
}

#[test]
fn matching_follows_arithmetic_lookup_and_control_hierarchies() {
    let mut rt = Runtime::default();
    for (kind, parent, expected) in [
        ("ZeroDivisionError", "ArithmeticError", true),
        ("ZeroDivisionError", "Exception", true),
        ("KeyError", "LookupError", true),
        ("UnboundLocalError", "NameError", true),
        ("RecursionError", "RuntimeError", true),
        ("ModuleNotFoundError", "ImportError", true),
        ("UnicodeDecodeError", "ValueError", true),
        ("FileNotFoundError", "OSError", true),
        ("KeyboardInterrupt", "Exception", false),
        ("KeyboardInterrupt", "BaseException", true),
        ("GeneratorExit", "Exception", false),
        ("SystemExit", "BaseException", true),
        ("TypeError", "ValueError", false),
    ] {
        let error = PyExc {
            kind,
            message: "test".into(),
        };
        let class = rt.builtin(parent).unwrap();
        assert_eq!(
            exceptions::matches(&error, &class).unwrap(),
            expected,
            "{kind} {parent}"
        );
    }
    let tuple = Value::tuple(vec![
        rt.builtin("TypeError").unwrap(),
        rt.builtin("LookupError").unwrap(),
    ]);
    assert!(exceptions::matches(&PyExc::key_error("x"), &tuple).unwrap());
    assert!(!exceptions::matches(&PyExc::key_error("x"), &Value::tuple(vec![])).unwrap());
}

#[test]
fn catch_tuples_validate_all_members_and_reject_nested_tuples() {
    let mut rt = Runtime::default();
    let error = PyExc::value_error("bad");
    let good = rt.builtin("ValueError").unwrap();
    for invalid in [
        Value::from(42i64),
        Value::tuple(vec![good.clone(), Value::from(42i64)]),
        Value::tuple(vec![Value::tuple(vec![good])]),
        rt.builtin("int").unwrap(),
    ] {
        let error = exceptions::matches(&error, &invalid).unwrap_err();
        assert_eq!(error.kind, "TypeError");
        assert_eq!(
            error.message,
            "catching classes that do not inherit from BaseException is not allowed"
        );
    }
}

#[test]
fn raise_accepts_exception_instances_or_classes_and_rejects_other_values() {
    let mut rt = Runtime::default();
    let instance = exception(&mut rt, "KeyError", vec![Value::from("missing")]);
    let raised = exceptions::raised(&mut rt, &instance).unwrap();
    assert_eq!(raised.kind, "KeyError");
    assert_eq!(raised.message, "'missing'");
    let class = rt.builtin("RuntimeError").unwrap();
    let raised = exceptions::raised(&mut rt, &class).unwrap();
    assert_eq!(raised.kind, "RuntimeError");
    assert_eq!(raised.message, "");
    for invalid in [Value::from("bad"), Value::None, rt.builtin("int").unwrap()] {
        assert_eq!(
            exceptions::raised(&mut rt, &invalid).unwrap_err().kind,
            "TypeError"
        );
    }
    let original = PyExc::key_error("'key'");
    let wrapped = exceptions::value(&original);
    assert_eq!(formatting::str(&wrapped).unwrap(), original.message);
    assert_eq!(exceptions::raised(&mut rt, &wrapped).unwrap(), original);
}

#[test]
fn exception_instance_checks_aliases_and_args_mutations() {
    let mut rt = Runtime::default();
    let value = exception(&mut rt, "KeyError", vec![Value::from("x")]);
    let check = rt.builtin("isinstance").unwrap();
    for name in ["BaseException", "Exception", "LookupError", "KeyError"] {
        let class = rt.builtin(name).unwrap();
        assert!(matches!(
            rt.call(&check, CallArgs::new(vec![value.clone(), class]))
                .unwrap(),
            Value::Bool(true)
        ));
    }
    assert!(
        rt.builtin("IOError")
            .unwrap()
            .is_identical(&rt.builtin("OSError").unwrap())
    );
    rt.setattr(&value, "args", Value::list(vec![Value::from("new")]))
        .unwrap();
    assert_eq!(formatting::str(&value).unwrap(), "'new'");
    rt.setattr(&value, "note", Value::from("detail")).unwrap();
    assert_eq!(
        rt.getattr(&value, "note").unwrap().as_str().unwrap(),
        "detail"
    );
    let other = exception(&mut rt, "KeyError", vec![Value::from("new")]);
    assert!(!value.equals(&other).unwrap());
}

#[test]
fn control_exception_payloads_and_re_pattern_error_use_real_exception_values() {
    let mut rt = Runtime::default();
    let stopped = exception(&mut rt, "StopIteration", vec![Value::from(7i64)]);
    assert_eq!(
        rt.getattr(&stopped, "value")
            .unwrap()
            .as_int()
            .unwrap()
            .to_string(),
        "7"
    );
    let stopped = exception(&mut rt, "StopIteration", vec![]);
    assert!(matches!(
        rt.getattr(&stopped, "value").unwrap(),
        Value::None
    ));
    let exit = exception(
        &mut rt,
        "SystemExit",
        vec![Value::from(2i64), Value::from("detail")],
    );
    assert!(
        rt.getattr(&exit, "code")
            .unwrap()
            .equals(&Value::tuple(vec![
                Value::from(2i64),
                Value::from("detail")
            ]))
            .unwrap()
    );
    let pattern = exceptions::exception_type(&mut rt, "PatternError").unwrap();
    assert!(pattern.is_identical(&exceptions::exception_type(&mut rt, "PatternError").unwrap()));
    let value = rt
        .call(&pattern, CallArgs::new(vec![Value::from("bad pattern")]))
        .unwrap();
    assert_eq!(
        exceptions::raised(&mut rt, &value).unwrap().kind,
        "PatternError"
    );
    assert!(
        exceptions::matches(
            &PyExc {
                kind: "PatternError",
                message: "bad".into()
            },
            &rt.builtin("Exception").unwrap()
        )
        .unwrap()
    );
    assert_eq!(
        rt.getattr(&pattern, "__module__")
            .unwrap()
            .as_str()
            .unwrap(),
        "re"
    );
}

#[test]
fn recursive_exception_args_raise_instead_of_overflowing_rust_stack() {
    let mut rt = Runtime::default();
    let value = exception(&mut rt, "Exception", vec![]);
    rt.setattr(&value, "args", Value::tuple(vec![value.clone()]))
        .unwrap();
    assert_eq!(formatting::repr(&value).unwrap_err().kind, "RecursionError");
    assert_eq!(formatting::str(&value).unwrap_err().kind, "RecursionError");
    rt.setattr(&value, "args", Value::tuple(vec![])).unwrap();
}
