//! Native standard-library objects shared by generated Rust modules.
use crate::context::RandomHandle;
use crate::datetime::{PyDate, PyDateTime, PyTimedelta};
use crate::object::{CallArgs, IteratorObject, NativeFunction, NativeType, PyObject};
use crate::random::{PyRandom, RandomState};
use crate::runtime::Runtime;
use crate::{PyExc, PyResult, Value, canon_json, from_canon_json};
use num_traits::{Signed, ToPrimitive};
use serde_json::{Value as Json, json};
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

pub fn register(runtime: &mut Runtime) {
    for (name, init) in [
        ("math", init_math as crate::runtime::ModuleInit),
        ("string", init_string),
        ("unicodedata", init_unicode),
        ("copy", init_copy),
        ("calendar", init_calendar),
        ("random", init_random),
        ("datetime", init_datetime),
        ("re", init_re),
    ] {
        runtime.register_module(name, init);
    }
}

fn set(runtime: &mut Runtime, module: &Value, name: &str, value: Value) -> PyResult<()> {
    runtime.setattr(module, name, value)
}

fn function(
    name: &str,
    body: impl Fn(&mut Runtime, CallArgs) -> PyResult<Value> + 'static,
) -> Value {
    let (module, short) = name.rsplit_once('.').unwrap_or(("builtins", name));
    NativeFunction::qualified(short.into(), short.into(), module.into(), body)
}

fn integer(value: &Value) -> PyResult<i64> {
    value
        .as_int()?
        .to_i64()
        .ok_or_else(|| PyExc::overflow_error("Python int too large to convert to C long"))
}

fn bind(
    args: CallArgs,
    names: &[&str],
    required: usize,
    defaults: Vec<Value>,
) -> PyResult<Vec<Value>> {
    if args.positional.len() > names.len() {
        return Err(PyExc::type_error("too many positional arguments"));
    }
    let mut values = vec![None; names.len()];
    for (slot, value) in values.iter_mut().zip(args.positional) {
        *slot = Some(value);
    }
    for (name, value) in args.keywords {
        let index = names
            .iter()
            .position(|n| *n == name)
            .ok_or_else(|| PyExc::type_error(format!("unexpected keyword argument '{name}'")))?;
        if values[index].is_some() {
            return Err(PyExc::type_error(format!(
                "multiple values for argument '{name}'"
            )));
        }
        values[index] = Some(value);
    }
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .or_else(|| {
                    index
                        .checked_sub(required)
                        .and_then(|i| defaults.get(i).cloned())
                })
                .ok_or_else(|| {
                    PyExc::type_error(format!("missing required argument '{}'", names[index]))
                })
        })
        .collect()
}

fn init_math(runtime: &mut Runtime, module: &Value) -> PyResult<()> {
    for name in [
        "sin", "cos", "asin", "ceil", "floor", "sqrt", "radians", "log", "log10",
    ] {
        set(
            runtime,
            module,
            name,
            function(&format!("math.{name}"), move |_, args| {
                args.reject_keywords()?;
                crate::math::call(name, &args.positional)
            }),
        )?;
    }
    for (name, value) in [
        ("pi", std::f64::consts::PI),
        ("e", std::f64::consts::E),
        ("tau", std::f64::consts::TAU),
        ("inf", f64::INFINITY),
        ("nan", f64::NAN),
    ] {
        set(runtime, module, name, Value::Float(value))?;
    }
    Ok(())
}

fn init_string(runtime: &mut Runtime, module: &Value) -> PyResult<()> {
    for (name, value) in [
        ("punctuation", crate::math::PUNCTUATION),
        ("ascii_lowercase", crate::math::ASCII_LOWERCASE),
        ("ascii_uppercase", crate::math::ASCII_UPPERCASE),
        ("ascii_letters", crate::math::ASCII_LETTERS),
        ("digits", crate::math::DIGITS),
        ("whitespace", crate::math::WHITESPACE),
        ("hexdigits", "0123456789abcdefABCDEF"),
        ("octdigits", "01234567"),
        (
            "printable",
            "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~ \t\n\r\x0b\x0c",
        ),
    ] {
        set(runtime, module, name, Value::from(value))?;
    }
    Ok(())
}

fn init_unicode(runtime: &mut Runtime, module: &Value) -> PyResult<()> {
    set(runtime, module, "unidata_version", Value::from("16.0.0"))?;
    for name in ["normalize", "combining"] {
        set(
            runtime,
            module,
            name,
            function(&format!("unicodedata.{name}"), move |_, args| {
                args.reject_keywords()?;
                crate::unicode::call(name, &args.positional)
            }),
        )?;
    }
    Ok(())
}

fn init_copy(runtime: &mut Runtime, module: &Value) -> PyResult<()> {
    set(
        runtime,
        module,
        "deepcopy",
        function("copy.deepcopy", |_, args| {
            args.reject_keywords()?;
            args.arity(1, 2)?;
            if args.positional.len() == 2 && !matches!(args.positional[1], Value::None) {
                return Err(PyExc::not_implemented_error("external deepcopy memo"));
            }
            Ok(args.positional[0].deepcopy())
        }),
    )?;
    Ok(())
}

fn init_calendar(runtime: &mut Runtime, module: &Value) -> PyResult<()> {
    set(
        runtime,
        module,
        "monthrange",
        function("calendar.monthrange", |_, args| {
            let args = bind(args, &["year", "month"], 2, vec![])?;
            let (weekday, days) =
                crate::datetime::monthrange(integer(&args[0])?, integer(&args[1])?)?;
            Ok(Value::tuple(vec![
                Value::from(i64::from(weekday)),
                Value::from(i64::from(days)),
            ]))
        }),
    )?;
    set(
        runtime,
        module,
        "isleap",
        function("calendar.isleap", |_, args| {
            let args = bind(args, &["year"], 1, vec![])?;
            Ok(Value::Bool(crate::datetime::is_leap(integer(&args[0])?)))
        }),
    )?;
    Ok(())
}

fn seed(value: &Value) -> PyResult<PyRandom> {
    match value {
        Value::Int(_) | Value::Bool(_) => Ok(PyRandom::from_int(&value.as_int()?)),
        Value::Float(f) => PyRandom::from_float(*f),
        Value::Str(s) => Ok(PyRandom::from_str(s.as_str())),
        _ => Err(PyExc::type_error(
            "The only supported seed types are: None, int, float, str, bytes, and bytearray.",
        )),
    }
}

fn raw_state_value(state: &RandomState) -> Value {
    let mut words: Vec<_> = state
        .words
        .iter()
        .map(|word| Value::Int((*word).into()))
        .collect();
    words.push(Value::from(state.index));
    Value::tuple(vec![
        Value::from(state.version),
        Value::tuple(words),
        state.gauss_next.map_or(Value::None, Value::Float),
    ])
}

#[derive(Debug)]
struct StateObject(RandomState);

impl PyObject for StateObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "tuple"
    }
    fn repr(&self) -> PyResult<String> {
        crate::formatting::repr(&raw_state_value(&self.0))
    }
    fn canonical(&self) -> PyResult<Option<Json>> {
        Ok(Some(canon_json(&raw_state_value(&self.0))?))
    }
    fn sequence(&self) -> PyResult<Vec<Value>> {
        raw_state_value(&self.0).sequence()
    }
    fn len(&self) -> PyResult<usize> {
        Ok(3)
    }
    fn get_item(&self, index: &Value) -> PyResult<Value> {
        crate::ops::getitem(&raw_state_value(&self.0), index)
    }
    fn equals(&self, other: &Value) -> PyResult<bool> {
        if let Value::Object(o) = other
            && let Some(other) = o.as_any().downcast_ref::<Self>()
        {
            return Ok(self.0 == other.0);
        }
        raw_state_value(&self.0).equals(other)
    }
}

