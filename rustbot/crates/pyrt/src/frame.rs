//! Lexical scopes preserve Python closure cells and global/nonlocal assignments.
use crate::{PyExc, PyResult, Value, runtime::Runtime};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::{Rc, Weak};

// Weak registration records scopes without extending their lifetime. Dead weak
// headers are pruned periodically rather than accumulating across call records.
type Namespace = RefCell<Vec<(Value, Value)>>;
struct Registration {
    frame: Weak<Frame>,
    globals: Weak<Namespace>,
    locals: Weak<Namespace>,
}
impl Registration {
    fn live(&self) -> bool {
        self.frame.strong_count() != 0 || self.locals.strong_count() != 0
    }
}
#[derive(Default)]
struct Registry {
    frames: Vec<Registration>,
    registrations: usize,
}
impl Registry {
    fn prune(&mut self) {
        self.frames.retain(Registration::live);
        self.shrink();
    }
    fn shrink(&mut self) {
        if self.frames.capacity() > 1024 && self.frames.len() < self.frames.capacity() / 4 {
            self.frames.shrink_to(self.frames.len().max(256));
        }
    }
}
thread_local! { static FRAMES: RefCell<Registry> = RefCell::new(Registry::default()); }

fn registered(frame: Frame) -> Rc<Frame> {
    let frame = Rc::new(frame);
    FRAMES.with(|registry| {
        let mut registry = registry.borrow_mut();
        registry.registrations = registry.registrations.wrapping_add(1);
        if registry.registrations % 256 == 0 {
            registry.prune();
        }
        let namespace = |value: &Value| {
            if let Value::Dict(dict) = value {
                Rc::downgrade(dict)
            } else {
                Weak::new()
            }
        };
        registry.frames.push(Registration {
            frame: Rc::downgrade(&frame),
            globals: namespace(&frame.globals),
            locals: namespace(&frame.locals),
        });
    });
    frame
}

/// Break closure/namespace cycles only for a context whose final strong owner
/// is being dropped. RuntimeState gathers module dictionary pointer identities.
/// This is context teardown, not a Python tracing garbage collector.
pub(crate) fn release_namespaces(ids: &HashSet<usize>) {
    let (_frames, namespaces) = FRAMES
        .try_with(|registry| {
            let mut registry = registry.borrow_mut();
            let mut selected = Vec::new();
            let mut namespaces = Vec::new();
            let mut seen = HashSet::new();
            registry.frames.retain(|entry| {
                if ids.contains(&(entry.globals.as_ptr() as usize)) {
                    if let Some(frame) = entry.frame.upgrade() {
                        selected.push(frame);
                    }
                    if let Some(locals) = entry.locals.upgrade()
                        && seen.insert(Rc::as_ptr(&locals) as usize)
                    {
                        namespaces.push(locals);
                    }
                    false
                } else {
                    entry.live()
                }
            });
            registry.shrink();
            (selected, namespaces)
        })
        .unwrap_or_default();
    // Keep every selected frame alive until all namespaces have been emptied.
    // Weak namespace metadata also catches locals()/vars() mapping-only cycles
    // after the Frame itself has died; it never keeps namespace data alive.
    for namespace in namespaces {
        let values = std::mem::take(&mut *namespace.borrow_mut());
        // Release the RefCell borrow before Rust values are dropped. No Python
        // callbacks/destructors execute as part of this cleanup operation.
        drop(values);
    }
}

#[derive(Debug)]
pub struct Frame {
    pub locals: Value,
    pub globals: Value,
    parent: Option<Rc<Frame>>,
    declared: HashSet<String>,
    global_names: HashSet<String>,
    nonlocal_names: HashSet<String>,
    module: bool,
}

