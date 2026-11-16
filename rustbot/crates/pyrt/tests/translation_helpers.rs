use pyrt::{PyExc, Value, canon_json, containers, object::IteratorObject, ops, runtime::Runtime};
use serde_json::json;
use std::{cell::Cell, rc::Rc};

#[test]
fn fixed_unpack_probes_only_one_extra_item_and_preserves_later_error() {
    let mut rt = Runtime::default();
    let calls = Rc::new(Cell::new(0usize));
    let count = calls.clone();
    let iterator = IteratorObject::value(move |_| {
        let n = count.get();
        count.set(n + 1);
        if n < 3 {
            Ok(Some(Value::from(n)))
        } else {
            Err(PyExc::value_error("later failure"))
        }
    });
    let error = containers::unpack(&mut rt, &iterator, 2, 0, false).unwrap_err();
    assert_eq!(error.message, "too many values to unpack (expected 2)");
    assert_eq!(calls.get(), 3);
    assert_eq!(rt.next(&iterator).unwrap_err().message, "later failure");
    assert_eq!(
        containers::unpack(&mut rt, &Value::list(vec![Value::None; 4]), 2, 0, false)
            .unwrap_err()
            .message,
        "too many values to unpack (expected 2, got 4)"
    );
    assert_eq!(
        containers::unpack(&mut rt, &Value::from(1i64), 2, 0, false)
            .unwrap_err()
            .message,
        "cannot unpack non-iterable int object"
    );
}

#[test]
fn unpack_starred_fixed_ends_and_too_few_match_python() {
    let mut rt = Runtime::default();
    let list = Value::list((0i64..6).map(Value::from).collect());
    let values = containers::unpack(&mut rt, &list, 1, 2, true).unwrap();
    assert_eq!(
        canon_json(&Value::list(values)).unwrap(),
        json!([0, [1, 2, 3], 4, 5])
    );
    assert_eq!(
        containers::unpack(&mut rt, &Value::tuple(vec![Value::None]), 1, 1, true)
            .unwrap_err()
            .message,
        "not enough values to unpack (expected at least 2, got 1)"
    );
    assert_eq!(
        containers::unpack(&mut rt, &Value::tuple(vec![]), 1, 1, true)
            .unwrap_err()
            .message,
        "not enough values to unpack (expected at least 2, got 0)"
    );
    let values = containers::unpack(
        &mut rt,
        &Value::tuple(vec![Value::Bool(true), Value::Bool(false)]),
        1,
        1,
        true,
    )
    .unwrap();
    assert_eq!(
        canon_json(&Value::list(values)).unwrap(),
        json!([true, [], false])
    );
    assert_eq!(
        containers::unpack(&mut rt, &Value::list(vec![Value::None]), 2, 0, false)
            .unwrap_err()
            .message,
        "not enough values to unpack (expected 2, got 1)"
    );
}

#[test]
fn list_inplace_add_preserves_alias_and_commits_rhs_values_before_error() {
    let mut rt = Runtime::default();
    let list = Value::list(vec![Value::from(0i64)]);
    let alias = list.clone();
    let mut n = 0;
    let rhs = IteratorObject::value(move |_| {
        n += 1;
        if n == 1 {
            Ok(Some(Value::from(1i64)))
        } else {
            Err(PyExc::value_error("rhs failed"))
        }
    });
    assert_eq!(
        ops::inplace(&mut rt, "+=", &list, &rhs)
            .unwrap_err()
            .message,
        "rhs failed"
    );
    assert_eq!(canon_json(&alias).unwrap(), json!([0, 1]));
    assert!(
        ops::inplace(&mut rt, "+", &list, &list)
            .unwrap()
            .is_identical(&alias)
    );
    assert_eq!(canon_json(&alias).unwrap(), json!([0, 1, 0, 1]));
    assert!(
        ops::inplace(&mut rt, "+", &list, &Value::from("é😀"))
            .unwrap()
            .is_identical(&alias)
    );
    assert_eq!(canon_json(&alias).unwrap(), json!([0, 1, 0, 1, "é", "😀"]));
}

#[test]
fn list_inplace_repetition_keeps_outer_and_inner_aliases() {
    let mut rt = Runtime::default();
    let child = Value::list(vec![]);
    let list = Value::list(vec![child.clone()]);
    let alias = list.clone();
    let repeated = ops::inplace(&mut rt, "*=", &list, &Value::from(3i64)).unwrap();
    assert!(repeated.is_identical(&alias));
    for value in repeated.sequence().unwrap() {
        assert!(value.is_identical(&child));
    }
    ops::inplace(&mut rt, "*", &list, &Value::from(-2i64)).unwrap();
    assert!(alias.sequence().unwrap().is_empty());
}

