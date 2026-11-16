use pyrt::{PyResult, Value, runtime::Runtime};
#[test]
fn package_initialization_importing_requested_child_initializes_it_only_once() {
    fn package(rt: &mut Runtime, module: &Value) -> PyResult<()> {
        let child = rt.import("reentry_fixture.child")?;
        rt.setattr(module, "first_child", child)
    }
    fn child(rt: &mut Runtime, module: &Value) -> PyResult<()> {
        let Value::List(calls) = rt.builtin("__child_calls")? else {
            panic!()
        };
        calls.borrow_mut().push(Value::None);
        rt.random.new_unseeded();
        rt.setattr(module, "serial", Value::from(calls.borrow().len()))
    }
    let mut rt = Runtime::default();
    let calls = Value::list(vec![]);
    rt.register_builtin("__child_calls", calls.clone());
    rt.register_module("reentry_fixture", package);
    rt.register_module("reentry_fixture.child", child);
    let child = rt.import("reentry_fixture.child").unwrap();
    let package = rt.import("reentry_fixture").unwrap();
    assert!(child.is_identical(&rt.getattr(&package, "first_child").unwrap()));
    assert_eq!(calls.sequence().unwrap().len(), 1);
    assert_eq!(
        rt.random.snapshot()["instances"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        rt.getattr(&child, "serial")
            .unwrap()
            .as_int()
            .unwrap()
            .to_string(),
        "1"
    );
}