fn parse_state(value: &Value) -> PyResult<RandomState> {
    if let Value::Object(object) = value
        && let Some(state) = object.as_any().downcast_ref::<StateObject>()
    {
        return Ok(state.0.clone());
    }
    let state = value.sequence()?;
    if state.len() != 3 {
        return Err(PyExc::value_error("state vector is the wrong size"));
    }
    let version = integer(&state[0])?;
    let words = state[1].sequence()?;
    if words.len() != 625 {
        return Err(PyExc::value_error("state vector is the wrong size"));
    }
    let mut mt = [0; 624];
    for (slot, value) in mt.iter_mut().zip(&words) {
        let value = value.as_int()?;
        if version == 2 {
            let modulus = num_bigint::BigInt::from(1_u64 << 32);
            let value = ((value % &modulus) + &modulus) % modulus;
            *slot = value.to_u32().unwrap();
        } else {
            let value = value.to_u64().ok_or_else(|| {
                PyExc::overflow_error("can't convert negative value to unsigned int")
            })?;
            *slot = value as u32;
        }
    }
    let index = words[624]
        .as_int()?
        .to_usize()
        .ok_or_else(|| PyExc::value_error("invalid state"))?;
    let gauss = if matches!(state[2], Value::None) {
        None
    } else {
        Some(state[2].as_float()?)
    };
    Ok(RandomState::from_raw(version, mt, index, gauss))
}

#[derive(Clone, Debug)]
struct RandomObject(RandomHandle);

fn random_method(handle: RandomHandle, name: &str) -> Value {
    let name = name.to_owned();
    function(&format!("random.{name}"), move |runtime, args| {
        match name.as_str() {
            "random" | "getstate" => {
                args.reject_keywords()?;
                args.arity(0, 0)?;
                if name == "random" {
                    Ok(Value::Float(handle.borrow_mut().random()))
                } else {
                    Ok(Value::Object(Rc::new(StateObject(
                        handle.borrow().getstate(),
                    ))))
                }
            }
            "setstate" => {
                args.reject_keywords()?;
                args.arity(1, 1)?;
                handle
                    .borrow_mut()
                    .setstate(&parse_state(&args.positional[0])?)?;
                Ok(Value::None)
            }
            "seed" => {
                let args = bind(
                    args,
                    &["a", "version"],
                    0,
                    vec![Value::None, Value::from(2_i64)],
                )?;
                if integer(&args[1])? != 2 {
                    return Err(PyExc::not_implemented_error("version-1 random seeding"));
                }
                if matches!(args[0], Value::None) {
                    return Err(PyExc::not_implemented_error(
                        "reseed(None) creation-order registration",
                    ));
                }
                *handle.borrow_mut() = seed(&args[0])?;
                Ok(Value::None)
            }
            "getrandbits" => {
                args.reject_keywords()?;
                args.arity(1, 1)?;
                let bits = args.positional[0].as_int()?;
                if bits.is_negative() {
                    return Err(PyExc::value_error("Cannot convert negative int"));
                }
                let bits = bits
                    .to_u64()
                    .ok_or_else(|| PyExc::overflow_error("Python int too large for C uint64_t"))?;
                Ok(Value::Int(handle.borrow_mut().getrandbits_u64(bits)?))
            }
            "randrange" => {
                let args = bind(
                    args,
                    &["start", "stop", "step"],
                    1,
                    vec![Value::None, Value::from(1_i64)],
                )?;
                let start = args[0].as_int()?;
                let stop = if matches!(args[1], Value::None) {
                    None
                } else {
                    Some(args[1].as_int()?)
                };
                let step = args[2].as_int()?;
                Ok(Value::Int(handle.borrow_mut().randrange(
                    &start,
                    stop.as_ref(),
                    Some(&step),
                )?))
            }
            "randint" => {
                let args = bind(args, &["a", "b"], 2, vec![])?;
                Ok(Value::Int(
                    handle
                        .borrow_mut()
                        .randint(&args[0].as_int()?, &args[1].as_int()?)?,
                ))
            }
            "choice" => {
                let args = bind(args, &["seq"], 1, vec![])?;
                let len = sequence_len(&args[0])?;
                let index = handle.borrow_mut().choice_index(len)?;
                get_item(runtime, &args[0], &Value::from(index))
            }
            "shuffle" => {
                let args = bind(args, &["x"], 1, vec![])?;
                let Value::List(items) = &args[0] else {
                    return Err(PyExc::type_error("object does not support item assignment"));
                };
                handle.borrow_mut().shuffle(&mut items.borrow_mut());
                Ok(Value::None)
            }
            "sample" => {
                let args = bind(args, &["population", "k", "counts"], 2, vec![Value::None])?;
                if !matches!(args[2], Value::None) {
                    return Err(PyExc::not_implemented_error("random.sample counts"));
                }
                if matches!(
                    args[0],
                    Value::Set(_) | Value::FrozenSet(_) | Value::Dict(_)
                ) {
                    return Err(PyExc::type_error(
                        "Population must be a sequence. For dicts or sets, use sorted(d).",
                    ));
                }
                let population = runtime.collect(&args[0])?;
                let k = args[1].as_int()?.to_usize().ok_or_else(|| {
                    PyExc::value_error("Sample larger than population or is negative")
                })?;
                let indices = handle.borrow_mut().sample_indices(population.len(), k)?;
                Ok(Value::list(
                    indices.into_iter().map(|i| population[i].clone()).collect(),
                ))
            }
            "choices" => {
                if args.positional.len() > 2 {
                    return Err(PyExc::type_error(
                        "choices takes at most 2 positional arguments",
                    ));
                }
                let args = bind(
                    args,
                    &["population", "weights", "cum_weights", "k"],
                    1,
                    vec![Value::None, Value::None, Value::from(1_i64)],
                )?;
                let population = runtime.collect(&args[0])?;
                let weights = |value: &Value| -> PyResult<Option<Vec<f64>>> {
                    if matches!(value, Value::None) {
                        Ok(None)
                    } else {
                        Ok(Some(
                            value
                                .sequence()?
                                .iter()
                                .map(Value::as_float)
                                .collect::<PyResult<_>>()?,
                        ))
                    }
                };
                let k = integer(&args[3])?.max(0) as usize;
                let (weights, cumulative) = (weights(&args[1])?, weights(&args[2])?);
                let indices = handle.borrow_mut().choices_indices(
                    population.len(),
                    weights.as_deref(),
                    cumulative.as_deref(),
                    k,
                )?;
                Ok(Value::list(
                    indices.into_iter().map(|i| population[i].clone()).collect(),
                ))
            }
            "uniform" => {
                let args = bind(args, &["a", "b"], 2, vec![])?;
                let difference = crate::ops::sub(&args[1], &args[0])?;
                let scaled =
                    crate::ops::mul(&difference, &Value::Float(handle.borrow_mut().random()))?;
                crate::ops::add(&args[0], &scaled)
            }
            _ => Err(PyExc::attribute_error(format!(
                "Random has no attribute '{name}'"
            ))),
        }
    })
}

impl PyObject for RandomObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "Random"
    }
    fn get_attr(&self, _runtime: &mut Runtime, name: &str) -> PyResult<Value> {
        if [
            "random",
            "getstate",
            "setstate",
            "seed",
            "getrandbits",
            "randrange",
            "randint",
            "choice",
            "shuffle",
            "sample",
            "choices",
            "uniform",
        ]
        .contains(&name)
        {
            Ok(random_method(self.0.clone(), name))
        } else {
            Err(PyExc::attribute_error(format!(
                "Random has no attribute '{name}'"
            )))
        }
    }
}

