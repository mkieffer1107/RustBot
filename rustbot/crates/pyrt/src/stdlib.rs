//! Native standard-library objects shared by generated Rust modules.
use crate::context::RandomHandle;
use crate::datetime::{PyDate, PyDateTime, PyTimedelta};
use crate::object::{CallArgs, IteratorObject, NativeFunction, NativeType, PyObject};
use crate::random::{PyRandom, RandomState};
use crate::runtime::Runtime;
use crate::{PyExc, PyResult, Value, canon_json, from_canon_json};
use num_traits::{Signed, ToPrimitive, Zero};
use serde_json::{Value as Json, json};
use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::rc::{Rc, Weak};

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
        ("difflib", init_difflib),
        ("hashlib", init_hashlib),
        ("operator", init_operator),
        ("ast", init_ast),
        ("json", init_json),
    ] {
        runtime.register_module(name, init);
    }
}

struct JsonOptions {
    ascii: bool,
    allow_nan: bool,
    sort_keys: bool,
    skip_keys: bool,
    indent: Option<String>,
    item_separator: String,
    key_separator: String,
    default: Value,
    check_circular: bool,
}

fn json_quote(text: &str, ascii: bool) -> String {
    let quoted = serde_json::to_string(text).expect("JSON string serialization");
    if !ascii {
        return quoted;
    }
    let mut result = String::new();
    for c in quoted.chars() {
        if c.is_ascii() {
            result.push(c);
        } else {
            for word in c.encode_utf16(&mut [0; 2]) {
                result.push_str(&format!("\\u{word:04x}"));
            }
        }
    }
    result
}

fn json_float(value: f64, options: &JsonOptions) -> PyResult<String> {
    if value.is_finite() {
        return Ok(crate::float_repr(value));
    }
    if !options.allow_nan {
        return Err(PyExc::value_error(
            "Out of range float values are not JSON compliant",
        ));
    }
    Ok(if value.is_nan() {
        "NaN"
    } else if value.is_sign_negative() {
        "-Infinity"
    } else {
        "Infinity"
    }
    .into())
}

fn json_encode(
    runtime: &mut Runtime,
    value: &Value,
    options: &JsonOptions,
    active: &mut HashSet<(u8, usize)>,
    depth: usize,
) -> PyResult<String> {
    if depth > 200 {
        return Err(PyExc::recursion_error(
            "maximum recursion depth exceeded while encoding a JSON object",
        ));
    }
    match value {
        Value::None => return Ok("null".into()),
        Value::Bool(b) => return Ok(if *b { "true" } else { "false" }.into()),
        Value::Int(n) => return Ok(n.to_string()),
        Value::Float(f) => return json_float(*f, options),
        Value::Str(s) => return Ok(json_quote(s.as_str(), options.ascii)),
        _ => (),
    }
    let (id, mapping, sequence) = match value {
        Value::List(v) => (
            Some((0, Rc::as_ptr(v) as usize)),
            None,
            Some(v.borrow().clone()),
        ),
        Value::Tuple(v) => (Some((1, v.as_ptr() as usize)), None, Some(v.to_vec())),
        Value::Dict(v) => (
            Some((2, Rc::as_ptr(v) as usize)),
            Some(v.borrow().clone()),
            None,
        ),
        _ => (None, None, None),
    };
    if let Some(id) = id.filter(|_| options.check_circular)
        && !active.insert(id)
    {
        return Err(PyExc::value_error("Circular reference detected"));
    }
    let result = if let Some(mut mapping) = mapping {
        if options.sort_keys {
            // Keep Python comparison errors rather than imposing an arbitrary
            // ordering on dictionaries containing mixed incomparable key types.
            let mut error = None;
            mapping.sort_by(|(a, _), (b, _)| match crate::ops::compare(a, b) {
                Ok(Some(o)) => o,
                Ok(None) => std::cmp::Ordering::Equal,
                Err(e) => {
                    error = Some(e);
                    std::cmp::Ordering::Equal
                }
            });
            if let Some(error) = error {
                return Err(error);
            }
        }
        let mut items = Vec::new();
        for (key, value) in mapping {
            let key = match key {
                Value::Str(s) => s.as_str().into(),
                Value::None => "null".into(),
                Value::Bool(b) => if b { "true" } else { "false" }.into(),
                Value::Int(n) => n.to_string(),
                Value::Float(f) => json_float(f, options)?,
                _ if options.skip_keys => continue,
                _ => {
                    return Err(PyExc::type_error(
                        "keys must be str, int, float, bool or None",
                    ));
                }
            };
            items.push(format!(
                "{}{}{}",
                json_quote(&key, options.ascii),
                options.key_separator,
                json_encode(runtime, &value, options, active, depth + 1)?
            ));
        }
        json_join(&items, '{', '}', options, depth)
    } else if let Some(sequence) = sequence {
        let items = sequence
            .iter()
            .map(|v| json_encode(runtime, v, options, active, depth + 1))
            .collect::<PyResult<Vec<_>>>()?;
        json_join(&items, '[', ']', options, depth)
    } else if !matches!(options.default, Value::None) {
        let replacement = runtime.call(&options.default, CallArgs::new(vec![value.clone()]))?;
        json_encode(runtime, &replacement, options, active, depth + 1)?
    } else {
        return Err(PyExc::type_error(format!(
            "Object of type {} is not JSON serializable",
            value.type_name()
        )));
    };
    if let Some(id) = id.filter(|_| options.check_circular) {
        active.remove(&id);
    }
    Ok(result)
}

fn json_join(
    items: &[String],
    open: char,
    close: char,
    options: &JsonOptions,
    depth: usize,
) -> String {
    if items.is_empty() {
        return format!("{open}{close}");
    }
    if let Some(indent) = &options.indent {
        let prefix = indent.repeat(depth + 1);
        format!(
            "{open}\n{prefix}{}\n{}{close}",
            items.join(&format!("{}\n{prefix}", options.item_separator)),
            indent.repeat(depth)
        )
    } else {
        format!("{open}{}{close}", items.join(&options.item_separator))
    }
}

