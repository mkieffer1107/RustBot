//! Builtins operate on the shared runtime and preserve iterator short-circuiting.
use crate::{
    PyExc, PyResult, Value, formatting,
    object::{CallArgs, IteratorObject, NativeFunction, NativeType},
    ops, protocols,
    runtime::Runtime,
    sets::PySet,
};
use num_bigint::BigInt;
use num_traits::{FromPrimitive, Signed, ToPrimitive};
use std::cmp::Ordering;

pub fn install(rt: &mut Runtime) {
    rt.register_builtin(
        "NoneType",
        NativeType::value(
            "NoneType",
            "builtins",
            |v| matches!(v, Value::None),
            |_, args| {
                args.reject_keywords()?;
                args.arity(0, 0)?;
                Ok(Value::None)
            },
        ),
    );
    for name in [
        "bool",
        "int",
        "float",
        "str",
        "list",
        "tuple",
        "dict",
        "set",
        "frozenset",
        "range",
        "object",
        "type",
    ] {
        let kind = name.to_owned();
        let ctor = kind.clone();
        let value = NativeType::value(
            name,
            "builtins",
            move |v| instance_of(v, &kind),
            move |rt, args| call(rt, &ctor, args),
        );
        rt.register_builtin(name, value);
    }
    for name in [
        "len",
        "repr",
        "ascii",
        "format",
        "hash",
        "abs",
        "all",
        "any",
        "callable",
        "enumerate",
        "zip",
        "map",
        "iter",
        "next",
        "reversed",
        "sorted",
        "min",
        "max",
        "sum",
        "round",
        "divmod",
        "pow",
        "ord",
        "chr",
        "isinstance",
        "issubclass",
        "getattr",
        "hasattr",
        "setattr",
        "id",
        "vars",
        "print",
    ] {
        rt.register_builtin(
            name,
            NativeFunction::value(name, move |rt, args| call(rt, name, args)),
        );
    }
    rt.register_builtin("None", Value::None);
    rt.register_builtin("True", Value::Bool(true));
    rt.register_builtin("False", Value::Bool(false));
    for (name, receiver) in [
        ("str", Value::from("")),
        ("list", Value::list(vec![])),
        ("tuple", Value::tuple(vec![])),
        ("dict", Value::dict()),
        ("set", Value::Set(PySet::new())),
        ("frozenset", Value::FrozenSet(PySet::new())),
        ("float", Value::Float(0.0)),
    ] {
        if let Ok(Value::Object(t)) = rt.builtin(name)
            && let Some(t) = t.as_any().downcast_ref::<NativeType>()
        {
            for method in crate::runtime::method_names(&receiver) {
                let method = (*method).to_owned();
                let assigned = method.clone();
                let expected = name.to_owned();
                t.attrs
                    .dict_set(
                        Value::from(assigned.clone()),
                        NativeFunction::value(assigned, move |rt, mut args| {
                            args.arity(1, usize::MAX)?;
                            let receiver = args.positional.remove(0);
                            if !instance_of(&receiver, &expected) {
                                return Err(PyExc::type_error(
                                    "descriptor does not apply to this type",
                                ));
                            }
                            let method = rt.getattr(&receiver, &method)?;
                            rt.call(&method, args)
                        }),
                    )
                    .expect("type attributes are strings");
            }
        }
    }
    install_type_attr(rt, "str", "maketrans", |_, args| {
        args.reject_keywords()?;
        crate::strings::maketrans(&args.positional)
    });
    install_type_attr(rt, "dict", "fromkeys", |rt, args| {
        args.reject_keywords()?;
        args.arity(1, 2)?;
        let out = Value::dict();
        let value = args.positional.get(1).cloned().unwrap_or(Value::None);
        for key in rt.collect(&args.positional[0])? {
            out.dict_set(key, value.clone())?;
        }
        Ok(out)
    });
    install_type_attr(rt, "int", "from_bytes", |rt, mut args| {
        let order = keyword(&mut args, "byteorder");
        let signed = keyword(&mut args, "signed").is_some_and(|v| v.is_truthy());
        args.reject_keywords()?;
        args.arity(1, 2)?;
        if order.is_some() && args.positional.len() == 2 {
            return Err(PyExc::type_error("multiple values for byteorder"));
        }
        let order = order
            .as_ref()
            .or(args.positional.get(1))
            .map(Value::as_str)
            .transpose()?
            .unwrap_or("big");
        let data = if let Ok(bytes) = crate::bytes::bytes(&args.positional[0]) {
            bytes
        } else {
            rt.collect(&args.positional[0])?
                .iter()
                .map(|v| {
                    v.as_int()?
                        .to_u8()
                        .ok_or_else(|| PyExc::value_error("bytes must be in range(0, 256)"))
                })
                .collect::<PyResult<Vec<_>>>()?
        };
        let n = match (order, signed) {
            ("big", true) => BigInt::from_signed_bytes_be(&data),
            ("little", true) => BigInt::from_signed_bytes_le(&data),
            ("big", false) => BigInt::from_bytes_be(num_bigint::Sign::Plus, &data),
            ("little", false) => BigInt::from_bytes_le(num_bigint::Sign::Plus, &data),
            _ => {
                return Err(PyExc::value_error(
                    "byteorder must be either 'little' or 'big'",
                ));
            }
        };
        Ok(Value::Int(n))
    });
}