fn init_random(runtime: &mut Runtime, module: &Value) -> PyResult<()> {
    for name in [
        "random",
        "getstate",
        "setstate",
        "seed",
        "getrandbits",
        "randrange",
        "randint",
        "choice",
        "shuffle",
        "sample",
        "choices",
        "uniform",
    ] {
        set(
            runtime,
            module,
            name,
            random_method(runtime.random.global(), name),
        )?;
    }
    set(
        runtime,
        module,
        "Random",
        NativeType::value(
            "Random",
            "random",
            |value| matches!(value, Value::Object(o) if o.as_any().is::<RandomObject>()),
            |runtime, args| {
                let args = bind(args, &["x"], 0, vec![Value::None])?;
                let handle = if matches!(args[0], Value::None) {
                    runtime.random.new_unseeded()
                } else {
                    Rc::new(RefCell::new(seed(&args[0])?))
                };
                Ok(Value::Object(Rc::new(RandomObject(handle))))
            },
        ),
    )?;
    Ok(())
}

fn sequence_len(value: &Value) -> PyResult<usize> {
    match value {
        Value::Object(o) => o.len(),
        Value::Str(s) => Ok(s.len()),
        Value::List(v) => Ok(v.borrow().len()),
        Value::Tuple(v) => Ok(v.len()),
        Value::Dict(v) => Ok(v.borrow().len()),
        Value::Set(v) | Value::FrozenSet(v) => Ok(v.len()),
        _ => Err(PyExc::type_error("object has no len()")),
    }
}

fn get_item(_runtime: &mut Runtime, value: &Value, index: &Value) -> PyResult<Value> {
    if let Value::Object(o) = value {
        o.get_item(index)
    } else {
        crate::ops::getitem(value, index)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Temporal {
    Date(PyDate),
    DateTime(PyDateTime),
    Delta(PyTimedelta),
}

#[derive(Clone, Copy, Debug)]
pub struct TemporalObject(pub Temporal);

pub fn temporal(value: &Value) -> Option<Temporal> {
    if let Value::Object(o) = value {
        o.as_any().downcast_ref::<TemporalObject>().map(|v| v.0)
    } else {
        match value {
            Value::Date(s) => PyDate::fromisoformat(s).ok().map(Temporal::Date),
            Value::DateTime(s) => PyDateTime::fromisoformat(s).ok().map(Temporal::DateTime),
            _ => None,
        }
    }
}

pub fn temporal_value(value: Temporal) -> Value {
    Value::Object(Rc::new(TemporalObject(value)))
}

/// Operator entry points let the shared operator module retain control of
/// dispatch without exposing the native wrapper's storage to generated code.
pub fn temporal_binary(op: &str, a: &Value, b: &Value) -> Option<PyResult<Value>> {
    let (a, b) = (temporal(a)?, temporal(b)?);
    let result = match (op, a, b) {
        ("+", Temporal::Date(d), Temporal::Delta(t))
        | ("+", Temporal::Delta(t), Temporal::Date(d)) => d.add_timedelta(t).map(Temporal::Date),
        ("+", Temporal::DateTime(d), Temporal::Delta(t))
        | ("+", Temporal::Delta(t), Temporal::DateTime(d)) => {
            d.add_timedelta(t).map(Temporal::DateTime)
        }
        ("+", Temporal::Delta(a), Temporal::Delta(b)) => a.checked_add(b).map(Temporal::Delta),
        ("-", Temporal::Date(d), Temporal::Delta(t)) => d.sub_timedelta(t).map(Temporal::Date),
        ("-", Temporal::DateTime(d), Temporal::Delta(t)) => {
            d.sub_timedelta(t).map(Temporal::DateTime)
        }
        ("-", Temporal::Date(a), Temporal::Date(b)) => Ok(Temporal::Delta(a.difference(b))),
        ("-", Temporal::DateTime(a), Temporal::DateTime(b)) => a.difference(b).map(Temporal::Delta),
        ("-", Temporal::Delta(a), Temporal::Delta(b)) => a.checked_sub(b).map(Temporal::Delta),
        _ => return None,
    };
    Some(result.map(temporal_value))
}

pub fn temporal_compare(a: &Value, b: &Value) -> Option<PyResult<std::cmp::Ordering>> {
    Some(match (temporal(a)?, temporal(b)?) {
        (Temporal::Date(a), Temporal::Date(b)) => Ok(a.cmp(&b)),
        (Temporal::DateTime(a), Temporal::DateTime(b)) => a.compare(b),
        (Temporal::Delta(a), Temporal::Delta(b)) => Ok(a.cmp(&b)),
        _ => return None,
    })
}

fn date_part(value: Temporal) -> Option<PyDate> {
    match value {
        Temporal::Date(d) => Some(d),
        Temporal::DateTime(d) => Some(d.date()),
        _ => None,
    }
}

#[derive(Clone, Debug)]
struct TimezoneObject {
    offset: i32,
    name: Option<String>,
}

fn timezone_value(offset: i32, name: Option<String>) -> Value {
    Value::Object(Rc::new(TimezoneObject { offset, name }))
}

fn timezone_offset(value: &Value) -> PyResult<Option<i32>> {
    if matches!(value, Value::None) {
        return Ok(None);
    }
    if let Value::Object(o) = value
        && let Some(zone) = o.as_any().downcast_ref::<TimezoneObject>()
    {
        return Ok(Some(zone.offset));
    }
    Err(PyExc::type_error(
        "tzinfo argument must be None or of a tzinfo subclass",
    ))
}

fn timedelta_repr(delta: PyTimedelta) -> String {
    let mut parts = Vec::new();
    if delta.days() != 0 {
        parts.push(format!("days={}", delta.days()));
    }
    if delta.seconds() != 0 {
        parts.push(format!("seconds={}", delta.seconds()));
    }
    if delta.microseconds() != 0 {
        parts.push(format!("microseconds={}", delta.microseconds()));
    }
    if parts.is_empty() {
        parts.push("0".into());
    }
    format!("datetime.timedelta({})", parts.join(", "))
}

impl PyObject for TimezoneObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "timezone"
    }
    fn repr(&self) -> PyResult<String> {
        if self.offset == 0 && self.name.is_none() {
            return Ok("datetime.timezone.utc".into());
        }
        let delta = timedelta_repr(PyTimedelta::new(0, i64::from(self.offset), 0)?);
        Ok(if let Some(name) = &self.name {
            format!(
                "datetime.timezone({delta}, {})",
                crate::formatting::repr(&Value::from(name.clone()))?
            )
        } else {
            format!("datetime.timezone({delta})")
        })
    }
    fn str(&self) -> PyResult<String> {
        if let Some(name) = &self.name {
            return Ok(name.clone());
        }
        if self.offset == 0 {
            return Ok("UTC".into());
        }
        let magnitude = self.offset.unsigned_abs();
        let mut text = format!(
            "UTC{}{:02}:{:02}",
            if self.offset < 0 { '-' } else { '+' },
            magnitude / 3600,
            magnitude / 60 % 60
        );
        if !magnitude.is_multiple_of(60) {
            text.push_str(&format!(":{:02}", magnitude % 60));
        }
        Ok(text)
    }
    fn equals(&self, other: &Value) -> PyResult<bool> {
        Ok(
            matches!(other, Value::Object(o) if o.as_any().downcast_ref::<Self>().is_some_and(|z| z.offset == self.offset)),
        )
    }
    fn get_attr(&self, _: &mut Runtime, name: &str) -> PyResult<Value> {
        let zone = self.clone();
        match name {
            "utcoffset" | "dst" | "tzname" => {
                let name = name.to_owned();
                Ok(function(&format!("timezone.{name}"), move |_, args| {
                    args.reject_keywords()?;
                    args.arity(1, 1)?;
                    match name.as_str() {
                        "utcoffset" => Ok(temporal_value(Temporal::Delta(PyTimedelta::new(
                            0,
                            i64::from(zone.offset),
                            0,
                        )?))),
                        "dst" => Ok(Value::None),
                        _ => Ok(Value::from(zone.str()?)),
                    }
                }))
            }
            _ => Err(PyExc::attribute_error(format!(
                "timezone has no attribute '{name}'"
            ))),
        }
    }
}