fn json_error(message: &str, position: usize) -> PyExc {
    PyExc {
        kind: "JSONDecodeError",
        message: format!("{message}: char {position}"),
    }
}

struct JsonParser<'a> {
    text: &'a str,
    at: usize,
    hooks: &'a [Value],
}

impl JsonParser<'_> {
    fn whitespace(&mut self) {
        while self
            .text
            .as_bytes()
            .get(self.at)
            .is_some_and(|b| b" \t\r\n".contains(b))
        {
            self.at += 1;
        }
    }
    fn string(&mut self) -> PyResult<String> {
        let start = self.at;
        self.at += 1;
        let mut escaped = false;
        while let Some(&c) = self.text.as_bytes().get(self.at) {
            self.at += 1;
            if c == b'"' && !escaped {
                return serde_json::from_str(&self.text[start..self.at])
                    .map_err(|_| json_error("Invalid string escape", start));
            }
            escaped = c == b'\\' && !escaped;
        }
        Err(json_error("Unterminated string", start))
    }
    fn value(&mut self, runtime: &mut Runtime, depth: usize) -> PyResult<Value> {
        if depth > 200 {
            return Err(PyExc::recursion_error(
                "maximum recursion depth exceeded while decoding a JSON object",
            ));
        }
        self.whitespace();
        let start = self.at;
        let Some(&first) = self.text.as_bytes().get(start) else {
            return Err(json_error("Expecting value", start));
        };
        if first == b'"' {
            return Ok(Value::from(self.string()?));
        }
        if first == b'[' {
            self.at += 1;
            self.whitespace();
            let mut items = Vec::new();
            if self.text.as_bytes().get(self.at) == Some(&b']') {
                self.at += 1;
                return Ok(Value::list(items));
            }
            loop {
                items.push(self.value(runtime, depth + 1)?);
                self.whitespace();
                match self.text.as_bytes().get(self.at) {
                    Some(b']') => {
                        self.at += 1;
                        break;
                    }
                    Some(b',') => self.at += 1,
                    _ => return Err(json_error("Expecting ',' delimiter", self.at)),
                }
            }
            return Ok(Value::list(items));
        }
        if first == b'{' {
            self.at += 1;
            self.whitespace();
            let dict = Value::dict();
            let mut pairs = Vec::new();
            if self.text.as_bytes().get(self.at) == Some(&b'}') {
                self.at += 1;
            } else {
                loop {
                    self.whitespace();
                    if self.text.as_bytes().get(self.at) != Some(&b'"') {
                        return Err(json_error(
                            "Expecting property name enclosed in double quotes",
                            self.at,
                        ));
                    }
                    let key = Value::from(self.string()?);
                    self.whitespace();
                    if self.text.as_bytes().get(self.at) != Some(&b':') {
                        return Err(json_error("Expecting ':' delimiter", self.at));
                    }
                    self.at += 1;
                    let value = self.value(runtime, depth + 1)?;
                    dict.dict_set(key.clone(), value.clone())?;
                    pairs.push(Value::tuple(vec![key, value]));
                    self.whitespace();
                    match self.text.as_bytes().get(self.at) {
                        Some(b'}') => {
                            self.at += 1;
                            break;
                        }
                        Some(b',') => self.at += 1,
                        _ => return Err(json_error("Expecting ',' delimiter", self.at)),
                    }
                }
            }
            if !matches!(self.hooks[4], Value::None) {
                return runtime.call(&self.hooks[4], CallArgs::new(vec![Value::list(pairs)]));
            }
            if !matches!(self.hooks[0], Value::None) {
                return runtime.call(&self.hooks[0], CallArgs::new(vec![dict]));
            }
            return Ok(dict);
        }
        for (literal, value) in [
            ("true", Value::Bool(true)),
            ("false", Value::Bool(false)),
            ("null", Value::None),
        ] {
            if self.text[start..].starts_with(literal) {
                self.at += literal.len();
                return Ok(value);
            }
        }
        for (literal, value) in [
            ("NaN", f64::NAN),
            ("Infinity", f64::INFINITY),
            ("-Infinity", f64::NEG_INFINITY),
        ] {
            if self.text[start..].starts_with(literal) {
                self.at += literal.len();
                return if matches!(self.hooks[3], Value::None) {
                    Ok(Value::Float(value))
                } else {
                    runtime.call(&self.hooks[3], CallArgs::new(vec![Value::from(literal)]))
                };
            }
        }
        if first == b'-' {
            self.at += 1;
        }
        if self.text.as_bytes().get(self.at) == Some(&b'0') {
            self.at += 1;
        } else {
            while self
                .text
                .as_bytes()
                .get(self.at)
                .is_some_and(u8::is_ascii_digit)
            {
                self.at += 1;
            }
        }
        if self.at == start || self.at == start + usize::from(first == b'-') {
            return Err(json_error("Expecting value", start));
        }
        let mut float = false;
        if self.text.as_bytes().get(self.at) == Some(&b'.')
            && self
                .text
                .as_bytes()
                .get(self.at + 1)
                .is_some_and(u8::is_ascii_digit)
        {
            float = true;
            self.at += 1;
            while self
                .text
                .as_bytes()
                .get(self.at)
                .is_some_and(u8::is_ascii_digit)
            {
                self.at += 1;
            }
        }
        if self
            .text
            .as_bytes()
            .get(self.at)
            .is_some_and(|c| matches!(c, b'e' | b'E'))
        {
            let mut end = self.at + 1;
            if self
                .text
                .as_bytes()
                .get(end)
                .is_some_and(|c| matches!(c, b'+' | b'-'))
            {
                end += 1;
            }
            if self
                .text
                .as_bytes()
                .get(end)
                .is_some_and(u8::is_ascii_digit)
            {
                float = true;
                self.at = end + 1;
                while self
                    .text
                    .as_bytes()
                    .get(self.at)
                    .is_some_and(u8::is_ascii_digit)
                {
                    self.at += 1;
                }
            }
        }
        let literal = &self.text[start..self.at];
        let hook = &self.hooks[if float { 1 } else { 2 }];
        if !matches!(hook, Value::None) {
            return runtime.call(hook, CallArgs::new(vec![Value::from(literal)]));
        }
        if float {
            Ok(Value::Float(
                literal
                    .parse()
                    .map_err(|_| json_error("Invalid number", start))?,
            ))
        } else {
            Ok(Value::Int(
                literal
                    .parse()
                    .map_err(|_| json_error("Invalid number", start))?,
            ))
        }
    }
}

