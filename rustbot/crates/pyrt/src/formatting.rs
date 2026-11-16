//! Python representations and the format specifications used by PyBot.

use crate::{PyExc, PyResult, Value, float_repr, from_canon_json};
use num_bigint::BigInt;
use num_traits::{FromPrimitive, Signed, ToPrimitive};
use serde_json::Value as Json;
use std::collections::HashSet;
use std::rc::Rc;

pub fn repr(value: &Value) -> PyResult<String> {
    repr_at(value, &mut HashSet::new(), 0)
}

fn repr_at(value: &Value, active: &mut HashSet<(u8, usize)>, depth: usize) -> PyResult<String> {
    if depth > 200 {
        return Err(PyExc::recursion_error(
            "maximum recursion depth exceeded while getting the repr of an object",
        ));
    }
    Ok(match value {
        Value::None => "None".into(),
        Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        Value::Int(n) => n.to_string(),
        Value::Float(f) => float_repr(*f),
        Value::Str(s) => quote(s.as_str()),
        Value::Opaque { text, .. } => text.clone(),
        Value::Set(items) | Value::FrozenSet(items) => {
            let frozen = matches!(value, Value::FrozenSet(_));
            if items.is_empty() {
                if frozen {
                    "frozenset()".into()
                } else {
                    "set()".into()
                }
            } else {
                let text = items
                    .values()
                    .iter()
                    .map(|v| repr_at(v, active, depth + 1))
                    .collect::<PyResult<Vec<_>>>()?
                    .join(", ");
                if frozen {
                    format!("frozenset({{{text}}})")
                } else {
                    format!("{{{text}}}")
                }
            }
        }
        Value::List(items) => {
            let id = (0, Rc::as_ptr(items) as usize);
            if !active.insert(id) {
                return Ok("[...]".into());
            }
            let text = items
                .borrow()
                .iter()
                .map(|v| repr_at(v, active, depth + 1))
                .collect::<PyResult<Vec<_>>>()?
                .join(", ");
            active.remove(&id);
            format!("[{text}]")
        }
        Value::Tuple(items) => {
            let id = (1, items.as_ptr() as usize);
            if !active.insert(id) {
                return Ok("(...)".into());
            }
            let text = items
                .iter()
                .map(|v| repr_at(v, active, depth + 1))
                .collect::<PyResult<Vec<_>>>()?
                .join(", ");
            active.remove(&id);
            if items.len() == 1 {
                format!("({text},)")
            } else {
                format!("({text})")
            }
        }
        Value::Dict(items) => {
            let id = (2, Rc::as_ptr(items) as usize);
            if !active.insert(id) {
                return Ok("{{...}}".into());
            }
            let text = items
                .borrow()
                .iter()
                .map(|(k, v)| {
                    Ok(format!(
                        "{}: {}",
                        repr_at(k, active, depth + 1)?,
                        repr_at(v, active, depth + 1)?
                    ))
                })
                .collect::<PyResult<Vec<_>>>()?
                .join(", ");
            active.remove(&id);
            format!("{{{text}}}")
        }
        Value::Date(iso) => date_repr(iso, false)?,
        Value::DateTime(iso) => date_repr(iso, true)?,
    })
}

pub fn str(value: &Value) -> PyResult<String> {
    match value {
        Value::Str(s) => Ok(s.as_str().into()),
        Value::Date(s) => Ok(s.clone()),
        Value::DateTime(s) => Ok(s.replacen('T', " ", 1)),
        _ => repr(value),
    }
}

fn quote(text: &str) -> String {
    let delimiter = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::new();
    out.push(delimiter);
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == delimiter => {
                out.push('\\');
                out.push(c);
            }
            c if crate::strings::is_printable_char(c) => out.push(c),
            c => out.push_str(&escape(c)),
        }
    }
    out.push(delimiter);
    out
}

fn escape(c: char) -> String {
    let n = c as u32;
    if n <= 0xff {
        format!("\\x{n:02x}")
    } else if n <= 0xffff {
        format!("\\u{n:04x}")
    } else {
        format!("\\U{n:08x}")
    }
}

fn ascii(value: &Value) -> PyResult<String> {
    Ok(repr(value)?
        .chars()
        .map(|c| {
            if c.is_ascii() {
                c.to_string()
            } else {
                escape(c)
            }
        })
        .collect())
}

