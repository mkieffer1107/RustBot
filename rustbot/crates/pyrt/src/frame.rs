//! Lexical scopes preserve Python closure cells and global/nonlocal assignments.
use crate::{PyExc, PyResult, Value, runtime::Runtime};
use std::collections::HashSet;
use std::rc::Rc;

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
        Rc::new(Self {
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
        Rc::new(Self {
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
                .or_else(|_| runtime.builtin(name));
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
            .or_else(|_| runtime.builtin(name))
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
