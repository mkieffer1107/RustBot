use pyrt::{
    PyExc, Value, canon_json, containers,
    object::IteratorObject,
    ops,
    protocols::{self, ViewKind},
    runtime::Runtime,
};
use serde_json::json;
use std::{cell::Cell, rc::Rc};
fn dictionary(values: &[(&str, Value)]) -> Value {
    let d = Value::dict();
    for (k, v) in values {
        d.dict_set(Value::from(*k), v.clone()).unwrap();
    }
    d
}
fn keys(text: &str) -> Value {
    let d = Value::dict();
    for c in text.chars() {
        d.dict_set(Value::from(c.to_string()), Value::None).unwrap();
    }
    protocols::dict_view(&d, ViewKind::Keys).unwrap()
}
fn order(v: &Value) -> String {
    v.sequence()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect()
}

#[test]
fn normalize_phrases_items_union_has_cpython_order_and_stable_sort_ties() {
    let a = dictionary(&[
        ("u r", Value::from("you are")),
        ("a/b", Value::from("and")),
        ("long one", Value::from("longer")),
        ("x&y", Value::from("xy")),
    ]);
    let b = dictionary(&[
        ("u r", Value::from("you are")),
        ("phrase two", Value::from("v")),
        ("third longer", Value::from("w")),
    ]);
    let a = containers::call_method(&a, "items", &[]).unwrap();
    let b = containers::call_method(&b, "items", &[]).unwrap();
    let result = ops::bitwise("|", &a, &b).unwrap();
    assert_eq!(
        canon_json(&Value::list(result.sequence().unwrap())).unwrap(),
        json!([{"$tuple":["x&y","xy"]},{"$tuple":["phrase two","v"]},{"$tuple":["a/b","and"]},{"$tuple":["long one","longer"]},{"$tuple":["third longer","w"]},{"$tuple":["u r","you are"]}])
    );
    let mut rt = Runtime::default();
    let key = pyrt::object::NativeFunction::value("length", |_, args| {
        let pair = args.positional[0].sequence()?;
        Ok(Value::from(-(pair[0].as_str()?.len() as i64)))
    });
    let sorted =
        pyrt::builtins::sort(&mut rt, result.sequence().unwrap(), Some(key), false).unwrap();
    assert_eq!(
        sorted
            .iter()
            .map(|v| v.sequence().unwrap()[0].as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        [
            "third longer",
            "phrase two",
            "long one",
            "x&y",
            "a/b",
            "u r"
        ]
    );
}

#[test]
fn dict_keys_algebra_matches_cpython_iteration_order() {
    let a = keys("abcdefghij");
    let b = keys("efghijklmnop");
    for (op, expected) in [
        ("|", "dfnghpkbicamojel"),
        ("&", "fghije"),
        ("^", "dnpkbcamol"),
        ("-", "dbca"),
    ] {
        let result = if op == "-" {
            ops::sub(&a, &b)
        } else {
            ops::bitwise(op, &a, &b)
        }
        .unwrap();
        assert_eq!(order(&result), expected, "{op}");
    }
}

#[test]
fn views_accept_reflected_sets_lists_and_values_as_other_iterables() {
    let a = keys("ab");
    let list = Value::list(vec![Value::from("b"), Value::from("c")]);
    let mut rt = Runtime::default();
    for (left, right) in [(&a, &list), (&list, &a)] {
        let union = ops::bitwise_rt(&mut rt, "|", left, right).unwrap();
        assert_eq!(union.sequence().unwrap().len(), 3);
    }
    assert_eq!(order(&ops::sub_rt(&mut rt, &list, &a).unwrap()), "c");
    let values =
        protocols::dict_view(&dictionary(&[("x", Value::from("c"))]), ViewKind::Values).unwrap();
    assert_eq!(
        ops::bitwise_rt(&mut rt, "|", &a, &values)
            .unwrap()
            .sequence()
            .unwrap()
            .len(),
        3
    );
    assert!(ops::bitwise_rt(&mut rt, "|", &values, &list).is_err());
    let plain = Value::Set(
        pyrt::sets::PySet::from_values([Value::from("b"), Value::from("c"), Value::from("d")])
            .unwrap(),
    );
    assert_eq!(
        order(&ops::bitwise_rt(&mut rt, "&", &a, &plain).unwrap()),
        "b"
    );
}

#[test]
fn items_symmetric_difference_can_cancel_equal_unhashable_values() {
    let a =
        protocols::dict_view(&dictionary(&[("a", Value::list(vec![]))]), ViewKind::Items).unwrap();
    let b =
        protocols::dict_view(&dictionary(&[("a", Value::list(vec![]))]), ViewKind::Items).unwrap();
    assert!(
        ops::bitwise("^", &a, &b)
            .unwrap()
            .sequence()
            .unwrap()
            .is_empty()
    );
    assert!(
        ops::bitwise("&", &a, &Value::list(vec![]))
            .unwrap()
            .sequence()
            .unwrap()
            .is_empty()
    );
    assert_eq!(ops::bitwise("|", &a, &b).unwrap_err().kind, "TypeError");
    assert_eq!(ops::bitwise("&", &a, &b).unwrap_err().kind, "TypeError");
    assert_eq!(ops::sub(&a, &b).unwrap_err().kind, "TypeError");
    let distinct = protocols::dict_view(
        &dictionary(&[("a", Value::list(vec![Value::None]))]),
        ViewKind::Items,
    )
    .unwrap();
    assert_eq!(
        ops::bitwise("^", &a, &distinct).unwrap_err().kind,
        "TypeError"
    );
}

#[test]
fn runtime_aware_view_union_stops_lazy_input_at_first_unhashable_value() {
    let a = keys("a");
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let iterator = IteratorObject::value(move |_| {
        let n = count.get();
        count.set(n + 1);
        match n {
            0 => Ok(Some(Value::from("b"))),
            1 => Ok(Some(Value::list(vec![]))),
            _ => Err(PyExc::value_error("later error must not run")),
        }
    });
    let mut rt = Runtime::default();
    assert_eq!(
        ops::bitwise_rt(&mut rt, "|", &a, &iterator)
            .unwrap_err()
            .kind,
        "TypeError"
    );
    assert_eq!(calls.get(), 2);
    assert_eq!(
        rt.next(&iterator).unwrap_err().message,
        "later error must not run"
    );
    let iterator = IteratorObject::from_values(vec![Value::from("a"), Value::from("c")]);
    assert_eq!(order(&ops::sub_rt(&mut rt, &iterator, &a).unwrap()), "c");
}
