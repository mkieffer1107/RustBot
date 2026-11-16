//! Import discovery, Cartesian products and memoization for translated modules.
use crate::{
    PyExc, PyResult, Value, formatting,
    object::{CallArgs, IteratorObject, NativeFunction, PyObject},
    runtime::Runtime,
};
use num_traits::ToPrimitive;
use std::{
    any::Any,
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
};

pub fn register(rt: &mut Runtime) {
    for (name, init) in [
        ("importlib", init_importlib as crate::runtime::ModuleInit),
        ("pkgutil", init_pkgutil),
        ("itertools", init_itertools),
        ("functools", init_functools),
        ("sys", init_sys),
        ("__future__", init_future),
    ] {
        rt.register_module(name, init);
    }
}
fn export(rt: &mut Runtime, module: &Value, name: &str, value: Value) -> PyResult<()> {
    rt.setattr(module, name, value)
}
fn take(args: &mut CallArgs, name: &str) -> Option<Value> {
    args.keywords
        .iter()
        .position(|(k, _)| k == name)
        .map(|i| args.keywords.remove(i).1)
}
fn parameter(args: &mut CallArgs, name: &str, index: usize) -> PyResult<Option<Value>> {
    let keyword = take(args, name);
    if keyword.is_some() && args.positional.len() > index {
        return Err(PyExc::type_error(format!(
            "multiple values for argument '{name}'"
        )));
    }
    Ok(keyword.or_else(|| args.positional.get(index).cloned()))
}
fn qualified(
    module: &str,
    name: &str,
    body: impl Fn(&mut Runtime, CallArgs) -> PyResult<Value> + 'static,
) -> Value {
    NativeFunction::qualified(name.into(), name.into(), module.into(), body)
}

fn init_importlib(rt: &mut Runtime, module: &Value) -> PyResult<()> {
    export(
        rt,
        module,
        "import_module",
        qualified("importlib", "import_module", |rt, mut args| {
            let name = parameter(&mut args, "name", 0)?.ok_or_else(|| {
                PyExc::type_error("import_module() missing required argument 'name'")
            })?;
            let package = parameter(&mut args, "package", 1)?;
            args.reject_keywords()?;
            args.arity(0, 2)?;
            let name = name.as_str()?;
            if !name.starts_with('.') {
                if name.is_empty() {
                    return Err(PyExc::value_error("Empty module name"));
                }
                return rt.import(name);
            }
            let package = package
                .filter(|p| !matches!(p, Value::None))
                .ok_or_else(|| {
                    PyExc::type_error(
                        "the 'package' argument is required to perform a relative import",
                    )
                })?;
            let package = package.as_str()?;
            if package.is_empty() {
                return Err(PyExc::type_error(
                    "the 'package' argument is required to perform a relative import",
                ));
            }
            let level = name.bytes().take_while(|b| *b == b'.').count();
            let parts: Vec<&str> = package.split('.').collect();
            if level > parts.len() {
                return Err(PyExc::import_error(
                    "attempted relative import beyond top-level package",
                ));
            }
            let mut absolute = parts[..parts.len() + 1 - level].join(".");
            let tail = &name[level..];
            if !tail.is_empty() {
                absolute.push('.');
                absolute.push_str(tail);
            }
            rt.import(&absolute)
        }),
    )
}

#[derive(Clone, Debug)]
struct ModuleInfo {
    name: String,
    ispkg: bool,
}
impl ModuleInfo {
    fn values(&self) -> Vec<Value> {
        vec![
            Value::None,
            Value::from(self.name.clone()),
            Value::Bool(self.ispkg),
        ]
    }
}
impl PyObject for ModuleInfo {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "ModuleInfo"
    }
    fn repr(&self) -> PyResult<String> {
        Ok(format!(
            "ModuleInfo(module_finder=None, name={}, ispkg={})",
            formatting::repr(&Value::from(self.name.clone()))?,
            if self.ispkg { "True" } else { "False" }
        ))
    }
    fn len(&self) -> PyResult<usize> {
        Ok(3)
    }
    fn sequence(&self) -> PyResult<Vec<Value>> {
        Ok(self.values())
    }
    fn get_item(&self, index: &Value) -> PyResult<Value> {
        crate::ops::getitem(&Value::tuple(self.values()), index)
    }
    fn get_attr(&self, _rt: &mut Runtime, name: &str) -> PyResult<Value> {
        match name {
            "name" => Ok(Value::from(self.name.clone())),
            "ispkg" => Ok(Value::Bool(self.ispkg)),
            "module_finder" => Ok(Value::None),
            _ => Err(PyExc::attribute_error(format!(
                "ModuleInfo has no attribute '{name}'"
            ))),
        }
    }
}
fn init_pkgutil(rt: &mut Runtime, module: &Value) -> PyResult<()> {
    export(
        rt,
        module,
        "iter_modules",
        qualified("pkgutil", "iter_modules", |_, mut args| {
            let path = parameter(&mut args, "path", 0)?.unwrap_or(Value::None);
            let prefix = parameter(&mut args, "prefix", 1)?.unwrap_or_else(|| Value::from(""));
            args.reject_keywords()?;
            args.arity(0, 2)?;
            let mut entries = None;
            Ok(IteratorObject::value(move |rt| {
                if entries.is_none() {
                    let prefix = prefix.as_str()?;
                    let paths = if matches!(path, Value::None) {
                        vec![Value::from("")]
                    } else {
                        rt.collect(&path)?
                    };
                    let mut found = HashSet::new();
                    let mut list = Vec::new();
                    for path in paths {
                        for (name, ispkg) in rt.module_names(path.as_str()?) {
                            let name = format!("{prefix}{name}");
                            if found.insert(name.clone()) {
                                list.push(Value::Object(Rc::new(ModuleInfo { name, ispkg })));
                            }
                        }
                    }
                    entries = Some(list.into_iter());
                }
                Ok(entries
                    .as_mut()
                    .expect("initialized module iterator")
                    .next())
            }))
        }),
    )
}

