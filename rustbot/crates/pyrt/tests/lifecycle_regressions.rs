use pyrt::{
    PyExc, PyResult, Value,
    frame::Frame,
    object::{CallArgs, NativeFunction, PyObject},
    runtime::Runtime,
};
use std::{
    any::Any,
    cell::Cell,
    rc::{Rc, Weak},
};

#[derive(Debug)]
struct Sentinel(Rc<Cell<usize>>);
impl Drop for Sentinel {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
impl PyObject for Sentinel {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "lifecycle_sentinel"
    }
}
fn empty_init(_: &mut Runtime, _: &Value) -> PyResult<()> {
    Ok(())
}
fn sentinel(rt: &mut Runtime) -> (Weak<Sentinel>, Rc<Cell<usize>>) {
    let count = Rc::new(Cell::new(0));
    let sentinel = Rc::new(Sentinel(count.clone()));
    let weak = Rc::downgrade(&sentinel);
    rt.register_builtin("__lifecycle_sentinel", Value::Object(sentinel));
    (weak, count)
}
fn module_frame(rt: &mut Runtime) -> Rc<Frame> {
    rt.register_module("lifecycle_fixture", empty_init);
    let module = rt.import("lifecycle_fixture").unwrap();
    Frame::module(rt.getattr(&module, "__dict__").unwrap())
}
fn global_cycle(rt: &mut Runtime) -> Weak<Frame> {
    let frame = module_frame(rt);
    frame
        .store("marker", rt.builtin("__lifecycle_sentinel").unwrap())
        .unwrap();
    let captured = frame.clone();
    let callback = NativeFunction::value("retained", move |rt, _| captured.load(rt, "marker"));
    frame.store("retained", callback).unwrap();
    Rc::downgrade(&frame)
}

#[test]
fn module_globals_closure_cycle_releases_rust_objects_at_final_context_drop() {
    let mut rt = Runtime::default();
    let (probe, count) = sentinel(&mut rt);
    let frame = global_cycle(&mut rt);
    assert!(frame.upgrade().is_some());
    drop(rt);
    assert!(frame.upgrade().is_none());
    assert!(probe.upgrade().is_none());
    assert_eq!(count.get(), 1);
}

#[test]
fn nested_decorator_local_cycles_are_broken_even_when_not_in_module_dictionary() {
    let mut rt = Runtime::default();
    let (probe, count) = sentinel(&mut rt);
    let parent = module_frame(&mut rt);
    let nested = Frame::function(parent, &["wrapped", "marker"], &[], &[], Value::dict());
    nested
        .store("marker", rt.builtin("__lifecycle_sentinel").unwrap())
        .unwrap();
    let captured = nested.clone();
    nested
        .store(
            "wrapped",
            NativeFunction::value("wrapped", move |rt, _| captured.load(rt, "marker")),
        )
        .unwrap();
    let weak = Rc::downgrade(&nested);
    drop(nested);
    assert!(weak.upgrade().is_some());
    drop(rt);
    assert!(weak.upgrade().is_none());
    assert!(probe.upgrade().is_none());
    assert_eq!(count.get(), 1);
}

#[test]
fn teardown_is_delayed_until_last_clone_and_weak_handle_cannot_revive_it() {
    let mut rt = Runtime::default();
    let (probe, _) = sentinel(&mut rt);
    let frame = global_cycle(&mut rt);
    let handle = rt.handle();
    let clone = rt.clone();
    drop(rt);
    assert!(probe.upgrade().is_some());
    assert!(handle.upgrade().is_ok());
    drop(clone);
    assert!(frame.upgrade().is_none());
    assert!(probe.upgrade().is_none());
    assert_eq!(handle.upgrade().err().unwrap().kind, "RuntimeError");
}

#[test]
fn dropping_one_context_does_not_clear_another_contexts_namespace() {
    let mut first = Runtime::default();
    let (first_probe, _) = sentinel(&mut first);
    global_cycle(&mut first);
    let mut second = Runtime::default();
    let (second_probe, count) = sentinel(&mut second);
    global_cycle(&mut second);
    drop(first);
    assert!(first_probe.upgrade().is_none());
    assert!(second_probe.upgrade().is_some());
    let module = second.import("lifecycle_fixture").unwrap();
    let callback = second.getattr(&module, "retained").unwrap();
    let value = second.call(&callback, CallArgs::default()).unwrap();
    assert_eq!(value.type_name(), "lifecycle_sentinel");
    drop(value);
    drop(callback);
    drop(module);
    drop(second);
    assert!(second_probe.upgrade().is_none());
    assert_eq!(count.get(), 1);
}