fn temporal_strftime(value: Temporal, spec: &str) -> PyResult<String> {
    let date =
        date_part(value).ok_or_else(|| PyExc::attribute_error("timedelta has no strftime"))?;
    let (hour, minute, second, microsecond, offset) = if let Temporal::DateTime(d) = value {
        (
            d.hour(),
            d.minute(),
            d.second(),
            d.microsecond(),
            d.offset_seconds(),
        )
    } else {
        (0, 0, 0, 0, None)
    };
    let mut output = String::new();
    let mut chars = spec.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            output.push(c);
            continue;
        }
        let Some(code) = chars.next() else {
            output.push('%');
            break;
        };
        let piece = match code {
            'H' => format!("{hour:02}"),
            'I' => format!("{:02}", (hour + 11) % 12 + 1),
            'M' => format!("{minute:02}"),
            'S' => format!("{second:02}"),
            'f' => format!("{microsecond:06}"),
            'p' => if hour < 12 { "AM" } else { "PM" }.into(),
            'y' => format!("{:02}", date.year() % 100),
            'j' => format!(
                "{:03}",
                date.toordinal() - PyDate::new(i64::from(date.year()), 1, 1)?.toordinal() + 1
            ),
            'w' => ((date.weekday() + 1) % 7).to_string(),
            'u' => date.isoweekday().to_string(),
            'V' => format!("{:02}", date.isocalendar().1),
            'G' => format!("{:04}", date.isocalendar().0),
            'z' => offset
                .map(|n| {
                    format!(
                        "{}{:02}{:02}{}",
                        if n < 0 { '-' } else { '+' },
                        n.unsigned_abs() / 3600,
                        n.unsigned_abs() / 60 % 60,
                        if n % 60 == 0 {
                            String::new()
                        } else {
                            format!("{:02}", n.unsigned_abs() % 60)
                        }
                    )
                })
                .unwrap_or_default(),
            'Z' => {
                if let Some(offset) = offset {
                    TimezoneObject { offset, name: None }.str()?
                } else {
                    String::new()
                }
            }
            'x' => format!(
                "{:02}/{:02}/{:02}",
                date.month(),
                date.day(),
                date.year() % 100
            ),
            'X' => format!("{hour:02}:{minute:02}:{second:02}"),
            'A' | 'a' | 'B' | 'b' | 'h' | 'Y' | 'm' | 'd' | 'F' | '%' => {
                crate::formatting::format(&date.to_value(), &format!("%{code}"))?
            }
            _ => {
                return Err(PyExc::not_implemented_error(format!(
                    "strftime directive %{code}"
                )));
            }
        };
        output.push_str(&piece);
    }
    Ok(output)
}

