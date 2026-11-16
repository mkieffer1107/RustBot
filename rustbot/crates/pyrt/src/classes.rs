//! Native Python classes, function descriptors and instance slots.
//!
//! Construction supports ordinary classes and builtin object bases. Custom
//! metaclasses, __new__, property descriptors and builtin storage layouts are
//! deliberately outside this first class implementation.
use crate::{
    PyExc, PyResult, Value,
    object::{CallArgs, NativeFunction, NativeType, PyObject},
    runtime::{Runtime, RuntimeHandle},
};
use num_traits::ToPrimitive;
use std::{
    any::Any,
    collections::HashSet,
    fmt,
    rc::{Rc, Weak},
};

pub struct ClassObject {
    name: String,
    module: String,
    namespace: Value,
    bases: Vec<Value>,
    ancestors: Vec<Value>,
    slots: HashSet<String>,
    has_dict: bool,
    layout_owner: Option<Weak<ClassObject>>,
    this: Weak<ClassObject>,
}

impl fmt::Debug for ClassObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ClassObject").field(&self.name).finish()
    }
}

fn class(value: &Value) -> Option<&ClassObject> {
    if let Value::Object(object) = value {
        object.as_any().downcast_ref::<ClassObject>()
    } else {
        None
    }
}
fn function(value: &Value) -> bool {
    matches!(value,Value::Object(object) if object.as_any().is::<NativeFunction>())
}
fn attribute_error(kind: &str, name: &str) -> PyExc {
    PyExc::attribute_error(format!("'{kind}' object has no attribute '{name}'"))
}
fn builtin_object(value: &Value) -> bool {
    matches!(value,Value::Object(object) if object.as_any().downcast_ref::<NativeType>().is_some_and(|t|t.name=="object"&&t.module=="builtins"))
}

