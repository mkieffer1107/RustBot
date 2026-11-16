use pyrt::{
    PyExc, Value, canon_json,
    comprehension::{self, Expr, Level},
    frame::Frame,
    object::NativeFunction,
    runtime::Runtime,
};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[derive(Debug)]
struct ProbeIterable(Rc<RefCell<Vec<&'static str>>>);
impl pyrt::object::PyObject for ProbeIterable {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn type_name(&self) -> &str {
        "probe"
    }
    fn iter(&self, _: &mut Runtime) -> pyrt::PyResult<Value> {
        self.0.borrow_mut().push("iter");
        Ok(pyrt::object::IteratorObject::from_values(vec![
            Value::from(1i64),
            Value::from(2i64),
        ]))
    }
}
fn parent() -> Rc<Frame> {
    Frame::module(Value::dict())
}
fn assign(name: &'static str) -> comprehension::Assign {
    Rc::new(move |_, frame, value| frame.store(name, value))
}
fn constant(value: Value) -> Expr {
    Rc::new(move |_, _| Ok(value.clone()))
}
fn variable(name: &'static str) -> Expr {
    Rc::new(move |rt, frame| frame.load(rt, name))
}
fn levels(values: Value) -> Vec<Level> {
    vec![Level {
        iterable: constant(values),
        assign: assign("x"),
        filters: vec![],
    }]
}

#[test]
fn first_iterable_is_eager_in_outer_scope_but_targets_and_body_are_lazy() {
    let mut rt = Runtime::default();
    let parent = parent();
    parent.store("x", Value::from(99i64)).unwrap();
    let events = Rc::new(RefCell::new(vec![]));
    let created = events.clone();
    let emitted = events.clone();
    let first: Expr = Rc::new(move |rt, outer| {
        created.borrow_mut().push("first");
        assert_eq!(outer.load(rt, "x")?.as_int()?.to_string(), "99");
        Ok(Value::Object(Rc::new(ProbeIterable(created.clone()))))
    });
    let emit: Expr = Rc::new(move |rt, inner| {
        emitted.borrow_mut().push("emit");
        inner.load(rt, "x")
    });
    let generator = comprehension::generator(
        &mut rt,
        parent.clone(),
        &["x"],
        vec![Level {
            iterable: first,
            assign: assign("x"),
            filters: vec![],
        }],
        emit,
    )
    .unwrap();
    assert_eq!(*events.borrow(), ["first", "iter"]);
    assert_eq!(
        parent
            .load(&mut rt, "x")
            .unwrap()
            .as_int()
            .unwrap()
            .to_string(),
        "99"
    );
    assert_eq!(
        rt.next(&generator)
            .unwrap()
            .unwrap()
            .as_int()
            .unwrap()
            .to_string(),
        "1"
    );
    assert_eq!(*events.borrow(), ["first", "iter", "emit"]);
    assert_eq!(
        parent
            .load(&mut rt, "x")
            .unwrap()
            .as_int()
            .unwrap()
            .to_string(),
        "99"
    );
}

#[test]
fn nested_loops_filters_shortcircuit_and_initialize_children_per_parent() {
    let mut rt = Runtime::default();
    let parents = Rc::new(Cell::new(0));
    let second = parents.clone();
    let skipped = Rc::new(Cell::new(0));
    let count = skipped.clone();
    let outer = Level {
        iterable: constant(Value::list(vec![
            Value::from(1i64),
            Value::from(2i64),
            Value::from(3i64),
        ])),
        assign: assign("x"),
        filters: vec![
            Rc::new(|rt, f| Ok(f.load(rt, "x")?.as_int()?.to_string() != "2")),
            Rc::new(move |_, _| {
                count.set(count.get() + 1);
                Ok(true)
            }),
        ],
    };
    let inner = Level {
        iterable: Rc::new(move |rt, f| {
            second.set(second.get() + 1);
            let x = f.load(rt, "x")?;
            Ok(Value::tuple(vec![x.clone(), x]))
        }),
        assign: assign("y"),
        filters: vec![],
    };
    let emit: Expr = Rc::new(|rt, f| Ok(Value::tuple(vec![f.load(rt, "x")?, f.load(rt, "y")?])));
    let generator =
        comprehension::generator(&mut rt, parent(), &["x", "y"], vec![outer, inner], emit).unwrap();
    assert_eq!(parents.get(), 0);
    assert_eq!(skipped.get(), 0);
    let values = rt.collect(&generator).unwrap();
    assert_eq!(parents.get(), 2);
    assert_eq!(skipped.get(), 2);
    assert_eq!(
        canon_json(&Value::list(values)).unwrap(),
        json!([{"$tuple":[1,1]},{"$tuple":[1,1]},{"$tuple":[3,3]},{"$tuple":[3,3]}])
    );
}

#[test]
fn partial_consumption_preserves_rng_and_body_errors_close_generator() {
    let mut rt = Runtime::default();
    let emit: Expr = Rc::new(|rt, _| Ok(Value::Float(rt.random.global().borrow_mut().random())));
    let generator = comprehension::generator(
        &mut rt,
        parent(),
        &["x"],
        levels(Value::list(vec![Value::None, Value::None, Value::None])),
        emit,
    )
    .unwrap();
    assert_eq!(rt.random.snapshot()["global"], 0);
    assert!(rt.next(&generator).unwrap().is_some());
    assert_eq!(rt.random.snapshot()["global"], 2);
    drop(generator);
    assert_eq!(rt.random.snapshot()["global"], 2);
    let calls = Rc::new(Cell::new(0));
    let counter = calls.clone();
    let bad: Expr = Rc::new(move |_, _| {
        counter.set(counter.get() + 1);
        Err(PyExc::value_error("body failed"))
    });
    let generator = comprehension::generator(
        &mut rt,
        parent(),
        &["x"],
        levels(Value::list(vec![Value::None, Value::None])),
        bad,
    )
    .unwrap();
    assert_eq!(rt.next(&generator).unwrap_err().kind, "ValueError");
    assert!(rt.next(&generator).unwrap().is_none());
    assert_eq!(calls.get(), 1);
}

#[test]
fn assignment_errors_are_lazy_close_and_do_not_evaluate_emitter() {
    let mut rt = Runtime::default();
    let assigned: comprehension::Assign = Rc::new(|rt, f, value| {
        let values = rt.collect(&value)?;
        if values.len() != 2 {
            return Err(PyExc::value_error(
                "not enough values to unpack (expected 2, got 1)",
            ));
        }
        f.store("x", values[0].clone())?;
        f.store("y", values[1].clone())
    });
    let level = Level {
        iterable: constant(Value::list(vec![Value::tuple(vec![Value::None])])),
        assign: assigned,
        filters: vec![],
    };
    let emit: Expr = Rc::new(|_, _| panic!("emitter ran after unpacking error"));
    let generator =
        comprehension::generator(&mut rt, parent(), &["x", "y"], vec![level], emit).unwrap();
    assert_eq!(rt.next(&generator).unwrap_err().kind, "ValueError");
    assert!(rt.next(&generator).unwrap().is_none());
}

#[test]
fn generator_stopiteration_differs_from_eager_comprehension() {
    let mut rt = Runtime::default();
    let emit: Expr = Rc::new(|_, _| Err(PyExc::stop_iteration("body stop")));
    let source = Value::list(vec![Value::None]);
    let generator = comprehension::generator(
        &mut rt,
        parent(),
        &["x"],
        levels(source.clone()),
        emit.clone(),
    )
    .unwrap();
    let error = rt.next(&generator).unwrap_err();
    assert_eq!(error.kind, "RuntimeError");
    assert_eq!(error.message, "generator raised StopIteration");
    assert!(rt.next(&generator).unwrap().is_none());
    assert_eq!(
        comprehension::collect(&mut rt, parent(), &["x"], levels(source), emit)
            .unwrap_err()
            .kind,
        "StopIteration"
    );
}

#[test]
fn captured_cells_remain_live_and_explicit_walrus_can_store_outer() {
    let mut rt = Runtime::default();
    let parent = parent();
    parent.store("outer", Value::from(1i64)).unwrap();
    let walrus_parent = parent.clone();
    let emit: Expr = Rc::new(move |rt, frame| {
        let value = frame.load(rt, "x")?;
        walrus_parent.store("last", value.clone())?;
        let cell = frame.clone();
        Ok(NativeFunction::value("closure", move |rt, _| {
            cell.load(rt, "x")
        }))
    });
    let closures = comprehension::collect(
        &mut rt,
        parent.clone(),
        &["x"],
        levels(Value::list(vec![Value::from(1i64), Value::from(2i64)])),
        emit,
    )
    .unwrap();
    assert_eq!(
        parent
            .load(&mut rt, "last")
            .unwrap()
            .as_int()
            .unwrap()
            .to_string(),
        "2"
    );
    assert_eq!(parent.load(&mut rt, "x").unwrap_err().kind, "NameError");
    for closure in closures {
        assert_eq!(
            rt.call(&closure, pyrt::object::CallArgs::default())
                .unwrap()
                .as_int()
                .unwrap()
                .to_string(),
            "2"
        );
    }
    let generator = comprehension::generator(
        &mut rt,
        parent.clone(),
        &["x"],
        levels(Value::list(vec![Value::None])),
        variable("outer"),
    )
    .unwrap();
    parent.store("outer", Value::from(9i64)).unwrap();
    assert_eq!(
        rt.next(&generator)
            .unwrap()
            .unwrap()
            .as_int()
            .unwrap()
            .to_string(),
        "9"
    );
}

#[test]
fn set_and_dict_comprehensions_insert_before_later_elements() {
    let mut rt = Runtime::default();
    let calls = Rc::new(Cell::new(0));
    let shared = calls.clone();
    let emit: Expr = Rc::new(move |_, _| {
        shared.set(shared.get() + 1);
        Ok(Value::list(vec![]))
    });
    assert_eq!(
        comprehension::collect_set(
            &mut rt,
            parent(),
            &["x"],
            levels(Value::list(vec![Value::None, Value::None])),
            emit
        )
        .unwrap_err()
        .kind,
        "TypeError"
    );
    assert_eq!(calls.get(), 1);
    let calls = Rc::new(Cell::new(0));
    let shared = calls.clone();
    let emit: Expr = Rc::new(move |_, _| {
        shared.set(shared.get() + 1);
        Ok(Value::tuple(vec![Value::list(vec![]), Value::None]))
    });
    assert_eq!(
        comprehension::collect_dict(
            &mut rt,
            parent(),
            &["x"],
            levels(Value::list(vec![Value::None, Value::None])),
            emit
        )
        .unwrap_err()
        .kind,
        "TypeError"
    );
    assert_eq!(calls.get(), 1);
    let values = Value::list(vec![Value::from(1i64), Value::Bool(true)]);
    let emit: Expr = Rc::new(|rt, f| {
        let x = f.load(rt, "x")?;
        Ok(Value::tuple(vec![x.clone(), x]))
    });
    let dict =
        comprehension::collect_dict(&mut rt, parent(), &["x"], levels(values), emit).unwrap();
    assert_eq!(canon_json(&dict).unwrap(), json!({"$dict":[[1,true]]}));
}