fn date_repr(iso: &str, datetime: bool) -> PyResult<String> {
    let date = iso
        .get(..10)
        .ok_or_else(|| PyExc::value_error("invalid ISO date"))?;
    let parts = date
        .split('-')
        .map(|p| p.parse::<u32>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| PyExc::value_error("invalid ISO date"))?;
    if parts.len() != 3 {
        return Err(PyExc::value_error("invalid ISO date"));
    }
    let mut args = parts.iter().map(u32::to_string).collect::<Vec<_>>();
    if datetime {
        let time = iso
            .get(11..)
            .ok_or_else(|| PyExc::value_error("invalid ISO datetime"))?;
        if time.contains('+') || time.ends_with('Z') {
            return Err(PyExc::not_implemented_error("timezone-aware datetime repr"));
        }
        let mut components = time.split(':');
        for _ in 0..2 {
            let n = components
                .next()
                .unwrap_or("0")
                .parse::<u32>()
                .map_err(|_| PyExc::value_error("invalid ISO datetime"))?;
            args.push(n.to_string());
        }
        let (seconds, fraction) = components
            .next()
            .unwrap_or("0")
            .split_once('.')
            .unwrap_or_else(|| (time.rsplit(':').next().unwrap_or("0"), ""));
        let sec = seconds
            .parse::<u32>()
            .map_err(|_| PyExc::value_error("invalid ISO datetime"))?;
        let micro = format!("{fraction:0<6}")
            .parse::<u32>()
            .map_err(|_| PyExc::value_error("invalid ISO datetime"))?;
        if sec != 0 || micro != 0 {
            args.push(sec.to_string());
        }
        if micro != 0 {
            args.push(micro.to_string());
        }
    }
    Ok(format!(
        "datetime.{}({})",
        if datetime { "datetime" } else { "date" },
        args.join(", ")
    ))
}

#[derive(Default, Debug)]
struct Spec {
    fill: Option<char>,
    align: Option<char>,
    sign: Option<char>,
    negative_zero: bool,
    alternate: bool,
    zero: bool,
    width: usize,
    grouping: Option<char>,
    precision: Option<usize>,
    fraction_grouping: Option<char>,
    code: Option<char>,
}

impl Spec {
    fn parse(spec: &str) -> PyResult<Self> {
        let mut out = Self::default();
        let chars: Vec<_> = spec.chars().collect();
        let mut i = 0;
        if chars.get(1).is_some_and(|c| "<>=^".contains(*c)) {
            out.fill = Some(chars[0]);
            out.align = Some(chars[1]);
            i = 2;
        } else if chars.first().is_some_and(|c| "<>=^".contains(*c)) {
            out.align = Some(chars[0]);
            i = 1;
        }
        if chars.get(i).is_some_and(|c| "+- ".contains(*c)) {
            out.sign = Some(chars[i]);
            i += 1;
        }
        if chars.get(i) == Some(&'z') {
            out.negative_zero = true;
            i += 1;
        }
        if chars.get(i) == Some(&'#') {
            out.alternate = true;
            i += 1;
        }
        if chars.get(i) == Some(&'0') {
            out.zero = true;
            i += 1;
        }
        out.width = parse_digits(&chars, &mut i)?.unwrap_or(0);
        if chars.get(i).is_some_and(|c| ",_".contains(*c)) {
            out.grouping = Some(chars[i]);
            i += 1;
        }
        if chars.get(i) == Some(&'.') {
            i += 1;
            out.precision = Some(
                parse_digits(&chars, &mut i)?
                    .ok_or_else(|| PyExc::value_error("Format specifier missing precision"))?,
            );
            if chars.get(i).is_some_and(|c| ",_".contains(*c)) {
                out.fraction_grouping = Some(chars[i]);
                i += 1;
            }
        }
        if i < chars.len() {
            out.code = Some(chars[i]);
            i += 1;
        }
        if i != chars.len() {
            return Err(PyExc::value_error(format!(
                "Invalid format specifier '{spec}'"
            )));
        }
        Ok(out)
    }
}

fn parse_digits(chars: &[char], i: &mut usize) -> PyResult<Option<usize>> {
    let start = *i;
    let mut n = 0_usize;
    while let Some(c) = chars.get(*i).and_then(|c| c.to_digit(10)) {
        n = n
            .checked_mul(10)
            .and_then(|n| n.checked_add(c as usize))
            .ok_or_else(|| PyExc::value_error("Too many decimal digits in format string"))?;
        *i += 1;
    }
    Ok((*i > start).then_some(n))
}

pub fn format(value: &Value, spec: &str) -> PyResult<String> {
    if spec.is_empty() {
        return str(value);
    }
    if let Value::Date(s) | Value::DateTime(s) = value {
        return date_format(s, spec);
    }
    let parsed = Spec::parse(spec)?;
    match value {
        Value::Str(s) => format_string(s.as_str(), &parsed),
        Value::Int(_) | Value::Bool(_) => format_int(&value.as_int()?, &parsed),
        Value::Float(f) => format_float(*f, &parsed),
        _ => Err(PyExc::type_error(format!(
            "unsupported format string passed to {}.__format__",
            value.type_name()
        ))),
    }
}

fn unknown_code(code: char, name: &str) -> PyExc {
    PyExc::value_error(format!(
        "Unknown format code '{code}' for object of type '{name}'"
    ))
}