pub fn create(
    name: &str,
    module: &str,
    mut bases: Vec<Value>,
    namespace: Value,
    rt: &Runtime,
) -> PyResult<Value> {
    let Value::Dict(ns) = &namespace else {
        return Err(PyExc::type_error("class namespace must be a dict"));
    };
    let namespace = Value::Dict(Rc::new(std::cell::RefCell::new(ns.borrow().clone())));
    if bases.is_empty() {
        let mut handle = rt.handle().upgrade()?;
        bases.push(handle.builtin("object")?);
    }
    for (i, base) in bases.iter().enumerate() {
        if class(base).is_none() && !builtin_object(base) {
            return Err(PyExc::type_error(
                "only native classes and object are supported as bases",
            ));
        }
        if bases[..i].iter().any(|b| b.is_identical(base)) {
            return Err(PyExc::type_error("duplicate base class"));
        }
    }
    let layouts: Vec<Rc<ClassObject>> = bases
        .iter()
        .filter_map(class)
        .filter_map(|base| base.layout_owner.as_ref().and_then(Weak::upgrade))
        .collect();
    for (i, a) in layouts.iter().enumerate() {
        if layouts[..i]
            .iter()
            .any(|b| !a.is_subclass(b) && !b.is_subclass(a))
        {
            return Err(PyExc::type_error(
                "multiple bases have instance lay-out conflict",
            ));
        }
    }
    let inherited_layout = layouts
        .iter()
        .find(|candidate| layouts.iter().all(|base| candidate.is_subclass(base)))
        .map(Rc::downgrade);
    let mut sequences = Vec::new();
    for base in &bases {
        let mut sequence = vec![base.clone()];
        if let Some(base) = class(base) {
            sequence.extend(base.ancestors.clone());
        }
        sequences.push(sequence);
    }
    sequences.push(bases.clone());
    let mut ancestors = Vec::new();
    while sequences.iter().any(|s| !s.is_empty()) {
        let head = sequences
            .iter()
            .filter_map(|s| s.first())
            .find(|head| {
                !sequences
                    .iter()
                    .any(|s| s.iter().skip(1).any(|v| v.is_identical(head)))
            })
            .cloned()
            .ok_or_else(|| {
                PyExc::type_error(
                    "Cannot create a consistent method resolution order (MRO) for bases",
                )
            })?;
        for seq in &mut sequences {
            if seq.first().is_some_and(|v| v.is_identical(&head)) {
                seq.remove(0);
            }
        }
        ancestors.push(head);
    }
    if namespace.dict_get(&Value::from("__eq__")).is_ok()
        && namespace.dict_get(&Value::from("__hash__")).is_err()
    {
        namespace.dict_set(Value::from("__hash__"), Value::None)?;
    }
    let mut slots = HashSet::new();
    let mut inherited_dict = false;
    for base in &bases {
        if let Some(base) = class(base) {
            slots.extend(base.slots.iter().cloned());
            inherited_dict |= base.has_dict;
        }
    }
    let declaration = namespace.dict_get(&Value::from("__slots__")).ok();
    let mut has_dict = inherited_dict || declaration.is_none();
    let mut owns_storage = false;
    if let Some(declaration) = declaration {
        let values = if matches!(declaration, Value::Str(_)) {
            vec![declaration]
        } else {
            declaration.sequence()?
        };
        for value in values {
            let slot = value
                .as_str()
                .map_err(|_| PyExc::type_error("__slots__ items must be strings"))?;
            if !crate::strings::call_method(slot, "isidentifier", &[])?.is_truthy() {
                return Err(PyExc::type_error("__slots__ must be identifiers"));
            }
            if slot == "__dict__" {
                if has_dict {
                    return Err(PyExc::type_error(
                        "__dict__ slot disallowed: we already got one",
                    ));
                }
                has_dict = true;
                continue;
            }
            if slot == "__weakref__" {
                continue;
            }
            if namespace.dict_get(&Value::from(slot)).is_ok() {
                return Err(PyExc::value_error(format!(
                    "'{slot}' in __slots__ conflicts with class variable"
                )));
            }
            let slot = if slot.starts_with("__")
                && !slot.ends_with("__")
                && !name.trim_start_matches('_').is_empty()
            {
                format!("_{}{slot}", name.trim_start_matches('_'))
            } else {
                slot.to_owned()
            };
            owns_storage = true;
            slots.insert(slot);
        }
    }
    let name = name.to_owned();
    let module = module.to_owned();
    Ok(Value::Object(Rc::new_cyclic(|this| ClassObject {
        name,
        module,
        namespace,
        bases,
        ancestors,
        slots,
        has_dict,
        layout_owner: if owns_storage {
            Some(this.clone())
        } else {
            inherited_layout
        },
        this: this.clone(),
    })))
}

impl ClassObject {
    fn value(&self) -> PyResult<Value> {
        Ok(Value::Object(self.this.upgrade().ok_or_else(|| PyExc {
            kind: "ReferenceError",
            message: "class no longer exists".into(),
        })?))
    }
    fn lookup(&self, name: &str) -> Option<Value> {
        if let Ok(value) = self.namespace.dict_get(&Value::from(name)) {
            return Some(value);
        }
        for ancestor in &self.ancestors {
            if let Some(ancestor) = class(ancestor)
                && let Ok(value) = ancestor.namespace.dict_get(&Value::from(name))
            {
                return Some(value);
            }
        }
        None
    }
    fn is_subclass(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
            || self
                .ancestors
                .iter()
                .any(|v| class(v).is_some_and(|c| std::ptr::eq(c, other)))
    }
}