#[test]
fn runtime_clones_share_random_creation_trace_and_exception_state() {
    let mut first = Runtime::default();
    let mut second = first.clone();
    let one = first.random.new_unseeded();
    let two = second.random.new_unseeded();
    let a = one.borrow_mut().random();
    let b = two.borrow_mut().random();
    assert_eq!(a, pyrt::random::PyRandom::from_str("0:0").random());
    assert_eq!(b, pyrt::random::PyRandom::from_str("0:1").random());
    assert_eq!(first.random.snapshot(), second.random.snapshot());
    assert_eq!(
        first.random.snapshot()["instances"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    first.begin_trace();
    second.trace_call("fixture.py", 1, 2, "sample");
    assert_eq!(first.end_trace().len(), 1);
    first
        .with_exception(PyExc::value_error("active"), |_| {
            assert_eq!(second.active_exception().unwrap().message, "active");
            Ok(())
        })
        .unwrap();
    assert!(second.active_exception().is_err());
}

#[test]
fn failed_import_closure_cycles_are_released_when_context_drops() {
    fn fail(rt: &mut Runtime, module: &Value) -> PyResult<()> {
        let frame = Frame::module(rt.getattr(module, "__dict__")?);
        frame.store("marker", rt.builtin("__lifecycle_sentinel")?)?;
        let captured = frame.clone();
        frame.store(
            "cycle",
            NativeFunction::value("cycle", move |rt, _| captured.load(rt, "marker")),
        )?;
        Err(PyExc::value_error("initializer failed"))
    }
    let mut rt = Runtime::default();
    let (probe, count) = sentinel(&mut rt);
    rt.register_module("broken_lifecycle", fail);
    assert_eq!(
        rt.import("broken_lifecycle").unwrap_err().kind,
        "ValueError"
    );
    drop(rt);
    assert!(probe.upgrade().is_none());
    assert_eq!(count.get(), 1);
}

#[test]
fn locals_builtin_mapping_cycles_are_released_after_the_frame_is_gone() {
    let mut rt = Runtime::default();
    let (probe, count) = sentinel(&mut rt);
    let parent = module_frame(&mut rt);
    let local = Frame::function(parent, &["self_locals", "marker"], &[], &[], Value::dict());
    local
        .store("marker", rt.builtin("__lifecycle_sentinel").unwrap())
        .unwrap();
    let weak = Rc::downgrade(&local);
    let locals_function = local.load(&mut rt, "locals").unwrap();
    local.store("self_locals", locals_function).unwrap();
    drop(local);
    assert!(weak.upgrade().is_none());
    assert!(probe.upgrade().is_some());
    drop(rt);
    assert!(probe.upgrade().is_none());
    assert_eq!(count.get(), 1);
}

#[test]
fn failed_import_mapping_cycle_is_released_after_global_dictionary_dies() {
    fn fail(rt: &mut Runtime, module: &Value) -> PyResult<()> {
        let parent = Frame::module(rt.getattr(module, "__dict__")?);
        let frame = Frame::function(parent, &["marker", "self_locals"], &[], &[], Value::dict());
        frame.store("marker", rt.builtin("__lifecycle_sentinel")?)?;
        frame.store("self_locals", frame.load(rt, "locals")?)?;
        Err(PyExc::value_error("failed with local mapping cycle"))
    }
    let mut rt = Runtime::default();
    let (probe, count) = sentinel(&mut rt);
    rt.register_module("failed_mapping_cycle", fail);
    assert_eq!(
        rt.import("failed_mapping_cycle").unwrap_err().kind,
        "ValueError"
    );
    assert!(probe.upgrade().is_some());
    drop(rt);
    assert!(probe.upgrade().is_none());
    assert_eq!(count.get(), 1);
}