impl Frame {
    pub fn module(globals: Value) -> Rc<Self> {
        registered(Self {
            locals: globals.clone(),
            globals,
            parent: None,
            declared: HashSet::new(),
            global_names: HashSet::new(),
            nonlocal_names: HashSet::new(),
            module: true,
        })
    }
    pub fn function(
        parent: Rc<Self>,
        declared: &[&str],
        globals: &[&str],
        nonlocals: &[&str],
        bound: Value,
    ) -> Rc<Self> {
        registered(Self {
            globals: parent.globals.clone(),
            locals: bound,
            parent: Some(parent),
            declared: declared.iter().map(|n| (*n).into()).collect(),
            global_names: globals.iter().map(|n| (*n).into()).collect(),
            nonlocal_names: nonlocals.iter().map(|n| (*n).into()).collect(),
            module: false,
        })
    }
    pub fn load(&self, runtime: &mut Runtime, name: &str) -> PyResult<Value> {
        let key = Value::from(name);
        if self.global_names.contains(name) {
            return self
                .globals
                .dict_get(&key)
                .or_else(|_| self.frame_builtin(runtime, name));
        }
        if self.nonlocal_names.contains(name) {
            return self
                .parent
                .as_ref()
                .ok_or_else(|| PyExc::name_error("no enclosing scope"))?
                .load_cell(name)?
                .ok_or_else(|| {
                    PyExc::name_error(format!("no binding for nonlocal '{name}' found"))
                });
        }
        if let Ok(value) = self.locals.dict_get(&key) {
            return Ok(value);
        }
        if self.declared.contains(name) {
            return Err(PyExc::unbound_local_error(format!(
                "cannot access local variable '{name}' where it is not associated with a value"
            )));
        }
        if let Some(parent) = &self.parent
            && let Some(value) = parent.load_cell(name)?
        {
            return Ok(value);
        }
        self.globals
            .dict_get(&key)
            .or_else(|_| self.frame_builtin(runtime, name))
    }
    fn frame_builtin(&self, runtime: &mut Runtime, name: &str) -> PyResult<Value> {
        if matches!(name, "globals" | "locals" | "vars") {
            let mapping = if name == "globals" {
                self.globals.clone()
            } else {
                self.locals.clone()
            };
            let name = name.to_owned();
            return Ok(crate::object::NativeFunction::value(
                name.clone(),
                move |rt, args| {
                    if name == "vars" && !args.positional.is_empty() {
                        return crate::builtins::call(rt, "vars", args);
                    }
                    args.reject_keywords()?;
                    args.arity(0, 0)?;
                    Ok(mapping.clone())
                },
            ));
        }
        runtime.builtin(name)
    }
    fn load_cell(&self, name: &str) -> PyResult<Option<Value>> {
        if self.global_names.contains(name) {
            return Ok(self.globals.dict_get(&Value::from(name)).ok());
        }
        if !self.module {
            if let Ok(value) = self.locals.dict_get(&Value::from(name)) {
                return Ok(Some(value));
            }
            if self.declared.contains(name) {
                return Err(PyExc::name_error(format!(
                    "cannot access free variable '{name}' where it is not associated with a value in enclosing scope"
                )));
            }
        }
        self.parent.as_ref().map_or(Ok(None), |p| p.load_cell(name))
    }
    pub fn store(&self, name: &str, value: Value) -> PyResult<()> {
        if self.global_names.contains(name) {
            return self.globals.dict_set(Value::from(name), value);
        }
        if self.nonlocal_names.contains(name) {
            return self
                .parent
                .as_ref()
                .ok_or_else(|| PyExc::name_error("no enclosing scope"))?
                .store_cell(name, value);
        }
        self.locals.dict_set(Value::from(name), value)
    }
    fn store_cell(&self, name: &str, value: Value) -> PyResult<()> {
        if self.global_names.contains(name) {
            return Err(PyExc::name_error(format!(
                "no binding for nonlocal '{name}' found"
            )));
        }
        if !self.module
            && (self.declared.contains(name) || self.locals.dict_get(&Value::from(name)).is_ok())
        {
            return self.locals.dict_set(Value::from(name), value);
        }
        self.parent
            .as_ref()
            .ok_or_else(|| PyExc::name_error(format!("no binding for nonlocal '{name}' found")))?
            .store_cell(name, value)
    }
    pub fn delete(&self, name: &str) -> PyResult<()> {
        if self.nonlocal_names.contains(name) {
            return self
                .parent
                .as_ref()
                .ok_or_else(|| PyExc::name_error("no enclosing scope"))?
                .delete_cell(name);
        }
        let mapping = if self.global_names.contains(name) {
            &self.globals
        } else {
            &self.locals
        };
        remove_name(mapping, name).map_err(|_| {
            if !self.module && self.declared.contains(name) {
                PyExc::unbound_local_error(format!("local variable '{name}' is not bound"))
            } else {
                PyExc::name_error(format!("name '{name}' is not defined"))
            }
        })
    }
    fn delete_cell(&self, name: &str) -> PyResult<()> {
        if !self.module && self.declared.contains(name) {
            return remove_name(&self.locals, name)
                .map_err(|_| PyExc::name_error(format!("free variable '{name}' is not bound")));
        }
        self.parent
            .as_ref()
            .ok_or_else(|| PyExc::name_error("no nonlocal binding"))?
            .delete_cell(name)
    }
}

