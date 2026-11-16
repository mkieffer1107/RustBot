use pyrt::{
    PyResult, Value, classes, formatting,
    object::{CallArgs, NativeFunction},
    runtime::Runtime,
};

fn namespace(methods: Vec<(&str, Value)>) -> Value {
    let ns = Value::dict();
    for (k, v) in methods {
        ns.dict_set(Value::from(k), v).unwrap();
    }
    ns
}
fn method(name: &str, body: impl Fn(&mut Runtime, CallArgs) -> PyResult<Value> + 'static) -> Value {
    NativeFunction::qualified(
        name.into(),
        format!("Widget.{name}"),
        "example.classes".into(),
        body,
    )
}
fn instantiate(rt: &mut Runtime, class: &Value, args: Vec<Value>) -> Value {
    rt.call(class, CallArgs::new(args)).unwrap()
}

#[test]
fn slots_init_repr_and_native_method_binding_preserve_metadata() {
    let mut rt = Runtime::default();
    let init = method("__init__", |rt, args| {
        args.arity(3, 3)?;
        rt.setattr(&args.positional[0], "left", args.positional[1].clone())?;
        rt.setattr(&args.positional[0], "right", args.positional[2].clone())?;
        Ok(Value::None)
    });
    let repr = method("__repr__", |rt, args| {
        let a = rt.getattr(&args.positional[0], "left")?;
        let b = rt.getattr(&args.positional[0], "right")?;
        Ok(Value::from(format!(
            "Pair({}, {})",
            formatting::repr(&a)?,
            formatting::repr(&b)?
        )))
    });
    let sum = method("total", |rt, args| {
        let a = rt.getattr(&args.positional[0], "left")?;
        let b = rt.getattr(&args.positional[0], "right")?;
        pyrt::ops::add(&a, &b)
    });
    let ns = namespace(vec![
        (
            "__slots__",
            Value::tuple(vec![Value::from("left"), Value::from("right")]),
        ),
        ("__init__", init),
        ("__repr__", repr),
        ("total", sum.clone()),
    ]);
    let pair = classes::create("Pair", "example.classes", vec![], ns, &rt).unwrap();
    let value = instantiate(&mut rt, &pair, vec![Value::from(3i64), Value::from(4i64)]);
    assert_eq!(formatting::repr(&value).unwrap(), "Pair(3, 4)");
    let bound = rt.getattr(&value, "total").unwrap();
    assert_eq!(
        rt.call(&bound, CallArgs::default())
            .unwrap()
            .as_int()
            .unwrap()
            .to_string(),
        "7"
    );
    assert!(rt.getattr(&bound, "__self__").unwrap().is_identical(&value));
    assert!(rt.getattr(&bound, "__func__").unwrap().is_identical(&sum));
    assert_eq!(
        rt.getattr(&bound, "__module__").unwrap().as_str().unwrap(),
        "example.classes"
    );
    assert!(rt.getattr(&value, "__class__").unwrap().is_identical(&pair));
    assert_eq!(
        rt.getattr(&value, "__dict__").unwrap_err().kind,
        "AttributeError"
    );
    assert_eq!(
        rt.setattr(&value, "extra", Value::None).unwrap_err().kind,
        "AttributeError"
    );
}

#[test]
fn unslotted_instance_dict_shadows_methods_without_binding_instance_callables() {
    let mut rt = Runtime::default();
    let original = method("greet", |_, args| {
        args.arity(1, 1)?;
        Ok(Value::from("class"))
    });
    let class = classes::create(
        "Greeting",
        "example",
        vec![],
        namespace(vec![("greet", original)]),
        &rt,
    )
    .unwrap();
    let instance = instantiate(&mut rt, &class, vec![]);
    let replacement = NativeFunction::value("replacement", |_, args| {
        args.arity(0, 0)?;
        Ok(Value::from("instance"))
    });
    rt.setattr(&instance, "greet", replacement.clone()).unwrap();
    let bound = rt.getattr(&instance, "greet").unwrap();
    assert!(bound.is_identical(&replacement));
    assert_eq!(
        rt.call(&bound, CallArgs::default())
            .unwrap()
            .as_str()
            .unwrap(),
        "instance"
    );
    let dict = rt.getattr(&instance, "__dict__").unwrap();
    dict.dict_set(Value::from("extra"), Value::from(9i64))
        .unwrap();
    assert_eq!(
        rt.getattr(&instance, "extra")
            .unwrap()
            .as_int()
            .unwrap()
            .to_string(),
        "9"
    );
    assert_eq!(
        formatting::repr(&instance).unwrap(),
        "<example.Greeting object at 0x0>"
    );
}