impl PyObject for TemporalObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        match self.0 {
            Temporal::Date(_) => "date",
            Temporal::DateTime(_) => "datetime",
            Temporal::Delta(_) => "timedelta",
        }
    }
    fn str(&self) -> PyResult<String> {
        Ok(match self.0 {
            Temporal::Date(d) => d.to_string(),
            Temporal::DateTime(d) => d.isoformat_with(' ', "auto")?,
            Temporal::Delta(d) => d.to_string(),
        })
    }
    fn format(&self, spec: &str) -> PyResult<String> {
        if spec.is_empty() {
            self.str()
        } else {
            temporal_strftime(self.0, spec)
        }
    }
    fn repr(&self) -> PyResult<String> {
        match self.0 {
            Temporal::Delta(d) => Ok(timedelta_repr(d)),
            Temporal::Date(d) => crate::formatting::repr(&d.to_value()),
            Temporal::DateTime(d) => {
                let mut args = vec![
                    d.year().to_string(),
                    d.month().to_string(),
                    d.day().to_string(),
                    d.hour().to_string(),
                    d.minute().to_string(),
                ];
                if d.second() != 0 || d.microsecond() != 0 {
                    args.push(d.second().to_string());
                }
                if d.microsecond() != 0 {
                    args.push(d.microsecond().to_string());
                }
                if let Some(offset) = d.offset_seconds() {
                    args.push(format!(
                        "tzinfo={}",
                        TimezoneObject { offset, name: None }.repr()?
                    ));
                }
                Ok(format!("datetime.datetime({})", args.join(", ")))
            }
        }
    }
    fn canonical(&self) -> PyResult<Option<Json>> {
        match self.0 {
            Temporal::Date(d) => Ok(Some(json!({"$date":d.isoformat()}))),
            Temporal::DateTime(d) => Ok(Some(json!({"$datetime":d.isoformat()}))),
            _ => Ok(None),
        }
    }
    fn is_truthy(&self) -> bool {
        !matches!(self.0, Temporal::Delta(d) if d.is_zero())
    }
    fn equals(&self, other: &Value) -> PyResult<bool> {
        Ok(match (self.0, temporal(other)) {
            (Temporal::Date(a), Some(Temporal::Date(b))) => a == b,
            (Temporal::DateTime(a), Some(Temporal::DateTime(b))) => {
                a.compare(b).is_ok_and(|c| c.is_eq())
            }
            (Temporal::Delta(a), Some(Temporal::Delta(b))) => a == b,
            _ => false,
        })
    }
    fn get_attr(&self, _: &mut Runtime, name: &str) -> PyResult<Value> {
        let value = self.0;
        if let Some(date) = date_part(value) {
            match name {
                "year" => return Ok(Value::from(i64::from(date.year()))),
                "month" => return Ok(Value::from(i64::from(date.month()))),
                "day" => return Ok(Value::from(i64::from(date.day()))),
                _ => (),
            }
        }
        if let Temporal::DateTime(d) = value {
            match name {
                "hour" => return Ok(Value::from(i64::from(d.hour()))),
                "minute" => return Ok(Value::from(i64::from(d.minute()))),
                "second" => return Ok(Value::from(i64::from(d.second()))),
                "microsecond" => return Ok(Value::from(i64::from(d.microsecond()))),
                "tzinfo" => {
                    return Ok(d
                        .offset_seconds()
                        .map_or(Value::None, |o| timezone_value(o, None)));
                }
                "fold" => return Ok(Value::from(0_i64)),
                _ => (),
            }
        }
        if let Temporal::Delta(d) = value {
            match name {
                "days" => return Ok(Value::from(i64::from(d.days()))),
                "seconds" => return Ok(Value::from(i64::from(d.seconds()))),
                "microseconds" => return Ok(Value::from(i64::from(d.microseconds()))),
                "total_seconds" => {
                    return Ok(function("timedelta.total_seconds", move |_, args| {
                        args.reject_keywords()?;
                        args.arity(0, 0)?;
                        Ok(Value::Float(d.total_seconds()))
                    }));
                }
                _ => {
                    return Err(PyExc::attribute_error(format!(
                        "timedelta has no attribute '{name}'"
                    )));
                }
            }
        }
        if ![
            "isoformat",
            "weekday",
            "isoweekday",
            "isocalendar",
            "toordinal",
            "strftime",
            "replace",
            "date",
            "astimezone",
            "utcoffset",
            "dst",
            "tzname",
        ]
        .contains(&name)
        {
            return Err(PyExc::attribute_error(format!(
                "{} has no attribute '{name}'",
                self.type_name()
            )));
        }
        let name = name.to_owned();
        Ok(function(
            &format!("{}.{name}", self.type_name()),
            move |_, args| {
                let date = date_part(value).unwrap();
                match name.as_str() {
                    "strftime" => {
                        args.reject_keywords()?;
                        args.arity(1, 1)?;
                        Ok(Value::from(temporal_strftime(
                            value,
                            args.positional[0].as_str()?,
                        )?))
                    }
                    "isoformat" => {
                        if let Temporal::DateTime(d) = value {
                            let args = bind(
                                args,
                                &["sep", "timespec"],
                                0,
                                vec![Value::from("T"), Value::from("auto")],
                            )?;
                            let sep = args[0].as_str()?;
                            if sep.chars().count() != 1 {
                                return Err(PyExc::type_error(
                                    "isoformat() argument 1 must be a unicode character",
                                ));
                            }
                            Ok(Value::from(d.isoformat_with(
                                sep.chars().next().unwrap(),
                                args[1].as_str()?,
                            )?))
                        } else {
                            args.reject_keywords()?;
                            args.arity(0, 0)?;
                            Ok(Value::from(date.isoformat()))
                        }
                    }
                    "replace" => {
                        let names = if matches!(value, Temporal::DateTime(_)) {
                            &[
                                "year",
                                "month",
                                "day",
                                "hour",
                                "minute",
                                "second",
                                "microsecond",
                                "tzinfo",
                                "fold",
                            ][..]
                        } else {
                            &["year", "month", "day"][..]
                        };
                        let defaults = if let Temporal::DateTime(d) = value {
                            vec![
                                Value::from(i64::from(date.year())),
                                Value::from(i64::from(date.month())),
                                Value::from(i64::from(date.day())),
                                Value::from(i64::from(d.hour())),
                                Value::from(i64::from(d.minute())),
                                Value::from(i64::from(d.second())),
                                Value::from(i64::from(d.microsecond())),
                                d.offset_seconds()
                                    .map_or(Value::None, |o| timezone_value(o, None)),
                                Value::from(0_i64),
                            ]
                        } else {
                            vec![
                                Value::from(i64::from(date.year())),
                                Value::from(i64::from(date.month())),
                                Value::from(i64::from(date.day())),
                            ]
                        };
                        let args = bind(args, names, 0, defaults)?;
                        let replaced = PyDate::new(
                            integer(&args[0])?,
                            integer(&args[1])?,
                            integer(&args[2])?,
                        )?;
                        if let Temporal::DateTime(d) = value {
                            if ![0, 1].contains(&integer(&args[8])?) {
                                return Err(PyExc::value_error("fold must be either 0 or 1"));
                            }
                            Ok(temporal_value(Temporal::DateTime(d.replace(
                                Some(replaced),
                                Some(integer(&args[3])?),
                                Some(integer(&args[4])?),
                                Some(integer(&args[5])?),
                                Some(integer(&args[6])?),
                                Some(timezone_offset(&args[7])?),
                            )?)))
                        } else {
                            Ok(temporal_value(Temporal::Date(replaced)))
                        }
                    }
                    "astimezone" => {
                        let Temporal::DateTime(d) = value else {
                            return Err(PyExc::attribute_error("date has no astimezone"));
                        };
                        let args = bind(args, &["tz"], 0, vec![timezone_value(0, None)])?;
                        Ok(temporal_value(Temporal::DateTime(
                            d.astimezone(timezone_offset(&args[0])?.unwrap_or(0))?,
                        )))
                    }
                    _ => {
                        args.reject_keywords()?;
                        args.arity(0, 0)?;
                        match name.as_str() {
                            "date" if matches!(value, Temporal::DateTime(_)) => {
                                Ok(temporal_value(Temporal::Date(date)))
                            }
                            "weekday" => Ok(Value::from(i64::from(date.weekday()))),
                            "isoweekday" => Ok(Value::from(i64::from(date.isoweekday()))),
                            "isocalendar" => {
                                let (y, w, d) = date.isocalendar();
                                Ok(Value::tuple(vec![
                                    Value::from(i64::from(y)),
                                    Value::from(i64::from(w)),
                                    Value::from(i64::from(d)),
                                ]))
                            }
                            "toordinal" => Ok(Value::from(date.toordinal())),
                            "utcoffset" => {
                                if let Temporal::DateTime(d) = value {
                                    Ok(d.offset_seconds().map_or(Value::None, |o| {
                                        temporal_value(Temporal::Delta(
                                            PyTimedelta::new(0, i64::from(o), 0).unwrap(),
                                        ))
                                    }))
                                } else {
                                    Err(PyExc::attribute_error("date has no utcoffset"))
                                }
                            }
                            "dst" if matches!(value, Temporal::DateTime(_)) => Ok(Value::None),
                            "tzname" => {
                                if let Temporal::DateTime(d) = value {
                                    d.offset_seconds().map_or(Ok(Value::None), |offset| {
                                        Ok(Value::from(
                                            TimezoneObject { offset, name: None }.str()?,
                                        ))
                                    })
                                } else {
                                    Err(PyExc::attribute_error("date has no tzname"))
                                }
                            }
                            _ => Err(PyExc::attribute_error(format!(
                                "date has no attribute '{name}'"
                            ))),
                        }
                    }
                }
            },
        ))
    }
}

