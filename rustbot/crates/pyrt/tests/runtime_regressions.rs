//! Handpicked Python semantic cases for dynamic calls, scopes and builtins.
use pyrt::{
    PyExc, PyResult, Value, canon_json,
    frame::Frame,
    object::{CallArgs, IteratorObject, NativeFunction, Signature},
    runtime::Runtime,
};
use serde_json::json;
use std::{cell::Cell, rc::Rc};

fn call(rt: &mut Runtime, name: &str, values: Vec<Value>) -> PyResult<Value> {
    let f = rt.builtin(name)?;
    rt.call(&f, CallArgs::new(values))
}
fn call_kw(
    rt: &mut Runtime,
    name: &str,
    values: Vec<Value>,
    keywords: Vec<(&str, Value)>,
) -> PyResult<Value> {
    let f = rt.builtin(name)?;
    rt.call(
        &f,
        CallArgs {
            positional: values,
            keywords: keywords.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        },
    )
}
fn signature() -> Signature {
    Signature {
        name: "f".into(),
        positional: vec!["x".into(), "default".into()],
        positional_only: 1,
        defaults: vec![Value::list(vec![])],
        keyword_only: vec![("required".into(), None)],
        varargs: Some("args".into()),
        varkw: Some("kwargs".into()),
    }
}
fn counted(values: Vec<Value>) -> (Value, Rc<Cell<usize>>) {
    let count = Rc::new(Cell::new(0));
    let shared = count.clone();
    let mut values = values.into_iter();
    (
        IteratorObject::value(move |_| {
            shared.set(shared.get() + 1);
            Ok(values.next())
        }),
        count,
    )
}

#[test]
fn signature_preserves_mutable_defaults_and_posonly_names_in_kwargs() {
    let signature = signature();
    let args = CallArgs {
        positional: vec![Value::from(1i64)],
        keywords: vec![
            ("required".into(), Value::None),
            ("x".into(), Value::from(99i64)),
        ],
    };
    let first = signature.bind(args.clone()).unwrap();
    let second = signature.bind(args).unwrap();
    let first_default = first.dict_get(&Value::from("default")).unwrap();
    assert!(first_default.is_identical(&second.dict_get(&Value::from("default")).unwrap()));
    if let Value::List(values) = first_default {
        values.borrow_mut().push(Value::from(7i64));
    }
    assert_eq!(
        canon_json(&second.dict_get(&Value::from("default")).unwrap()).unwrap(),
        json!([7])
    );
    assert_eq!(
        canon_json(&first.dict_get(&Value::from("x")).unwrap()).unwrap(),
        json!(1)
    );
    assert_eq!(
        canon_json(&first.dict_get(&Value::from("kwargs")).unwrap()).unwrap(),
        json!({"x":99})
    );
    let missing = signature
        .bind(CallArgs {
            positional: vec![],
            keywords: vec![
                ("x".into(), Value::from(1i64)),
                ("required".into(), Value::None),
            ],
        })
        .unwrap_err();
    assert_eq!(missing.kind, "TypeError");
    let duplicate = signature
        .bind(CallArgs {
            positional: vec![Value::None, Value::None],
            keywords: vec![
                ("default".into(), Value::None),
                ("required".into(), Value::None),
            ],
        })
        .unwrap_err();
    assert_eq!(duplicate.kind, "TypeError");
}

#[test]
fn callargs_reject_duplicate_expansion_and_nonstring_keyword_names() {
    let mut args = CallArgs::default();
    args.keyword("x".into(), Value::None).unwrap();
    let mapping = Value::dict();
    mapping
        .dict_set(Value::from("x"), Value::from(2i64))
        .unwrap();
    assert_eq!(
        args.extend_keywords(&mapping).unwrap_err().kind,
        "TypeError"
    );
    let invalid = Value::dict();
    invalid.dict_set(Value::from(1i64), Value::None).unwrap();
    assert_eq!(
        CallArgs::default()
            .extend_keywords(&invalid)
            .unwrap_err()
            .kind,
        "TypeError"
    );
}

