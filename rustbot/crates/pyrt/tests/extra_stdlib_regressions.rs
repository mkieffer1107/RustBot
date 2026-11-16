use pyrt::{
    PyExc, PyResult, Value, canon_json,
    object::{CallArgs, IteratorObject, NativeFunction},
    runtime::Runtime,
};
use serde_json::json;
use std::{cell::Cell, rc::Rc};
fn call(
    rt: &mut Runtime,
    function: &Value,
    positional: Vec<Value>,
    keywords: Vec<(&str, Value)>,
) -> PyResult<Value> {
    rt.call(
        function,
        CallArgs {
            positional,
            keywords: keywords.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        },
    )
}
fn exported(rt: &mut Runtime, module: &str, name: &str) -> Value {
    let m = rt.import(module).unwrap();
    rt.getattr(&m, name).unwrap()
}
fn decorator(rt: &mut Runtime, maxsize: Value, typed: bool) -> Value {
    let lru = exported(rt, "functools", "lru_cache");
    call(
        rt,
        &lru,
        vec![],
        vec![("maxsize", maxsize), ("typed", Value::Bool(typed))],
    )
    .unwrap()
}
fn cached(rt: &mut Runtime, maxsize: Value, typed: bool, calls: Rc<Cell<i64>>) -> Value {
    let f = NativeFunction::qualified(
        "f".into(),
        "scope.f".into(),
        "example".into(),
        move |_, _| {
            calls.set(calls.get() + 1);
            Ok(Value::list(vec![Value::from(calls.get())]))
        },
    );
    let dec = decorator(rt, maxsize, typed);
    call(rt, &dec, vec![f], vec![]).unwrap()
}
fn info(rt: &mut Runtime, wrapped: &Value, field: &str) -> Value {
    let method = rt.getattr(wrapped, "cache_info").unwrap();
    let value = rt.call(&method, CallArgs::default()).unwrap();
    rt.getattr(&value, field).unwrap()
}
fn number(value: &Value) -> i64 {
    value.as_int().unwrap().to_string().parse().unwrap()
}

#[test]
fn imports_resolve_relative_names_and_package_discovery_without_loading_children() {
    fn init(_: &mut Runtime, _: &Value) -> PyResult<()> {
        Ok(())
    }
    let mut rt = Runtime::default();
    for name in ["pkg", "pkg.alpha", "pkg.beta", "pkg.beta.child"] {
        rt.register_module(name, init);
    }
    let package = rt.import("pkg").unwrap();
    assert_eq!(
        rt.getattr(&package, "__package__")
            .unwrap()
            .as_str()
            .unwrap(),
        "pkg"
    );
    let path = rt.getattr(&package, "__path__").unwrap();
    let modules = exported(&mut rt, "pkgutil", "iter_modules");
    let iterator = call(
        &mut rt,
        &modules,
        vec![path],
        vec![("prefix", Value::from("pkg."))],
    )
    .unwrap();
    let entries = rt.collect(&iterator).unwrap();
    let names: Vec<String> = entries
        .iter()
        .map(|v| rt.getattr(v, "name").unwrap().as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names, ["pkg.alpha", "pkg.beta"]);
    assert!(matches!(
        rt.getattr(&entries[1], "ispkg").unwrap(),
        Value::Bool(true)
    ));
    let import = exported(&mut rt, "importlib", "import_module");
    let alpha = call(
        &mut rt,
        &import,
        vec![Value::from(".alpha"), Value::from("pkg")],
        vec![],
    )
    .unwrap();
    assert!(alpha.is_identical(&rt.import("pkg.alpha").unwrap()));
    let alpha_again = call(
        &mut rt,
        &import,
        vec![Value::from("..alpha"), Value::from("pkg.beta")],
        vec![],
    )
    .unwrap();
    assert!(alpha.is_identical(&alpha_again));
    assert_eq!(
        call(
            &mut rt,
            &import,
            vec![Value::from("..alpha"), Value::from("pkg")],
            vec![]
        )
        .unwrap_err()
        .kind,
        "ImportError"
    );
    assert_eq!(
        call(&mut rt, &import, vec![Value::from(".alpha")], vec![])
            .unwrap_err()
            .kind,
        "TypeError"
    );
}

#[test]
fn product_consumes_pools_eagerly_but_yields_cartesian_rows_lazily() {
    let mut rt = Runtime::default();
    let product = exported(&mut rt, "itertools", "product");
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let mut values = vec![Value::from(1i64), Value::from(2i64)].into_iter();
    let source = IteratorObject::value(move |_| {
        count.set(count.get() + 1);
        Ok(values.next())
    });
    let iterator = call(&mut rt, &product, vec![source, Value::from("ab")], vec![]).unwrap();
    assert_eq!(calls.get(), 3);
    assert_eq!(
        canon_json(&rt.next(&iterator).unwrap().unwrap()).unwrap(),
        json!({"$tuple":[1,"a"]})
    );
    assert_eq!(
        canon_json(&Value::list(rt.collect(&iterator).unwrap())).unwrap(),
        json!([{"$tuple":[1,"b"]},{"$tuple":[2,"a"]},{"$tuple":[2,"b"]}])
    );
    let calls = Rc::new(Cell::new(0));
    let counter = calls.clone();
    let failing = IteratorObject::value(move |_| {
        counter.set(counter.get() + 1);
        Err(PyExc::value_error("must not iterate"))
    });
    let iterator = call(
        &mut rt,
        &product,
        vec![failing],
        vec![("repeat", Value::from(0i64))],
    )
    .unwrap();
    assert_eq!(
        canon_json(&Value::list(rt.collect(&iterator).unwrap())).unwrap(),
        json!([{"$tuple":[]} ])
    );
    assert_eq!(calls.get(), 0);
    let iterator = call(
        &mut rt,
        &product,
        vec![Value::from("ab")],
        vec![("repeat", Value::from(2i64))],
    )
    .unwrap();
    assert_eq!(rt.collect(&iterator).unwrap().len(), 4);
}