fn init_datetime(runtime: &mut Runtime, module: &Value) -> PyResult<()> {
    let date_attrs = Value::dict();
    date_attrs.dict_set(
        Value::from("today"),
        function("date.today", |rt, args| {
            args.reject_keywords()?;
            args.arity(0, 0)?;
            Ok(temporal_value(Temporal::Date(rt.clock.today())))
        }),
    )?;
    for (name, d) in [
        ("min", PyDate::new(1, 1, 1)?),
        ("max", PyDate::new(9999, 12, 31)?),
    ] {
        date_attrs.dict_set(Value::from(name), temporal_value(Temporal::Date(d)))?;
    }
    date_attrs.dict_set(
        Value::from("resolution"),
        temporal_value(Temporal::Delta(PyTimedelta::new(1, 0, 0)?)),
    )?;
    for name in ["fromisoformat", "fromordinal", "fromisocalendar"] {
        date_attrs.dict_set(
            Value::from(name),
            function(&format!("date.{name}"), move |_, args| {
                args.reject_keywords()?;
                args.arity(
                    if name == "fromisocalendar" { 3 } else { 1 },
                    if name == "fromisocalendar" { 3 } else { 1 },
                )?;
                let d = match name {
                    "fromisoformat" => PyDate::fromisoformat(args.positional[0].as_str()?)?,
                    "fromordinal" => PyDate::fromordinal(integer(&args.positional[0])?)?,
                    _ => PyDate::fromisocalendar(
                        integer(&args.positional[0])?,
                        integer(&args.positional[1])?,
                        integer(&args.positional[2])?,
                    )?,
                };
                Ok(temporal_value(Temporal::Date(d)))
            }),
        )?;
    }
    set(
        runtime,
        module,
        "date",
        NativeType::with_attrs(
            "date",
            "datetime",
            date_attrs,
            |v| matches!(temporal(v), Some(Temporal::Date(_) | Temporal::DateTime(_))),
            |_, args| {
                let args = bind(args, &["year", "month", "day"], 3, vec![])?;
                Ok(temporal_value(Temporal::Date(PyDate::new(
                    integer(&args[0])?,
                    integer(&args[1])?,
                    integer(&args[2])?,
                )?)))
            },
        ),
    )?;
    let datetime_attrs = Value::dict();
    for name in ["now", "today"] {
        datetime_attrs.dict_set(
            Value::from(name),
            function(&format!("datetime.{name}"), move |rt, args| {
                let args = if name == "today" {
                    args.reject_keywords()?;
                    args.arity(0, 0)?;
                    vec![Value::None]
                } else {
                    bind(args, &["tz"], 0, vec![Value::None])?
                };
                Ok(temporal_value(Temporal::DateTime(
                    rt.clock.now(timezone_offset(&args[0])?)?,
                )))
            }),
        )?;
    }
    datetime_attrs.dict_set(
        Value::from("fromisoformat"),
        function("datetime.fromisoformat", |_, args| {
            args.reject_keywords()?;
            args.arity(1, 1)?;
            Ok(temporal_value(Temporal::DateTime(
                PyDateTime::fromisoformat(args.positional[0].as_str()?)?,
            )))
        }),
    )?;
    set(
        runtime,
        module,
        "datetime",
        NativeType::with_attrs(
            "datetime",
            "datetime",
            datetime_attrs,
            |v| matches!(temporal(v), Some(Temporal::DateTime(_))),
            |_, args| {
                let args = bind(
                    args,
                    &[
                        "year",
                        "month",
                        "day",
                        "hour",
                        "minute",
                        "second",
                        "microsecond",
                        "tzinfo",
                        "fold",
                    ],
                    3,
                    vec![
                        Value::from(0_i64),
                        Value::from(0_i64),
                        Value::from(0_i64),
                        Value::from(0_i64),
                        Value::None,
                        Value::from(0_i64),
                    ],
                )?;
                if ![0, 1].contains(&integer(&args[8])?) {
                    return Err(PyExc::value_error("fold must be either 0 or 1"));
                }
                let date = PyDate::new(integer(&args[0])?, integer(&args[1])?, integer(&args[2])?)?;
                Ok(temporal_value(Temporal::DateTime(PyDateTime::new(
                    date,
                    integer(&args[3])?,
                    integer(&args[4])?,
                    integer(&args[5])?,
                    integer(&args[6])?,
                    timezone_offset(&args[7])?,
                )?)))
            },
        ),
    )?;
    set(
        runtime,
        module,
        "timedelta",
        NativeType::value(
            "timedelta",
            "datetime",
            |v| matches!(temporal(v), Some(Temporal::Delta(_))),
            |_, args| {
                let args = bind(
                    args,
                    &[
                        "days",
                        "seconds",
                        "microseconds",
                        "milliseconds",
                        "minutes",
                        "hours",
                        "weeks",
                    ],
                    0,
                    vec![Value::from(0_i64); 7],
                )?;
                let delta = if args
                    .iter()
                    .all(|v| matches!(v, Value::Int(_) | Value::Bool(_)))
                {
                    let args = args.iter().map(integer).collect::<PyResult<Vec<_>>>()?;
                    PyTimedelta::from_parts(
                        args[0], args[1], args[2], args[3], args[4], args[5], args[6],
                    )?
                } else {
                    let args = args
                        .iter()
                        .map(Value::as_float)
                        .collect::<PyResult<Vec<_>>>()?;
                    PyTimedelta::from_float_parts(
                        args[0], args[1], args[2], args[3], args[4], args[5], args[6],
                    )?
                };
                Ok(temporal_value(Temporal::Delta(delta)))
            },
        ),
    )?;
    let utc = timezone_value(0, None);
    let timezone_attrs = Value::dict();
    timezone_attrs.dict_set(Value::from("utc"), utc.clone())?;
    set(runtime, module, "UTC", utc)?;
    set(
        runtime,
        module,
        "timezone",
        NativeType::with_attrs(
            "timezone",
            "datetime",
            timezone_attrs,
            |v| matches!(v, Value::Object(o) if o.as_any().is::<TimezoneObject>()),
            |_, args| {
                let args = bind(args, &["offset", "name"], 1, vec![Value::None])?;
                let Some(Temporal::Delta(delta)) = temporal(&args[0]) else {
                    return Err(PyExc::type_error(
                        "timezone() argument 1 must be datetime.timedelta",
                    ));
                };
                let offset = delta.total_microseconds();
                if offset % 1_000_000 != 0 {
                    return Err(PyExc::not_implemented_error(
                        "fractional-second timezone offsets",
                    ));
                }
                let seconds = i32::try_from(offset / 1_000_000).ok().filter(|o| o.abs() < 86400).ok_or_else(|| PyExc::value_error("offset must be strictly between -timedelta(hours=24) and timedelta(hours=24)"))?;
                let name = if matches!(args[1], Value::None) {
                    None
                } else {
                    Some(args[1].as_str()?.to_owned())
                };
                Ok(timezone_value(seconds, name))
            },
        ),
    )?;
    Ok(())
}

#[derive(Clone, Debug)]
struct PatternObject(Rc<crate::regex::CompiledPattern>);

fn pattern_value(compiled: Rc<crate::regex::CompiledPattern>) -> Value {
    Value::Object(Rc::new(PatternObject(compiled)))
}

fn compile(value: &Value, flags: i64) -> PyResult<Rc<crate::regex::CompiledPattern>> {
    if let Value::Object(object) = value
        && let Some(pattern) = object.as_any().downcast_ref::<PatternObject>()
    {
        if flags != 0 {
            return Err(PyExc::value_error(
                "cannot process flags argument with a compiled pattern",
            ));
        }
        return Ok(pattern.0.clone());
    }
    crate::regex::compile_pattern(value.as_str()?, flags)
}

#[derive(Clone, Debug)]
struct MatchObject {
    compiled: Rc<crate::regex::CompiledPattern>,
    text: String,
    data: Json,
    pos: i64,
    endpos: i64,
}