impl PyObject for ClassObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "type"
    }
    fn repr(&self) -> PyResult<String> {
        Ok(if self.module == "builtins" {
            format!("<class '{}'>", self.name)
        } else {
            format!("<class '{}.{}'>", self.module, self.name)
        })
    }
    fn is_callable(&self) -> bool {
        true
    }
    fn hash(&self) -> PyResult<i64> {
        Ok(pointer_hash(self as *const Self as usize))
    }
    fn instance_check(&self, value: &Value) -> Option<bool> {
        Some(
            if let Value::Object(o) = value
                && let Some(instance) = o.as_any().downcast_ref::<InstanceObject>()
            {
                class(&instance.class).is_some_and(|c| c.is_subclass(self))
            } else {
                false
            },
        )
    }
    fn call(&self, rt: &mut Runtime, args: CallArgs) -> PyResult<Value> {
        let value = self.value()?;
        let kind = self.name.clone();
        let handle = rt.handle();
        let instance = Rc::new_cyclic(|this| InstanceObject {
            class: value,
            kind,
            fields: Value::dict(),
            dict: Value::dict(),
            this: this.clone(),
            handle,
        });
        let result = Value::Object(instance.clone());
        if let Some(init) = self.lookup("__init__") {
            let init = bind(init, result.clone());
            let returned = rt.call(&init, args)?;
            if !matches!(returned, Value::None) {
                return Err(PyExc::type_error(format!(
                    "__init__() should return None, not '{}'",
                    returned.type_name()
                )));
            }
        } else if !args.positional.is_empty() || !args.keywords.is_empty() {
            return Err(PyExc::type_error(format!(
                "{}() takes no arguments",
                self.name
            )));
        }
        Ok(result)
    }
    fn get_attr(&self, rt: &mut Runtime, name: &str) -> PyResult<Value> {
        match name {
            "__name__" | "__qualname__" => Ok(Value::from(self.name.clone())),
            "__module__" => Ok(Value::from(self.module.clone())),
            "__dict__" => Ok(self.namespace.clone()),
            "__class__" => rt.builtin("type"),
            "__bases__" => Ok(Value::tuple(self.bases.clone())),
            "__mro__" => {
                let mut values = vec![self.value()?];
                values.extend(self.ancestors.clone());
                Ok(Value::tuple(values))
            }
            _ => self.lookup(name).ok_or_else(|| {
                PyExc::attribute_error(format!(
                    "type object '{}' has no attribute '{name}'",
                    self.name
                ))
            }),
        }
    }
    fn set_attr(&self, name: &str, value: Value) -> PyResult<()> {
        self.namespace.dict_set(Value::from(name), value)
    }
}

fn pointer_hash(pointer: usize) -> i64 {
    let h = pointer.rotate_right(4) as i64;
    if h == -1 { -2 } else { h }
}

