use num_bigint::BigInt;
use pyrt::{
    Value, canon_json, containers,
    object::CallArgs,
    protocols::{self, ViewKind},
    runtime::Runtime,
};
use serde_json::json;
fn object(value: &Value) -> &dyn pyrt::object::PyObject {
    let Value::Object(object) = value else {
        panic!("expected object")
    };
    &**object
}
fn number(value: &Value) -> String {
    value.as_int().unwrap().to_string()
}
fn collect(rt: &mut Runtime, iterator: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    while let Some(v) = rt.next(iterator).unwrap() {
        out.push(v);
    }
    out
}

#[test]
fn enormous_range_supports_lazy_iteration_indexing_and_reverse() {
    let huge: BigInt = BigInt::from(10).pow(100);
    let range = protocols::range(huge.clone(), &huge + BigInt::from(10), 3.into()).unwrap();
    assert_eq!(object(&range).len().unwrap(), 4);
    assert_eq!(
        number(&object(&range).get_item(&Value::from(-1i64)).unwrap()),
        (&huge + 9i64).to_string()
    );
    assert!(object(&range).contains(&Value::Int(&huge + 6i64)).unwrap());
    assert!(!object(&range).contains(&Value::Int(&huge + 7i64)).unwrap());
    let mut rt = Runtime::default();
    let reversed = protocols::iter(&range, true).unwrap();
    assert_eq!(
        number(&rt.next(&reversed).unwrap().unwrap()),
        (&huge + 9i64).to_string()
    );
    let enormous = protocols::range(0.into(), huge.clone(), 1.into()).unwrap();
    assert_eq!(object(&enormous).len().unwrap_err().kind, "OverflowError");
    let iterator = protocols::iter(&enormous, false).unwrap();
    assert_eq!(number(&rt.next(&iterator).unwrap().unwrap()), "0");
    assert_eq!(number(&rt.next(&iterator).unwrap().unwrap()), "1");
    assert_eq!(
        number(
            &object(&enormous)
                .get_item(&Value::Int(&huge - 1i64))
                .unwrap()
        ),
        (&huge - 1i64).to_string()
    );
    assert_eq!(
        protocols::range(0.into(), 1.into(), 0.into())
            .unwrap_err()
            .kind,
        "ValueError"
    );
}

#[test]
fn ranges_membership_equality_hash_and_slices() {
    let a = protocols::range(0.into(), 10.into(), 2.into()).unwrap();
    let b = protocols::range(0.into(), 9.into(), 2.into()).unwrap();
    assert!(a.equals(&b).unwrap());
    assert_eq!(object(&a).hash().unwrap(), object(&b).hash().unwrap());
    assert_eq!(object(&a).hash().unwrap(), -6143170332311272345);
    assert!(object(&a).contains(&Value::Float(4.0)).unwrap());
    assert!(!object(&a).contains(&Value::Float(4.5)).unwrap());
    assert!(!object(&a).contains(&Value::from("4")).unwrap());
    let singleton_a = protocols::range(1.into(), 2.into(), 1.into()).unwrap();
    let singleton_b = protocols::range(1.into(), (-30).into(), (-99).into()).unwrap();
    assert!(singleton_a.equals(&singleton_b).unwrap());
    assert_eq!(
        object(&singleton_a).hash().unwrap(),
        object(&singleton_b).hash().unwrap()
    );
    let empty_a = protocols::range(0.into(), 0.into(), 1.into()).unwrap();
    let empty_b = protocols::range(2.into(), 100.into(), (-7).into()).unwrap();
    assert!(empty_a.equals(&empty_b).unwrap());
    assert_eq!(object(&empty_a).hash().unwrap(), 2676694398852732306);
    assert_eq!(object(&singleton_a).hash().unwrap(), -1269592299258668772);
    assert_eq!(
        object(&empty_a).hash().unwrap(),
        object(&empty_b).hash().unwrap()
    );
    let reversed = protocols::slice_range(&a, None, None, (-1).into()).unwrap();
    assert_eq!(
        canon_json(&Value::list(reversed.sequence().unwrap())).unwrap(),
        json!([8, 6, 4, 2, 0])
    );
    let sub = protocols::slice_range(&a, Some((-100).into()), Some(100.into()), 2.into()).unwrap();
    assert_eq!(
        canon_json(&Value::list(sub.sequence().unwrap())).unwrap(),
        json!([0, 4, 8])
    );
    assert_eq!(
        object(&a).get_item(&Value::Float(1.0)).unwrap_err().kind,
        "TypeError"
    );
    assert_eq!(
        object(&a).get_item(&Value::from(-6i64)).unwrap_err().kind,
        "IndexError"
    );
}