#[test]
fn frames_share_closure_cells_and_route_global_nonlocal_assignments() {
    let globals = Value::dict();
    globals
        .dict_set(Value::from("x"), Value::from(99i64))
        .unwrap();
    let module = Frame::module(globals.clone());
    let outer = Frame::function(module, &["x"], &[], &[], Value::dict());
    outer.store("x", Value::from(1i64)).unwrap();
    let inner = Frame::function(outer.clone(), &[], &[], &["x"], Value::dict());
    let mut rt = Runtime::default();
    assert_eq!(
        canon_json(&inner.load(&mut rt, "x").unwrap()).unwrap(),
        json!(1)
    );
    inner.store("x", Value::from(2i64)).unwrap();
    assert_eq!(
        canon_json(&outer.load(&mut rt, "x").unwrap()).unwrap(),
        json!(2)
    );
    let global_user = Frame::function(outer.clone(), &[], &["x"], &[], Value::dict());
    global_user.store("x", Value::from(100i64)).unwrap();
    assert_eq!(
        canon_json(&globals.dict_get(&Value::from("x")).unwrap()).unwrap(),
        json!(100)
    );
    assert_eq!(
        canon_json(&outer.load(&mut rt, "x").unwrap()).unwrap(),
        json!(2)
    );
    inner.delete("x").unwrap();
    assert_eq!(
        outer.load(&mut rt, "x").unwrap_err().kind,
        "UnboundLocalError"
    );
}

#[test]
fn unbound_free_cell_must_not_fall_back_to_module_global() {
    let globals = Value::dict();
    globals
        .dict_set(Value::from("x"), Value::from(99i64))
        .unwrap();
    let outer = Frame::function(Frame::module(globals), &["x"], &[], &[], Value::dict());
    let inner = Frame::function(outer, &[], &[], &[], Value::dict());
    assert_eq!(
        inner.load(&mut Runtime::default(), "x").unwrap_err().kind,
        "NameError"
    );
}

#[test]
fn enclosing_global_declaration_blocks_more_distant_closure_cell() {
    let globals = Value::dict();
    globals
        .dict_set(Value::from("x"), Value::from(99i64))
        .unwrap();
    let outer = Frame::function(Frame::module(globals), &["x"], &[], &[], Value::dict());
    outer.store("x", Value::from(1i64)).unwrap();
    let middle = Frame::function(outer, &[], &["x"], &[], Value::dict());
    let inner = Frame::function(middle, &[], &[], &[], Value::dict());
    assert_eq!(
        canon_json(&inner.load(&mut Runtime::default(), "x").unwrap()).unwrap(),
        json!(99)
    );
}

#[test]
fn all_any_and_min_max_preserve_shortcircuit_and_first_tie() {
    let mut rt = Runtime::default();
    let (source, count) = counted(vec![Value::Bool(false), Value::None]);
    assert!(matches!(
        call(&mut rt, "all", vec![source.clone()]).unwrap(),
        Value::Bool(false)
    ));
    assert_eq!(count.get(), 1);
    assert!(matches!(rt.next(&source).unwrap(), Some(Value::None)));
    let (source, count) = counted(vec![Value::Bool(true), Value::None]);
    assert!(matches!(
        call(&mut rt, "any", vec![source]).unwrap(),
        Value::Bool(true)
    ));
    assert_eq!(count.get(), 1);
    let first = Value::list(vec![Value::from(1i64)]);
    let second = Value::list(vec![Value::from(2i64)]);
    let keys = Rc::new(Cell::new(0));
    let shared = keys.clone();
    let key = NativeFunction::value("key", move |_, _| {
        shared.set(shared.get() + 1);
        Ok(Value::from(0i64))
    });
    let result = call_kw(
        &mut rt,
        "max",
        vec![Value::list(vec![first.clone(), second])],
        vec![("key", key)],
    )
    .unwrap();
    assert!(result.is_identical(&first));
    assert_eq!(keys.get(), 2);
    let default = Value::list(vec![]);
    let key = NativeFunction::value("must_not_run", |_, _| {
        Err(PyExc::value_error("key called on default"))
    });
    assert!(
        call_kw(
            &mut rt,
            "min",
            vec![Value::list(vec![])],
            vec![("default", default.clone()), ("key", key)]
        )
        .unwrap()
        .is_identical(&default)
    );
}