fn install_type_attr(
    rt: &mut Runtime,
    class: &str,
    name: &str,
    body: impl Fn(&mut Runtime, CallArgs) -> PyResult<Value> + 'static,
) {
    if let Ok(Value::Object(t)) = rt.builtin(class)
        && let Some(t) = t.as_any().downcast_ref::<NativeType>()
    {
        t.attrs
            .dict_set(Value::from(name), NativeFunction::value(name, body))
            .expect("type attribute key");
    }
}

pub fn instance_of(value: &Value, name: &str) -> bool {
    name == "object"
        || value.type_name() == name
        || name == "int" && matches!(value, Value::Bool(_))
        || name == "date" && matches!(value, Value::DateTime(_))
}

pub fn len(value: &Value) -> PyResult<usize> {
    let length = match value {
        Value::Str(s) => s.len(),
        Value::List(v) => v.borrow().len(),
        Value::Tuple(v) => v.len(),
        Value::Dict(v) => v.borrow().len(),
        Value::Set(v) | Value::FrozenSet(v) => v.len(),
        Value::Object(o) => o.len()?,
        _ => {
            return Err(PyExc::type_error(format!(
                "object of type '{}' has no len()",
                value.type_name()
            )));
        }
    };
    if length > isize::MAX as usize {
        Err(PyExc::overflow_error(
            "Python int too large to convert to C ssize_t",
        ))
    } else {
        Ok(length)
    }
}

fn decimal(c: char) -> Option<u32> {
    if c.is_ascii_digit() {
        return c.to_digit(10);
    }
    if !crate::unicode_data::property(c, 1 << 2) {
        return None;
    }
    let mut start = c as u32;
    while start > 0
        && char::from_u32(start - 1).is_some_and(|c| crate::unicode_data::property(c, 1 << 2))
    {
        start -= 1;
    }
    Some((c as u32 - start) % 10)
}

fn number_text(text: &str) -> String {
    text.chars()
        .map(|c| decimal(c).map_or(c, |n| char::from_digit(n, 10).unwrap()))
        .collect()
}

