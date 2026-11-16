//! Python string methods, with positions measured in Unicode code points.
use crate::{PyExc, PyResult, Value, unicode_data as u};
use num_traits::ToPrimitive;

fn arity(args: &[Value], min: usize, max: usize) -> PyResult<()> {
    if args.len() < min || args.len() > max {
        Err(PyExc::type_error("incorrect number of arguments"))
    } else {
        Ok(())
    }
}
fn integer(v: &Value) -> PyResult<isize> {
    let n = v.as_int()?;
    Ok(n.to_isize().unwrap_or_else(|| {
        if n.to_string().starts_with('-') {
            isize::MIN
        } else {
            isize::MAX
        }
    }))
}
fn property(c: char, bit: u16) -> bool {
    u::property(c, 1 << bit)
}
pub(crate) fn is_printable_char(c: char) -> bool {
    property(c, 9)
}
fn whitespace(c: char) -> bool {
    property(c, 5)
}
fn mapped(c: char, f: fn(char) -> Option<&'static str>, out: &mut String) {
    if let Some(m) = f(c) {
        out.push_str(m);
    } else {
        out.push(c);
    }
}
fn cased(c: char) -> bool {
    property(c, 12)
}
fn lower_at(cs: &[char], i: usize, out: &mut String) {
    let c = cs[i];
    if c == 'Σ' {
        let before = cs[..i]
            .iter()
            .rev()
            .copied()
            .find(|x| !property(*x, 13))
            .is_some_and(cased);
        let after = cs[i + 1..]
            .iter()
            .copied()
            .find(|x| !property(*x, 13))
            .is_some_and(cased);
        out.push(if before && !after { 'ς' } else { 'σ' });
    } else {
        mapped(c, u::lower, out);
    }
}
fn case(s: &str, method: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut previous_cased = false;
    for (i, &c) in cs.iter().enumerate() {
        match method {
            "lower" => lower_at(&cs, i, &mut out),
            "upper" => mapped(c, u::upper, &mut out),
            "casefold" => mapped(c, u::casefold, &mut out),
            "capitalize" if i == 0 => mapped(c, u::title, &mut out),
            "capitalize" => lower_at(&cs, i, &mut out),
            "title" if !previous_cased => mapped(c, u::title, &mut out),
            "title" => lower_at(&cs, i, &mut out),
            "swapcase" if property(c, 6) => lower_at(&cs, i, &mut out),
            "swapcase" if property(c, 7) => mapped(c, u::upper, &mut out),
            _ => out.push(c),
        }
        previous_cased = cased(c);
    }
    out
}
fn slice_bounds(len: usize, args: &[Value], start_arg: usize) -> PyResult<(usize, usize, bool)> {
    let norm = |v: isize| -> usize {
        if v < 0 {
            len.saturating_sub(v.unsigned_abs())
        } else {
            (v as usize).min(len)
        }
    };
    let start = if let Some(v) = args.get(start_arg) {
        if matches!(v, Value::None) {
            0
        } else {
            let n = integer(v)?;
            if n >= 0 && n as usize > len {
                return Ok((len, len, true));
            }
            norm(n)
        }
    } else {
        0
    };
    let end = if let Some(v) = args.get(start_arg + 1) {
        if matches!(v, Value::None) {
            len
        } else {
            norm(integer(v)?)
        }
    } else {
        len
    };
    Ok((start, end, false))
}
fn find_at(h: &[char], n: &[char], reverse: bool) -> Option<usize> {
    if n.len() > h.len() {
        return None;
    }
    let it = 0..=h.len() - n.len();
    if reverse {
        it.rev().find(|&i| h[i..i + n.len()] == *n)
    } else {
        it.into_iter().find(|&i| h[i..i + n.len()] == *n)
    }
}
fn split(s: &str, sep: Option<&str>, max: isize, reverse: bool) -> PyResult<Vec<String>> {
    let cs: Vec<char> = s.chars().collect();
    let mut start = 0;
    let mut end = cs.len();
    let mut out = Vec::new();
    let unlimited = max < 0;
    let mut count = 0isize;
    if let Some(sep) = sep {
        let needle: Vec<char> = sep.chars().collect();
        if needle.is_empty() {
            return Err(PyExc::value_error("empty separator"));
        }
        while unlimited || count < max {
            if let Some(pos) = find_at(&cs[start..end], &needle, reverse) {
                let p = start + pos;
                if reverse {
                    out.push(cs[p + needle.len()..end].iter().collect());
                    end = p;
                } else {
                    out.push(cs[start..p].iter().collect());
                    start = p + needle.len();
                }
                count += 1;
            } else {
                break;
            }
        }
        out.push(cs[start..end].iter().collect());
    } else if reverse {
        while end > 0 && whitespace(cs[end - 1]) {
            end -= 1;
        }
        while end > 0 {
            if !unlimited && count >= max {
                out.push(cs[..end].iter().collect());
                break;
            }
            let mut p = end;
            while p > 0 && !whitespace(cs[p - 1]) {
                p -= 1;
            }
            out.push(cs[p..end].iter().collect());
            end = p;
            while end > 0 && whitespace(cs[end - 1]) {
                end -= 1;
            }
            count += 1;
        }
    } else {
        while start < end && whitespace(cs[start]) {
            start += 1;
        }
        while start < end {
            if !unlimited && count >= max {
                out.push(cs[start..end].iter().collect());
                break;
            }
            let mut p = start;
            while p < end && !whitespace(cs[p]) {
                p += 1;
            }
            out.push(cs[start..p].iter().collect());
            start = p;
            while start < end && whitespace(cs[start]) {
                start += 1;
            }
            count += 1;
        }
    }
    if reverse {
        out.reverse();
    }
    Ok(out)
}
fn strings_value(v: Vec<String>) -> Value {
    Value::list(v.into_iter().map(Value::from).collect())
}
fn arg_str(args: &[Value], i: usize) -> PyResult<&str> {
    args[i].as_str()
}