#[test]
fn list_iterators_observe_append_remove_and_reverse_start() {
    let list = Value::list(vec![Value::from(0i64), Value::from(1i64)]);
    let mut rt = Runtime::default();
    let forward = protocols::iter(&list, false).unwrap();
    assert_eq!(number(&rt.next(&forward).unwrap().unwrap()), "0");
    containers::call_method(&list, "append", &[Value::from(2i64)]).unwrap();
    assert_eq!(
        canon_json(&Value::list(collect(&mut rt, &forward))).unwrap(),
        json!([1, 2])
    );
    containers::call_method(&list, "append", &[Value::from(3i64)]).unwrap();
    assert!(rt.next(&forward).unwrap().is_none());
    let reverse = protocols::iter(&list, true).unwrap();
    containers::call_method(&list, "append", &[Value::from(4i64)]).unwrap();
    assert_eq!(number(&rt.next(&reverse).unwrap().unwrap()), "3");
    containers::call_method(&list, "clear", &[]).unwrap();
    assert!(rt.next(&reverse).unwrap().is_none());
    let unicode = protocols::iter(&Value::from("aé😀"), true).unwrap();
    assert_eq!(
        canon_json(&Value::list(collect(&mut rt, &unicode))).unwrap(),
        json!(["😀", "é", "a"])
    );
}

#[test]
fn dictionary_views_are_live_and_iterator_values_are_current() {
    let dict = Value::dict();
    dict.dict_set(Value::from("a"), Value::from(1i64)).unwrap();
    let keys = protocols::dict_view(&dict, ViewKind::Keys).unwrap();
    let values = protocols::dict_view(&dict, ViewKind::Values).unwrap();
    let items = containers::call_method(&dict, "items", &[]).unwrap();
    dict.dict_set(Value::from("b"), Value::from(2i64)).unwrap();
    assert_eq!(object(&keys).len().unwrap(), 2);
    assert_eq!(object(&keys).repr().unwrap(), "dict_keys(['a', 'b'])");
    assert!(object(&keys).contains(&Value::from("b")).unwrap());
    assert!(object(&values).contains(&Value::Float(2.0)).unwrap());
    assert!(
        object(&items)
            .contains(&Value::tuple(vec![Value::from("a"), Value::Bool(true)]))
            .unwrap()
    );
    let mut rt = Runtime::default();
    let iterator = protocols::iter(&values, false).unwrap();
    dict.dict_set(Value::from("a"), Value::from(10i64)).unwrap();
    assert_eq!(number(&rt.next(&iterator).unwrap().unwrap()), "10");
    let reverse = protocols::iter(&keys, true).unwrap();
    assert_eq!(rt.next(&reverse).unwrap().unwrap().as_str().unwrap(), "b");
    let set =
        Value::Set(pyrt::sets::PySet::from_values([Value::from("a"), Value::from("b")]).unwrap());
    assert!(keys.equals(&set).unwrap());
    let disjoint = object(&keys).get_attr(&mut rt, "isdisjoint").unwrap();
    assert!(matches!(
        rt.call(&disjoint, CallArgs::new(vec![Value::from("xyz")]))
            .unwrap(),
        Value::Bool(true)
    ));
}

#[test]
fn dictionary_iterator_size_errors_are_sticky_but_exhaustion_is_final() {
    let dict = Value::dict();
    dict.dict_set(Value::from("a"), Value::None).unwrap();
    let mut rt = Runtime::default();
    let iterator = protocols::iter(&dict, false).unwrap();
    dict.dict_set(Value::from("b"), Value::None).unwrap();
    let error = rt.next(&iterator).unwrap_err();
    assert_eq!(error.kind, "RuntimeError");
    assert_eq!(error.message, "dictionary changed size during iteration");
    containers::delitem(&dict, &Value::from("b")).unwrap();
    assert_eq!(rt.next(&iterator).unwrap_err().kind, "RuntimeError");
    let exhausted = protocols::iter(&dict, false).unwrap();
    assert!(rt.next(&exhausted).unwrap().is_some());
    assert!(rt.next(&exhausted).unwrap().is_none());
    dict.dict_set(Value::from("c"), Value::None).unwrap();
    assert!(rt.next(&exhausted).unwrap().is_none());
}