pub fn int(value: &Value, base: Option<&Value>) -> PyResult<Value> {
    if base.is_none() {
        match value {
            Value::Int(n) => return Ok(Value::Int(n.clone())),
            Value::Bool(b) => return Ok(Value::from(i64::from(*b))),
            Value::Float(x) => {
                if x.is_nan() {
                    return Err(PyExc::value_error("cannot convert float NaN to integer"));
                }
                if x.is_infinite() {
                    return Err(PyExc::overflow_error(
                        "cannot convert float infinity to integer",
                    ));
                }
                return Ok(Value::Int(BigInt::from_f64(x.trunc()).unwrap()));
            }
            _ => {}
        }
    }
    let s = number_text(value.as_str()?).trim().to_owned();
    let (negative, raw) = if let Some(s) = s.strip_prefix('-') {
        (true, s)
    } else {
        (false, s.strip_prefix('+').unwrap_or(&s))
    };
    let base = base
        .map(Value::as_int)
        .transpose()?
        .map(|n| {
            n.to_u32()
                .ok_or_else(|| PyExc::value_error("int() base must be >= 2 and <= 36, or 0"))
        })
        .transpose()?
        .unwrap_or(10);
    if base != 0 && !(2..=36).contains(&base) {
        return Err(PyExc::value_error(
            "int() base must be >= 2 and <= 36, or 0",
        ));
    }
    let detected = if raw.starts_with("0x") || raw.starts_with("0X") {
        16
    } else if raw.starts_with("0o") || raw.starts_with("0O") {
        8
    } else if raw.starts_with("0b") || raw.starts_with("0B") {
        2
    } else {
        10
    };
    let radix = if base == 0 { detected } else { base };
    let prefix = raw.len() >= 2 && detected != 10 && radix == detected;
    let digits = if prefix { &raw[2..] } else { raw };
    let invalid = || PyExc::value_error("invalid literal for int()");
    let chars: Vec<char> = digits.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if *c == '_' {
            if (i == 0 && !prefix)
                || i + 1 == chars.len()
                || i > 0 && chars[i - 1] == '_'
                || !chars[i + 1].is_digit(radix)
            {
                return Err(invalid());
            }
        } else if !c.is_digit(radix) {
            return Err(invalid());
        }
    }
    let cleaned = digits.replace('_', "");
    if base == 0 && !prefix && cleaned.starts_with('0') && cleaned.chars().any(|c| c != '0') {
        return Err(invalid());
    }
    let n = BigInt::parse_bytes(cleaned.as_bytes(), radix).ok_or_else(invalid)?;
    Ok(Value::Int(if negative { -n } else { n }))
}

pub fn float(value: &Value) -> PyResult<Value> {
    if let Value::Str(s) = value {
        let text = number_text(s.as_str());
        let text = text.trim();
        let chars: Vec<char> = text.chars().collect();
        for (i, c) in chars.iter().enumerate() {
            if *c == '_'
                && (i == 0
                    || i + 1 == chars.len()
                    || !chars[i - 1].is_ascii_digit()
                    || !chars[i + 1].is_ascii_digit())
            {
                return Err(PyExc::value_error("could not convert string to float"));
            }
        }
        let normalized = text.replace('_', "").to_ascii_lowercase();
        let x = match normalized.as_str() {
            "inf" | "+inf" | "infinity" | "+infinity" => f64::INFINITY,
            "-inf" | "-infinity" => f64::NEG_INFINITY,
            "nan" | "+nan" => f64::NAN,
            "-nan" => -f64::NAN,
            _ => normalized
                .parse()
                .map_err(|_| PyExc::value_error("could not convert string to float"))?,
        };
        Ok(Value::Float(x))
    } else {
        Ok(Value::Float(value.as_float()?))
    }
}