/// Construct the ordinal translation dictionary used by str.translate.
pub fn maketrans(args: &[Value]) -> PyResult<Value> {
    arity(args, 1, 3)?;
    let out = Value::dict();
    if args.len() == 1 {
        let Value::Dict(table) = &args[0] else {
            return Err(PyExc::type_error(
                "if you give only one argument to maketrans it must be a dict",
            ));
        };
        for (key, value) in table.borrow().iter() {
            let ordinal = match key {
                Value::Int(_) | Value::Bool(_) => key.clone(),
                Value::Str(text) => {
                    let mut chars = text.as_str().chars();
                    let first =
                        chars
                            .next()
                            .filter(|_| chars.next().is_none())
                            .ok_or_else(|| {
                                PyExc::value_error(
                                    "string keys in translate table must be of length 1",
                                )
                            })?;
                    Value::from(first as i64)
                }
                _ => {
                    return Err(PyExc::type_error(
                        "keys in translate table must be strings or integers",
                    ));
                }
            };
            // CPython deliberately defers validating replacement values until
            // that character is encountered by translate.
            out.dict_set(ordinal, value.clone())?;
        }
    } else {
        let from = args[0].as_str()?;
        let to = args[1].as_str()?;
        if from.chars().count() != to.chars().count() {
            return Err(PyExc::value_error(
                "the first two maketrans arguments must have equal length",
            ));
        }
        for (from, to) in from.chars().zip(to.chars()) {
            out.dict_set(Value::from(from as i64), Value::from(to as i64))?;
        }
        if let Some(delete) = args.get(2) {
            for c in delete.as_str()?.chars() {
                out.dict_set(Value::from(c as i64), Value::None)?;
            }
        }
    }
    Ok(out)
}