fn init_json(runtime: &mut Runtime, module: &Value) -> PyResult<()> {
    set(
        runtime,
        module,
        "dumps",
        function("json.dumps", |runtime, args| {
            if args.positional.len() > 1 {
                return Err(PyExc::type_error("dumps takes 1 positional argument"));
            }
            let args = bind(
                args,
                &[
                    "obj",
                    "skipkeys",
                    "ensure_ascii",
                    "check_circular",
                    "allow_nan",
                    "cls",
                    "indent",
                    "separators",
                    "default",
                    "sort_keys",
                ],
                1,
                vec![
                    Value::Bool(false),
                    Value::Bool(true),
                    Value::Bool(true),
                    Value::Bool(true),
                    Value::None,
                    Value::None,
                    Value::None,
                    Value::None,
                    Value::Bool(false),
                ],
            )?;
            if !matches!(args[5], Value::None) {
                return Err(PyExc::not_implemented_error("custom JSON encoder class"));
            }
            let indent = match &args[6] {
                Value::None => None,
                Value::Str(s) => Some(s.as_str().into()),
                _ => Some(" ".repeat(integer(&args[6])?.max(0) as usize)),
            };
            let (item_separator, key_separator) = if matches!(args[7], Value::None) {
                (
                    if indent.is_some() { "," } else { ", " }.into(),
                    ": ".into(),
                )
            } else {
                let separators = args[7].sequence()?;
                if separators.len() != 2 {
                    return Err(PyExc::value_error("separators must contain two strings"));
                }
                (
                    separators[0].as_str()?.into(),
                    separators[1].as_str()?.into(),
                )
            };
            let options = JsonOptions {
                ascii: args[2].is_truthy(),
                allow_nan: args[4].is_truthy(),
                sort_keys: args[9].is_truthy(),
                skip_keys: args[1].is_truthy(),
                indent,
                item_separator,
                key_separator,
                default: args[8].clone(),
                check_circular: args[3].is_truthy(),
            };
            Ok(Value::from(json_encode(
                runtime,
                &args[0],
                &options,
                &mut HashSet::new(),
                0,
            )?))
        }),
    )?;
    set(
        runtime,
        module,
        "loads",
        function("json.loads", |runtime, args| {
            if args.positional.len() > 1 {
                return Err(PyExc::type_error("loads takes 1 positional argument"));
            }
            let args = bind(
                args,
                &[
                    "s",
                    "cls",
                    "object_hook",
                    "parse_float",
                    "parse_int",
                    "parse_constant",
                    "object_pairs_hook",
                ],
                1,
                vec![Value::None; 6],
            )?;
            if !matches!(args[1], Value::None) {
                return Err(PyExc::not_implemented_error("custom JSON decoder class"));
            }
            let text = if let Value::Str(s) = &args[0] {
                if s.as_str().starts_with('\u{feff}') {
                    return Err(json_error("Unexpected UTF-8 BOM", 0));
                }
                s.as_str().to_owned()
            } else {
                let bytes = crate::bytes::bytes(&args[0])?;
                let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
                String::from_utf8(bytes.to_vec()).map_err(|_| PyExc {
                    kind: "UnicodeDecodeError",
                    message: "invalid UTF-8 JSON input".into(),
                })?
            };
            let mut parser = JsonParser {
                text: &text,
                at: 0,
                hooks: &args[2..],
            };
            let value = parser.value(runtime, 0)?;
            parser.whitespace();
            if parser.at != text.len() {
                return Err(json_error("Extra data", parser.at));
            }
            Ok(value)
        }),
    )?;
    Ok(())
}

#[derive(Clone, Debug)]
struct MatcherData {
    a: Vec<char>,
    b: Vec<char>,
    b2j: HashMap<char, Vec<usize>>,
    junk: HashSet<char>,
    isjunk: Value,
    autojunk: bool,
}

impl MatcherData {
    fn chain_b(&mut self, runtime: &mut Runtime) -> PyResult<()> {
        self.b2j.clear();
        self.junk.clear();
        let mut order = Vec::new();
        for (index, c) in self.b.iter().copied().enumerate() {
            if !self.b2j.contains_key(&c) {
                order.push(c);
            }
            self.b2j.entry(c).or_default().push(index);
        }
        if !matches!(self.isjunk, Value::None) {
            for c in order {
                if runtime
                    .call(
                        &self.isjunk,
                        CallArgs::new(vec![Value::from(c.to_string())]),
                    )?
                    .is_truthy()
                {
                    self.junk.insert(c);
                    self.b2j.remove(&c);
                }
            }
        }
        if self.autojunk && self.b.len() >= 200 {
            let threshold = self.b.len() / 100 + 1;
            self.b2j.retain(|_, indices| indices.len() <= threshold);
        }
        Ok(())
    }
    fn longest(&self, alo: usize, ahi: usize, blo: usize, bhi: usize) -> (usize, usize, usize) {
        let (mut ai, mut bi, mut size) = (alo, blo, 0);
        let mut previous = HashMap::new();
        for i in alo..ahi {
            let mut current = HashMap::new();
            if let Some(indices) = self.b2j.get(&self.a[i]) {
                for j in indices
                    .iter()
                    .copied()
                    .filter(|j| *j >= blo)
                    .take_while(|j| *j < bhi)
                {
                    let length = j
                        .checked_sub(1)
                        .and_then(|j| previous.get(&j))
                        .copied()
                        .unwrap_or(0)
                        + 1;
                    current.insert(j, length);
                    if length > size {
                        ai = i + 1 - length;
                        bi = j + 1 - length;
                        size = length;
                    }
                }
            }
            previous = current;
        }
        for junk in [false, true] {
            while ai > alo
                && bi > blo
                && self.junk.contains(&self.b[bi - 1]) == junk
                && self.a[ai - 1] == self.b[bi - 1]
            {
                ai -= 1;
                bi -= 1;
                size += 1;
            }
            while ai + size < ahi
                && bi + size < bhi
                && self.junk.contains(&self.b[bi + size]) == junk
                && self.a[ai + size] == self.b[bi + size]
            {
                size += 1;
            }
        }
        (ai, bi, size)
    }
    fn blocks(&self) -> Vec<(usize, usize, usize)> {
        let mut queue = vec![(0, self.a.len(), 0, self.b.len())];
        let mut blocks = Vec::new();
        while let Some((alo, ahi, blo, bhi)) = queue.pop() {
            let (ai, bi, size) = self.longest(alo, ahi, blo, bhi);
            if size == 0 {
                continue;
            }
            blocks.push((ai, bi, size));
            if alo < ai && blo < bi {
                queue.push((alo, ai, blo, bi));
            }
            if ai + size < ahi && bi + size < bhi {
                queue.push((ai + size, ahi, bi + size, bhi));
            }
        }
        blocks.sort_unstable();
        let mut collapsed: Vec<(usize, usize, usize)> = Vec::new();
        for (a, b, n) in blocks {
            if let Some(last) = collapsed.last_mut()
                && last.0 + last.2 == a
                && last.1 + last.2 == b
            {
                last.2 += n;
            } else {
                collapsed.push((a, b, n));
            }
        }
        collapsed.push((self.a.len(), self.b.len(), 0));
        collapsed
    }
    fn ratio(&self, matches: usize) -> f64 {
        let length = self.a.len() + self.b.len();
        if length == 0 {
            1.0
        } else {
            2.0 * matches as f64 / length as f64
        }
    }
}