#[test]
fn map_and_zip_are_lazy_and_zip_consumes_left_to_right() {
    let mut rt = Runtime::default();
    let (left, count) = counted(vec![Value::from(1i64), Value::from(2i64)]);
    let mapped_calls = Rc::new(Cell::new(0));
    let shared = mapped_calls.clone();
    let identity = NativeFunction::value("identity", move |_, args| {
        shared.set(shared.get() + 1);
        Ok(args.positional[0].clone())
    });
    let mapped = call(&mut rt, "map", vec![identity, left.clone()]).unwrap();
    assert_eq!(count.get(), 0);
    assert_eq!(mapped_calls.get(), 0);
    assert_eq!(
        canon_json(&rt.next(&mapped).unwrap().unwrap()).unwrap(),
        json!(1)
    );
    assert_eq!(count.get(), 1);
    assert_eq!(mapped_calls.get(), 1);
    let (right, right_count) = counted(vec![]);
    let zipped = call(&mut rt, "zip", vec![left, right]).unwrap();
    assert_eq!(right_count.get(), 0);
    assert!(rt.next(&zipped).unwrap().is_none());
    assert_eq!(count.get(), 2);
    assert_eq!(right_count.get(), 1);
}

#[test]
fn sorting_evaluates_key_once_and_keeps_ties_stable_when_reversed() {
    let mut rt = Runtime::default();
    let values = vec![
        Value::from("a"),
        Value::from("bb"),
        Value::from("c"),
        Value::from("dd"),
    ];
    let calls = Rc::new(Cell::new(0));
    let shared = calls.clone();
    let key = NativeFunction::value("length", move |_, args| {
        shared.set(shared.get() + 1);
        Ok(Value::from(args.positional[0].as_str()?.len()))
    });
    let result = call_kw(
        &mut rt,
        "sorted",
        vec![Value::list(values)],
        vec![("key", key), ("reverse", Value::Bool(true))],
    )
    .unwrap();
    assert_eq!(canon_json(&result).unwrap(), json!(["bb", "dd", "a", "c"]));
    assert_eq!(calls.get(), 4);
}

#[test]
fn builtin_start_rejects_duplicate_positional_and_keyword() {
    let mut rt = Runtime::default();
    assert_eq!(
        call_kw(
            &mut rt,
            "sum",
            vec![Value::list(vec![]), Value::from(1i64)],
            vec![("start", Value::from(2i64))]
        )
        .unwrap_err()
        .kind,
        "TypeError"
    );
}

#[test]
fn enumerate_start_rejects_duplicate_positional_and_keyword() {
    let mut rt = Runtime::default();
    assert_eq!(
        call_kw(
            &mut rt,
            "enumerate",
            vec![Value::list(vec![]), Value::from(1i64)],
            vec![("start", Value::from(2i64))]
        )
        .unwrap_err()
        .kind,
        "TypeError"
    );
}

