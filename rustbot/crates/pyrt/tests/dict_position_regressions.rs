use pyrt::{PyExc, PyResult, Value, containers, object::PyObject};
use std::{any::Any, cell::Cell, rc::Rc};
#[derive(Debug, Default)]
struct Counts {
    hashes: Cell<usize>,
    comparisons: Cell<usize>,
}
#[derive(Debug)]
struct Key {
    counts: Rc<Counts>,
    equal: bool,
    raises: bool,
}
impl PyObject for Key {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "Key"
    }
    fn hash(&self) -> PyResult<i64> {
        self.counts.hashes.set(self.counts.hashes.get() + 1);
        Ok(7)
    }
    fn equals(&self, _: &Value) -> PyResult<bool> {
        self.counts
            .comparisons
            .set(self.counts.comparisons.get() + 1);
        if self.raises {
            Err(PyExc::key_error("from equality"))
        } else {
            Ok(self.equal)
        }
    }
}
fn key(counts: &Rc<Counts>, equal: bool, raises: bool) -> Value {
    Value::Object(Rc::new(Key {
        counts: Rc::clone(counts),
        equal,
        raises,
    }))
}
fn reset(counts: &Counts) {
    counts.hashes.set(0);
    counts.comparisons.set(0);
}
#[test]
fn methods_hash_and_compare_once_and_propagate_equality_errors() {
    for method in ["get", "setdefault", "pop"] {
        let counts = Rc::new(Counts::default());
        let dict = Value::dict();
        let stored = Value::list(vec![]);
        dict.dict_set(key(&counts, true, false), stored.clone())
            .unwrap();
        reset(&counts);
        let found = containers::call_method(&dict, method, &[key(&counts, true, false)]).unwrap();
        assert!(found.is_identical(&stored), "{method}");
        assert_eq!(counts.hashes.get(), 1, "{method}");
        assert_eq!(counts.comparisons.get(), 1, "{method}");
        let dict = Value::dict();
        dict.dict_set(key(&counts, true, true), Value::None)
            .unwrap();
        reset(&counts);
        let e = containers::call_method(&dict, method, &[key(&counts, true, true)]).unwrap_err();
        assert_eq!(e.kind, "KeyError", "{method}");
        assert_eq!(e.message, "from equality");
        assert_eq!(counts.hashes.get(), 1);
        assert_eq!(counts.comparisons.get(), 1);
    }
}
#[test]
fn missing_setdefault_inserts_with_one_hash_and_comparison() {
    let counts = Rc::new(Counts::default());
    let dict = Value::dict();
    dict.dict_set(key(&counts, false, false), Value::None)
        .unwrap();
    reset(&counts);
    let fallback = Value::list(vec![]);
    let newkey = key(&counts, false, false);
    let result =
        containers::call_method(&dict, "setdefault", &[newkey.clone(), fallback.clone()]).unwrap();
    assert!(result.is_identical(&fallback));
    assert_eq!(counts.hashes.get(), 1);
    assert_eq!(counts.comparisons.get(), 1);
    assert!(dict.dict_get(&newkey).unwrap().is_identical(&fallback));
}
#[test]
fn empty_pop_preserves_python_shortcut_and_nonempty_checks_hashability() {
    let dict = Value::dict();
    let fallback = Value::list(vec![]);
    let unhashable = Value::list(vec![]);
    assert!(
        containers::call_method(&dict, "pop", &[unhashable.clone(), fallback.clone()])
            .unwrap()
            .is_identical(&fallback)
    );
    assert_eq!(
        containers::call_method(&dict, "pop", std::slice::from_ref(&unhashable))
            .unwrap_err()
            .kind,
        "KeyError"
    );
    dict.dict_set(Value::from("a"), Value::None).unwrap();
    assert_eq!(
        containers::call_method(&dict, "pop", &[unhashable, fallback])
            .unwrap_err()
            .kind,
        "TypeError"
    );
}
