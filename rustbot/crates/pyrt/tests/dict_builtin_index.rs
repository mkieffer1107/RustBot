use pyrt::{PyResult, Value, containers, object::PyObject};
use std::{any::Any, cell::Cell, rc::Rc};
fn fill(d: &Value) {
    for i in 0..40 {
        d.dict_set(Value::from(format!("filler-{i}")), Value::from(i as i64))
            .unwrap();
    }
}
fn tup(v: Vec<Value>) -> Value {
    Value::tuple(v)
}
#[test]
fn mixed_phrase_keys_nested_numeric_aliases_and_order() {
    let d = Value::dict();
    fill(&d);
    let alias = d.clone();
    let phrase = Value::from("coffee and cake");
    let bare = tup(vec![Value::from("bare"), phrase.clone()]);
    let stored = Value::list(vec![]);
    d.dict_set(phrase.clone(), Value::from("phrase")).unwrap();
    d.dict_set(bare.clone(), stored.clone()).unwrap();
    assert!(
        alias
            .dict_get(&tup(vec![
                Value::from("bare"),
                Value::from("coffee and cake")
            ]))
            .unwrap()
            .is_identical(&stored)
    );
    let one = Value::from(1_i64);
    d.dict_set(one.clone(), Value::from("one")).unwrap();
    d.dict_set(Value::from(true), Value::from("updated"))
        .unwrap();
    assert_eq!(d.dict_get(&one).unwrap().as_str().unwrap(), "updated");
    let nested = tup(vec![
        Value::from("nested"),
        tup(vec![Value::from(true), Value::from(2_i64)]),
    ]);
    let equivalent = tup(vec![
        Value::from("nested"),
        tup(vec![Value::from(1_i64), Value::from(2_i64)]),
    ]);
    d.dict_set(nested.clone(), stored.clone()).unwrap();
    d.dict_set(equivalent.clone(), Value::from("nested-updated"))
        .unwrap();
    let keys = d.sequence().unwrap();
    assert!(keys.iter().any(|k| k.is_identical(&nested)));
    assert!(!keys.iter().any(|k| k.is_identical(&equivalent)));
    containers::call_method(&d, "pop", std::slice::from_ref(&bare)).unwrap();
    d.dict_set(bare.clone(), stored.clone()).unwrap();
    assert!(d.sequence().unwrap().last().unwrap().is_identical(&bare));
    let copy = containers::call_method(&d, "copy", &[]).unwrap();
    d.dict_set(Value::from("new"), Value::None).unwrap();
    assert!(copy.dict_get(&Value::from("new")).is_err());
}
#[derive(Debug)]
struct Key {
    calls: Rc<Cell<usize>>,
}
impl PyObject for Key {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "Key"
    }
    fn hash(&self) -> PyResult<i64> {
        Ok(17)
    }
    fn equals(&self, other: &Value) -> PyResult<bool> {
        self.calls.set(self.calls.get() + 1);
        Ok(other.as_str().ok() == Some("probe"))
    }
}
#[test]
fn unsupported_custom_and_float_keys_retain_linear_semantics() {
    let d = Value::dict();
    fill(&d);
    let calls = Rc::new(Cell::new(0));
    let custom = Value::Object(Rc::new(Key {
        calls: calls.clone(),
    }));
    d.dict_set(custom, Value::from(7_i64)).unwrap();
    calls.set(0);
    assert_eq!(
        d.dict_get(&Value::from("probe")).unwrap().as_int().unwrap(),
        7.into()
    );
    assert_eq!(calls.get(), 1);
    d.dict_set(Value::Float(2.0), Value::from("float")).unwrap();
    assert_eq!(
        d.dict_get(&Value::from(2_i64)).unwrap().as_str().unwrap(),
        "float"
    );
    d.dict_set(Value::Float(f64::NAN), Value::from("nan"))
        .unwrap();
    assert!(d.dict_get(&Value::Float(f64::NAN)).is_err());
    assert_eq!(
        d.dict_get(&Value::from("filler-5"))
            .unwrap()
            .as_int()
            .unwrap(),
        5.into()
    );
}
#[test]
fn unsupported_queries_and_deep_tuples_keep_original_fallback_behavior() {
    let d = Value::dict();
    fill(&d);
    let calls = Rc::new(Cell::new(0));
    let query = Value::Object(Rc::new(Key {
        calls: calls.clone(),
    }));
    d.dict_set(Value::from("probe"), Value::from(9_i64))
        .unwrap();
    calls.set(0);
    assert_eq!(d.dict_get(&query).unwrap().as_int().unwrap(), 9.into());
    assert!(calls.get() > 0);
    fn nested(n: usize) -> Value {
        let mut v = Value::from(1_i64);
        for _ in 0..n {
            v = tup(vec![v]);
        }
        v
    }
    let d = Value::dict();
    fill(&d);
    d.dict_set(nested(202), Value::from(1_i64)).unwrap();
    assert_eq!(d.dict_get(&nested(202)).unwrap_err().kind, "RecursionError");
}

#[test]
fn float_queries_fall_back_even_when_existing_dictionary_is_indexed() {
    let d = Value::dict();
    fill(&d);
    d.dict_set(Value::from(1_i64), Value::from("one")).unwrap();
    let nested = tup(vec![Value::from("n"), Value::from(2_i64)]);
    d.dict_set(nested, Value::from("two")).unwrap();
    assert_eq!(
        d.dict_get(&Value::from("filler-1"))
            .unwrap()
            .as_int()
            .unwrap(),
        1.into()
    );
    assert_eq!(
        d.dict_get(&Value::Float(1.0)).unwrap().as_str().unwrap(),
        "one"
    );
    assert_eq!(
        d.dict_get(&tup(vec![Value::from("n"), Value::Float(2.0)]))
            .unwrap()
            .as_str()
            .unwrap(),
        "two"
    );
}

#[test]
fn raw_equal_builtin_keys_keep_the_first_entry() {
    let d = Value::dict();
    fill(&d);
    if let Value::Dict(entries) = &d {
        let mut entries = entries.borrow_mut();
        entries.push((Value::from(1_i64), Value::from("first")));
        entries.push((Value::from(true), Value::from("second")));
    }
    assert_eq!(
        d.dict_get(&Value::from(true)).unwrap().as_str().unwrap(),
        "first"
    );
}