#[derive(Clone, Debug)]
struct SequenceMatcherObject(Rc<RefCell<MatcherData>>);

fn block_value((a, b, size): (usize, usize, usize)) -> Value {
    Value::tuple(vec![Value::from(a), Value::from(b), Value::from(size)])
}

impl PyObject for SequenceMatcherObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "SequenceMatcher"
    }
    fn get_attr(&self, _: &mut Runtime, name: &str) -> PyResult<Value> {
        if ![
            "ratio",
            "quick_ratio",
            "real_quick_ratio",
            "find_longest_match",
            "get_matching_blocks",
            "get_opcodes",
            "set_seqs",
            "set_seq1",
            "set_seq2",
        ]
        .contains(&name)
        {
            return Err(PyExc::attribute_error(format!(
                "SequenceMatcher has no attribute '{name}'"
            )));
        }
        let data = self.0.clone();
        let name = name.to_owned();
        Ok(function(
            &format!("SequenceMatcher.{name}"),
            move |runtime, args| {
                if name.starts_with("set_seq") {
                    let names = if name == "set_seqs" {
                        &["a", "b"][..]
                    } else if name == "set_seq1" {
                        &["a"][..]
                    } else {
                        &["b"][..]
                    };
                    let args = bind(args, names, names.len(), vec![])?;
                    let mut replacement = data.borrow().clone();
                    if name != "set_seq2" {
                        replacement.a = args[0].as_str()?.chars().collect();
                    }
                    if name != "set_seq1" {
                        replacement.b = args[usize::from(name == "set_seqs")]
                            .as_str()?
                            .chars()
                            .collect();
                        replacement.chain_b(runtime)?;
                    }
                    *data.borrow_mut() = replacement;
                    return Ok(Value::None);
                }
                let data = data.borrow();
                if name == "find_longest_match" {
                    let args = bind(
                        args,
                        &["alo", "ahi", "blo", "bhi"],
                        0,
                        vec![
                            Value::from(0_i64),
                            Value::from(data.a.len()),
                            Value::from(0_i64),
                            Value::from(data.b.len()),
                        ],
                    )?;
                    let bounds = args
                        .iter()
                        .map(|v| {
                            v.as_int()?
                                .to_usize()
                                .ok_or_else(|| PyExc::index_error("sequence index out of range"))
                        })
                        .collect::<PyResult<Vec<_>>>()?;
                    if bounds[1] > data.a.len() || bounds[3] > data.b.len() {
                        return Err(PyExc::index_error("sequence index out of range"));
                    }
                    return Ok(block_value(
                        data.longest(bounds[0], bounds[1], bounds[2], bounds[3]),
                    ));
                }
                args.reject_keywords()?;
                args.arity(0, 0)?;
                match name.as_str() {
                    "ratio" => Ok(Value::Float(
                        data.ratio(data.blocks().iter().map(|b| b.2).sum()),
                    )),
                    "real_quick_ratio" => {
                        Ok(Value::Float(data.ratio(data.a.len().min(data.b.len()))))
                    }
                    "quick_ratio" => {
                        let mut available = HashMap::<char, usize>::new();
                        for c in &data.b {
                            *available.entry(*c).or_default() += 1;
                        }
                        let mut matches = 0;
                        for c in &data.a {
                            if let Some(count) = available.get_mut(c)
                                && *count != 0
                            {
                                *count -= 1;
                                matches += 1;
                            }
                        }
                        Ok(Value::Float(data.ratio(matches)))
                    }
                    "get_matching_blocks" => Ok(Value::list(
                        data.blocks().into_iter().map(block_value).collect(),
                    )),
                    "get_opcodes" => {
                        let mut result = Vec::new();
                        let (mut i, mut j) = (0, 0);
                        for (ai, bj, size) in data.blocks() {
                            let tag = if i < ai && j < bj {
                                Some("replace")
                            } else if i < ai {
                                Some("delete")
                            } else if j < bj {
                                Some("insert")
                            } else {
                                None
                            };
                            if let Some(tag) = tag {
                                result.push(Value::tuple(vec![
                                    Value::from(tag),
                                    Value::from(i),
                                    Value::from(ai),
                                    Value::from(j),
                                    Value::from(bj),
                                ]));
                            }
                            if size != 0 {
                                result.push(Value::tuple(vec![
                                    Value::from("equal"),
                                    Value::from(ai),
                                    Value::from(ai + size),
                                    Value::from(bj),
                                    Value::from(bj + size),
                                ]));
                            }
                            i = ai + size;
                            j = bj + size;
                        }
                        Ok(Value::list(result))
                    }
                    _ => unreachable!(),
                }
            },
        ))
    }
}