pub fn translate(text: &str, table: &Value) -> PyResult<String> {
    let mut result = String::new();
    for c in text.chars() {
        let ordinal = Value::from(c as i64);
        let replacement = match table {
            Value::Object(object) => object.get_item(&ordinal),
            _ => crate::ops::getitem(table, &ordinal),
        };
        match replacement {
            Err(error) if matches!(error.kind, "LookupError" | "KeyError" | "IndexError") => {
                result.push(c)
            }
            Err(error) => return Err(error),
            Ok(Value::None) => (),
            Ok(Value::Str(text)) => result.push_str(text.as_str()),
            Ok(value @ (Value::Int(_) | Value::Bool(_))) => {
                let cp = value
                    .as_int()?
                    .to_u32()
                    .filter(|cp| *cp <= 0x10ffff)
                    .ok_or_else(|| {
                        PyExc::value_error("character mapping must be in range(0x110000)")
                    })?;
                let c = char::from_u32(cp).ok_or_else(|| {
                    PyExc::not_implemented_error(
                        "surrogate code points are not representable by PyStr",
                    )
                })?;
                result.push(c);
            }
            Ok(_) => {
                return Err(PyExc::type_error(
                    "character mapping must return integer, None or str",
                ));
            }
        }
    }
    Ok(result)
}

pub fn format_map(text: &str, mapping: &Value) -> PyResult<String> {
    // format_map rejects positional fields, including fields nested in specs.
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '{' {
            i += 1;
            continue;
        }
        if chars.get(i + 1) == Some(&'{') {
            i += 2;
            continue;
        }
        let first = i + 1;
        let mut end = first;
        while end < chars.len() && !matches!(chars[end], '.' | '[' | ':' | '!' | '}') {
            end += 1;
        }
        if end == first || chars[first..end].iter().all(|c| property(*c, 2)) {
            return Err(PyExc::value_error(
                "Format string contains positional fields",
            ));
        }
        i += 1;
    }
    if !matches!(mapping, Value::Dict(_)) {
        return Err(PyExc::type_error("format_map() argument must be a mapping"));
    }
    crate::formatting::template(text, &[], mapping)
}