#[test]
fn sums_keep_large_integers_compensation_and_signed_zero() {
    let mut rt = Runtime::default();
    let huge: num_bigint::BigInt = num_bigint::BigInt::from(1) << 100usize;
    let result = call(
        &mut rt,
        "sum",
        vec![Value::list(vec![
            Value::Int(huge.clone()),
            Value::from(1i64),
            Value::Int(-huge),
        ])],
    )
    .unwrap();
    assert_eq!(canon_json(&result).unwrap(), json!(1));
    let result = call(
        &mut rt,
        "sum",
        vec![Value::list(vec![
            Value::Float(1e16),
            Value::Float(1.0),
            Value::Float(-1e16),
        ])],
    )
    .unwrap();
    assert!(matches!(result,Value::Float(x)if x==1.0));
    let Value::Float(empty) = call(
        &mut rt,
        "sum",
        vec![Value::list(vec![]), Value::Float(-0.0)],
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(empty.to_bits(), (-0.0f64).to_bits());
    assert_eq!(
        call(&mut rt, "sum", vec![Value::list(vec![]), Value::from("")])
            .unwrap_err()
            .kind,
        "TypeError"
    );
}

#[test]
fn integer_and_float_parsing_follow_python_literal_rules() {
    let mut rt = Runtime::default();
    for (text, base, expected) in [
        ("١٢", 10, "12"),
        ("１２", 10, "12"),
        ("𝟙𝟚", 10, "12"),
        ("  -0x_ff  ", 0, "-255"),
        ("00_0", 0, "0"),
        ("0b10", 16, "2832"),
        ("z", 36, "35"),
    ] {
        assert_eq!(
            call(
                &mut rt,
                "int",
                vec![Value::from(text), Value::from(i64::from(base))]
            )
            .unwrap()
            .as_int()
            .unwrap()
            .to_string(),
            expected
        );
    }
    for text in ["²", "①", "1__2", "1_", "_1", "0x_", "01", "\u{1c}12\u{1f}"] {
        assert_eq!(
            call(&mut rt, "int", vec![Value::from(text), Value::from(0i64)])
                .unwrap_err()
                .kind,
            "ValueError",
            "{text:?}"
        );
    }
    for (text, expected) in [
        ("١٢.٥", 12.5),
        (" +1_2.5_0e-1 ", 1.25),
        ("1.", 1.0),
        (".5", 0.5),
    ] {
        assert!(
            matches!(call(&mut rt,"float",vec![Value::from(text)]).unwrap(),Value::Float(x)if x==expected)
        );
    }
    for text in ["²", "1_.0", "1._0", "_1", "nan_", "\u{1c}12"] {
        assert_eq!(
            call(&mut rt, "float", vec![Value::from(text)])
                .unwrap_err()
                .kind,
            "ValueError"
        );
    }
    assert!(
        matches!(call(&mut rt,"float",vec![Value::from("-Infinity")]).unwrap(),Value::Float(x)if x==f64::NEG_INFINITY)
    );
    assert_eq!(
        call(&mut rt, "int", vec![Value::Float(f64::NAN)])
            .unwrap_err()
            .kind,
        "ValueError"
    );
    assert_eq!(
        call(&mut rt, "int", vec![Value::Float(f64::INFINITY)])
            .unwrap_err()
            .kind,
        "OverflowError"
    );
}

#[test]
fn module_import_caches_identity_sets_parent_attribute_and_retries_failure() {
    fn init(rt: &mut Runtime, module: &Value) -> PyResult<()> {
        let Value::List(calls) = rt.builtin("__import_calls")? else {
            panic!()
        };
        calls.borrow_mut().push(rt.getattr(module, "__name__")?);
        Ok(())
    }
    fn fail(rt: &mut Runtime, _: &Value) -> PyResult<()> {
        let Value::List(calls) = rt.builtin("__import_calls")? else {
            panic!()
        };
        calls.borrow_mut().push(Value::from("failure"));
        Err(PyExc::value_error("module failed"))
    }
    let mut rt = Runtime::default();
    let calls = Value::list(vec![]);
    rt.register_builtin("__import_calls", calls.clone());
    rt.register_module("pkg", init);
    rt.register_module("pkg.child", init);
    rt.register_module("broken", fail);
    let child = rt.import("pkg.child").unwrap();
    let cached = rt.import("pkg.child").unwrap();
    assert!(child.is_identical(&cached));
    let parent = rt.import("pkg").unwrap();
    assert!(rt.getattr(&parent, "child").unwrap().is_identical(&child));
    assert_eq!(canon_json(&calls).unwrap(), json!(["pkg", "pkg.child"]));
    assert_eq!(
        rt.getattr(&child, "__package__").unwrap().as_str().unwrap(),
        "pkg"
    );
    assert_eq!(rt.import("broken").unwrap_err().kind, "ValueError");
    assert_eq!(rt.import("broken").unwrap_err().kind, "ValueError");
    assert_eq!(
        canon_json(&calls).unwrap(),
        json!(["pkg", "pkg.child", "failure", "failure"])
    );
}

#[test]
fn fresh_object_instances_compare_by_identity() {
    let mut rt = Runtime::default();
    let first = call(&mut rt, "object", vec![]).unwrap();
    let second = call(&mut rt, "object", vec![]).unwrap();
    assert!(!first.equals(&second).unwrap());
    assert!(first.equals(&first.clone()).unwrap());
}