fn init_difflib(runtime: &mut Runtime, module: &Value) -> PyResult<()> {
    set(
        runtime,
        module,
        "SequenceMatcher",
        NativeType::value(
            "SequenceMatcher",
            "difflib",
            |v| matches!(v, Value::Object(o) if o.as_any().is::<SequenceMatcherObject>()),
            |runtime, args| {
                let args = bind(
                    args,
                    &["isjunk", "a", "b", "autojunk"],
                    0,
                    vec![
                        Value::None,
                        Value::from(""),
                        Value::from(""),
                        Value::Bool(true),
                    ],
                )?;
                let mut data = MatcherData {
                    a: args[1].as_str()?.chars().collect(),
                    b: args[2].as_str()?.chars().collect(),
                    b2j: HashMap::new(),
                    junk: HashSet::new(),
                    isjunk: args[0].clone(),
                    autojunk: args[3].is_truthy(),
                };
                data.chain_b(runtime)?;
                Ok(Value::Object(Rc::new(SequenceMatcherObject(Rc::new(
                    RefCell::new(data),
                )))))
            },
        ),
    )?;
    Ok(())
}

#[derive(Clone, Debug)]
struct HashObject(Rc<RefCell<sha1::Sha1>>);

impl PyObject for HashObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "HASH"
    }
    fn get_attr(&self, _: &mut Runtime, name: &str) -> PyResult<Value> {
        use sha1::Digest;
        match name {
            "name" => return Ok(Value::from("sha1")),
            "digest_size" => return Ok(Value::from(20_i64)),
            "block_size" => return Ok(Value::from(64_i64)),
            _ => (),
        }
        if !["update", "digest", "hexdigest", "copy"].contains(&name) {
            return Err(PyExc::attribute_error(format!(
                "HASH has no attribute '{name}'"
            )));
        }
        let hash = self.0.clone();
        let name = name.to_owned();
        Ok(function(&format!("HASH.{name}"), move |_, args| {
            args.reject_keywords()?;
            args.arity(usize::from(name == "update"), usize::from(name == "update"))?;
            match name.as_str() {
                "update" => {
                    hash.borrow_mut()
                        .update(crate::bytes::bytes(&args.positional[0])?);
                    Ok(Value::None)
                }
                "copy" => Ok(Value::Object(Rc::new(HashObject(Rc::new(RefCell::new(
                    hash.borrow().clone(),
                )))))),
                "digest" => Ok(crate::bytes::BytesObject::value(
                    hash.borrow().clone().finalize().to_vec(),
                )),
                _ => Ok(Value::from(
                    hash.borrow()
                        .clone()
                        .finalize()
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>(),
                )),
            }
        }))
    }
}

fn init_hashlib(runtime: &mut Runtime, module: &Value) -> PyResult<()> {
    set(
        runtime,
        module,
        "sha1",
        function("hashlib.sha1", |_, args| {
            use sha1::Digest;
            if args.positional.len() > 1 {
                return Err(PyExc::type_error(
                    "sha1() takes at most 1 positional argument",
                ));
            }
            let args = bind(
                args,
                &["string", "usedforsecurity"],
                0,
                vec![crate::bytes::BytesObject::value(vec![]), Value::Bool(true)],
            )?;
            let mut hash = sha1::Sha1::new();
            hash.update(crate::bytes::bytes(&args[0])?);
            Ok(Value::Object(Rc::new(HashObject(Rc::new(RefCell::new(
                hash,
            ))))))
        }),
    )?;
    Ok(())
}

fn init_operator(runtime: &mut Runtime, module: &Value) -> PyResult<()> {
    for name in [
        "add", "sub", "mul", "truediv", "floordiv", "mod", "pow", "pos", "neg", "invert",
    ] {
        set(
            runtime,
            module,
            name,
            function(&format!("operator.{name}"), move |_, args| {
                args.reject_keywords()?;
                let unary = ["pos", "neg", "invert"].contains(&name);
                args.arity(if unary { 1 } else { 2 }, if unary { 1 } else { 2 })?;
                let a = &args.positional[0];
                if unary {
                    return match name {
                        "pos" => crate::ops::pos(a),
                        "neg" => crate::ops::neg(a),
                        _ => crate::ops::invert(a),
                    };
                }
                let b = &args.positional[1];
                match name {
                    "add" => crate::ops::add(a, b),
                    "sub" => crate::ops::sub(a, b),
                    "mul" => crate::ops::mul(a, b),
                    "truediv" => crate::ops::true_div(a, b),
                    "floordiv" => crate::ops::floor_div(a, b),
                    "mod" => crate::ops::modulo(a, b),
                    "pow" => crate::ops::power(a, b),
                    _ => unreachable!(),
                }
            }),
        )?;
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct AstObject {
    kind: &'static str,
    fields: Value,
}

impl PyObject for AstObject {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        self.kind
    }
    fn repr(&self) -> PyResult<String> {
        Ok(format!("<ast.{} object at 0x0>", self.kind))
    }
    fn hash(&self) -> PyResult<i64> {
        Ok((self as *const Self as usize).rotate_right(4) as i64)
    }
    fn get_attr(&self, runtime: &mut Runtime, name: &str) -> PyResult<Value> {
        if name == "__class__" {
            let module = runtime.import("ast")?;
            return runtime.getattr(&module, self.kind);
        }
        if name == "__dict__" {
            return Ok(self.fields.clone());
        }
        if name == "_fields" {
            return Ok(Value::tuple(
                ast_fields(self.kind)
                    .iter()
                    .map(|n| Value::from(*n))
                    .collect(),
            ));
        }
        self.fields.dict_get(&Value::from(name)).map_err(|_| {
            PyExc::attribute_error(format!("'{}' object has no attribute '{name}'", self.kind))
        })
    }
    fn set_attr(&self, name: &str, value: Value) -> PyResult<()> {
        self.fields.dict_set(Value::from(name), value)
    }
}

fn ast_fields(kind: &str) -> &'static [&'static str] {
    match kind {
        "Expression" => &["body"],
        "Constant" => &["value", "kind"],
        "BinOp" => &["left", "op", "right"],
        "UnaryOp" => &["op", "operand"],
        "Name" => &["id", "ctx"],
        _ => &[],
    }
}