fn format_string(text: &str, spec: &Spec) -> PyResult<String> {
    if spec.code.is_some_and(|c| c != 's') {
        return Err(unknown_code(spec.code.unwrap(), "str"));
    }
    if spec.sign.is_some() {
        return Err(PyExc::value_error(
            "Sign not allowed in string format specifier",
        ));
    }
    if spec.negative_zero {
        return Err(PyExc::value_error(
            "Negative zero coercion (z) not allowed in string format specifier",
        ));
    }
    if spec.alternate {
        return Err(PyExc::value_error(
            "Alternate form (#) not allowed in string format specifier",
        ));
    }
    if spec.grouping.is_some() || spec.fraction_grouping.is_some() {
        return Err(PyExc::value_error("Cannot specify grouping with 's'."));
    }
    if spec.align == Some('=') {
        return Err(PyExc::value_error(
            "'=' alignment not allowed in string format specifier",
        ));
    }
    let text: String = text
        .chars()
        .take(spec.precision.unwrap_or(usize::MAX))
        .collect();
    pad(&text, "", spec, '<')
}

fn group_digits(digits: &str, separator: char, size: usize) -> String {
    let chars: Vec<_> = digits.chars().collect();
    let mut out = String::new();
    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i).is_multiple_of(size) {
            out.push(separator);
        }
        out.push(*c);
    }
    out
}

fn numeric_prefix(negative: bool, spec: &Spec) -> String {
    if negative {
        "-".into()
    } else if spec.sign == Some('+') {
        "+".into()
    } else if spec.sign == Some(' ') {
        " ".into()
    } else {
        String::new()
    }
}

fn format_int(n: &BigInt, spec: &Spec) -> PyResult<String> {
    let code = spec.code.unwrap_or('d');
    if "eEfFgG%".contains(code) {
        let value = n
            .to_f64()
            .filter(|v| v.is_finite())
            .ok_or_else(|| PyExc::overflow_error("int too large to convert to float"))?;
        return format_float(value, spec);
    }
    if !"dboxXnc".contains(code) {
        return Err(unknown_code(code, "int"));
    }
    if spec.precision.is_some() {
        return Err(PyExc::value_error(
            "Precision not allowed in integer format specifier",
        ));
    }
    if spec.negative_zero {
        return Err(PyExc::value_error(
            "Negative zero coercion (z) not allowed in integer format specifier",
        ));
    }
    if code == 'c' {
        if spec.sign.is_some() || spec.alternate {
            return Err(PyExc::value_error(
                "Sign not allowed with integer format specifier 'c'",
            ));
        }
        let c = n
            .to_u32()
            .and_then(char::from_u32)
            .ok_or_else(|| PyExc::overflow_error("%c arg not in range(0x110000)"))?;
        return pad(&c.to_string(), "", spec, '>');
    }
    let radix = match code {
        'b' => 2,
        'o' => 8,
        'x' | 'X' => 16,
        _ => 10,
    };
    if spec.grouping == Some(',') && radix != 10 {
        return Err(PyExc::value_error(format!(
            "Cannot specify ',' with '{code}'."
        )));
    }
    if code == 'n' && spec.grouping.is_some() {
        return Err(PyExc::value_error("Cannot specify grouping with 'n'."));
    }
    let mut digits = n.abs().to_str_radix(radix);
    if code == 'X' {
        digits.make_ascii_uppercase();
    }
    let mut prefix = numeric_prefix(n.is_negative(), spec);
    if spec.alternate {
        prefix.push_str(match code {
            'b' => "0b",
            'o' => "0o",
            'x' => "0x",
            'X' => "0X",
            _ => "",
        });
    }
    let group_size = if radix == 10 { 3 } else { 4 };
    if let Some(separator) = spec.grouping {
        if spec.zero && spec.fill.is_none() && !matches!(spec.align, Some('<' | '^' | '>')) {
            while prefix.chars().count()
                + group_digits(&digits, separator, group_size).chars().count()
                < spec.width
            {
                digits.insert(0, '0');
            }
        }
        digits = group_digits(&digits, separator, group_size);
    }
    pad(&digits, &prefix, spec, '>')
}

fn exponent_text(text: &str, upper: bool) -> String {
    if let Some((mantissa, exponent)) = text.split_once('e') {
        let exponent: i32 = exponent.parse().expect("Rust exponent");
        format!("{mantissa}{}{exponent:+03}", if upper { 'E' } else { 'e' })
    } else {
        text.into()
    }
}