fn init_itertools(rt: &mut Runtime, module: &Value) -> PyResult<()> {
    export(
        rt,
        module,
        "product",
        qualified("itertools", "product", product),
    )
}
fn product(rt: &mut Runtime, mut args: CallArgs) -> PyResult<Value> {
    let repeat = take(&mut args, "repeat").unwrap_or_else(|| Value::from(1i64));
    args.reject_keywords()?;
    let repeat = repeat.as_int()?;
    if repeat.sign() == num_bigint::Sign::Minus {
        return Err(PyExc::value_error("repeat argument cannot be negative"));
    }
    let repeat = repeat
        .to_usize()
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| PyExc::overflow_error("Python int too large to convert to C ssize_t"))?;
    let count = args
        .positional
        .len()
        .checked_mul(repeat)
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| PyExc::overflow_error("repeat argument too large"))?;
    let mut pools = Vec::new();
    pools
        .try_reserve(count)
        .map_err(|_| PyExc::memory_error("cannot allocate product pools"))?;
    if repeat != 0 {
        let original = args
            .positional
            .iter()
            .map(|v| rt.collect(v).map(Rc::new))
            .collect::<PyResult<Vec<_>>>()?;
        if !original.is_empty() {
            for _ in 0..repeat {
                pools.extend(original.iter().cloned());
            }
        }
    }
    let mut indices = Vec::new();
    indices
        .try_reserve(count)
        .map_err(|_| PyExc::memory_error("cannot allocate product indices"))?;
    indices.resize(count, 0);
    let mut first = true;
    let mut done = pools.iter().any(|p| p.is_empty());
    Ok(IteratorObject::value(move |_| {
        if done {
            return Ok(None);
        }
        if first {
            first = false;
        } else {
            let mut carry = true;
            for i in (0..indices.len()).rev() {
                indices[i] += 1;
                if indices[i] < pools[i].len() {
                    carry = false;
                    break;
                }
                indices[i] = 0;
            }
            if carry {
                done = true;
                return Ok(None);
            }
        }
        Ok(Some(Value::tuple(
            pools
                .iter()
                .zip(&indices)
                .map(|(pool, index)| pool[*index].clone())
                .collect(),
        )))
    }))
}