fn ast_node(kind: &'static str, fields: Vec<(&str, Value)>) -> PyResult<Value> {
    let dict = Value::dict();
    for (name, value) in fields {
        dict.dict_set(Value::from(name), value)?;
    }
    Ok(Value::Object(Rc::new(AstObject { kind, fields: dict })))
}

fn syntax_error() -> PyExc {
    PyExc {
        kind: "SyntaxError",
        message: "invalid syntax".into(),
    }
}

#[derive(Clone, Debug, PartialEq)]
enum ExprToken {
    Number(String),
    Name(String),
    Symbol(char),
    Power,
    FloorDiv,
    Newline,
    End,
}

fn expression_tokens(source: &str) -> PyResult<Vec<ExprToken>> {
    let chars: Vec<_> = source.chars().collect();
    let (mut i, mut parentheses) = (0, 0_usize);
    let mut tokens = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            if c == '\n' && parentheses == 0 {
                tokens.push(ExprToken::Newline);
            }
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || c == '.' {
            let start = i;
            let hexadecimal = c == '0' && chars.get(i + 1).is_some_and(|c| "xXoObB".contains(*c));
            i += 1;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric()
                    || matches!(chars[i], '.' | '_')
                    || !hexadecimal
                        && matches!(chars[i], '+' | '-')
                        && matches!(chars[i - 1], 'e' | 'E'))
            {
                i += 1;
            }
            tokens.push(ExprToken::Number(chars[start..i].iter().collect()));
            continue;
        }
        if c == '_' || c.is_alphabetic() {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i] == '_' || chars[i].is_alphanumeric()) {
                i += 1;
            }
            tokens.push(ExprToken::Name(chars[start..i].iter().collect()));
            continue;
        }
        if c == '*' && chars.get(i + 1) == Some(&'*') {
            tokens.push(ExprToken::Power);
            i += 2;
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            tokens.push(ExprToken::FloorDiv);
            i += 2;
            continue;
        }
        if c == '(' {
            parentheses += 1;
        }
        if c == ')' {
            parentheses = parentheses.checked_sub(1).ok_or_else(syntax_error)?;
        }
        if "+-*/%@()~".contains(c) {
            tokens.push(ExprToken::Symbol(c));
            i += 1;
            continue;
        }
        return Err(syntax_error());
    }
    if parentheses != 0 {
        return Err(syntax_error());
    }
    tokens.push(ExprToken::End);
    Ok(tokens)
}

struct ExpressionParser {
    tokens: Vec<ExprToken>,
    at: usize,
    depth: usize,
}

impl ExpressionParser {
    fn sum(&mut self) -> PyResult<Value> {
        let mut left = self.product()?;
        while let ExprToken::Symbol(op @ ('+' | '-')) = self.tokens[self.at] {
            self.at += 1;
            let right = self.product()?;
            left = ast_node(
                "BinOp",
                vec![
                    ("left", left),
                    (
                        "op",
                        ast_node(if op == '+' { "Add" } else { "Sub" }, vec![])?,
                    ),
                    ("right", right),
                ],
            )?;
        }
        Ok(left)
    }
    fn product(&mut self) -> PyResult<Value> {
        let mut left = self.factor()?;
        loop {
            let kind = match self.tokens[self.at] {
                ExprToken::Symbol('*') => "Mult",
                ExprToken::Symbol('/') => "Div",
                ExprToken::Symbol('%') => "Mod",
                ExprToken::Symbol('@') => "MatMult",
                ExprToken::FloorDiv => "FloorDiv",
                _ => break,
            };
            self.at += 1;
            let right = self.factor()?;
            left = ast_node(
                "BinOp",
                vec![
                    ("left", left),
                    ("op", ast_node(kind, vec![])?),
                    ("right", right),
                ],
            )?;
        }
        Ok(left)
    }
    fn factor(&mut self) -> PyResult<Value> {
        self.depth += 1;
        if self.depth > 200 {
            return Err(PyExc::recursion_error(
                "maximum recursion depth exceeded during ast construction",
            ));
        }
        let result = if let ExprToken::Symbol(op @ ('+' | '-' | '~')) = self.tokens[self.at] {
            self.at += 1;
            let operand = self.factor()?;
            ast_node(
                "UnaryOp",
                vec![
                    (
                        "op",
                        ast_node(
                            match op {
                                '+' => "UAdd",
                                '-' => "USub",
                                _ => "Invert",
                            },
                            vec![],
                        )?,
                    ),
                    ("operand", operand),
                ],
            )
        } else {
            let atom = self.atom()?;
            if self.tokens[self.at] == ExprToken::Power {
                self.at += 1;
                let exponent = self.factor()?;
                ast_node(
                    "BinOp",
                    vec![
                        ("left", atom),
                        ("op", ast_node("Pow", vec![])?),
                        ("right", exponent),
                    ],
                )
            } else {
                Ok(atom)
            }
        };
        self.depth -= 1;
        result
    }
    fn atom(&mut self) -> PyResult<Value> {
        let token = self.tokens[self.at].clone();
        match token {
            ExprToken::Number(raw) => {
                self.at += 1;
                if raw.starts_with('_') || raw.ends_with('_') || raw.contains("__") {
                    return Err(syntax_error());
                }
                let clean = raw.replace('_', "");
                let value = if clean.len() >= 2
                    && clean.starts_with('0')
                    && "xXoObB".contains(clean.as_bytes()[1] as char)
                {
                    let radix = match clean.as_bytes()[1] {
                        b'x' | b'X' => 16,
                        b'o' | b'O' => 8,
                        _ => 2,
                    };
                    Value::Int(
                        num_bigint::BigInt::parse_bytes(&clean.as_bytes()[2..], radix)
                            .ok_or_else(syntax_error)?,
                    )
                } else if clean.contains(['.', 'e', 'E']) {
                    Value::Float(clean.parse::<f64>().map_err(|_| syntax_error())?)
                } else {
                    if clean.len() > 1 && clean.starts_with('0') && clean.chars().any(|c| c != '0')
                    {
                        return Err(syntax_error());
                    }
                    Value::Int(clean.parse().map_err(|_| syntax_error())?)
                };
                ast_node("Constant", vec![("value", value), ("kind", Value::None)])
            }
            ExprToken::Name(name) => {
                self.at += 1;
                match name.as_str() {
                    "True" | "False" | "None" => ast_node(
                        "Constant",
                        vec![
                            (
                                "value",
                                match name.as_str() {
                                    "True" => Value::Bool(true),
                                    "False" => Value::Bool(false),
                                    _ => Value::None,
                                },
                            ),
                            ("kind", Value::None),
                        ],
                    ),
                    _ => ast_node(
                        "Name",
                        vec![
                            ("id", Value::from(name)),
                            ("ctx", ast_node("Load", vec![])?),
                        ],
                    ),
                }
            }
            ExprToken::Symbol('(') => {
                self.at += 1;
                let value = self.sum()?;
                if self.tokens[self.at] != ExprToken::Symbol(')') {
                    return Err(syntax_error());
                }
                self.at += 1;
                Ok(value)
            }
            _ => Err(syntax_error()),
        }
    }
}