fn general_float(value: f64, precision: usize, alternate: bool, retain_dot: bool) -> String {
    let precision = precision.max(1);
    let scientific = format!("{:.*e}", precision - 1, value);
    let exponent: i32 = scientific.rsplit_once('e').unwrap().1.parse().unwrap();
    let use_exp = exponent < -4 || exponent >= precision as i32 - i32::from(retain_dot);
    let mut text = if use_exp {
        scientific
    } else {
        format!(
            "{:.*}",
            (precision as i32 - exponent - 1).max(0) as usize,
            value
        )
    };
    let exp = text.find('e');
    let end = exp.unwrap_or(text.len());
    let mut mantissa = text[..end].to_string();
    if !alternate && mantissa.contains('.') {
        while mantissa.ends_with('0') {
            mantissa.pop();
        }
        if mantissa.ends_with('.') {
            if retain_dot && !use_exp {
                mantissa.push('0');
            } else {
                mantissa.pop();
            }
        }
    }
    if alternate && !mantissa.contains('.') {
        mantissa.push('.');
    }
    if retain_dot && !use_exp && !mantissa.contains('.') {
        mantissa.push_str(".0");
    }
    text = format!("{mantissa}{}", exp.map_or("", |i| &text[i..]));
    exponent_text(&text, false)
}

fn format_float(value: f64, spec: &Spec) -> PyResult<String> {
    let code = spec.code.unwrap_or('\0');
    if !"\0eEfFgGn%".contains(code) {
        return Err(unknown_code(code, "float"));
    }
    if code == 'n' && spec.grouping.is_some() {
        return Err(PyExc::value_error("Cannot specify grouping with 'n'."));
    }
    let percent = code == '%';
    let value = if percent { value * 100.0 } else { value };
    let negative = value.is_sign_negative() && !value.is_nan();
    let positive = value.abs();
    let precision = spec.precision.unwrap_or(6);
    // Excessively large precision is rejected before allocating, as Python does
    // for values outside the platform's signed-size range.
    if precision > i32::MAX as usize {
        return Err(PyExc::value_error("precision too big"));
    }
    let mut body = if positive.is_nan() {
        "nan".into()
    } else if positive.is_infinite() {
        "inf".into()
    } else {
        match code {
            'f' | 'F' | '%' => format!("{positive:.precision$}"),
            'e' | 'E' => exponent_text(&format!("{positive:.precision$e}"), false),
            'g' | 'G' | 'n' => general_float(positive, precision, spec.alternate, false),
            _ if spec.precision.is_some() => {
                general_float(positive, precision, spec.alternate, true)
            }
            _ => float_repr(positive),
        }
    };
    if spec.alternate && positive.is_finite() {
        let end = body.find(['e', 'E']).unwrap_or(body.len());
        if !body[..end].contains('.') {
            body.insert(end, '.');
        }
    }
    if "FEG".contains(code) {
        body.make_ascii_uppercase();
    }
    let mut prefix = numeric_prefix(negative, spec);
    if spec.negative_zero && body.parse::<f64>().is_ok_and(|v| v == 0.0) {
        prefix = numeric_prefix(false, spec);
    }
    if let Some(separator) = spec.grouping
        && positive.is_finite()
    {
        let end = body.find(['.', 'e', 'E']).unwrap_or(body.len());
        let mut integer = body[..end].to_string();
        if spec.zero && spec.fill.is_none() && !matches!(spec.align, Some('<' | '^' | '>')) {
            while prefix.len()
                + group_digits(&integer, separator, 3).len()
                + body[end..].len()
                + usize::from(percent)
                < spec.width
            {
                integer.insert(0, '0');
            }
        }
        body = format!("{}{}", group_digits(&integer, separator, 3), &body[end..]);
    }
    if let Some(separator) = spec.fraction_grouping
        && let Some(dot) = body.find('.')
    {
        let end = body.find(['e', 'E']).unwrap_or(body.len());
        let fraction: Vec<_> = body[dot + 1..end].chars().collect();
        let mut grouped = String::new();
        for (i, c) in fraction.iter().enumerate() {
            if i > 0 && i.is_multiple_of(3) {
                grouped.push(separator);
            }
            grouped.push(*c);
        }
        body = format!("{}{}{}", &body[..dot + 1], grouped, &body[end..]);
    }
    if percent {
        body.push('%');
    }
    pad(&body, &prefix, spec, '>')
}

fn pad(body: &str, prefix: &str, spec: &Spec, default_align: char) -> PyResult<String> {
    let len = prefix.chars().count() + body.chars().count();
    let count = spec.width.saturating_sub(len);
    let mut out = String::new();
    out.try_reserve(prefix.len() + body.len() + count.saturating_mul(4))
        .map_err(|_| PyExc {
            kind: "MemoryError",
            message: String::new(),
        })?;
    let fill = spec.fill.unwrap_or(if spec.zero { '0' } else { ' ' });
    let align = spec.align.unwrap_or(if spec.zero && default_align == '>' {
        '='
    } else {
        default_align
    });
    let repeat = |n| std::iter::repeat_n(fill, n).collect::<String>();
    match align {
        '<' => {
            out.push_str(prefix);
            out.push_str(body);
            out.push_str(&repeat(count));
        }
        '^' => {
            out.push_str(&repeat(count / 2));
            out.push_str(prefix);
            out.push_str(body);
            out.push_str(&repeat(count - count / 2));
        }
        '=' => {
            out.push_str(prefix);
            out.push_str(&repeat(count));
            out.push_str(body);
        }
        _ => {
            out.push_str(&repeat(count));
            out.push_str(prefix);
            out.push_str(body);
        }
    }
    Ok(out)
}

