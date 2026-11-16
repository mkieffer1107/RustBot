use pyrt::{PyResult, Value, containers, object::PyObject};
use std::{any::Any, cell::Cell, rc::Rc};
fn get(dict: &Value, key: &str) -> Value {
    dict.dict_get(&Value::from(key)).unwrap()
}
fn keys(dict: &Value) -> Vec<String> {
    dict.sequence()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect()
}
#[test]
fn mutations_aliases_copy_views_and_reinsertion_preserve_order() {
    let dict = Value::dict();
    let alias = dict.clone();
    let first = Value::list(vec![]);
    for (k, v) in [
        ("a", first.clone()),
        ("b", Value::from(2_i64)),
        ("c", Value::from(3_i64)),
    ] {
        dict.dict_set(Value::from(k), v).unwrap();
    }
    assert!(get(&alias, "a").is_identical(&first));
    let view = containers::call_method(&dict, "keys", &[]).unwrap();
    let copy = containers::call_method(&dict, "copy", &[]).unwrap();
    containers::call_method(&alias, "pop", &[Value::from("b")]).unwrap();
    dict.dict_set(Value::from("b"), Value::from(4_i64)).unwrap();
    assert_eq!(keys(&dict), vec!["a", "c", "b"]);
    assert_eq!(keys(&copy), vec!["a", "b", "c"]);
    assert_eq!(
        view.sequence()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        vec!["a", "c", "b"]
    );
    assert_eq!(get(&alias, "b").as_int().unwrap(), 4.into());
    if let Value::Dict(entries) = &dict {
        entries.borrow_mut().reverse();
    }
    assert_eq!(keys(&dict), vec!["b", "c", "a"]);
    assert!(get(&dict, "a").is_identical(&first));
    containers::call_method(&dict, "clear", &[]).unwrap();
    dict.dict_set(Value::from("z"), Value::from(9_i64)).unwrap();
    assert_eq!(keys(&dict), vec!["z"]);
    assert_eq!(get(&dict, "z").as_int().unwrap(), 9.into());
}
#[derive(Debug)]
struct Mixed {
    calls: Rc<Cell<usize>>,
}
impl PyObject for Mixed {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "Mixed"
    }
    fn hash(&self) -> PyResult<i64> {
        Ok(8)
    }
    fn equals(&self, other: &Value) -> PyResult<bool> {
        self.calls.set(self.calls.get() + 1);
        Ok(other.as_str().ok() == Some("probe"))
    }
}
#[test]
fn mixed_keys_preserve_custom_equality_and_numeric_semantics() {
    let dict = Value::dict();
    for i in 0..40 {
        dict.dict_set(Value::from(format!("filler-{i}")), Value::from(i as i64))
            .unwrap();
    }
    dict.dict_set(Value::from("other"), Value::from(1_i64))
        .unwrap();
    assert!(dict.dict_get(&Value::from("none")).is_err());
    let calls = Rc::new(Cell::new(0));
    dict.dict_set(
        Value::Object(Rc::new(Mixed {
            calls: calls.clone(),
        })),
        Value::from(7_i64),
    )
    .unwrap();
    calls.set(0);
    assert_eq!(get(&dict, "probe").as_int().unwrap(), 7.into());
    assert_eq!(calls.get(), 1);
    dict.dict_set(Value::from(1_i64), Value::from("number"))
        .unwrap();
    assert_eq!(
        dict.dict_get(&Value::from(true)).unwrap().as_str().unwrap(),
        "number"
    );
}
#[test]
fn deepcopy_preserves_cycles_and_reindexes_without_sharing_mutations() {
    let dict = Value::dict();
    dict.dict_set(Value::from("self"), dict.clone()).unwrap();
    let copy = dict.deepcopy();
    assert!(get(&copy, "self").is_identical(&copy));
    assert!(!copy.is_identical(&dict));
    copy.dict_set(Value::from("fresh"), Value::from(1_i64))
        .unwrap();
    assert!(dict.dict_get(&Value::from("fresh")).is_err());
    if let Value::Dict(d) = dict {
        d.borrow_mut().clear();
    }
    if let Value::Dict(d) = copy {
        d.borrow_mut().clear();
    }
}

#[test]
fn overwriting_equal_string_keeps_original_key_identity() {
    let dict = Value::dict();
    let original = Value::from(String::from("identical"));
    let equal = Value::from(String::from("identical"));
    assert!(!original.is_identical(&equal));
    dict.dict_set(original.clone(), Value::from(1_i64)).unwrap();
    dict.dict_set(equal, Value::from(2_i64)).unwrap();
    assert!(dict.sequence().unwrap()[0].is_identical(&original));
    assert_eq!(get(&dict, "identical").as_int().unwrap(), 2.into());
}

#[test]
fn raw_duplicate_storage_keeps_first_match_and_invalidates_after_key_edits() {
    let dict = Value::dict();
    if let Value::Dict(entries) = &dict {
        let mut e = entries.borrow_mut();
        for i in 0..40 {
            e.push((Value::from(format!("key-{i}")), Value::from(i as i64)));
        }
        e.push((Value::from("key-0"), Value::from(999_i64)));
    }
    assert_eq!(get(&dict, "key-0").as_int().unwrap(), 0.into());
    if let Value::Dict(entries) = &dict {
        entries.borrow_mut()[0].0 = Value::from("changed");
    }
    assert_eq!(get(&dict, "key-0").as_int().unwrap(), 999.into());
    assert_eq!(get(&dict, "changed").as_int().unwrap(), 0.into());
    if let Value::Dict(entries) = &dict {
        let mut e = entries.borrow_mut();
        let item = e
            .iter_mut()
            .find(|(k, _)| k.as_str().ok() == Some("key-1"))
            .unwrap();
        item.0 = Value::from("changed-again");
    }
    assert_eq!(get(&dict, "changed-again").as_int().unwrap(), 1.into());
    assert!(dict.dict_get(&Value::from("key-1")).is_err());
}

#[test]
fn removed_mixed_key_and_whole_storage_swap_rebuild_correctly() {
    let dict = Value::dict();
    for i in 0..40 {
        dict.dict_set(Value::from(format!("key-{i}")), Value::from(i as i64))
            .unwrap();
    }
    dict.dict_set(Value::from(42_i64), Value::from("numeric"))
        .unwrap();
    assert_eq!(get(&dict, "key-8").as_int().unwrap(), 8.into());
    containers::call_method(&dict, "pop", &[Value::from(42_i64)]).unwrap();
    assert_eq!(get(&dict, "key-8").as_int().unwrap(), 8.into());
    if let Value::Dict(entries) = &dict {
        let old = std::mem::take(&mut *entries.borrow_mut());
        assert_eq!(old.len(), 40);
        assert!(dict.dict_get(&Value::from("key-8")).is_err());
        *entries.borrow_mut() = old;
    }
    assert_eq!(get(&dict, "key-8").as_int().unwrap(), 8.into());
}