fn init_ast(runtime: &mut Runtime, module: &Value) -> PyResult<()> {
    for name in [
        "AST",
        "expr",
        "operator",
        "unaryop",
        "Expression",
        "Constant",
        "BinOp",
        "UnaryOp",
        "Name",
        "Load",
        "Add",
        "Sub",
        "Mult",
        "Div",
        "FloorDiv",
        "Mod",
        "Pow",
        "MatMult",
        "UAdd",
        "USub",
        "Invert",
        "Not",
    ] {
        let class = NativeType::value(
            name,
            "ast",
            move |value| {
                let Value::Object(object) = value else {
                    return false;
                };
                let Some(node) = object.as_any().downcast_ref::<AstObject>() else {
                    return false;
                };
                name == "AST"
                    || name == node.kind
                    || name == "expr"
                        && ["Constant", "BinOp", "UnaryOp", "Name"].contains(&node.kind)
                    || name == "operator"
                        && [
                            "Add", "Sub", "Mult", "Div", "FloorDiv", "Mod", "Pow", "MatMult",
                        ]
                        .contains(&node.kind)
                    || name == "unaryop" && ["UAdd", "USub", "Invert", "Not"].contains(&node.kind)
            },
            move |_, args| {
                let fields = ast_fields(name);
                if args.positional.len() > fields.len() {
                    return Err(PyExc::type_error(
                        "AST constructor has too many positional arguments",
                    ));
                }
                let node = ast_node(
                    name,
                    args.positional
                        .into_iter()
                        .enumerate()
                        .map(|(i, v)| (fields[i], v))
                        .collect(),
                )?;
                if let Value::Object(object) = &node {
                    for (key, value) in args.keywords {
                        object.set_attr(&key, value)?;
                    }
                }
                Ok(node)
            },
        );
        set(runtime, module, name, class)?;
    }
    set(
        runtime,
        module,
        "parse",
        function("ast.parse", |_, args| {
            let args = bind(
                args,
                &[
                    "source",
                    "filename",
                    "mode",
                    "type_comments",
                    "feature_version",
                    "optimize",
                ],
                1,
                vec![
                    Value::from("<unknown>"),
                    Value::from("exec"),
                    Value::Bool(false),
                    Value::None,
                    Value::from(-1_i64),
                ],
            )?;
            if args[2].as_str()? != "eval" {
                return Err(PyExc::not_implemented_error(
                    "AST statement parsing; only arithmetic eval expressions are supported",
                ));
            }
            let source = if let Value::Str(s) = &args[0] {
                s.as_str().to_owned()
            } else {
                String::from_utf8(crate::bytes::bytes(&args[0])?).map_err(|_| syntax_error())?
            };
            let mut parser = ExpressionParser {
                tokens: expression_tokens(&source)?,
                at: 0,
                depth: 0,
            };
            let body = parser.sum()?;
            while parser.tokens[parser.at] == ExprToken::Newline {
                parser.at += 1;
            }
            if parser.tokens[parser.at] != ExprToken::End {
                return Err(syntax_error());
            }
            ast_node("Expression", vec![("body", body)])
        }),
    )?;
    Ok(())
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

#[derive(Debug)]
struct PatternObject(
    Rc<crate::regex::CompiledPattern>,
    Value,
    Weak<PatternObject>,
);

// Python-level pattern ownership belongs to each imported re module; the
// engine's independent cache must never resurrect an evicted wrapper.
type RegexKey = (String, i64);
#[derive(Default)]
struct ReCache {
    lru: HashMap<RegexKey, (Value, u64)>,
    order: BTreeMap<u64, RegexKey>,
    fifo: HashMap<RegexKey, Value>,
    fifo_order: VecDeque<RegexKey>,
    next: u64,
}

fn pattern_value(compiled: Rc<crate::regex::CompiledPattern>, source: &Value) -> Value {
    let text = if matches!(source, Value::Str(_)) {
        source.clone()
    } else {
        Value::from(compiled.pattern())
    };
    Value::Object(Rc::new_cyclic(|this| {
        PatternObject(compiled, text, this.clone())
    }))
}

fn cache_full(runtime: &mut Runtime, name: &str, len: usize) -> PyResult<bool> {
    let module = runtime.import("re")?;
    let maximum = runtime.getattr(&module, name)?;
    match crate::ops::compare(&Value::from(len), &maximum)? {
        Some(order) => Ok(order != std::cmp::Ordering::Less),
        None => Ok(false),
    }
}

fn cached_pattern(
    runtime: &mut Runtime,
    cache: &RefCell<ReCache>,
    value: &Value,
    flags: i64,
) -> PyResult<Value> {
    if let Value::Object(object) = value
        && object.as_any().is::<PatternObject>()
    {
        return if flags == 0 {
            Ok(value.clone())
        } else {
            Err(PyExc::value_error(
                "cannot process flags argument with a compiled pattern",
            ))
        };
    }
    let key = (value.as_str()?.to_owned(), flags);
    if let Some(pattern) = cache.borrow().fifo.get(&key).cloned() {
        return Ok(pattern);
    }
    let previous = {
        let mut cache = cache.borrow_mut();
        cache.lru.remove(&key).map(|(pattern, position)| {
            cache.order.remove(&position);
            pattern
        })
    };
    let fresh = previous.is_none();
    let result = match previous {
        Some(pattern) => pattern,
        None => pattern_value(crate::regex::compile_pattern(&key.0, flags)?, value),
    };
    if flags & 128 != 0 {
        return Ok(result);
    }
    if fresh && cache_full(runtime, "_MAXCACHE", cache.borrow().lru.len())? {
        let mut cache = cache.borrow_mut();
        if let Some((_, old)) = cache.order.pop_first() {
            cache.lru.remove(&old);
        }
    }
    {
        let mut cache = cache.borrow_mut();
        let position = cache.next;
        cache.next = cache
            .next
            .checked_add(1)
            .ok_or_else(|| PyExc::overflow_error("regex cache age overflow"))?;
        cache.order.insert(position, key.clone());
        cache.lru.insert(key.clone(), (result.clone(), position));
    }
    if cache_full(runtime, "_MAXCACHE2", cache.borrow().fifo.len())? {
        let mut cache = cache.borrow_mut();
        if let Some(old) = cache.fifo_order.pop_front() {
            cache.fifo.remove(&old);
        }
    }
    {
        let mut cache = cache.borrow_mut();
        cache.fifo_order.push_back(key.clone());
        cache.fifo.insert(key, result.clone());
    }
    Ok(result)
}

#[derive(Clone, Debug)]
struct MatchObject {
    owner: Value,
    original: Value,
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
            "string" => return Ok(self.original.clone()),
            "re" => return Ok(self.owner.clone()),
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
    owner: &Value,
    original: &Value,
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
        owner: owner.clone(),
        original: original.clone(),
        compiled,
        text: original.as_str()?.into(),
        data,
        pos,
        endpos,
    })))
}

