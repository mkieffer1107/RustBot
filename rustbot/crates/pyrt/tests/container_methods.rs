use pyrt::{
    Value, canon_json,
    containers::{self, call_method},
    sets::{self, PySet},
};
use serde_json::json;

fn ints(n: i64) -> Value {
    Value::list((0..n).map(Value::from).collect())
}
fn set(text: &str) -> Value {
    Value::Set(PySet::from_values(text.chars().map(|c| Value::from(c.to_string()))).unwrap())
}
fn order(value: &Value) -> String {
    value
        .sequence()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect()
}

#[test]
fn list_mutations_handle_self_aliases_and_keep_child_aliases() {
    let a = ints(3);
    call_method(&a, "extend", std::slice::from_ref(&a)).unwrap();
    assert_eq!(canon_json(&a).unwrap(), json!([0, 1, 2, 0, 1, 2]));
    containers::setslice(&a, Some(1), Some(3), 1, &a).unwrap();
    assert_eq!(
        canon_json(&a).unwrap(),
        json!([0, 0, 1, 2, 0, 1, 2, 0, 1, 2])
    );
    let copied = call_method(&a, "copy", &[]).unwrap();
    call_method(&copied, "clear", &[]).unwrap();
    assert_eq!(a.sequence().unwrap().len(), 10);
    let cyclic = Value::list(vec![]);
    call_method(&cyclic, "append", std::slice::from_ref(&cyclic)).unwrap();
    assert_eq!(
        call_method(&cyclic, "count", std::slice::from_ref(&cyclic))
            .unwrap()
            .as_int()
            .unwrap()
            .to_string(),
        "1"
    );
    call_method(&cyclic, "remove", std::slice::from_ref(&cyclic)).unwrap();
    assert!(cyclic.sequence().unwrap().is_empty());
}

#[test]
fn slices_assignment_deletion_and_negative_steps() {
    let a = ints(8);
    assert_eq!(
        canon_json(&containers::slice(&a, None, None, -2).unwrap()).unwrap(),
        json!([7, 5, 3, 1])
    );
    containers::setslice(
        &a,
        Some(6),
        Some(1),
        -2,
        &Value::tuple(vec![
            Value::from(60i64),
            Value::from(40i64),
            Value::from(20i64),
        ]),
    )
    .unwrap();
    assert_eq!(canon_json(&a).unwrap(), json!([0, 1, 20, 3, 40, 5, 60, 7]));
    containers::delslice(&a, None, None, -3).unwrap();
    assert_eq!(canon_json(&a).unwrap(), json!([0, 20, 3, 5, 60]));
    let before = canon_json(&a).unwrap();
    assert_eq!(
        containers::setslice(&a, None, None, 2, &Value::list(vec![]))
            .unwrap_err()
            .kind,
        "ValueError"
    );
    assert_eq!(canon_json(&a).unwrap(), before);
    assert_eq!(
        containers::slice(&Value::from("aé😀z"), None, None, -1)
            .unwrap()
            .as_str()
            .unwrap(),
        "z😀éa"
    );
    containers::setslice(&a, Some(4), Some(1), 1, &Value::from("ab")).unwrap();
    assert_eq!(canon_json(&a).unwrap(), json!([0, 20, 3, 5, "a", "b", 60]));
    assert_eq!(
        containers::slice(&a, None, None, 0).unwrap_err().kind,
        "ValueError"
    );
}

