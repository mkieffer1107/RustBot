//! Small hand-written CPython semantic regression cases.
use pyrt::{Value, sets::PySet};

#[test]
fn deepcopy_repeated_sets_preserves_shared_reference() {
    let child = Value::Set(PySet::from_values([Value::from(1_i64)]).unwrap());
    let original = Value::list(vec![child.clone(), child.clone()]);
    let copied = original.deepcopy().sequence().unwrap();
    assert!(copied[0].is_identical(&copied[1]));
    assert!(!copied[0].is_identical(&child));
}

#[test]
fn deepcopy_repeated_frozensets_preserves_shared_reference() {
    let child = Value::FrozenSet(PySet::from_values([Value::from(1_i64)]).unwrap());
    let original = Value::list(vec![child.clone(), child.clone()]);
    let copied = original.deepcopy().sequence().unwrap();
    assert!(copied[0].is_identical(&copied[1]));
    assert!(!copied[0].is_identical(&child));
}

#[test]
fn deepcopy_immutable_tuple_returns_original() {
    let original = Value::tuple(vec![Value::from(1_i64), Value::from("a")]);
    assert!(original.is_identical(&original.deepcopy()));
    let empty = Value::tuple(vec![]);
    assert!(empty.is_identical(&empty.deepcopy()));
}

#[test]
fn deepcopy_tuple_cycle_keeps_recursive_reference() {
    let list = Value::list(vec![]);
    let tuple = Value::tuple(vec![list.clone()]);
    if let Value::List(items) = &list {
        items.borrow_mut().push(tuple.clone());
    }
    let copied = tuple.deepcopy();
    let copied_list = copied.sequence().unwrap()[0].clone();
    assert!(copied.is_identical(&copied_list.sequence().unwrap()[0]));
    for v in [list, copied_list] {
        if let Value::List(items) = v {
            items.borrow_mut().clear();
        }
    }
}

#[test]
fn float_repr_uses_python_boundaries_and_exponents() {
    for (value, expected) in [
        (1e-5, "1e-05"),
        (1e-4, "0.0001"),
        (1e15, "1000000000000000.0"),
        (1e16, "1e+16"),
        (1e23, "1e+23"),
        (-0.0, "-0.0"),
        (f64::from_bits(1), "5e-324"),
        (f64::MAX, "1.7976931348623157e+308"),
    ] {
        assert_eq!(pyrt::float_repr(value), expected);
    }
}

#[test]
fn integer_float_dictionary_keys_compare_without_loss() {
    let d = Value::dict();
    d.dict_set(Value::from(9_007_199_254_740_993_i64), Value::from("exact"))
        .unwrap();
    d.dict_set(
        Value::Float(9_007_199_254_740_992.0),
        Value::from("rounded"),
    )
    .unwrap();
    assert_eq!(d.sequence().unwrap().len(), 2);
    assert_eq!(
        d.dict_get(&Value::from(9_007_199_254_740_993_i64))
            .unwrap()
            .as_str()
            .unwrap(),
        "exact"
    );
    assert_eq!(
        d.dict_get(&Value::Float(9_007_199_254_740_992.0))
            .unwrap()
            .as_str()
            .unwrap(),
        "rounded"
    );
}

#[test]
fn float_divmod_handles_infinite_divisors_and_roundoff() {
    for (a, b, q, r) in [
        (1.0, 0.1, 9.0, 0.09999999999999995),
        (-1.0, f64::INFINITY, -1.0, f64::INFINITY),
        (1.0, f64::NEG_INFINITY, -1.0, f64::NEG_INFINITY),
        (1.0, f64::INFINITY, 0.0, 1.0),
    ] {
        let Value::Float(actual_q) =
            pyrt::ops::floor_div(&Value::Float(a), &Value::Float(b)).unwrap()
        else {
            panic!()
        };
        let Value::Float(actual_r) = pyrt::ops::modulo(&Value::Float(a), &Value::Float(b)).unwrap()
        else {
            panic!()
        };
        assert_eq!(actual_q, q);
        assert_eq!(actual_r, r);
    }
}