#[derive(Clone)]
struct Key {
    hash: i64,
    fast: bool,
    args: Vec<Value>,
    kwargs: Vec<(String, Value)>,
    types: Vec<String>,
}
impl Key {
    fn new(args: &CallArgs, typed: bool) -> PyResult<Self> {
        let fast = !typed
            && args.keywords.is_empty()
            && args.positional.len() == 1
            && matches!(args.positional[0], Value::Int(_) | Value::Str(_));
        let mut values = args.positional.clone();
        if !args.keywords.is_empty() {
            values.push(Value::None);
            for (name, value) in &args.keywords {
                values.push(Value::from(name.clone()));
                values.push(value.clone());
            }
        }
        let types = if typed {
            args.positional
                .iter()
                .chain(args.keywords.iter().map(|(_, v)| v))
                .map(|v| v.type_name().to_owned())
                .collect()
        } else {
            Vec::new()
        };
        if typed {
            values.extend(types.iter().cloned().map(Value::from));
        }
        let hash = crate::sets::hash(&if fast {
            values[0].clone()
        } else {
            Value::tuple(values)
        })?;
        Ok(Self {
            hash,
            fast,
            args: args.positional.clone(),
            kwargs: args.keywords.clone(),
            types,
        })
    }
    fn equals(&self, other: &Self) -> PyResult<bool> {
        if self.fast != other.fast
            || self.types != other.types
            || self.args.len() != other.args.len()
            || self.kwargs.len() != other.kwargs.len()
        {
            return Ok(false);
        }
        for (a, b) in self.args.iter().zip(&other.args) {
            if !a.is_identical(b) && !a.equals(b)? {
                return Ok(false);
            }
        }
        for ((ak, av), (bk, bv)) in self.kwargs.iter().zip(&other.kwargs) {
            if ak != bk || !av.is_identical(bv) && !av.equals(bv)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
#[derive(Clone)]
struct Entry {
    id: u64,
    key: Key,
    value: Value,
}
#[derive(Default)]
struct Cache {
    entries: HashMap<i64, Vec<Entry>>,
    order: VecDeque<(i64, u64)>,
    next: u64,
    hits: u64,
    misses: u64,
    size: usize,
}
fn lookup(cache: &Rc<RefCell<Cache>>, key: &Key) -> PyResult<Option<Entry>> {
    let candidates = cache
        .borrow()
        .entries
        .get(&key.hash)
        .cloned()
        .unwrap_or_default();
    for entry in candidates {
        if entry.key.equals(key)?
            && cache
                .borrow()
                .entries
                .get(&key.hash)
                .is_some_and(|bucket| bucket.iter().any(|v| v.id == entry.id))
        {
            return Ok(Some(entry));
        }
    }
    Ok(None)
}
fn touch(cache: &mut Cache, hash: i64, id: u64) {
    if let Some(index) = cache.order.iter().position(|p| *p == (hash, id)) {
        cache.order.remove(index);
    }
    cache.order.push_back((hash, id));
}

#[derive(Clone, Debug)]
struct CacheInfo {
    hits: u64,
    misses: u64,
    maxsize: Option<usize>,
    currsize: usize,
}
impl CacheInfo {
    fn values(&self) -> Vec<Value> {
        vec![
            Value::Int(self.hits.into()),
            Value::Int(self.misses.into()),
            self.maxsize.map_or(Value::None, Value::from),
            Value::from(self.currsize),
        ]
    }
}
impl PyObject for CacheInfo {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "CacheInfo"
    }
    fn repr(&self) -> PyResult<String> {
        Ok(format!(
            "CacheInfo(hits={}, misses={}, maxsize={}, currsize={})",
            self.hits,
            self.misses,
            self.maxsize.map_or("None".into(), |n| n.to_string()),
            self.currsize
        ))
    }
    fn sequence(&self) -> PyResult<Vec<Value>> {
        Ok(self.values())
    }
    fn len(&self) -> PyResult<usize> {
        Ok(4)
    }
    fn get_item(&self, index: &Value) -> PyResult<Value> {
        crate::ops::getitem(&Value::tuple(self.values()), index)
    }
    fn get_attr(&self, _rt: &mut Runtime, name: &str) -> PyResult<Value> {
        let i = match name {
            "hits" => 0,
            "misses" => 1,
            "maxsize" => 2,
            "currsize" => 3,
            _ => {
                return Err(PyExc::attribute_error(format!(
                    "CacheInfo has no attribute '{name}'"
                )));
            }
        };
        Ok(self.values()[i].clone())
    }
}

fn cached(
    rt: &mut Runtime,
    function: Value,
    maxsize: Option<usize>,
    typed: bool,
) -> PyResult<Value> {
    if !matches!(&function,Value::Object(object) if object.is_callable()) {
        return Err(PyExc::type_error("the first argument must be callable"));
    }
    let cache = Rc::new(RefCell::new(Cache::default()));
    let state = cache.clone();
    let underlying = function.clone();
    let attrs = Value::dict();
    if let Ok(Value::Dict(original)) = rt.getattr(&function, "__dict__") {
        for (k, v) in original.borrow().iter() {
            attrs.dict_set(k.clone(), v.clone())?;
        }
    }
    for name in [
        "__module__",
        "__name__",
        "__qualname__",
        "__doc__",
        "__annotations__",
    ] {
        if let Ok(value) = rt.getattr(&function, name) {
            attrs.dict_set(Value::from(name), value)?;
        }
    }
    let get_text = |name: &str, fallback: &str| {
        attrs
            .dict_get(&Value::from(name))
            .ok()
            .and_then(|v| v.as_str().ok().map(str::to_owned))
            .unwrap_or_else(|| fallback.into())
    };
    let name = get_text("__name__", "cached");
    let qualname = get_text("__qualname__", &name);
    let module = get_text("__module__", "functools");
    attrs.dict_set(Value::from("__wrapped__"), function)?;
    let info = cache.clone();
    attrs.dict_set(
        Value::from("cache_info"),
        qualified("functools", "cache_info", move |_, args| {
            args.reject_keywords()?;
            args.arity(0, 0)?;
            let c = info.borrow();
            Ok(Value::Object(Rc::new(CacheInfo {
                hits: c.hits,
                misses: c.misses,
                maxsize,
                currsize: c.size,
            })))
        }),
    )?;
    let clear = cache.clone();
    attrs.dict_set(
        Value::from("cache_clear"),
        qualified("functools", "cache_clear", move |_, args| {
            args.reject_keywords()?;
            args.arity(0, 0)?;
            let mut cache = clear.borrow_mut();
            let next = cache.next;
            *cache = Cache {
                next,
                ..Cache::default()
            };
            Ok(Value::None)
        }),
    )?;
    attrs.dict_set(
        Value::from("cache_parameters"),
        qualified("functools", "cache_parameters", move |_, args| {
            args.reject_keywords()?;
            args.arity(0, 0)?;
            let out = Value::dict();
            out.dict_set(
                Value::from("maxsize"),
                maxsize.map_or(Value::None, Value::from),
            )?;
            out.dict_set(Value::from("typed"), Value::Bool(typed))?;
            Ok(out)
        }),
    )?;
    Ok(NativeFunction::qualified_with_attrs(
        name,
        qualname,
        module,
        attrs,
        move |rt, args| {
            if maxsize == Some(0) {
                state.borrow_mut().misses += 1;
                return rt.call(&underlying, args);
            }
            let key = Key::new(&args, typed)?;
            if let Some(entry) = lookup(&state, &key)? {
                let mut cache = state.borrow_mut();
                cache.hits += 1;
                if maxsize.is_some() {
                    touch(&mut cache, key.hash, entry.id);
                }
                return Ok(entry.value);
            }
            state.borrow_mut().misses += 1;
            let result = rt.call(&underlying, args)?;
            if lookup(&state, &key)?.is_none() {
                let mut cache = state.borrow_mut();
                let id = cache.next;
                cache.next += 1;
                cache.entries.entry(key.hash).or_default().push(Entry {
                    id,
                    key: key.clone(),
                    value: result.clone(),
                });
                cache.size += 1;
                if let Some(limit) = maxsize {
                    touch(&mut cache, key.hash, id);
                    while cache.size > limit {
                        let (hash, id) = cache.order.pop_front().expect("LRU entry");
                        if let Some(bucket) = cache.entries.get_mut(&hash) {
                            bucket.retain(|e| e.id != id);
                            if bucket.is_empty() {
                                cache.entries.remove(&hash);
                            }
                        }
                        cache.size -= 1;
                    }
                }
            }
            Ok(result)
        },
    ))
}
fn init_functools(rt: &mut Runtime, module: &Value) -> PyResult<()> {
    export(
        rt,
        module,
        "lru_cache",
        qualified("functools", "lru_cache", |rt, mut args| {
            let maxsize =
                parameter(&mut args, "maxsize", 0)?.unwrap_or_else(|| Value::from(128i64));
            let typed = parameter(&mut args, "typed", 1)?.is_some_and(|v| v.is_truthy());
            args.reject_keywords()?;
            args.arity(0, 2)?;
            if matches!(&maxsize,Value::Object(o)if o.is_callable()) {
                return cached(rt, maxsize, Some(128), typed);
            }
            let maxsize = if matches!(maxsize, Value::None) {
                None
            } else {
                let n = maxsize.as_int().map_err(|_| {
                    PyExc::type_error(
                        "Expected first argument to be an integer, a callable, or None",
                    )
                })?;
                if n.sign() == num_bigint::Sign::Minus {
                    Some(0)
                } else {
                    Some(
                        n.to_usize()
                            .filter(|n| *n <= isize::MAX as usize)
                            .ok_or_else(|| {
                                PyExc::overflow_error(
                                    "Python int too large to convert to C ssize_t",
                                )
                            })?,
                    )
                }
            };
            Ok(qualified(
                "functools",
                "decorating_function",
                move |rt, args| {
                    args.reject_keywords()?;
                    args.arity(1, 1)?;
                    cached(rt, args.positional[0].clone(), maxsize, typed)
                },
            ))
        }),
    )
}
fn init_sys(rt: &mut Runtime, module: &Value) -> PyResult<()> {
    export(
        rt,
        module,
        "argv",
        Value::list(vec![Value::from("rustbot")]),
    )
}
fn init_future(rt: &mut Runtime, module: &Value) -> PyResult<()> {
    export(
        rt,
        module,
        "annotations",
        Value::Opaque {
            text: "_Feature((3, 7, 0, 'beta', 1), None, 16777216)".into(),
            type_name: "_Feature".into(),
        },
    )
}