#[test]
fn set_inplace_operations_preserve_alias_and_require_set_rhs() {
    let mut rt = Runtime::default();
    let set =
        Value::Set(pyrt::sets::PySet::from_values([Value::from(1i64), Value::from(2i64)]).unwrap());
    let alias = set.clone();
    let other = Value::FrozenSet(
        pyrt::sets::PySet::from_values([Value::from(2i64), Value::from(3i64)]).unwrap(),
    );
    assert!(
        ops::inplace(&mut rt, "|=", &set, &other)
            .unwrap()
            .is_identical(&alias)
    );
    assert_eq!(set.sequence().unwrap().len(), 3);
    assert!(
        ops::inplace(&mut rt, "&", &set, &other)
            .unwrap()
            .is_identical(&alias)
    );
    assert_eq!(set.sequence().unwrap().len(), 2);
    assert_eq!(
        ops::inplace(&mut rt, "|", &set, &Value::list(vec![Value::None]))
            .unwrap_err()
            .kind,
        "TypeError"
    );
    assert!(
        ops::inplace(&mut rt, "^", &set, &set)
            .unwrap()
            .is_identical(&alias)
    );
    assert!(alias.sequence().unwrap().is_empty());
}

#[test]
fn immutable_augassign_numeric_floor_and_string_percent_use_python_operators() {
    let mut rt = Runtime::default();
    assert_eq!(
        canon_json(&ops::inplace(&mut rt, "//=", &Value::from(-7i64), &Value::from(3i64)).unwrap())
            .unwrap(),
        json!(-3)
    );
    assert_eq!(
        canon_json(&ops::inplace(&mut rt, "%=", &Value::from(-7i64), &Value::from(3i64)).unwrap())
            .unwrap(),
        json!(2)
    );
    assert_eq!(
        ops::inplace(
            &mut rt,
            "%=",
            &Value::from("%s:%04d"),
            &Value::tuple(vec![Value::from("é"), Value::from(7i64)])
        )
        .unwrap()
        .as_str()
        .unwrap(),
        "é:0007"
    );
    let tuple = Value::tuple(vec![Value::None]);
    let result = ops::inplace(&mut rt, "+", &tuple, &tuple).unwrap();
    assert!(!result.is_identical(&tuple));
    assert_eq!(result.sequence().unwrap().len(), 2);
}

#[test]
fn nested_assignment_callback_keeps_earlier_stores_when_inner_unpack_fails() {
    use pyrt::{
        comprehension::{self, Level},
        frame::Frame,
    };
    use std::cell::RefCell;
    let mut rt = Runtime::default();
    let observed = Rc::new(RefCell::new(None));
    let captured = observed.clone();
    let level = Level {
        iterable: Rc::new(|_, _| {
            Ok(Value::list(vec![Value::tuple(vec![
                Value::from(1i64),
                Value::list(vec![Value::from(2i64)]),
            ])]))
        }),
        assign: Rc::new(move |rt, frame, value| {
            *captured.borrow_mut() = Some(frame.clone());
            let outer = containers::unpack(rt, &value, 2, 0, false)?;
            frame.store("a", outer[0].clone())?;
            let inner = containers::unpack(rt, &outer[1], 2, 0, false)?;
            frame.store("b", inner[0].clone())?;
            frame.store("c", inner[1].clone())
        }),
        filters: vec![],
    };
    let generator = comprehension::generator(
        &mut rt,
        Frame::module(Value::dict()),
        &["a", "b", "c"],
        vec![level],
        Rc::new(|_, _| panic!("failed assignment must not emit")),
    )
    .unwrap();
    assert_eq!(
        rt.next(&generator).unwrap_err().message,
        "not enough values to unpack (expected 2, got 1)"
    );
    let frame = observed.borrow().clone().unwrap();
    assert_eq!(
        frame
            .load(&mut rt, "a")
            .unwrap()
            .as_int()
            .unwrap()
            .to_string(),
        "1"
    );
    assert_eq!(
        frame.load(&mut rt, "b").unwrap_err().kind,
        "UnboundLocalError"
    );
    assert!(rt.next(&generator).unwrap().is_none());
}