fn conversion(value: Value, conv: Option<char>) -> PyResult<Value> {
    match conv {
        None => Ok(value),
        Some('s') => Ok(Value::from(str(&value)?)),
        Some('r') => Ok(Value::from(repr(&value)?)),
        Some('a') => Ok(Value::from(ascii(&value)?)),
        Some(c) => Err(PyExc::value_error(format!(
            "Unknown conversion specifier {c}"
        ))),
    }
}

#[derive(Default)]
struct FieldState {
    automatic: usize,
    mode: Option<bool>,
}

pub fn template(template: &str, args: &[Value], kwargs: &Value) -> PyResult<String> {
    template_at(template, args, kwargs, &mut FieldState::default(), 0)
}

fn template_at(
    text: &str,
    args: &[Value],
    kwargs: &Value,
    state: &mut FieldState,
    depth: usize,
) -> PyResult<String> {
    if depth > 2 {
        return Err(PyExc::value_error("Max string recursion exceeded"));
    }
    let chars: Vec<_> = text.chars().collect();
    let mut i = 0;
    let mut out = String::new();
    while i < chars.len() {
        if matches!(chars[i], '{' | '}') && chars.get(i + 1) == Some(&chars[i]) {
            out.push(chars[i]);
            i += 2;
            continue;
        }
        if chars[i] == '}' {
            return Err(PyExc::value_error(
                "Single '}' encountered in format string",
            ));
        }
        if chars[i] != '{' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        i += 1;
        let start = i;
        let mut nesting = 0;
        let mut bracket = false;
        let (mut conv_at, mut spec_at) = (None, None);
        while i < chars.len() {
            match chars[i] {
                '[' if nesting == 0 && spec_at.is_none() => bracket = true,
                ']' if bracket => bracket = false,
                '{' if !bracket => nesting += 1,
                '}' if !bracket => {
                    if nesting == 0 {
                        break;
                    }
                    nesting -= 1;
                }
                '!' if nesting == 0 && !bracket && conv_at.is_none() && spec_at.is_none() => {
                    conv_at = Some(i)
                }
                ':' if nesting == 0 && !bracket && spec_at.is_none() => spec_at = Some(i),
                _ => (),
            }
            i += 1;
        }
        if i == chars.len() {
            return Err(PyExc::value_error("expected '}' before end of string"));
        }
        let end = conv_at.or(spec_at).unwrap_or(i);
        let field: String = chars[start..end].iter().collect();
        let value = resolve_field(&field, args, kwargs, state)?;
        let conv = if let Some(at) = conv_at {
            let end = spec_at.unwrap_or(i);
            if end != at + 2 {
                return Err(PyExc::value_error(
                    "expected ':' after conversion specifier",
                ));
            }
            Some(chars[at + 1])
        } else {
            None
        };
        let spec = spec_at
            .map(|at| chars[at + 1..i].iter().collect::<String>())
            .unwrap_or_default();
        let spec = template_at(&spec, args, kwargs, state, depth + 1)?;
        out.push_str(&format(&conversion(value, conv)?, &spec)?);
        i += 1;
    }
    Ok(out)
}