/// Dispatch a Python `str` method. Unknown methods return `AttributeError`.
pub fn call_method(s: &str, method: &str, args: &[Value]) -> PyResult<Value> {
    match method {
        "encode" => {
            arity(args, 0, 2)?;
            crate::bytes::encode(
                s,
                args.first()
                    .map(Value::as_str)
                    .transpose()?
                    .unwrap_or("utf-8"),
                args.get(1)
                    .map(Value::as_str)
                    .transpose()?
                    .unwrap_or("strict"),
            )
        }
        "translate" => {
            arity(args, 1, 1)?;
            Ok(Value::from(translate(s, &args[0])?))
        }
        "maketrans" => maketrans(args),
        "format_map" => {
            arity(args, 1, 1)?;
            Ok(Value::from(format_map(s, &args[0])?))
        }

        "lower" | "upper" | "title" | "capitalize" | "casefold" | "swapcase" => {
            arity(args, 0, 0)?;
            Ok(Value::from(case(s, method)))
        }
        "isalpha" | "isdigit" | "isdecimal" | "isnumeric" | "isalnum" | "isspace"
        | "isprintable" | "isidentifier" | "isascii" | "isupper" | "islower" | "istitle" => {
            arity(args, 0, 0)?;
            let cs: Vec<char> = s.chars().collect();
            let result = match method {
                "isascii" => s.is_ascii(),
                "isprintable" => cs.iter().copied().all(is_printable_char),
                "isidentifier" => {
                    !cs.is_empty()
                        && property(cs[0], 10)
                        && cs[1..].iter().all(|&c| property(c, 11))
                }
                "isupper" | "islower" => {
                    let bit = if method == "isupper" { 6 } else { 7 };
                    cs.iter().any(|&c| property(c, bit))
                        && cs.iter().all(|&c| !cased(c) || property(c, bit))
                }
                "istitle" => {
                    let mut prev = false;
                    let mut found = false;
                    let mut ok = true;
                    for &c in &cs {
                        if cased(c) {
                            if (!prev && !(property(c, 6) || property(c, 8)))
                                || (prev && !property(c, 7))
                            {
                                ok = false;
                            }
                            prev = true;
                            found = true;
                        } else {
                            prev = false;
                        }
                    }
                    ok && found
                }
                _ => {
                    let bit = match method {
                        "isalpha" => 0,
                        "isdigit" => 1,
                        "isdecimal" => 2,
                        "isnumeric" => 3,
                        "isalnum" => 4,
                        _ => 5,
                    };
                    !cs.is_empty() && cs.iter().all(|&c| property(c, bit))
                }
            };
            Ok(Value::from(result))
        }
        "strip" | "lstrip" | "rstrip" => {
            arity(args, 0, 1)?;
            let chars = match args.first() {
                None | Some(Value::None) => None,
                Some(v) => Some(v.as_str()?),
            };
            let pred = |c: char| chars.map_or_else(|| whitespace(c), |x| x.contains(c));
            let mut x = s;
            if method != "rstrip" {
                x = x.trim_start_matches(pred);
            }
            if method != "lstrip" {
                x = x.trim_end_matches(pred);
            }
            Ok(Value::from(x))
        }
        "split" | "rsplit" => {
            arity(args, 0, 2)?;
            let sep = match args.first() {
                None | Some(Value::None) => None,
                Some(v) => Some(v.as_str()?),
            };
            let max = if args.len() > 1 {
                integer(&args[1])?
            } else {
                -1
            };
            Ok(strings_value(split(s, sep, max, method == "rsplit")?))
        }
        "partition" | "rpartition" => {
            arity(args, 1, 1)?;
            let sep = arg_str(args, 0)?;
            if sep.is_empty() {
                return Err(PyExc::value_error("empty separator"));
            }
            let p = if method == "partition" {
                s.find(sep)
            } else {
                s.rfind(sep)
            };
            let (a, b, c) = match p {
                Some(i) => (&s[..i], sep, &s[i + sep.len()..]),
                None if method == "partition" => (s, "", ""),
                None => ("", "", s),
            };
            Ok(Value::tuple(vec![
                Value::from(a),
                Value::from(b),
                Value::from(c),
            ]))
        }
        "find" | "rfind" | "index" | "rindex" | "count" | "startswith" | "endswith" => {
            arity(args, 1, 3)?;
            let cs: Vec<char> = s.chars().collect();
            let (start, end, over) = slice_bounds(cs.len(), args, 1)?;
            let hay = if !over && start <= end {
                &cs[start..end]
            } else {
                &[]
            };
            if method == "startswith" || method == "endswith" {
                let vals = match &args[0] {
                    Value::Tuple(_) => args[0].sequence()?,
                    _ => vec![args[0].clone()],
                };
                let mut yes = false;
                for v in vals {
                    let n: Vec<char> = v.as_str()?.chars().collect();
                    if !over
                        && start <= end
                        && if method == "startswith" {
                            hay.starts_with(&n)
                        } else {
                            hay.ends_with(&n)
                        }
                    {
                        yes = true;
                        break;
                    }
                }
                return Ok(Value::from(yes));
            }
            let n: Vec<char> = arg_str(args, 0)?.chars().collect();
            if method == "count" {
                let mut count = 0usize;
                if !over && start <= end {
                    if n.is_empty() {
                        count = hay.len() + 1;
                    } else {
                        let mut i = 0;
                        while i + n.len() <= hay.len() {
                            if hay[i..i + n.len()] == n {
                                count += 1;
                                i += n.len();
                            } else {
                                i += 1;
                            }
                        }
                    }
                }
                return Ok(Value::from(count));
            }
            let pos = if !over && start <= end {
                find_at(hay, &n, method == "rfind" || method == "rindex").map(|i| i + start)
            } else {
                None
            };
            match pos {
                Some(i) => Ok(Value::from(i)),
                None if method == "index" || method == "rindex" => {
                    Err(PyExc::value_error("substring not found"))
                }
                None => Ok(Value::from(-1i64)),
            }
        }
        "removeprefix" | "removesuffix" => {
            arity(args, 1, 1)?;
            let sub = arg_str(args, 0)?;
            Ok(Value::from(if method == "removeprefix" {
                s.strip_prefix(sub).unwrap_or(s)
            } else {
                s.strip_suffix(sub).unwrap_or(s)
            }))
        }
        "replace" => {
            arity(args, 2, 3)?;
            let old = arg_str(args, 0)?;
            let new = arg_str(args, 1)?;
            let n = if args.len() > 2 {
                integer(&args[2])?
            } else {
                -1
            };
            if n == 0 {
                return Ok(Value::from(s));
            }
            let lim = if n < 0 { usize::MAX } else { n as usize };
            if old.is_empty() {
                let mut out = String::new();
                let mut count = 0;
                for c in s.chars() {
                    if count < lim {
                        out.push_str(new);
                        count += 1;
                    }
                    out.push(c);
                }
                if count < lim {
                    out.push_str(new);
                }
                Ok(Value::from(out))
            } else {
                Ok(Value::from(s.replacen(old, new, lim)))
            }
        }
        "center" | "ljust" | "rjust" | "zfill" => {
            arity(args, 1, if method == "zfill" { 1 } else { 2 })?;
            let width = integer(&args[0])?.max(0) as usize;
            let len = s.chars().count();
            let n = width.saturating_sub(len);
            if n > 10_000_000 {
                return Err(PyExc::overflow_error("result too large"));
            }
            let fill = if args.len() > 1 {
                let f = arg_str(args, 1)?;
                if f.chars().count() != 1 {
                    return Err(PyExc::type_error(
                        "fill character must be exactly one character",
                    ));
                }
                f.chars().next().unwrap()
            } else if method == "zfill" {
                '0'
            } else {
                ' '
            };
            let out = if method == "zfill" && (s.starts_with('+') || s.starts_with('-')) {
                format!("{}{}{}", &s[..1], "0".repeat(n), &s[1..])
            } else {
                let left = match method {
                    "rjust" | "zfill" => n,
                    "center" => n / 2 + (n & width & 1),
                    _ => 0,
                };
                format!(
                    "{}{}{}",
                    fill.to_string().repeat(left),
                    s,
                    fill.to_string().repeat(n - left)
                )
            };
            Ok(Value::from(out))
        }
        "join" => {
            arity(args, 1, 1)?;
            let vals = args[0].sequence()?;
            let mut parts = Vec::with_capacity(vals.len());
            for v in &vals {
                parts.push(v.as_str()?);
            }
            Ok(Value::from(parts.join(s)))
        }
        "__contains__" => {
            arity(args, 1, 1)?;
            Ok(Value::from(s.contains(arg_str(args, 0)?)))
        }
        "__getitem__" => {
            arity(args, 1, 1)?;
            let cs: Vec<char> = s.chars().collect();
            let n = integer(&args[0])?;
            let idx = if n < 0 {
                cs.len().checked_sub(n.unsigned_abs())
            } else {
                Some(n as usize)
            };
            match idx.and_then(|i| cs.get(i)) {
                Some(c) => Ok(Value::from(c.to_string())),
                None => Err(PyExc::index_error("string index out of range")),
            }
        }
        "__mul__" => {
            arity(args, 1, 1)?;
            let n = integer(&args[0])?.max(0) as usize;
            if s.len().checked_mul(n).is_none_or(|x| x > 10_000_000) {
                return Err(PyExc::overflow_error("repeated string is too long"));
            }
            Ok(Value::from(s.repeat(n)))
        }
        "expandtabs" => {
            arity(args, 0, 1)?;
            let tab = if args.is_empty() {
                8
            } else {
                integer(&args[0])?.max(0) as usize
            };
            let mut col = 0;
            let mut out = String::new();
            for c in s.chars() {
                match c {
                    '\t' => {
                        let n = if tab == 0 { 0 } else { tab - col % tab };
                        if n > 10_000_000 {
                            return Err(PyExc::overflow_error("result too large"));
                        }
                        out.push_str(&" ".repeat(n));
                        col += n;
                    }
                    '\n' | '\r' => {
                        out.push(c);
                        col = 0;
                    }
                    _ => {
                        out.push(c);
                        col += 1;
                    }
                }
            }
            Ok(Value::from(out))
        }
        "splitlines" => {
            arity(args, 0, 1)?;
            let keep = args.first().is_some_and(Value::is_truthy);
            let cs: Vec<char> = s.chars().collect();
            let mut out = Vec::new();
            let (mut start, mut i) = (0, 0);
            while i < cs.len() {
                if matches!(
                    cs[i],
                    '\n' | '\r'
                        | '\u{b}'
                        | '\u{c}'
                        | '\u{1c}'
                        | '\u{1d}'
                        | '\u{1e}'
                        | '\u{85}'
                        | '\u{2028}'
                        | '\u{2029}'
                ) {
                    let end = i;
                    i += 1;
                    if cs[end] == '\r' && i < cs.len() && cs[i] == '\n' {
                        i += 1;
                    }
                    out.push(cs[start..if keep { i } else { end }].iter().collect());
                    start = i;
                } else {
                    i += 1;
                }
            }
            if start < cs.len() {
                out.push(cs[start..].iter().collect());
            }
            Ok(strings_value(out))
        }
        _ => Err(PyExc::attribute_error(format!(
            "str has no attribute {method}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn str_result(s: &str, method: &str, args: Vec<Value>) -> String {
        call_method(s, method, &args)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned()
    }
    #[test]
    fn unicode_casing_and_indices() {
        assert_eq!(str_result("ὈΔΥΣΣΕΎΣ", "lower", vec![]), "ὀδυσσεύς");
        assert_eq!(str_result("straße", "upper", vec![]), "STRASSE");
        assert_eq!(str_result("ǅungla ﬁne", "title", vec![]), "ǅungla Fine");
        assert_eq!(
            call_method("😀a😀", "rfind", &[Value::from("😀")])
                .unwrap()
                .as_int()
                .unwrap()
                .to_string(),
            "2"
        );
    }
    #[test]
    fn final_sigma_with_cased_case_ignorable_characters() {
        for (input, method, expected) in [
            ("AΣʰ", "lower", "aςʰ"),
            ("AΣͅ", "lower", "aςͅ"),
            ("AΣʰa", "lower", "aσʰa"),
            ("AΣͅa", "lower", "aσͅa"),
            ("AΣʰ", "title", "Aςʰ"),
            ("AΣͅ", "title", "Aςͅ"),
            ("AΣʰ", "capitalize", "Aςʰ"),
            ("AΣͅ", "capitalize", "Aςͅ"),
            ("AΣʰ", "swapcase", "aςʰ"),
            ("AΣͅ", "swapcase", "aςΙ"),
        ] {
            assert_eq!(
                str_result(input, method, vec![]),
                expected,
                "{method}({input:?})"
            );
        }
    }
    #[test]
    fn python_split_and_empty_search() {
        let got = call_method("  α β  ", "split", &[Value::None, Value::from(0i64)])
            .unwrap()
            .sequence()
            .unwrap();
        assert_eq!(got[0].as_str().unwrap(), "α β  ");
        assert_eq!(
            call_method("abc", "find", &[Value::from(""), Value::from(4i64)])
                .unwrap()
                .as_int()
                .unwrap()
                .to_string(),
            "-1"
        );
        assert_eq!(
            str_result(
                "abc",
                "replace",
                vec![Value::from(""), Value::from("-"), Value::from(2i64)]
            ),
            "-a-bc"
        );
    }
    #[test]
    fn classification_and_errors() {
        assert!(call_method("²", "isdigit", &[]).unwrap().is_truthy());
        assert!(!call_method("²", "isdecimal", &[]).unwrap().is_truthy());
        assert_eq!(
            call_method("abc", "split", &[Value::from("")])
                .unwrap_err()
                .kind,
            "ValueError"
        );
        assert_eq!(
            call_method("", "__getitem__", &[Value::from(0i64)])
                .unwrap_err()
                .kind,
            "IndexError"
        );
    }
}