pub struct InstanceObject {
    class: Value,
    kind: String,
    fields: Value,
    dict: Value,
    this: Weak<InstanceObject>,
    handle: RuntimeHandle,
}
impl fmt::Debug for InstanceObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("InstanceObject").field(&self.kind).finish()
    }
}
impl InstanceObject {
    fn class(&self) -> &ClassObject {
        class(&self.class).expect("instance native class")
    }
    fn value(&self) -> PyResult<Value> {
        Ok(Value::Object(self.this.upgrade().ok_or_else(|| PyExc {
            kind: "ReferenceError",
            message: "instance no longer exists".into(),
        })?))
    }
    fn invoke(&self, name: &str, args: Vec<Value>) -> PyResult<Option<Value>> {
        let Some(method) = self.class().lookup(name) else {
            return Ok(None);
        };
        let method = bind(method, self.value()?);
        let mut rt = self.handle.upgrade()?;
        rt.call(&method, CallArgs::new(args)).map(Some)
    }
    fn text(&self, method: &str) -> PyResult<Option<String>> {
        self.invoke(method, vec![])?
            .map(|v| {
                v.as_str().map(str::to_owned).map_err(|_| {
                    PyExc::type_error(format!(
                        "{method} returned non-string (type {})",
                        v.type_name()
                    ))
                })
            })
            .transpose()
    }
}
impl PyObject for InstanceObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        &self.kind
    }
    fn repr(&self) -> PyResult<String> {
        Ok(self
            .text("__repr__")?
            .unwrap_or_else(|| format!("<{}.{} object at 0x0>", self.class().module, self.kind)))
    }
    fn str(&self) -> PyResult<String> {
        self.text("__str__")?.map_or_else(|| self.repr(), Ok)
    }
    fn format(&self, spec: &str) -> PyResult<String> {
        if let Some(value) = self.invoke("__format__", vec![Value::from(spec)])? {
            return value
                .as_str()
                .map(str::to_owned)
                .map_err(|_| PyExc::type_error("__format__ must return a str, not another type"));
        }
        if spec.is_empty() {
            self.str()
        } else {
            Err(PyExc::type_error(format!(
                "unsupported format string passed to {}.__format__",
                self.kind
            )))
        }
    }
    fn is_truthy(&self) -> bool {
        // The current Value truth hook cannot propagate exceptions. The source
        // sentinel's __bool__ returns an ordinary bool; checked hooks can follow.
        if let Ok(Some(Value::Bool(value))) = self.invoke("__bool__", vec![]) {
            return value;
        }
        if self.class().lookup("__bool__").is_none() && self.class().lookup("__len__").is_some() {
            return self.len() != Ok(0);
        }
        true
    }
    fn len(&self) -> PyResult<usize> {
        let value = self.invoke("__len__", vec![])?.ok_or_else(|| {
            PyExc::type_error(format!("object of type '{}' has no len()", self.kind))
        })?;
        let n = value.as_int()?;
        if n.sign() == num_bigint::Sign::Minus {
            return Err(PyExc::value_error("__len__() should return >= 0"));
        }
        n.to_usize()
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or_else(|| PyExc::overflow_error("cannot fit int into an index-sized integer"))
    }
    fn equals(&self, other: &Value) -> PyResult<bool> {
        self.invoke("__eq__", vec![other.clone()])
            .map(|v| v.is_some_and(|v| v.is_truthy()))
    }
    fn hash(&self) -> PyResult<i64> {
        if let Some(method) = self.class().lookup("__hash__") {
            if matches!(method, Value::None) {
                return Err(PyExc::type_error(format!(
                    "unhashable type: '{}'",
                    self.kind
                )));
            }
            let value = self.invoke("__hash__", vec![])?.expect("hash exists");
            if !matches!(value, Value::Int(_) | Value::Bool(_)) {
                return Err(PyExc::type_error(
                    "__hash__ method should return an integer",
                ));
            }
            crate::sets::hash(&value)
        } else {
            Ok(pointer_hash(self as *const Self as usize))
        }
    }
    fn get_attr(&self, rt: &mut Runtime, name: &str) -> PyResult<Value> {
        if name == "__class__" {
            return Ok(self.class.clone());
        }
        if name == "__dict__" {
            return if self.class().has_dict {
                Ok(self.dict.clone())
            } else {
                Err(attribute_error(&self.kind, name))
            };
        }
        if self.class().slots.contains(name) {
            return self
                .fields
                .dict_get(&Value::from(name))
                .map_err(|_| attribute_error(&self.kind, name));
        }
        if self.class().has_dict
            && let Ok(value) = self.dict.dict_get(&Value::from(name))
        {
            return Ok(value);
        }
        if let Some(value) = self.class().lookup(name) {
            return Ok(bind(value, self.value()?));
        }
        if let Some(method) = self.class().lookup("__getattr__") {
            return rt.call(
                &bind(method, self.value()?),
                CallArgs::new(vec![Value::from(name)]),
            );
        }
        Err(attribute_error(&self.kind, name))
    }
    fn set_attr(&self, name: &str, value: Value) -> PyResult<()> {
        if self.class().slots.contains(name) {
            return self.fields.dict_set(Value::from(name), value);
        }
        if self.class().has_dict {
            return self.dict.dict_set(Value::from(name), value);
        }
        Err(attribute_error(&self.kind, name))
    }
    fn get_item(&self, index: &Value) -> PyResult<Value> {
        self.invoke("__getitem__", vec![index.clone()])?
            .ok_or_else(|| {
                PyExc::type_error(format!("'{}' object is not subscriptable", self.kind))
            })
    }
    fn call(&self, rt: &mut Runtime, args: CallArgs) -> PyResult<Value> {
        let method = self
            .class()
            .lookup("__call__")
            .ok_or_else(|| PyExc::type_error(format!("'{}' object is not callable", self.kind)))?;
        rt.call(&bind(method, self.value()?), args)
    }
    fn is_callable(&self) -> bool {
        self.class().lookup("__call__").is_some()
    }
}