fn resolve_field(
    field: &str,
    args: &[Value],
    kwargs: &Value,
    state: &mut FieldState,
) -> PyResult<Value> {
    let split = field.find(['.', '[']).unwrap_or(field.len());
    let first = &field[..split];
    let mut value = if first.is_empty() {
        if state.mode == Some(false) {
            return Err(PyExc::value_error(
                "cannot switch from manual field specification to automatic field numbering",
            ));
        }
        state.mode = Some(true);
        let index = state.automatic;
        state.automatic += 1;
        args.get(index).cloned().ok_or_else(|| {
            PyExc::index_error(format!(
                "Replacement index {index} out of range for positional args tuple"
            ))
        })?
    } else if first.chars().all(|c| c.is_ascii_digit()) {
        if state.mode == Some(true) {
            return Err(PyExc::value_error(
                "cannot switch from automatic field numbering to manual field specification",
            ));
        }
        state.mode = Some(false);
        let index: usize = first
            .parse()
            .map_err(|_| PyExc::value_error("Too many decimal digits in format string"))?;
        args.get(index).cloned().ok_or_else(|| {
            PyExc::index_error(format!(
                "Replacement index {index} out of range for positional args tuple"
            ))
        })?
    } else {
        kwargs
            .dict_get(&Value::from(first))
            .map_err(|_| PyExc::key_error(first))?
    };
    let mut remaining = &field[split..];
    while !remaining.is_empty() {
        if let Some(after) = remaining.strip_prefix('.') {
            let end = after.find(['.', '[']).unwrap_or(after.len());
            let attribute = &after[..end];
            if attribute.is_empty() {
                return Err(PyExc::value_error("Empty attribute in format string"));
            }
            value = match (&value, attribute) {
                (Value::Int(_) | Value::Float(_), "real") => value.clone(),
                (Value::Bool(b), "real" | "numerator") => Value::Int(BigInt::from(*b)),
                (Value::Int(_), "numerator") => value.clone(),
                (Value::Int(_) | Value::Bool(_), "imag") => Value::from(0_i64),
                (Value::Float(_), "imag") => Value::Float(0.0),
                (Value::Int(_) | Value::Bool(_), "denominator") => Value::from(1_i64),
                _ => {
                    return Err(PyExc::attribute_error(format!(
                        "'{}' object has no attribute '{attribute}'",
                        value.type_name()
                    )));
                }
            };
            remaining = &after[end..];
            continue;
        }
        let after = remaining.strip_prefix('[').ok_or_else(|| {
            PyExc::value_error("Only '.' or '[' may follow ']' in format field specifier")
        })?;
        let end = after
            .find(']')
            .ok_or_else(|| PyExc::value_error("Missing ']' in format string"))?;
        let key = &after[..end];
        if key.is_empty() {
            return Err(PyExc::value_error("Empty attribute in format string"));
        }
        value = if matches!(&value, Value::Dict(_)) {
            let key = if key.chars().all(|c| c.is_ascii_digit()) {
                Value::Int(
                    key.parse()
                        .map_err(|_| PyExc::value_error("invalid index"))?,
                )
            } else {
                Value::from(key)
            };
            value.dict_get(&key)?
        } else {
            let index: usize = key.parse().map_err(|_| {
                PyExc::type_error(format!(
                    "{} indices must be integers, not 'str'",
                    value.type_name()
                ))
            })?;
            value
                .sequence()?
                .get(index)
                .cloned()
                .ok_or_else(|| PyExc::index_error("index out of range"))?
        };
        remaining = &after[end + 1..];
    }
    Ok(value)
}