fn pattern_method(compiled: Rc<crate::regex::CompiledPattern>, owner: Value, name: &str) -> Value {
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
                    let matched = match_value(compiled.clone(), &owner, &args[1], json, 0, end)?;
                    let value = runtime.call(&args[0], CallArgs::new(vec![matched]))?;
                    if matches!(value, Value::None) {
                        Ok(None)
                    } else {
                        Ok(Some(value.as_str()?.into()))
                    }
                })?;
                let output = if replaced == 0 {
                    args[1].clone()
                } else {
                    Value::from(output)
                };
                return Ok(if name == "sub" {
                    output
                } else {
                    Value::tuple(vec![output, Value::from(replaced)])
                });
            }
            let json_args = args.iter().map(canon_json).collect::<PyResult<Vec<_>>>()?;
            // subn exposes the real replacement count; equality of output text
            // alone cannot decide whether Python preserves the input object.
            let result = crate::regex::run_compiled(&compiled, "subn", &json_args)?;
            let result = from_canon_json(&result)?.sequence()?;
            let output = if result[1].as_int()?.is_zero() {
                args[1].clone()
            } else {
                result[0].clone()
            };
            return Ok(if name == "sub" {
                output
            } else {
                Value::tuple(vec![output, result[1].clone()])
            });
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
                .map(|v| match_value(compiled.clone(), &owner, &args[0], v.clone(), pos, end))
                .collect::<PyResult<Vec<_>>>()?;
            return Ok(IteratorObject::from_values(values));
        }
        match_value(compiled.clone(), &owner, &args[0], result, pos, end)
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
            "pattern" => Ok(self.1.clone()),
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
                let owner = self
                    .2
                    .upgrade()
                    .ok_or_else(|| PyExc::runtime_error("pattern owner no longer exists"))?;
                Ok(pattern_method(self.0.clone(), Value::Object(owner), name))
            }
            _ => Err(PyExc::attribute_error(format!(
                "Pattern has no attribute '{name}'"
            ))),
        }
    }
}

fn init_re(runtime: &mut Runtime, module: &Value) -> PyResult<()> {
    let cache = Rc::new(RefCell::new(ReCache::default()));
    set(runtime, module, "_MAXCACHE", Value::from(512_i64))?;
    set(runtime, module, "_MAXCACHE2", Value::from(256_i64))?;
    let purge_cache = Rc::clone(&cache);
    set(
        runtime,
        module,
        "purge",
        function("re.purge", move |_, args| {
            bind(args, &[], 0, vec![])?;
            *purge_cache.borrow_mut() = ReCache::default();
            Ok(Value::None)
        }),
    )?;
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
    let error = crate::exceptions::exception_type(runtime, "PatternError")?;
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
    let compile_cache = Rc::clone(&cache);
    set(
        runtime,
        module,
        "compile",
        function("re.compile", move |runtime, args| {
            let args = bind(args, &["pattern", "flags"], 1, vec![Value::from(0_i64)])?;
            cached_pattern(runtime, &compile_cache, &args[0], integer(&args[1])?)
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
        let method_cache = Rc::clone(&cache);
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
                let owner = cached_pattern(
                    runtime,
                    &method_cache,
                    &args[0],
                    integer(args.last().unwrap())?,
                )?;
                let compiled = match &owner {
                    Value::Object(object) => object
                        .as_any()
                        .downcast_ref::<PatternObject>()
                        .unwrap()
                        .0
                        .clone(),
                    _ => unreachable!(),
                };
                let forwarded = args[1..args.len() - 1].to_vec();
                runtime.call(
                    &pattern_method(compiled, owner, name),
                    CallArgs::new(forwarded),
                )
            }),
        )?;
    }
    Ok(())
}