impl MatchObject {
    fn index(&self, value: &Value) -> PyResult<usize> {
        if let Value::Str(name) = value {
            self.compiled
                .group_names()
                .iter()
                .position(|n| n.as_deref() == Some(name.as_str()))
                .map(|i| i + 1)
                .ok_or_else(|| PyExc::index_error("no such group"))
        } else {
            let i = value
                .as_int()
                .ok()
                .and_then(|n| n.to_usize())
                .filter(|n| *n <= self.compiled.group_count())
                .ok_or_else(|| PyExc::index_error("no such group"))?;
            Ok(i)
        }
    }
    fn span(&self, group: usize) -> PyResult<(i64, i64)> {
        let span = if group == 0 {
            &self.data["span"]
        } else {
            &self.data["spans"][group - 1]
        };
        if span.is_null() {
            return Ok((-1, -1));
        }
        Ok((
            span[0]
                .as_i64()
                .ok_or_else(|| PyExc::value_error("invalid match span"))?,
            span[1]
                .as_i64()
                .ok_or_else(|| PyExc::value_error("invalid match span"))?,
        ))
    }
    fn group(&self, group: usize) -> PyResult<Value> {
        if group == 0 {
            let (start, end) = self.span(0)?;
            Ok(Value::from(
                self.text
                    .chars()
                    .skip(start as usize)
                    .take((end - start) as usize)
                    .collect::<String>(),
            ))
        } else {
            from_canon_json(&self.data["groups"][group - 1])
        }
    }
    fn expand(&self, template: &str) -> PyResult<String> {
        let mut chars = template.chars().peekable();
        let mut text = String::new();
        while let Some(c) = chars.next() {
            if c != '\\' {
                text.push(c);
                continue;
            }
            let c = chars.next().ok_or_else(|| PyExc {
                kind: "PatternError",
                message: "bad escape at end of replacement".into(),
            })?;
            let group = match c {
                'g' => {
                    if chars.next() != Some('<') {
                        return Err(PyExc {
                            kind: "PatternError",
                            message: "missing <".into(),
                        });
                    }
                    let mut name = String::new();
                    loop {
                        match chars.next() {
                            Some('>') => break,
                            Some(c) => name.push(c),
                            None => {
                                return Err(PyExc {
                                    kind: "PatternError",
                                    message: "missing >".into(),
                                });
                            }
                        }
                    }
                    Some(if name.chars().all(|c| c.is_ascii_digit()) {
                        self.index(&Value::Int(
                            name.parse()
                                .map_err(|_| PyExc::index_error("no such group"))?,
                        ))?
                    } else {
                        self.index(&Value::from(name))?
                    })
                }
                '1'..='9' => {
                    let mut number = c.to_digit(10).unwrap() as usize;
                    if chars.peek().is_some_and(char::is_ascii_digit) {
                        number = number * 10 + chars.next().unwrap().to_digit(10).unwrap() as usize;
                    }
                    if number > self.compiled.group_count() {
                        return Err(PyExc {
                            kind: "PatternError",
                            message: "invalid group reference".into(),
                        });
                    }
                    Some(number)
                }
                'n' => {
                    text.push('\n');
                    None
                }
                't' => {
                    text.push('\t');
                    None
                }
                'r' => {
                    text.push('\r');
                    None
                }
                'f' => {
                    text.push('\u{c}');
                    None
                }
                'v' => {
                    text.push('\u{b}');
                    None
                }
                'a' => {
                    text.push('\u{7}');
                    None
                }
                'b' => {
                    text.push('\u{8}');
                    None
                }
                '\\' => {
                    text.push('\\');
                    None
                }
                c if c.is_ascii_alphabetic() => {
                    return Err(PyExc {
                        kind: "PatternError",
                        message: format!("bad escape \\{c}"),
                    });
                }
                c => {
                    text.push('\\');
                    text.push(c);
                    None
                }
            };
            if let Some(group) = group {
                let value = self.group(group)?;
                if !matches!(value, Value::None) {
                    text.push_str(value.as_str()?);
                }
            }
        }
        Ok(text)
    }
}

impl PyObject for MatchObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "Match"
    }
    fn repr(&self) -> PyResult<String> {
        let (start, end) = self.span(0)?;
        Ok(format!(
            "<re.Match object; span=({start}, {end}), match={}>",
            crate::formatting::repr(&self.group(0)?)?
        ))
    }
    fn get_item(&self, index: &Value) -> PyResult<Value> {
        self.group(self.index(index)?)
    }
    fn get_attr(&self, _: &mut Runtime, name: &str) -> PyResult<Value> {
        match name {
            "lastindex" | "lastgroup" => return from_canon_json(&self.data[name]),
            "string" => return Ok(Value::from(self.text.clone())),
            "re" => return Ok(pattern_value(self.compiled.clone())),
            "pos" => return Ok(Value::from(self.pos)),
            "endpos" => return Ok(Value::from(self.endpos)),
            "regs" => {
                return Ok(Value::tuple(
                    (0..=self.compiled.group_count())
                        .map(|i| {
                            self.span(i)
                                .map(|(a, b)| Value::tuple(vec![Value::from(a), Value::from(b)]))
                        })
                        .collect::<PyResult<_>>()?,
                ));
            }
            _ => (),
        }
        if ![
            "group",
            "groups",
            "groupdict",
            "span",
            "start",
            "end",
            "expand",
        ]
        .contains(&name)
        {
            return Err(PyExc::attribute_error(format!(
                "Match has no attribute '{name}'"
            )));
        }
        let this = self.clone();
        let name = name.to_owned();
        Ok(function(
            &format!("Match.{name}"),
            move |_, args| match name.as_str() {
                "group" => {
                    args.reject_keywords()?;
                    let values = if args.positional.is_empty() {
                        vec![this.group(0)?]
                    } else {
                        args.positional
                            .iter()
                            .map(|v| this.group(this.index(v)?))
                            .collect::<PyResult<Vec<_>>>()?
                    };
                    Ok(if values.len() == 1 {
                        values[0].clone()
                    } else {
                        Value::tuple(values)
                    })
                }
                "groups" | "groupdict" => {
                    let args = bind(args, &["default"], 0, vec![Value::None])?;
                    let default = &args[0];
                    let replace = |value: Value| {
                        if matches!(value, Value::None) {
                            default.clone()
                        } else {
                            value
                        }
                    };
                    if name == "groups" {
                        Ok(Value::tuple(
                            (1..=this.compiled.group_count())
                                .map(|i| this.group(i).map(&replace))
                                .collect::<PyResult<_>>()?,
                        ))
                    } else {
                        let dict = Value::dict();
                        for (i, name) in this.compiled.group_names().iter().enumerate() {
                            if let Some(name) = name {
                                dict.dict_set(
                                    Value::from(name.clone()),
                                    replace(this.group(i + 1)?),
                                )?;
                            }
                        }
                        Ok(dict)
                    }
                }
                "expand" => {
                    args.reject_keywords()?;
                    args.arity(1, 1)?;
                    Ok(Value::from(this.expand(args.positional[0].as_str()?)?))
                }
                _ => {
                    args.reject_keywords()?;
                    args.arity(0, 1)?;
                    let group = args.positional.first().map_or(Ok(0), |g| this.index(g))?;
                    let (start, end) = this.span(group)?;
                    Ok(match name.as_str() {
                        "start" => Value::from(start),
                        "end" => Value::from(end),
                        _ => Value::tuple(vec![Value::from(start), Value::from(end)]),
                    })
                }
            },
        ))
    }
}

fn match_value(
    compiled: Rc<crate::regex::CompiledPattern>,
    text: &str,
    json: Json,
    pos: i64,
    endpos: i64,
) -> PyResult<Value> {
    if json.is_null() {
        return Ok(Value::None);
    }
    let data = json
        .get("$match")
        .cloned()
        .ok_or_else(|| PyExc::value_error("missing match data"))?;
    Ok(Value::Object(Rc::new(MatchObject {
        compiled,
        text: text.into(),
        data,
        pos,
        endpos,
    })))
}

