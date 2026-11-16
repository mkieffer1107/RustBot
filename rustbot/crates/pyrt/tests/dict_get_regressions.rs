use pyrt::{PyExc, PyResult, Value, containers, object::PyObject};
use std::{any::Any, cell::Cell, rc::Rc};

#[derive(Debug)]
struct StatefulKey {
    comparisons: Rc<Cell<usize>>,
    raises: bool,
}
impl PyObject for StatefulKey {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "StatefulKey"
    }
    fn hash(&self) -> PyResult<i64> {
        Ok(7)
    }
    fn equals(&self, _: &Value) -> PyResult<bool> {
        self.comparisons.set(self.comparisons.get() + 1);
        if self.raises {
            return Err(PyExc::key_error("from equality"));
        }
        Ok(self.comparisons.get() == 1)
    }
}
fn key(counter: &Rc<Cell<usize>>, raises: bool) -> Value {
    Value::Object(Rc::new(StatefulKey {
        comparisons: Rc::clone(counter),
        raises,
    }))
}
#[test]
fn get_compares_once_and_returns_the_stored_object() {
    let comparisons = Rc::new(Cell::new(0));
    let dict = Value::dict();
    let stored = Value::list(vec![Value::from(42_i64)]);
    dict.dict_set(key(&comparisons, false), stored.clone())
        .unwrap();
    let result =
        containers::call_method(&dict, "get", &[key(&comparisons, false), Value::None]).unwrap();
    assert!(result.is_identical(&stored));
    assert_eq!(comparisons.get(), 1);
}
#[test]
fn get_does_not_swallow_keyerror_from_equality() {
    let comparisons = Rc::new(Cell::new(0));
    let dict = Value::dict();
    dict.dict_set(key(&comparisons, true), Value::None).unwrap();
    let error = containers::call_method(&dict, "get", &[key(&comparisons, true)]).unwrap_err();
    assert_eq!(error.kind, "KeyError");
    assert_eq!(error.message, "from equality");
    assert_eq!(comparisons.get(), 1);
}
#[test]
fn get_preserves_defaults_hashability_and_key_identity() {
    let dict = Value::dict();
    let fallback = Value::list(vec![]);
    let result =
        containers::call_method(&dict, "get", &[Value::from("missing"), fallback.clone()]).unwrap();
    assert!(result.is_identical(&fallback));
    assert!(matches!(
        containers::call_method(&dict, "get", &[Value::from("missing")]).unwrap(),
        Value::None
    ));
    assert_eq!(
        containers::call_method(&dict, "get", &[Value::list(vec![])])
            .unwrap_err()
            .kind,
        "TypeError"
    );
    let comparisons = Rc::new(Cell::new(0));
    let original = key(&comparisons, true);
    dict.dict_set(original.clone(), fallback.clone()).unwrap();
    let found = containers::call_method(&dict, "get", &[original]).unwrap();
    assert!(found.is_identical(&fallback));
    assert_eq!(comparisons.get(), 0);
}