#[derive(Debug)]
pub struct BoundMethod {
    function: Value,
    receiver: Value,
}
fn bind(function_value: Value, receiver: Value) -> Value {
    if function(&function_value) {
        Value::Object(Rc::new(BoundMethod {
            function: function_value,
            receiver,
        }))
    } else {
        function_value
    }
}
impl PyObject for BoundMethod {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "method"
    }
    fn is_callable(&self) -> bool {
        true
    }
    fn call(&self, rt: &mut Runtime, mut args: CallArgs) -> PyResult<Value> {
        args.positional.insert(0, self.receiver.clone());
        rt.call(&self.function, args)
    }
    fn get_attr(&self, rt: &mut Runtime, name: &str) -> PyResult<Value> {
        match name {
            "__self__" => Ok(self.receiver.clone()),
            "__func__" => Ok(self.function.clone()),
            _ => rt.getattr(&self.function, name),
        }
    }
}

/// Copy native instance storage without invoking __init__. The caller supplies
/// the same memo used for surrounding builtin containers, preserving mixed
/// container/instance cycles and aliases to an instance's exposed __dict__.
pub(crate) fn deepcopy_object(
    value: &Value,
    memo: &mut std::collections::HashMap<(u8, usize), Value>,
    mut copy: impl FnMut(&Value, &mut std::collections::HashMap<(u8, usize), Value>) -> Value,
) -> Option<Value> {
    let Value::Object(object) = value else {
        return None;
    };
    let is_instance = object.as_any().is::<InstanceObject>();
    let is_plain = object.as_any().is::<crate::object::PlainObject>();
    if !is_instance && !is_plain {
        return None;
    }
    let id = (4, Rc::as_ptr(object) as *const () as usize);
    if let Some(existing) = memo.get(&id) {
        return Some(existing.clone());
    }
    if is_plain {
        let result = Value::Object(Rc::new(crate::object::PlainObject));
        memo.insert(id, result.clone());
        return Some(result);
    }
    let source = object
        .as_any()
        .downcast_ref::<InstanceObject>()
        .expect("native instance");
    let namespace_id = |v: &Value| {
        let Value::Dict(dict) = v else {
            unreachable!("instance namespaces are dictionaries")
        };
        (1, Rc::as_ptr(dict) as usize)
    };
    let fields_id = namespace_id(&source.fields);
    let dict_id = namespace_id(&source.dict);
    let previous_fields = memo.get(&fields_id).cloned();
    let previous_dict = memo.get(&dict_id).cloned();
    let fields = previous_fields.clone().unwrap_or_else(Value::dict);
    let dict = previous_dict.clone().unwrap_or_else(Value::dict);
    let instance = Rc::new_cyclic(|this| InstanceObject {
        class: source.class.clone(),
        kind: source.kind.clone(),
        fields,
        dict,
        this: this.clone(),
        handle: source.handle.clone(),
    });
    let result = Value::Object(instance.clone());
    memo.insert(id, result.clone());
    memo.entry(fields_id)
        .or_insert_with(|| instance.fields.clone());
    memo.entry(dict_id).or_insert_with(|| instance.dict.clone());
    for (source, target, previous) in [
        (&source.fields, &instance.fields, previous_fields),
        (&source.dict, &instance.dict, previous_dict),
    ] {
        if previous.is_some() {
            continue;
        }
        let (Value::Dict(source), Value::Dict(target)) = (source, target) else {
            unreachable!()
        };
        let entries = source.borrow().clone();
        let entries = entries
            .iter()
            .map(|(key, value)| (copy(key, memo), copy(value, memo)))
            .collect();
        *target.borrow_mut() = entries;
    }
    Some(result)
}