fn pattern_method(compiled: Rc<crate::regex::CompiledPattern>, name: &str) -> Value {
    let name = name.to_owned();
    function(&format!("Pattern.{name}"), move |runtime, args| {
        if name == "sub" || name == "subn" {
            let args = bind(
                args,
                &["repl", "string", "count"],
                2,
                vec![Value::from(0_i64)],
            )?;
            let text = args[1].as_str()?;
            let count = integer(&args[2])?;
            if matches!(&args[0], Value::Object(o) if o.is_callable()) {
                let end = text.chars().count() as i64;
                let (output, replaced) = crate::regex::sub_with(&compiled, text, count, |json| {
                    let matched = match_value(compiled.clone(), text, json, 0, end)?;
                    let value = runtime.call(&args[0], CallArgs::new(vec![matched]))?;
                    if matches!(value, Value::None) {
                        Ok(None)
                    } else {
                        Ok(Some(value.as_str()?.into()))
                    }
                })?;
                return Ok(if name == "sub" {
                    Value::from(output)
                } else {
                    Value::tuple(vec![Value::from(output), Value::from(replaced)])
                });
            }
            let json_args = args.iter().map(canon_json).collect::<PyResult<Vec<_>>>()?;
            return from_canon_json(&crate::regex::run_compiled(&compiled, &name, &json_args)?);
        }
        if name == "split" {
            let args = bind(args, &["string", "maxsplit"], 1, vec![Value::from(0_i64)])?;
            return from_canon_json(&crate::regex::run_compiled(
                &compiled,
                &name,
                &args.iter().map(canon_json).collect::<PyResult<Vec<_>>>()?,
            )?);
        }
        let args = bind(
            args,
            &["string", "pos", "endpos"],
            1,
            vec![Value::from(0_i64), Value::None],
        )?;
        let text = args[0].as_str()?;
        let length = text.chars().count() as i64;
        let pos = integer(&args[1])?.clamp(0, length);
        let end = if matches!(args[2], Value::None) {
            length
        } else {
            integer(&args[2])?.clamp(0, length)
        };
        let result =
            crate::regex::run_compiled(&compiled, &name, &[json!(text), json!(pos), json!(end)])?;
        if name == "findall" {
            return from_canon_json(&result);
        }
        if name == "finditer" {
            let values = result
                .as_array()
                .ok_or_else(|| PyExc::value_error("invalid match iteration"))?
                .iter()
                .map(|v| match_value(compiled.clone(), text, v.clone(), pos, end))
                .collect::<PyResult<Vec<_>>>()?;
            return Ok(IteratorObject::from_values(values));
        }
        match_value(compiled.clone(), text, result, pos, end)
    })
}

impl PyObject for PatternObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "Pattern"
    }
    fn repr(&self) -> PyResult<String> {
        let flags = [
            (1, "TEMPLATE"),
            (2, "IGNORECASE"),
            (4, "LOCALE"),
            (8, "MULTILINE"),
            (16, "DOTALL"),
            (64, "VERBOSE"),
            (128, "DEBUG"),
            (256, "ASCII"),
        ]
        .iter()
        .filter(|(flag, _)| self.0.flags() & flag != 0)
        .map(|(_, name)| format!("re.{name}"))
        .collect::<Vec<_>>()
        .join("|");
        Ok(format!(
            "re.compile({}{})",
            crate::formatting::repr(&Value::from(self.0.pattern()))?,
            if flags.is_empty() {
                String::new()
            } else {
                format!(", {flags}")
            }
        ))
    }
    fn equals(&self, other: &Value) -> PyResult<bool> {
        Ok(
            matches!(other, Value::Object(o) if o.as_any().downcast_ref::<Self>().is_some_and(|p| p.0.pattern() == self.0.pattern() && p.0.flags() == self.0.flags())),
        )
    }
    fn get_attr(&self, _: &mut Runtime, name: &str) -> PyResult<Value> {
        match name {
            "pattern" => Ok(Value::from(self.0.pattern())),
            "flags" => Ok(Value::from(self.0.flags())),
            "groups" => Ok(Value::from(self.0.group_count())),
            "groupindex" => {
                let dict = Value::dict();
                for (i, name) in self.0.group_names().iter().enumerate() {
                    if let Some(name) = name {
                        dict.dict_set(Value::from(name.clone()), Value::from(i + 1))?;
                    }
                }
                Ok(dict)
            }
            name if [
                "match",
                "search",
                "fullmatch",
                "finditer",
                "findall",
                "split",
                "sub",
                "subn",
            ]
            .contains(&name) =>
            {
                Ok(pattern_method(self.0.clone(), name))
            }
            _ => Err(PyExc::attribute_error(format!(
                "Pattern has no attribute '{name}'"
            ))),
        }
    }
}

fn init_re(runtime: &mut Runtime, module: &Value) -> PyResult<()> {
    for (name, flag) in [
        ("NOFLAG", 0),
        ("T", 1),
        ("TEMPLATE", 1),
        ("I", 2),
        ("IGNORECASE", 2),
        ("L", 4),
        ("LOCALE", 4),
        ("M", 8),
        ("MULTILINE", 8),
        ("S", 16),
        ("DOTALL", 16),
        ("U", 32),
        ("UNICODE", 32),
        ("X", 64),
        ("VERBOSE", 64),
        ("DEBUG", 128),
        ("A", 256),
        ("ASCII", 256),
    ] {
        set(runtime, module, name, Value::from(flag as i64))?;
    }
    let error = NativeType::value(
        "PatternError",
        "re",
        |v| v.type_name() == "PatternError",
        |_, args| {
            args.reject_keywords()?;
            args.arity(1, 3)?;
            Ok(Value::Opaque {
                text: args.positional[0].as_str()?.into(),
                type_name: "PatternError".into(),
            })
        },
    );
    set(runtime, module, "error", error.clone())?;
    set(runtime, module, "PatternError", error)?;
    set(
        runtime,
        module,
        "Pattern",
        NativeType::value(
            "Pattern",
            "re",
            |v| matches!(v, Value::Object(o) if o.as_any().is::<PatternObject>()),
            |_, _| Err(PyExc::type_error("cannot create 're.Pattern' instances")),
        ),
    )?;
    set(
        runtime,
        module,
        "Match",
        NativeType::value(
            "Match",
            "re",
            |v| matches!(v, Value::Object(o) if o.as_any().is::<MatchObject>()),
            |_, _| Err(PyExc::type_error("cannot create 're.Match' instances")),
        ),
    )?;
    set(
        runtime,
        module,
        "escape",
        function("re.escape", |_, args| {
            let args = bind(args, &["pattern"], 1, vec![])?;
            let mut escaped = String::new();
            for c in args[0].as_str()?.chars() {
                if "()[]{}?*+-|^$\\.&~# \t\n\r\x0b\x0c".contains(c) {
                    escaped.push('\\');
                }
                escaped.push(c);
            }
            Ok(Value::from(escaped))
        }),
    )?;
    set(
        runtime,
        module,
        "compile",
        function("re.compile", |_, args| {
            let args = bind(args, &["pattern", "flags"], 1, vec![Value::from(0_i64)])?;
            if matches!(&args[0], Value::Object(o) if o.as_any().is::<PatternObject>())
                && integer(&args[1])? == 0
            {
                return Ok(args[0].clone());
            }
            Ok(pattern_value(compile(&args[0], integer(&args[1])?)?))
        }),
    )?;
    for name in [
        "match",
        "search",
        "fullmatch",
        "findall",
        "finditer",
        "split",
        "sub",
        "subn",
    ] {
        set(
            runtime,
            module,
            name,
            function(&format!("re.{name}"), move |runtime, args| {
                let (names, required, defaults) = match name {
                    "sub" | "subn" => (
                        &["pattern", "repl", "string", "count", "flags"][..],
                        3,
                        vec![Value::from(0_i64), Value::from(0_i64)],
                    ),
                    "split" => (
                        &["pattern", "string", "maxsplit", "flags"][..],
                        2,
                        vec![Value::from(0_i64), Value::from(0_i64)],
                    ),
                    _ => (
                        &["pattern", "string", "flags"][..],
                        2,
                        vec![Value::from(0_i64)],
                    ),
                };
                let args = bind(args, names, required, defaults)?;
                let compiled = compile(&args[0], integer(args.last().unwrap())?)?;
                let forwarded = args[1..args.len() - 1].to_vec();
                runtime.call(&pattern_method(compiled, name), CallArgs::new(forwarded))
            }),
        )?;
    }
    Ok(())
}