fn isinstance(value: &Value, types: &Value) -> PyResult<bool> {
    if let Value::Tuple(types) = types {
        for t in types.iter() {
            if isinstance(value, t)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if let Value::Object(o) = types
        && let Some(result) = o.instance_check(value)
    {
        return Ok(result);
    }
    Err(PyExc::type_error(
        "isinstance() arg 2 must be a type, a tuple of types, or a union",
    ))
}

fn keyword(args: &mut CallArgs, name: &str) -> Option<Value> {
    args.keywords
        .iter()
        .position(|(n, _)| n == name)
        .map(|i| args.keywords.remove(i).1)
}

pub fn sort(
    rt: &mut Runtime,
    mut values: Vec<Value>,
    key: Option<Value>,
    reverse: bool,
) -> PyResult<Vec<Value>> {
    let mut decorated = Vec::with_capacity(values.len());
    for value in values.drain(..) {
        let sort_key = if let Some(key) = &key {
            if matches!(key, Value::None) {
                value.clone()
            } else {
                rt.call(key, CallArgs::new(vec![value.clone()]))?
            }
        } else {
            value.clone()
        };
        decorated.push((sort_key, value));
    }
    let mut error = None;
    decorated.sort_by(|(a, _), (b, _)| match ops::compare(a, b) {
        Ok(order) => {
            let order = order.unwrap_or(Ordering::Equal);
            if reverse { order.reverse() } else { order }
        }
        Err(e) => {
            if error.is_none() {
                error = Some(e);
            }
            Ordering::Equal
        }
    });
    if let Some(error) = error {
        return Err(error);
    }
    Ok(decorated.into_iter().map(|(_, v)| v).collect())
}

pub fn call(rt: &mut Runtime, name: &str, mut args: CallArgs) -> PyResult<Value> {
    match name {
        "bool" => {
            args.reject_keywords()?;
            args.arity(0, 1)?;
            Ok(Value::Bool(
                args.positional.first().is_some_and(Value::is_truthy),
            ))
        }
        "int" => {
            let base = keyword(&mut args, "base");
            args.reject_keywords()?;
            args.arity(0, 2)?;
            if base.is_some() && args.positional.len() == 2 {
                return Err(PyExc::type_error("multiple values for base"));
            }
            if args.positional.is_empty() {
                if base.is_some() {
                    return Err(PyExc::type_error("int() missing string argument"));
                }
                Ok(Value::from(0_i64))
            } else {
                int(
                    &args.positional[0],
                    base.as_ref().or(args.positional.get(1)),
                )
            }
        }
        "float" => {
            args.reject_keywords()?;
            args.arity(0, 1)?;
            args.positional.first().map_or(Ok(Value::Float(0.0)), float)
        }
        "str" | "repr" => {
            args.reject_keywords()?;
            args.arity(usize::from(name == "repr"), 1)?;
            Ok(Value::from(if let Some(value) = args.positional.first() {
                if name == "repr" {
                    formatting::repr(value)?
                } else {
                    formatting::str(value)?
                }
            } else {
                String::new()
            }))
        }
        "ascii" => {
            args.reject_keywords()?;
            args.arity(1, 1)?;
            let text = formatting::repr(&args.positional[0])?;
            Ok(Value::from(
                text.chars()
                    .map(|c| {
                        if c.is_ascii() {
                            c.to_string()
                        } else if c as u32 <= 255 {
                            format!("\\x{:02x}", c as u32)
                        } else if c as u32 <= 65535 {
                            format!("\\u{:04x}", c as u32)
                        } else {
                            format!("\\U{:08x}", c as u32)
                        }
                    })
                    .collect::<String>(),
            ))
        }
        "list" | "tuple" | "set" | "frozenset" => {
            args.reject_keywords()?;
            args.arity(0, 1)?;
            let values = if let Some(value) = args.positional.first() {
                rt.collect(value)?
            } else {
                Vec::new()
            };
            Ok(match name {
                "list" => Value::list(values),
                "tuple" => Value::tuple(values),
                "set" => Value::Set(PySet::from_values(values)?),
                _ => Value::FrozenSet(PySet::from_values(values)?),
            })
        }
        "dict" => {
            args.arity(0, 1)?;
            let out = Value::dict();
            if let Some(value) = args.positional.first() {
                if let Value::Dict(items) = value {
                    for (k, v) in items.borrow().iter() {
                        out.dict_set(k.clone(), v.clone())?;
                    }
                } else {
                    for (index, pair) in rt.collect(value)?.into_iter().enumerate() {
                        let pair = rt.collect(&pair)?;
                        if pair.len() != 2 {
                            return Err(PyExc::value_error(format!(
                                "dictionary update sequence element #{index} has length {}; 2 is required",
                                pair.len()
                            )));
                        }
                        out.dict_set(pair[0].clone(), pair[1].clone())?;
                    }
                }
            }
            for (name, value) in args.keywords {
                out.dict_set(Value::from(name), value)?;
            }
            Ok(out)
        }
        "range" => {
            args.reject_keywords()?;
            args.arity(1, 3)?;
            let a = args.positional[0].as_int()?;
            let (start, stop) = if args.positional.len() == 1 {
                (0.into(), a)
            } else {
                (a, args.positional[1].as_int()?)
            };
            let step = args
                .positional
                .get(2)
                .map(Value::as_int)
                .transpose()?
                .unwrap_or_else(|| 1.into());
            protocols::range(start, stop, step)
        }
        "len" => {
            args.reject_keywords()?;
            args.arity(1, 1)?;
            Ok(Value::from(len(&args.positional[0])?))
        }
        "hash" => {
            args.reject_keywords()?;
            args.arity(1, 1)?;
            let h = if let Value::Object(o) = &args.positional[0] {
                o.hash()?
            } else {
                crate::sets::hash(&args.positional[0])?
            };
            Ok(Value::from(h))
        }
        "abs" => {
            args.reject_keywords()?;
            args.arity(1, 1)?;
            match &args.positional[0] {
                Value::Int(n) => Ok(Value::Int(n.abs())),
                Value::Bool(b) => Ok(Value::from(i64::from(*b))),
                Value::Float(x) => Ok(Value::Float(x.abs())),
                _ => Err(PyExc::type_error("bad operand type for abs()")),
            }
        }
        "format" => {
            args.reject_keywords()?;
            args.arity(1, 2)?;
            Ok(Value::from(formatting::format(
                &args.positional[0],
                args.positional
                    .get(1)
                    .map(Value::as_str)
                    .transpose()?
                    .unwrap_or(""),
            )?))
        }
        "round" => {
            let ndigits = keyword(&mut args, "ndigits");
            let number = keyword(&mut args, "number");
            args.reject_keywords()?;
            if let Some(number) = number {
                if !args.positional.is_empty() {
                    return Err(PyExc::type_error("multiple values for number"));
                }
                args.positional.push(number);
            }
            args.arity(1, 2)?;
            if ndigits.is_some() && args.positional.len() == 2 {
                return Err(PyExc::type_error("multiple values for ndigits"));
            }
            crate::math::round(
                &args.positional[0],
                ndigits.as_ref().or(args.positional.get(1)),
            )
        }
        "pow" => {
            args.reject_keywords()?;
            args.arity(2, 3)?;
            if let Some(modulus) = args.positional.get(2) {
                if matches!(modulus, Value::None) {
                    ops::power(&args.positional[0], &args.positional[1])
                } else {
                    ops::pow_mod(&args.positional[0], &args.positional[1], modulus)
                }
            } else {
                ops::power(&args.positional[0], &args.positional[1])
            }
        }
        "divmod" => {
            args.reject_keywords()?;
            args.arity(2, 2)?;
            ops::divmod(&args.positional[0], &args.positional[1])
        }
        "ord" => {
            args.reject_keywords()?;
            args.arity(1, 1)?;
            let s = args.positional[0].as_str()?;
            let mut chars = s.chars();
            let c = chars
                .next()
                .filter(|_| chars.next().is_none())
                .ok_or_else(|| PyExc::type_error("ord() expected a character"))?;
            Ok(Value::from(c as i64))
        }
        "chr" => {
            args.reject_keywords()?;
            args.arity(1, 1)?;
            let n = args.positional[0]
                .as_int()?
                .to_u32()
                .ok_or_else(|| PyExc::value_error("chr() arg not in range(0x110000)"))?;
            Ok(Value::from(
                char::from_u32(n)
                    .ok_or_else(|| PyExc::value_error("chr() code point is not supported"))?
                    .to_string(),
            ))
        }
        "callable" => {
            args.reject_keywords()?;
            args.arity(1, 1)?;
            Ok(Value::Bool(
                matches!(&args.positional[0],Value::Object(o) if o.is_callable()),
            ))
        }
        "isinstance" => {
            args.reject_keywords()?;
            args.arity(2, 2)?;
            Ok(Value::Bool(isinstance(
                &args.positional[0],
                &args.positional[1],
            )?))
        }
        "issubclass" => {
            args.reject_keywords()?;
            args.arity(2, 2)?;
            Ok(Value::Bool(issubclass(
                &args.positional[0],
                &args.positional[1],
            )?))
        }
        "type" => {
            args.reject_keywords()?;
            args.arity(1, 1)?;
            rt.builtin(args.positional[0].type_name())
        }
        "getattr" | "hasattr" => {
            args.reject_keywords()?;
            args.arity(2, if name == "hasattr" { 2 } else { 3 })?;
            match rt.getattr(&args.positional[0], args.positional[1].as_str()?) {
                Ok(v) => Ok(if name == "hasattr" {
                    Value::Bool(true)
                } else {
                    v
                }),
                Err(e) if e.kind == "AttributeError" => {
                    if name == "hasattr" {
                        Ok(Value::Bool(false))
                    } else if let Some(default) = args.positional.get(2) {
                        Ok(default.clone())
                    } else {
                        Err(e)
                    }
                }
                Err(e) => Err(e),
            }
        }
        "setattr" => {
            args.reject_keywords()?;
            args.arity(3, 3)?;
            rt.setattr(
                &args.positional[0],
                args.positional[1].as_str()?,
                args.positional[2].clone(),
            )?;
            Ok(Value::None)
        }
        "vars" => {
            args.reject_keywords()?;
            args.arity(1, 1)?;
            rt.getattr(&args.positional[0], "__dict__")
        }
        "id" => {
            args.reject_keywords()?;
            args.arity(1, 1)?;
            let id = match &args.positional[0] {
                Value::Object(v) => std::rc::Rc::as_ptr(v) as *const () as usize,
                Value::List(v) => std::rc::Rc::as_ptr(v) as usize,
                Value::Dict(v) => std::rc::Rc::as_ptr(v) as usize,
                Value::Tuple(v) => v.as_ptr() as usize,
                Value::None => 0,
                Value::Bool(b) => 1 + usize::from(*b),
                v => v as *const Value as usize,
            };
            Ok(Value::from(id))
        }
        "all" | "any" => {
            args.reject_keywords()?;
            args.arity(1, 1)?;
            let iterator = rt.iter(&args.positional[0])?;
            while let Some(value) = rt.next(&iterator)? {
                if value.is_truthy() == (name == "any") {
                    return Ok(Value::Bool(name == "any"));
                }
            }
            Ok(Value::Bool(name == "all"))
        }
        "iter" => {
            args.reject_keywords()?;
            args.arity(1, 2)?;
            if args.positional.len() == 1 {
                rt.iter(&args.positional[0])
            } else {
                let callable = args.positional[0].clone();
                let sentinel = args.positional[1].clone();
                if !matches!(&callable,Value::Object(o) if o.is_callable()) {
                    return Err(PyExc::type_error("iter(v, w): v must be callable"));
                }
                Ok(IteratorObject::value(move |rt| {
                    let v = rt.call(&callable, CallArgs::default())?;
                    if v.equals(&sentinel)? {
                        Ok(None)
                    } else {
                        Ok(Some(v))
                    }
                }))
            }
        }
        "next" => {
            args.reject_keywords()?;
            args.arity(1, 2)?;
            rt.next(&args.positional[0])?
                .or_else(|| args.positional.get(1).cloned())
                .ok_or_else(|| PyExc::stop_iteration(""))
        }
        "reversed" => {
            args.reject_keywords()?;
            args.arity(1, 1)?;
            protocols::iter(&args.positional[0], true)
        }
        "enumerate" => {
            let start = keyword(&mut args, "start");
            let iterable = keyword(&mut args, "iterable");
            args.reject_keywords()?;
            if let Some(v) = iterable {
                if !args.positional.is_empty() {
                    return Err(PyExc::type_error("multiple values for iterable"));
                }
                args.positional.push(v);
            }
            args.arity(1, 2)?;
            if start.is_some() && args.positional.len() == 2 {
                return Err(PyExc::type_error("multiple values for start"));
            }
            let iterator = rt.iter(&args.positional[0])?;
            let mut index = start
                .as_ref()
                .or(args.positional.get(1))
                .map(Value::as_int)
                .transpose()?
                .unwrap_or_else(|| 0.into());
            Ok(IteratorObject::value(move |rt| {
                Ok(rt.next(&iterator)?.map(|value| {
                    let current = Value::Int(index.clone());
                    index += 1;
                    Value::tuple(vec![current, value])
                }))
            }))
        }
        "zip" | "map" => {
            let strict = keyword(&mut args, "strict").is_some_and(|v| v.is_truthy());
            args.reject_keywords()?;
            let callable = if name == "map" {
                args.arity(2, usize::MAX)?;
                Some(args.positional.remove(0))
            } else {
                None
            };
            let iterators = args
                .positional
                .iter()
                .map(|v| rt.iter(v))
                .collect::<PyResult<Vec<_>>>()?;
            Ok(IteratorObject::value(move |rt| {
                if iterators.is_empty() {
                    return Ok(None);
                }
                let mut row = Vec::with_capacity(iterators.len());
                for (i, iter) in iterators.iter().enumerate() {
                    if let Some(value) = rt.next(iter)? {
                        row.push(value);
                    } else {
                        if strict {
                            if i > 0 {
                                return Err(PyExc::value_error(
                                    "argument is shorter than the preceding arguments",
                                ));
                            }
                            for next in iterators.iter().skip(i + 1) {
                                if rt.next(next)?.is_some() {
                                    return Err(PyExc::value_error(
                                        "argument is longer than the preceding arguments",
                                    ));
                                }
                            }
                        }
                        return Ok(None);
                    }
                }
                if let Some(callable) = &callable {
                    Ok(Some(rt.call(callable, CallArgs::new(row))?))
                } else {
                    Ok(Some(Value::tuple(row)))
                }
            }))
        }
        "sorted" => {
            let key = keyword(&mut args, "key");
            let reverse = keyword(&mut args, "reverse").is_some_and(|v| v.is_truthy());
            args.reject_keywords()?;
            args.arity(1, 1)?;
            let values = rt.collect(&args.positional[0])?;
            Ok(Value::list(sort(rt, values, key, reverse)?))
        }
        "min" | "max" => {
            let key = keyword(&mut args, "key");
            let default = keyword(&mut args, "default");
            args.reject_keywords()?;
            args.arity(1, usize::MAX)?;
            if args.positional.len() > 1 && default.is_some() {
                return Err(PyExc::type_error(
                    "Cannot specify a default with multiple positional arguments",
                ));
            }
            let iter = if args.positional.len() == 1 {
                rt.iter(&args.positional[0])?
            } else {
                IteratorObject::from_values(args.positional)
            };
            let mut chosen: Option<(Value, Value)> = None;
            while let Some(value) = rt.next(&iter)? {
                let k = if let Some(key) = &key {
                    if matches!(key, Value::None) {
                        value.clone()
                    } else {
                        rt.call(key, CallArgs::new(vec![value.clone()]))?
                    }
                } else {
                    value.clone()
                };
                let replace = if let Some((old, _)) = &chosen {
                    ops::compare(&k, old)?.is_some_and(|o| {
                        o == if name == "min" {
                            Ordering::Less
                        } else {
                            Ordering::Greater
                        }
                    })
                } else {
                    true
                };
                if replace {
                    chosen = Some((k, value));
                }
            }
            chosen
                .map(|(_, v)| v)
                .or(default)
                .ok_or_else(|| PyExc::value_error(format!("{name}() iterable argument is empty")))
        }
        "sum" => {
            let start = keyword(&mut args, "start");
            args.reject_keywords()?;
            args.arity(1, 2)?;
            if start.is_some() && args.positional.len() == 2 {
                return Err(PyExc::type_error("multiple values for start"));
            }
            let mut total = start
                .or_else(|| args.positional.get(1).cloned())
                .unwrap_or(Value::from(0_i64));
            if matches!(total, Value::Str(_)) {
                return Err(PyExc::type_error("sum() can't sum strings"));
            }
            let mut compensation = 0.0;
            let iter = rt.iter(&args.positional[0])?;
            while let Some(value) = rt.next(&iter)? {
                if matches!(total, Value::Float(_))
                    && matches!(value, Value::Int(_) | Value::Bool(_) | Value::Float(_))
                {
                    let (a, b) = (total.as_float()?, value.as_float()?);
                    let next = a + b;
                    compensation += if a.abs() >= b.abs() {
                        (a - next) + b
                    } else {
                        (b - next) + a
                    };
                    total = Value::Float(next);
                } else {
                    total = ops::add(&total, &value)?;
                }
            }
            if let Value::Float(v) = total {
                Ok(Value::Float(
                    if compensation != 0.0 && compensation.is_finite() {
                        v + compensation
                    } else {
                        v
                    },
                ))
            } else {
                Ok(total)
            }
        }
        "print" => {
            let sep = keyword(&mut args, "sep").unwrap_or(Value::from(" "));
            let end = keyword(&mut args, "end").unwrap_or(Value::from("\n"));
            args.reject_keywords()?;
            let text = args
                .positional
                .iter()
                .map(formatting::str)
                .collect::<PyResult<Vec<_>>>()?
                .join(sep.as_str()?);
            eprint!("{text}{}", end.as_str()?);
            Ok(Value::None)
        }
        "object" => {
            args.reject_keywords()?;
            args.arity(0, 0)?;
            Ok(Value::Object(std::rc::Rc::new(crate::object::PlainObject)))
        }
        _ => Err(PyExc::not_implemented_error(format!(
            "builtin {name} is not implemented"
        ))),
    }
}

fn issubclass(value: &Value, types: &Value) -> PyResult<bool> {
    let Value::Object(class) = value else {
        return Err(PyExc::type_error("issubclass() arg 1 must be a class"));
    };
    let class = class
        .as_any()
        .downcast_ref::<NativeType>()
        .ok_or_else(|| PyExc::type_error("issubclass() arg 1 must be a class"))?;
    if let Value::Tuple(types) = types {
        for t in types.iter() {
            if issubclass(value, t)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    let Value::Object(base) = types else {
        return Err(PyExc::type_error(
            "issubclass() arg 2 must be a class or tuple of classes",
        ));
    };
    let base = base
        .as_any()
        .downcast_ref::<NativeType>()
        .ok_or_else(|| PyExc::type_error("issubclass() arg 2 must be a class"))?;
    Ok(base.name == "object"
        || base.name == class.name && base.module == class.module
        || class.name == "bool" && base.name == "int"
        || class.name == "datetime" && base.name == "date")
}