#[test]
fn list_search_pop_and_tuple_methods() {
    let a = Value::list(vec![
        Value::Bool(true),
        Value::from(2i64),
        Value::Float(1.0),
    ]);
    assert_eq!(
        canon_json(&call_method(&a, "count", &[Value::from(1i64)]).unwrap()).unwrap(),
        json!(2)
    );
    assert_eq!(
        canon_json(&call_method(&a, "index", &[Value::from(1i64), Value::from(1i64)]).unwrap())
            .unwrap(),
        json!(2)
    );
    assert_eq!(
        canon_json(&call_method(&a, "pop", &[Value::from(-2i64)]).unwrap()).unwrap(),
        json!(2)
    );
    call_method(&a, "reverse", &[]).unwrap();
    assert_eq!(canon_json(&a).unwrap(), json!([{"$float":"1.0"},true]));
    let tuple = Value::tuple(a.sequence().unwrap());
    assert_eq!(
        call_method(&tuple, "append", &[Value::None])
            .unwrap_err()
            .kind,
        "AttributeError"
    );
    assert_eq!(
        call_method(&tuple, "index", &[Value::None])
            .unwrap_err()
            .kind,
        "ValueError"
    );
    let huge = Value::Int(num_bigint::BigInt::from(1) << 100usize);
    assert_eq!(
        call_method(&Value::list(vec![]), "pop", &[huge])
            .unwrap_err()
            .kind,
        "OverflowError"
    );
}

#[test]
fn dictionary_updates_numeric_keys_partial_failure_and_live_views() {
    let d = Value::dict();
    d.dict_set(Value::Bool(true), Value::from("first")).unwrap();
    call_method(
        &d,
        "update",
        &[Value::list(vec![
            Value::tuple(vec![Value::Float(1.0), Value::from("updated")]),
            Value::tuple(vec![Value::from("x"), Value::from(3i64)]),
        ])],
    )
    .unwrap();
    assert_eq!(
        canon_json(&d).unwrap(),
        json!({"$dict":[[true,"updated"],["x",3]]})
    );
    call_method(&d, "update", std::slice::from_ref(&d)).unwrap();
    let keys = call_method(&d, "keys", &[]).unwrap();
    let error = call_method(
        &d,
        "update",
        &[Value::list(vec![
            Value::tuple(vec![Value::from("y"), Value::from(4i64)]),
            Value::tuple(vec![Value::None]),
        ])],
    )
    .unwrap_err();
    assert_eq!(error.kind, "ValueError");
    assert!(d.dict_get(&Value::from("y")).is_ok());
    assert_eq!(keys.sequence().unwrap().len(), 3);
    assert_eq!(
        canon_json(&call_method(&d, "popitem", &[]).unwrap()).unwrap(),
        json!({"$tuple":["y",4]})
    );
    let default = Value::list(vec![]);
    assert!(
        call_method(&d, "setdefault", &[Value::from("new"), default.clone()])
            .unwrap()
            .is_identical(&default)
    );
    assert!(
        call_method(
            &Value::dict(),
            "pop",
            &[Value::list(vec![]), default.clone()]
        )
        .unwrap()
        .is_identical(&default)
    );
    containers::delitem(&d, &Value::Float(1.0)).unwrap();
    assert!(d.dict_get(&Value::Bool(true)).is_err());
}

#[test]
fn cpython_set_operations_preserve_exact_iteration_order() {
    let a = set("abcdefghij");
    let b = set("efghijklmnop");
    for (method, expected) in [
        ("union", "dnbamojfghkicpel"),
        ("intersection", "fghije"),
        ("difference", "dcab"),
        ("symmetric_difference", "dnpkbcm aol"),
    ] {
        let expected = expected.replace(' ', "");
        assert_eq!(
            order(&call_method(&a, method, std::slice::from_ref(&b)).unwrap()),
            expected,
            "{method}"
        );
    }
    let s = set("abcdefghijklmnop");
    call_method(&s, "difference_update", &[set("abcdefghi")]).unwrap();
    assert_eq!(order(&s), "nkjmopl");
    call_method(&s, "update", &[set("qrstuvwxyz")]).unwrap();
    assert_eq!(order(&s), "yquxnszrtkjmvopwl");
    let s = set("abcdefghijklmnop");
    call_method(&s, "intersection_update", &[set("efghijklmnopq")]).unwrap();
    assert_eq!(order(&s), "fnghkijmopel");
    let s = set("abcdefghij");
    call_method(&s, "symmetric_difference_update", &[b]).unwrap();
    assert_eq!(order(&s), "dnpkbcamol");
}