pub fn percent(template: &str, args: &Value) -> PyResult<String> {
    let positional = if let Value::Tuple(v) = args {
        v.to_vec()
    } else {
        vec![args.clone()]
    };
    let mut consumed = 0;
    let mut mapping_used = false;
    let chars: Vec<_> = template.chars().collect();
    let mut i = 0;
    let mut out = String::new();
    while i < chars.len() {
        if chars[i] != '%' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        i += 1;
        if chars.get(i) == Some(&'%') {
            out.push('%');
            i += 1;
            continue;
        }
        let mapping = if chars.get(i) == Some(&'(') {
            mapping_used = true;
            if !matches!(args, Value::Dict(_)) {
                return Err(PyExc::type_error("format requires a mapping"));
            }
            i += 1;
            let start = i;
            let mut level = 1;
            while i < chars.len() {
                if chars[i] == '(' {
                    level += 1;
                }
                if chars[i] == ')' {
                    level -= 1;
                    if level == 0 {
                        break;
                    }
                }
                i += 1;
            }
            if i == chars.len() {
                return Err(PyExc::value_error("incomplete format key"));
            }
            let key: String = chars[start..i].iter().collect();
            i += 1;
            Some(args.dict_get(&Value::from(key))?)
        } else {
            None
        };
        let mut spec = Spec::default();
        let mut left = false;
        while let Some(c) = chars.get(i).filter(|c| "#0- +".contains(**c)) {
            match c {
                '#' => spec.alternate = true,
                '0' => spec.zero = true,
                '-' => left = true,
                '+' => spec.sign = Some('+'),
                ' ' if spec.sign.is_none() => spec.sign = Some(' '),
                _ => (),
            }
            i += 1;
        }
        let take = |consumed: &mut usize| -> PyResult<Value> {
            let value = positional
                .get(*consumed)
                .cloned()
                .ok_or_else(|| PyExc::type_error("not enough arguments for format string"))?;
            *consumed += 1;
            Ok(value)
        };
        if chars.get(i) == Some(&'*') {
            i += 1;
            let width = take(&mut consumed)?
                .as_int()
                .map_err(|_| PyExc::type_error("* wants int"))?
                .to_i64()
                .ok_or_else(|| {
                    PyExc::overflow_error("Python int too large to convert to C ssize_t")
                })?;
            left |= width < 0;
            spec.width = width.unsigned_abs() as usize;
        } else {
            spec.width = parse_digits(&chars, &mut i)?.unwrap_or(0);
        }
        if chars.get(i) == Some(&'.') {
            i += 1;
            spec.precision = if chars.get(i) == Some(&'*') {
                i += 1;
                let precision = take(&mut consumed)?
                    .as_int()
                    .map_err(|_| PyExc::type_error("* wants int"))?
                    .to_i64()
                    .ok_or_else(|| {
                        PyExc::overflow_error("Python int too large to convert to C ssize_t")
                    })?;
                Some(precision.max(0) as usize)
            } else {
                Some(parse_digits(&chars, &mut i)?.unwrap_or(0))
            };
        }
        if chars.get(i).is_some_and(|c| "hlL".contains(*c)) {
            i += 1;
        }
        let code = *chars
            .get(i)
            .ok_or_else(|| PyExc::value_error("incomplete format"))?;
        let code_index = i;
        i += 1;
        let value = mapping.map_or_else(|| take(&mut consumed), Ok)?;
        if !"diouxXeEfFgGcrsa".contains(code) {
            return Err(PyExc::value_error(format!(
                "unsupported format character '{}' (0x{:x}) at index {code_index}",
                code.escape_default(),
                code as u32
            )));
        }
        if left {
            spec.align = Some('<');
            spec.zero = false;
        }
        let rendered = match code {
            's' | 'r' | 'a' | 'c' => {
                let text = match code {
                    's' => str(&value)?,
                    'r' => repr(&value)?,
                    'a' => ascii(&value)?,
                    _ => {
                        if let Value::Str(s) = &value {
                            if s.len() != 1 {
                                return Err(PyExc::type_error("%c requires int or char"));
                            }
                            s.as_str().into()
                        } else {
                            value
                                .as_int()?
                                .to_u32()
                                .and_then(char::from_u32)
                                .ok_or_else(|| {
                                    PyExc::overflow_error("%c arg not in range(0x110000)")
                                })?
                                .to_string()
                        }
                    }
                };
                spec.sign = None;
                spec.alternate = false;
                spec.zero = false;
                let text: String = text
                    .chars()
                    .take(if code == 'c' {
                        usize::MAX
                    } else {
                        spec.precision.unwrap_or(usize::MAX)
                    })
                    .collect();
                pad(&text, "", &spec, '>')?
            }
            'd' | 'i' | 'u' | 'o' | 'x' | 'X' => {
                let n = if matches!(value, Value::Float(_)) && "diu".contains(code) {
                    let f = value.as_float()?;
                    if f.is_infinite() {
                        return Err(PyExc::overflow_error(
                            "cannot convert float infinity to integer",
                        ));
                    }
                    BigInt::from_f64(f.trunc())
                        .ok_or_else(|| PyExc::value_error("cannot convert float NaN to integer"))?
                } else {
                    value.as_int().map_err(|_| {
                        PyExc::type_error(format!(
                            "%{code} format: a {} is required, not {}",
                            if "diu".contains(code) {
                                "real number"
                            } else {
                                "integer"
                            },
                            value.type_name()
                        ))
                    })?
                };
                let radix = match code {
                    'o' => 8,
                    'x' | 'X' => 16,
                    _ => 10,
                };
                let mut digits = n.abs().to_str_radix(radix);
                let precision = spec.precision.take().unwrap_or(0);
                if precision > digits.len() {
                    digits = format!("{}{}", "0".repeat(precision - digits.len()), digits);
                }
                if code == 'X' {
                    digits.make_ascii_uppercase();
                }
                let mut prefix = numeric_prefix(n.is_negative(), &spec);
                if spec.alternate {
                    prefix.push_str(match code {
                        'o' => "0o",
                        'x' => "0x",
                        'X' => "0X",
                        _ => "",
                    });
                }
                pad(&digits, &prefix, &spec, '>')?
            }
            _ => {
                spec.code = Some(code);
                format_float(value.as_float()?, &spec)?
            }
        };
        out.push_str(&rendered);
    }
    if !mapping_used
        && consumed < positional.len()
        && !matches!(args, Value::Dict(_) | Value::List(_))
    {
        return Err(PyExc::type_error(
            "not all arguments converted during string formatting",
        ));
    }
    Ok(out)
}

fn date_format(iso: &str, spec: &str) -> PyResult<String> {
    let parts: Vec<_> = iso
        .get(..10)
        .unwrap_or("")
        .split('-')
        .map(|p| p.parse::<i32>().unwrap_or(0))
        .collect();
    if parts.len() != 3 {
        return Err(PyExc::value_error("invalid ISO date"));
    }
    let (year, month, day) = (parts[0], parts[1], parts[2]);
    let months = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    if !(1..=12).contains(&month) {
        return Err(PyExc::value_error("invalid ISO date"));
    }
    let adjusted_year = year - i32::from(month < 3);
    let offsets = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let weekday = (adjusted_year + adjusted_year / 4 - adjusted_year / 100
        + adjusted_year / 400
        + offsets[month as usize - 1]
        + day)
        .rem_euclid(7) as usize;
    let weekdays = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    let mut chars = spec.chars();
    let mut out = String::new();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some('A') => out.push_str(weekdays[weekday]),
            Some('a') => out.push_str(&weekdays[weekday][..3]),
            Some('B') => out.push_str(months[month as usize - 1]),
            Some('b' | 'h') => out.push_str(&months[month as usize - 1][..3]),
            Some('Y') => out.push_str(&format!("{year:04}")),
            Some('m') => out.push_str(&format!("{month:02}")),
            Some('d') => out.push_str(&format!("{day:02}")),
            Some('F') => out.push_str(&format!("{year:04}-{month:02}-{day:02}")),
            Some(c) => {
                return Err(PyExc::not_implemented_error(format!(
                    "date format directive %{c}"
                )));
            }
            None => out.push('%'),
        }
    }
    Ok(out)
}