fn remove_name(mapping: &Value, name: &str) -> PyResult<()> {
    let Value::Dict(items) = mapping else {
        return Err(PyExc::type_error("scope namespace must be a dict"));
    };
    let mut items = items.borrow_mut();
    let index = items
        .iter()
        .position(|(k, _)| k.as_str().is_ok_and(|s| s == name))
        .ok_or_else(|| PyExc::key_error(name))?;
    items.remove(index);
    Ok(())
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    #[test]
    fn release_only_breaks_cycles_for_selected_global_namespaces() {
        fn cyclic() -> (Value, Weak<Frame>) {
            let globals = Value::dict();
            let frame = Frame::module(globals.clone());
            let captured = frame.clone();
            frame
                .store(
                    "cycle",
                    crate::object::NativeFunction::value("cycle", move |_, _| {
                        Ok(captured.locals.clone())
                    }),
                )
                .unwrap();
            (globals, Rc::downgrade(&frame))
        }
        fn id(value: &Value) -> usize {
            let Value::Dict(dict) = value else {
                unreachable!()
            };
            Rc::as_ptr(dict) as usize
        }
        let (first, first_frame) = cyclic();
        let (second, second_frame) = cyclic();
        release_namespaces(&HashSet::from([id(&first)]));
        assert!(first_frame.upgrade().is_none());
        assert!(first.sequence().unwrap().is_empty());
        assert!(second_frame.upgrade().is_some());
        release_namespaces(&HashSet::from([id(&second)]));
        assert!(second_frame.upgrade().is_none());
    }
    #[test]
    fn mapping_only_cycle_survives_pruning_until_its_owner_namespace_is_released() {
        let globals = Value::dict();
        let Value::Dict(dictionary) = &globals else {
            unreachable!()
        };
        let id = Rc::as_ptr(dictionary) as usize;
        let frame = Frame::function(
            Frame::module(globals.clone()),
            &["locals_callback"],
            &[],
            &[],
            Value::dict(),
        );
        let locals = if let Value::Dict(locals) = &frame.locals {
            Rc::downgrade(locals)
        } else {
            unreachable!()
        };
        let frame_weak = Rc::downgrade(&frame);
        let mut runtime = Runtime::default();
        frame
            .store(
                "locals_callback",
                frame.load(&mut runtime, "locals").unwrap(),
            )
            .unwrap();
        drop(frame);
        drop(globals);
        assert!(frame_weak.upgrade().is_none());
        for _ in 0..512 {
            drop(Frame::module(Value::dict()));
        }
        assert!(locals.upgrade().is_some());
        release_namespaces(&HashSet::from([id]));
        assert!(locals.upgrade().is_none());
    }
    #[test]
    fn ordinary_frames_are_weakly_registered_and_dead_headers_are_pruned() {
        let frame = Frame::module(Value::dict());
        let weak = Rc::downgrade(&frame);
        drop(frame);
        assert!(weak.upgrade().is_none());
        for _ in 0..2048 {
            drop(Frame::module(Value::dict()));
        }
        FRAMES.with(|registry| {
            let registry = registry.borrow();
            assert!(registry.frames.len() <= 256);
            assert!(
                registry
                    .frames
                    .iter()
                    .all(|entry| entry.frame.strong_count() == 0)
            );
        });
    }
}