#[test]
fn unbounded_lru_preserves_result_identity_and_metadata() {
    let mut rt = Runtime::default();
    let calls = Rc::new(Cell::new(0));
    let wrapped = cached(&mut rt, Value::None, false, calls.clone());
    let args = vec![Value::from("cue"), Value::from("shape")];
    let first = call(&mut rt, &wrapped, args.clone(), vec![]).unwrap();
    let second = call(&mut rt, &wrapped, args, vec![]).unwrap();
    assert!(first.is_identical(&second));
    assert_eq!(calls.get(), 1);
    assert_eq!(number(&info(&mut rt, &wrapped, "hits")), 1);
    assert_eq!(
        rt.getattr(&wrapped, "__name__").unwrap().as_str().unwrap(),
        "f"
    );
    assert_eq!(
        rt.getattr(&wrapped, "__qualname__")
            .unwrap()
            .as_str()
            .unwrap(),
        "scope.f"
    );
    let original = rt.getattr(&wrapped, "__wrapped__").unwrap();
    assert_eq!(
        rt.getattr(&original, "__module__")
            .unwrap()
            .as_str()
            .unwrap(),
        "example"
    );
}

#[test]
fn lru_key_semantics_preserve_fast_numeric_keys_typed_and_keyword_order() {
    let mut rt = Runtime::default();
    let calls = Rc::new(Cell::new(0));
    let wrapped = cached(&mut rt, Value::None, false, calls.clone());
    let int = call(&mut rt, &wrapped, vec![Value::from(1i64)], vec![]).unwrap();
    let float = call(&mut rt, &wrapped, vec![Value::Float(1.0)], vec![]).unwrap();
    let bool_ = call(&mut rt, &wrapped, vec![Value::Bool(true)], vec![]).unwrap();
    assert!(!int.is_identical(&float));
    assert!(float.is_identical(&bool_));
    assert_eq!(calls.get(), 2);
    let a = call(
        &mut rt,
        &wrapped,
        vec![],
        vec![("a", Value::from(1i64)), ("b", Value::from(2i64))],
    )
    .unwrap();
    let b = call(
        &mut rt,
        &wrapped,
        vec![],
        vec![("b", Value::from(2i64)), ("a", Value::from(1i64))],
    )
    .unwrap();
    assert!(!a.is_identical(&b));
    let typed = cached(&mut rt, Value::None, true, Rc::new(Cell::new(0)));
    let a = call(&mut rt, &typed, vec![Value::from(1i64)], vec![]).unwrap();
    let b = call(&mut rt, &typed, vec![Value::Float(1.0)], vec![]).unwrap();
    assert!(!a.is_identical(&b));
}

#[test]
fn bounded_lru_eviction_clear_and_zero_capacity_follow_python() {
    let mut rt = Runtime::default();
    let calls = Rc::new(Cell::new(0));
    let wrapped = cached(&mut rt, Value::from(2i64), false, calls.clone());
    for s in ["a", "b", "a", "c", "b"] {
        call(&mut rt, &wrapped, vec![Value::from(s)], vec![]).unwrap();
    }
    assert_eq!(calls.get(), 4);
    assert_eq!(number(&info(&mut rt, &wrapped, "hits")), 1);
    assert_eq!(number(&info(&mut rt, &wrapped, "currsize")), 2);
    let clear = rt.getattr(&wrapped, "cache_clear").unwrap();
    rt.call(&clear, CallArgs::default()).unwrap();
    assert_eq!(number(&info(&mut rt, &wrapped, "hits")), 0);
    assert_eq!(number(&info(&mut rt, &wrapped, "misses")), 0);
    assert_eq!(number(&info(&mut rt, &wrapped, "currsize")), 0);
    let zero = cached(&mut rt, Value::from(0i64), false, calls.clone());
    call(&mut rt, &zero, vec![Value::list(vec![])], vec![]).unwrap();
    call(&mut rt, &zero, vec![Value::list(vec![])], vec![]).unwrap();
    assert_eq!(number(&info(&mut rt, &zero, "misses")), 2);
    assert_eq!(number(&info(&mut rt, &zero, "currsize")), 0);
}

#[test]
fn failing_functions_are_not_cached_and_unhashable_args_fail_before_call() {
    let mut rt = Runtime::default();
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let f = NativeFunction::value("bad", move |_, _| {
        count.set(count.get() + 1);
        Err(PyExc::value_error("bad"))
    });
    let dec = decorator(&mut rt, Value::None, false);
    let wrapped = call(&mut rt, &dec, vec![f], vec![]).unwrap();
    for _ in 0..2 {
        assert_eq!(
            call(&mut rt, &wrapped, vec![Value::from("x")], vec![])
                .unwrap_err()
                .kind,
            "ValueError"
        );
    }
    assert_eq!(calls.get(), 2);
    assert_eq!(number(&info(&mut rt, &wrapped, "currsize")), 0);
    assert_eq!(
        call(&mut rt, &wrapped, vec![Value::list(vec![])], vec![])
            .unwrap_err()
            .kind,
        "TypeError"
    );
    assert_eq!(calls.get(), 2);
    let sys = rt.import("sys").unwrap();
    assert_eq!(
        rt.getattr(&sys, "argv").unwrap().sequence().unwrap().len(),
        1
    );
    let future = rt.import("__future__").unwrap();
    assert_eq!(
        rt.getattr(&future, "annotations").unwrap().type_name(),
        "_Feature"
    );
}