pub fn run_case(case: &Json) -> PyResult<Value> {
    let text = |key: &str| {
        case.get(key)
            .and_then(Json::as_str)
            .ok_or_else(|| PyExc::value_error(format!("missing {key}")))
    };
    let decode = |key: &str| {
        from_canon_json(
            case.get(key)
                .ok_or_else(|| PyExc::value_error(format!("missing {key}")))?,
        )
    };
    let result = match text("op")? {
        "repr" => repr(&decode("value")?)?,
        "str" => str(&decode("value")?)?,
        "fstring" => {
            let conv = case.get("conv").and_then(Json::as_str).unwrap_or("");
            let value = conversion(decode("value")?, conv.chars().next())?;
            format(
                &value,
                case.get("spec").and_then(Json::as_str).unwrap_or(""),
            )?
        }
        "percent" => percent(text("fmt")?, &decode("args")?)?,
        "format" => {
            let args = decode("args")?.sequence()?;
            let kwargs = case
                .get("kwargs")
                .map(from_canon_json)
                .transpose()?
                .unwrap_or_else(Value::dict);
            template(text("template")?, &args, &kwargs)?
        }
        op => return Err(PyExc::value_error(format!("unknown format op {op}"))),
    };
    Ok(Value::from(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_representations_and_cycles() {
        assert_eq!(repr(&Value::from("it's fine")).unwrap(), "\"it's fine\"");
        assert_eq!(
            repr(&Value::from("tab\t\n\0é😀\u{200b}")).unwrap(),
            "'tab\\t\\n\\x00é😀\\u200b'"
        );
        assert_eq!(
            repr(&Value::tuple(vec![Value::from("a")])).unwrap(),
            "('a',)"
        );
        let cyclic = Value::list(vec![]);
        if let Value::List(items) = &cyclic {
            items.borrow_mut().push(cyclic.clone());
        }
        assert_eq!(repr(&cyclic).unwrap(), "[[...]]");
        if let Value::List(items) = cyclic {
            items.borrow_mut().clear();
        }
    }

    #[test]
    fn python_numeric_specs() {
        for (value, spec, expected) in [
            (Value::from(-42_i64), "08d", "-0000042"),
            (Value::from(1234_i64), "08,", "0,001,234"),
            (Value::from(255_i64), "#06x", "0x00ff"),
            (Value::Float(2.675), ".2f", "2.67"),
            (Value::Float(12345.6789), ",.2f", "12,345.68"),
            (Value::Float(0.00001), "g", "1e-05"),
            (Value::Float(999999.9), "g", "1e+06"),
            (Value::Float(1.0), ".2", "1.0"),
            (Value::Float(-0.0001), "z.2f", "0.00"),
            (Value::Float(1.0), "#.0e", "1.e+00"),
        ] {
            assert_eq!(format(&value, spec).unwrap(), expected, "{spec}");
        }
        assert_eq!(format(&Value::from("é😀"), "*^7.1s").unwrap(), "***é***");
    }

    #[test]
    fn python_templates_and_percent() {
        let kwargs = Value::dict();
        kwargs.dict_set(Value::from("x"), Value::from("é")).unwrap();
        assert_eq!(
            template("{{{0!r}}} {x!a:>8}", &[Value::from("q")], &kwargs).unwrap(),
            "{'q'}   '\\xe9'"
        );
        assert_eq!(
            template("{:{}}", &[Value::from("x"), Value::from(4_i64)], &kwargs).unwrap(),
            "x   "
        );
        assert_eq!(
            percent(
                "%s: %04d %.2f %%",
                &Value::tuple(vec![
                    Value::from("a"),
                    Value::from(-3_i64),
                    Value::Float(2.675)
                ])
            )
            .unwrap(),
            "a: -003 2.67 %"
        );
        assert_eq!(percent("%(x)s", &kwargs).unwrap(), "é");
        assert_eq!(
            percent(
                "%*.*f",
                &Value::tuple(vec![
                    Value::from(8_i64),
                    Value::from(2_i64),
                    Value::Float(1.5)
                ])
            )
            .unwrap(),
            "    1.50"
        );
    }

    #[test]
    fn python_dates() {
        assert_eq!(
            repr(&Value::Date("2026-06-15".into())).unwrap(),
            "datetime.date(2026, 6, 15)"
        );
        assert_eq!(
            repr(&Value::DateTime("2026-06-15T12:00:00".into())).unwrap(),
            "datetime.datetime(2026, 6, 15, 12, 0)"
        );
        assert_eq!(
            format(&Value::Date("2026-06-15".into()), "%A, %B %d").unwrap(),
            "Monday, June 15"
        );
    }
}