#[test]
fn set_pop_finger_clear_aliases_and_frozenset_immutability() {
    let s = set("abcdefghijklmnop");
    assert_eq!(call_method(&s, "pop", &[]).unwrap().as_str().unwrap(), "d");
    assert_eq!(call_method(&s, "pop", &[]).unwrap().as_str().unwrap(), "f");
    call_method(&s, "clear", &[]).unwrap();
    call_method(&s, "update", &[Value::from("abcdefghij")]).unwrap();
    assert_eq!(call_method(&s, "pop", &[]).unwrap().as_str().unwrap(), "g");
    assert_eq!(call_method(&s, "pop", &[]).unwrap().as_str().unwrap(), "h");
    call_method(&s, "update", std::slice::from_ref(&s)).unwrap();
    call_method(&s, "difference_update", std::slice::from_ref(&s)).unwrap();
    assert!(s.sequence().unwrap().is_empty());
    let frozen = Value::FrozenSet(PySet::from_values([Value::from(1i64)]).unwrap());
    assert!(
        call_method(&frozen, "copy", &[])
            .unwrap()
            .is_identical(&frozen)
    );
    assert_eq!(
        call_method(&frozen, "clear", &[]).unwrap_err().kind,
        "AttributeError"
    );
    assert!(matches!(
        call_method(&frozen, "union", &[Value::list(vec![Value::from(2i64)])]).unwrap(),
        Value::FrozenSet(_)
    ));
    assert!(matches!(
        call_method(
            &set(""),
            "issuperset",
            &[Value::list(vec![Value::from(1i64), Value::list(vec![])])]
        )
        .unwrap(),
        Value::Bool(false)
    ));
}

#[test]
fn date_hashes_match_cpython_zero_seed() {
    for (value, expected) in [
        (Value::Date("2026-06-15".into()), 452924636303377564),
        (
            Value::DateTime("2026-06-15T12:30:59.123456".into()),
            -3609695824440973690,
        ),
        (
            Value::DateTime("2026-06-15T12:30:59.123456+05:30".into()),
            -2647155010060751506,
        ),
        (
            Value::DateTime("0001-01-01T00:00:00+23:59".into()),
            1783508967117499556,
        ),
    ] {
        assert_eq!(sets::hash(&value).unwrap(), expected);
    }
}

#[test]
fn python_314_argument_and_unhashable_errors() {
    for (receiver, method, args, kind, message) in [
        (
            Value::list(vec![]),
            "append",
            vec![],
            "TypeError",
            "list.append() takes exactly one argument (0 given)",
        ),
        (
            Value::list(vec![]),
            "insert",
            vec![Value::None],
            "TypeError",
            "insert expected 2 arguments, got 1",
        ),
        (
            Value::list(vec![]),
            "index",
            vec![Value::None, Value::Float(1.2)],
            "TypeError",
            "slice indices must be integers or have an __index__ method",
        ),
        (
            Value::list(vec![]),
            "pop",
            vec![Value::Float(1.2)],
            "TypeError",
            "'float' object cannot be interpreted as an integer",
        ),
        (
            set(""),
            "add",
            vec![Value::list(vec![])],
            "TypeError",
            "cannot use 'list' as a set element (unhashable type: 'list')",
        ),
        (
            set(""),
            "discard",
            vec![Value::list(vec![])],
            "TypeError",
            "cannot use 'list' as a set element (unhashable type: 'list')",
        ),
        (
            set(""),
            "intersection",
            vec![Value::list(vec![Value::list(vec![])])],
            "TypeError",
            "unhashable type: 'list'",
        ),
        (
            set(""),
            "isdisjoint",
            vec![Value::list(vec![Value::list(vec![])])],
            "TypeError",
            "cannot use 'list' as a set element (unhashable type: 'list')",
        ),
    ] {
        let error = call_method(&receiver, method, &args).unwrap_err();
        assert_eq!(error.kind, kind);
        assert_eq!(error.message, message);
    }
    let error =
        containers::setitem(&Value::list(vec![]), &Value::Float(1.2), Value::None).unwrap_err();
    assert_eq!(
        error.message,
        "list indices must be integers or slices, not float"
    );
}