#[test]
fn c3_diamond_uses_python_order_and_inherited_methods() {
    let mut rt = Runtime::default();
    let root = classes::create(
        "Root",
        "example",
        vec![],
        namespace(vec![("value", Value::from("root"))]),
        &rt,
    )
    .unwrap();
    let left = classes::create(
        "Left",
        "example",
        vec![root.clone()],
        namespace(vec![]),
        &rt,
    )
    .unwrap();
    let right = classes::create(
        "Right",
        "example",
        vec![root],
        namespace(vec![("value", Value::from("right"))]),
        &rt,
    )
    .unwrap();
    let diamond = classes::create(
        "Diamond",
        "example",
        vec![left.clone(), right.clone()],
        namespace(vec![]),
        &rt,
    )
    .unwrap();
    let instance = instantiate(&mut rt, &diamond, vec![]);
    assert_eq!(
        rt.getattr(&instance, "value").unwrap().as_str().unwrap(),
        "right"
    );
    let mro = rt.getattr(&diamond, "__mro__").unwrap().sequence().unwrap();
    let names: Vec<String> = mro
        .iter()
        .map(|c| {
            rt.getattr(c, "__name__")
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(names, ["Diamond", "Left", "Right", "Root", "object"]);
    let reverse = classes::create(
        "Reverse",
        "example",
        vec![right, left],
        namespace(vec![]),
        &rt,
    )
    .unwrap();
    assert_eq!(
        classes::create(
            "Impossible",
            "example",
            vec![diamond, reverse],
            namespace(vec![]),
            &rt
        )
        .unwrap_err()
        .kind,
        "TypeError"
    );
}

#[test]
fn subclass_isinstance_and_inherited_slots_with_optional_dict() {
    let mut rt = Runtime::default();
    let base = classes::create(
        "Slotted",
        "example",
        vec![],
        namespace(vec![("__slots__", Value::from("field"))]),
        &rt,
    )
    .unwrap();
    let child = classes::create(
        "Child",
        "example",
        vec![base.clone()],
        namespace(vec![]),
        &rt,
    )
    .unwrap();
    let value = instantiate(&mut rt, &child, vec![]);
    rt.setattr(&value, "field", Value::from(5i64)).unwrap();
    rt.setattr(&value, "extra", Value::from(6i64)).unwrap();
    assert!(
        rt.getattr(&value, "__dict__")
            .unwrap()
            .dict_get(&Value::from("field"))
            .is_err()
    );
    assert_eq!(
        rt.getattr(&value, "field")
            .unwrap()
            .as_int()
            .unwrap()
            .to_string(),
        "5"
    );
    let isinstance = rt.builtin("isinstance").unwrap();
    assert!(matches!(
        rt.call(&isinstance, CallArgs::new(vec![value, base]))
            .unwrap(),
        Value::Bool(true)
    ));
    let no_dict = classes::create(
        "NoDict",
        "example",
        vec![child],
        namespace(vec![("__slots__", Value::tuple(vec![]))]),
        &rt,
    )
    .unwrap();
    let value = instantiate(&mut rt, &no_dict, vec![]);
    assert!(rt.getattr(&value, "__dict__").is_ok());
}

#[test]
fn falsey_repr_and_length_hash_hooks_execute_with_shared_runtime() {
    let mut rt = Runtime::default();
    rt.register_builtin("__class_marker", Value::from("shared"));
    let class = classes::create(
        "Sentinel",
        "example",
        vec![],
        namespace(vec![
            (
                "__bool__",
                method("__bool__", |_, _| Ok(Value::Bool(false))),
            ),
            (
                "__repr__",
                method("__repr__", |rt, _| {
                    rt.register_builtin("__callback_written", Value::Bool(true));
                    rt.builtin("__class_marker")
                }),
            ),
            ("__str__", method("__str__", |_, _| Ok(Value::from("text")))),
            ("__len__", method("__len__", |_, _| Ok(Value::from(3i64)))),
            (
                "__hash__",
                method("__hash__", |_, _| {
                    Ok(Value::Int(num_bigint::BigInt::from(1) << 100usize))
                }),
            ),
        ]),
        &rt,
    )
    .unwrap();
    let value = instantiate(&mut rt, &class, vec![]);
    assert!(!value.is_truthy());
    assert_eq!(formatting::repr(&value).unwrap(), "shared");
    assert!(matches!(
        rt.builtin("__callback_written").unwrap(),
        Value::Bool(true)
    ));
    assert_eq!(formatting::str(&value).unwrap(), "text");
    assert_eq!(pyrt::builtins::len(&value).unwrap(), 3);
    let hash = rt.builtin("hash").unwrap();
    assert_eq!(
        rt.call(&hash, CallArgs::new(vec![value]))
            .unwrap()
            .as_int()
            .unwrap()
            .to_string(),
        "549755813888"
    );
}

#[test]
fn equality_definitions_disable_hash_and_compare_fields() {
    let mut rt = Runtime::default();
    let ns = namespace(vec![
        (
            "__init__",
            method("__init__", |rt, args| {
                rt.setattr(&args.positional[0], "key", args.positional[1].clone())?;
                Ok(Value::None)
            }),
        ),
        (
            "__eq__",
            method("__eq__", |rt, args| {
                let a = rt.getattr(&args.positional[0], "key")?;
                let b = rt.getattr(&args.positional[1], "key")?;
                Ok(Value::Bool(a.equals(&b)?))
            }),
        ),
    ]);
    let class = classes::create("Key", "example", vec![], ns, &rt).unwrap();
    let a = instantiate(&mut rt, &class, vec![Value::from(7i64)]);
    let b = instantiate(&mut rt, &class, vec![Value::from(7i64)]);
    assert!(a.equals(&b).unwrap());
    let hash = rt.builtin("hash").unwrap();
    assert_eq!(
        rt.call(&hash, CallArgs::new(vec![a])).unwrap_err().kind,
        "TypeError"
    );
}

#[test]
fn invalid_slot_and_hook_results_raise_python_errors() {
    let mut rt = Runtime::default();
    assert_eq!(
        classes::create(
            "Bad",
            "example",
            vec![],
            namespace(vec![("__slots__", Value::from("not valid"))]),
            &rt
        )
        .unwrap_err()
        .kind,
        "TypeError"
    );
    assert_eq!(
        classes::create(
            "Conflict",
            "example",
            vec![],
            namespace(vec![
                ("__slots__", Value::from("field")),
                ("field", Value::None)
            ]),
            &rt
        )
        .unwrap_err()
        .kind,
        "ValueError"
    );
    let bad_init = classes::create(
        "BadInit",
        "example",
        vec![],
        namespace(vec![(
            "__init__",
            method("__init__", |_, _| Ok(Value::from(1i64))),
        )]),
        &rt,
    )
    .unwrap();
    assert_eq!(
        rt.call(&bad_init, CallArgs::default()).unwrap_err().kind,
        "TypeError"
    );
    let bad_len = classes::create(
        "BadLen",
        "example",
        vec![],
        namespace(vec![(
            "__len__",
            method("__len__", |_, _| Ok(Value::from(-1i64))),
        )]),
        &rt,
    )
    .unwrap();
    let value = instantiate(&mut rt, &bad_len, vec![]);
    assert_eq!(pyrt::builtins::len(&value).unwrap_err().kind, "ValueError");
    let bad_repr = classes::create(
        "BadRepr",
        "example",
        vec![],
        namespace(vec![(
            "__repr__",
            method("__repr__", |_, _| Ok(Value::None)),
        )]),
        &rt,
    )
    .unwrap();
    let value = instantiate(&mut rt, &bad_repr, vec![]);
    assert_eq!(formatting::repr(&value).unwrap_err().kind, "TypeError");
}

#[test]
fn slot_layout_conflicts_distinguish_common_ancestor_from_unrelated_storage() {
    let rt = Runtime::default();
    let root = classes::create(
        "Storage",
        "example",
        vec![],
        namespace(vec![("__slots__", Value::from("field"))]),
        &rt,
    )
    .unwrap();
    let left = classes::create(
        "Left",
        "example",
        vec![root.clone()],
        namespace(vec![("__slots__", Value::tuple(vec![]))]),
        &rt,
    )
    .unwrap();
    let right = classes::create(
        "Right",
        "example",
        vec![root],
        namespace(vec![("__slots__", Value::tuple(vec![]))]),
        &rt,
    )
    .unwrap();
    assert!(
        classes::create(
            "Diamond",
            "example",
            vec![left.clone(), right],
            namespace(vec![("__slots__", Value::tuple(vec![]))]),
            &rt
        )
        .is_ok()
    );
    let separate = classes::create(
        "Separate",
        "example",
        vec![],
        namespace(vec![("__slots__", Value::from("other"))]),
        &rt,
    )
    .unwrap();
    assert_eq!(
        classes::create(
            "Conflict",
            "example",
            vec![left, separate],
            namespace(vec![]),
            &rt
        )
        .unwrap_err()
        .kind,
        "TypeError"
    );
}
