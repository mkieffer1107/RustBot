//! Initial CPython `re.Pattern` compatibility layer.
use crate::{PyExc, PyResult, unicode_data as unicode};
use fancy_regex::{Captures, Regex};
use serde_json::{Value, json};
use std::sync::OnceLock;

fn pattern_error(message: impl Into<String>) -> PyExc {
    PyExc {
        kind: "PatternError",
        message: message.into(),
    }
}
fn re_error(e: impl std::fmt::Display) -> PyExc {
    pattern_error(e.to_string())
}
fn as_str(v: &Value) -> PyResult<&str> {
    v.as_str()
        .ok_or_else(|| PyExc::type_error("expected string"))
}
fn as_int(v: &Value) -> PyResult<i64> {
    v.as_i64()
        .or_else(|| v.as_bool().map(i64::from))
        .ok_or_else(|| PyExc::type_error("integer argument expected"))
}
fn positions(text: &str) -> Vec<usize> {
    text.char_indices()
        .map(|x| x.0)
        .chain(std::iter::once(text.len()))
        .collect()
}
fn codepoint(offset: usize, positions: &[usize]) -> usize {
    positions
        .binary_search(&offset)
        .expect("regex match must be on a code point boundary")
}
fn arg_count(args: &[Value], min: usize, max: usize) -> PyResult<()> {
    if args.len() < min || args.len() > max {
        Err(PyExc::type_error("incorrect number of arguments"))
    } else {
        Ok(())
    }
}
fn class_body(kind: char, ascii: bool) -> &'static str {
    match (kind, ascii) {
        ('w', true) => "A-Za-z0-9_",
        ('d', true) => "0-9",
        ('s', true) => r"\x{9}-\x{d}\x{20}",
        ('w', false) => unicode::REGEX_WORD,
        ('d', false) => unicode::REGEX_DIGIT,
        ('s', false) => unicode::REGEX_SPACE,
        _ => unreachable!(),
    }
}
fn boundary(ascii: bool) -> &'static str {
    static UNICODE: OnceLock<String> = OnceLock::new();
    static ASCII: OnceLock<String> = OnceLock::new();
    let slot = if ascii { &ASCII } else { &UNICODE };
    slot.get_or_init(|| {
        let w = class_body('w', ascii);
        format!(r"(?:(?<![{w}])(?=[{w}])|(?<=[{w}])(?![{w}]))")
    })
}
/// Convert character classes for Python semantics while leaving regex syntax intact.
fn translate(pattern: &str, flags: i64) -> PyResult<String> {
    if flags & !(1 | 2 | 4 | 8 | 16 | 32 | 64 | 128 | 256) != 0 || flags & 4 != 0 {
        return Err(PyExc::value_error("unsupported regex flags"));
    }
    if flags & 128 != 0 {
        return Err(PyExc::not_implemented_error(
            "re.DEBUG output is unsupported",
        ));
    }
    if flags & 256 != 0 && flags & 32 != 0 {
        return Err(PyExc::value_error(
            "ASCII and UNICODE flags are incompatible",
        ));
    }
    let mut ascii = flags & 256 != 0;
    let mut p = pattern;
    // Global inline flags are only legal at the start in Python.
    let mut inline = String::new();
    if p.starts_with("(?")
        && let Some(end) = p.find(')')
    {
        let cand = &p[2..end];
        if !cand.is_empty() && cand.chars().all(|c| "aiLmsux".contains(c)) {
            for c in cand.chars() {
                match c {
                    'a' => ascii = true,
                    'i' | 'm' | 's' | 'x' => inline.push(c),
                    'u' => {}
                    _ => return Err(pattern_error("invalid inline flags")),
                }
            }
            p = &p[end + 1..];
        }
    }
    for (bit, ch) in [(2, 'i'), (8, 'm'), (16, 's'), (64, 'x')] {
        if flags & bit != 0 && !inline.contains(ch) {
            inline.push(ch);
        }
    }
    let mut out = String::new();
    if !inline.is_empty() {
        out.push_str("(?");
        out.push_str(&inline);
        out.push(')');
    }
    let mut chars = p.chars();
    let mut in_class = false;
    let mut class_first = false;
    while let Some(c) = chars.next() {
        if c == '\\' {
            let Some(n) = chars.next() else {
                return Err(pattern_error("bad escape"));
            };
            match n {
                'w' | 'd' | 's' | 'W' | 'D' | 'S' => {
                    let neg = n.is_ascii_uppercase();
                    let body = class_body(n.to_ascii_lowercase(), ascii);
                    if in_class {
                        if neg {
                            return Err(PyExc::not_implemented_error(
                                "negated category inside a character class",
                            ));
                        }
                        out.push_str(body);
                    } else {
                        out.push('[');
                        if neg {
                            out.push('^');
                        }
                        out.push_str(body);
                        out.push(']');
                    }
                }
                'b' | 'B' if !in_class => {
                    if n == 'B' {
                        out.push_str("(?!(?:");
                        out.push_str(boundary(ascii));
                        out.push_str("))");
                    } else {
                        out.push_str(boundary(ascii));
                    }
                }
                'b' if in_class => out.push_str(r"\x{8}"),
                'Z' | 'z' if !in_class => out.push_str(r"\z"),
                _ => {
                    out.push('\\');
                    out.push(n);
                }
            }
            class_first = false;
        } else {
            if c == '[' && !in_class {
                in_class = true;
                class_first = true;
            } else if c == ']' && in_class && !class_first {
                in_class = false;
            } else if c != '^' {
                class_first = false;
            }
            if c == '$' && !in_class && !inline.contains('m') {
                out.push_str(r"(?=\n?\z)");
            } else {
                out.push(c);
            }
        }
    }
    Ok(out)
}
fn compile(p: &str, flags: i64, full: bool) -> PyResult<Regex> {
    let pat = translate(p, flags)?;
    let pat = if full { format!("(?:{pat})\\z") } else { pat };
    Regex::new(&pat).map_err(re_error)
}
fn match_json(c: &Captures<'_>, re: &Regex, pos: &[usize]) -> Value {
    let m = c.get(0).unwrap();
    let mut groups = Vec::new();
    let mut spans = Vec::new();
    let mut last = None;
    let mut last_end = 0;
    for i in 1..re.captures_len() {
        if let Some(x) = c.get(i) {
            groups.push(json!(x.as_str()));
            spans.push(json!([codepoint(x.start(), pos), codepoint(x.end(), pos)]));
            if last.is_none() || x.end() > last_end {
                last = Some(i);
                last_end = x.end();
            }
        } else {
            groups.push(Value::Null);
            spans.push(Value::Null);
        }
    }
    let names: Vec<_> = re.capture_names().collect();
    let mut dict = serde_json::Map::new();
    for (i, name) in names.iter().enumerate() {
        if let Some(name) = name {
            dict.insert(
                (*name).to_owned(),
                c.get(i).map_or(Value::Null, |x| json!(x.as_str())),
            );
        }
    }
    let lastgroup = last.and_then(|i| names.get(i).copied().flatten());
    json!({"$match":{"span":[codepoint(m.start(),pos),codepoint(m.end(),pos)],"groups":groups,"spans":spans,"groupdict":dict,"lastindex":last,"lastgroup":lastgroup}})
}
fn replacement(template: &str, c: Option<&Captures<'_>>, re: &Regex) -> PyResult<String> {
    let mut out = String::new();
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let Some(n) = chars.next() else {
            return Err(pattern_error("bad escape at end of replacement"));
        };
        match n {
            'g' => {
                if chars.next() != Some('<') {
                    return Err(pattern_error("missing <"));
                }
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('>') => break,
                        Some(c) => name.push(c),
                        None => return Err(pattern_error("missing >")),
                    }
                }
                let group = if let Ok(i) = name.parse::<usize>() {
                    if i >= re.captures_len() {
                        return Err(pattern_error("invalid group reference"));
                    }
                    c.and_then(|captures| captures.get(i))
                } else {
                    if !re.capture_names().flatten().any(|n| n == name) {
                        return Err(PyExc::index_error("unknown group name"));
                    }
                    c.and_then(|captures| captures.name(&name))
                };
                if let Some(g) = group {
                    out.push_str(g.as_str());
                }
            }
            '1'..='9' => {
                let mut idx = n.to_digit(10).unwrap() as usize;
                if chars.peek().is_some_and(char::is_ascii_digit) {
                    idx = idx * 10 + chars.next().unwrap().to_digit(10).unwrap() as usize;
                }
                if idx >= re.captures_len() {
                    return Err(pattern_error("invalid group reference"));
                }
                if let Some(g) = c.and_then(|captures| captures.get(idx)) {
                    out.push_str(g.as_str());
                }
            }
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'f' => out.push('\u{c}'),
            'v' => out.push('\u{b}'),
            'a' => out.push('\u{7}'),
            'b' => out.push('\u{8}'),
            '\\' => out.push('\\'),
            c if c.is_ascii_alphabetic() => return Err(pattern_error("bad escape")),
            c => {
                out.push('\\');
                out.push(c);
            }
        }
    }
    Ok(out)
}
/// Execute a serialized regex case and return the check's canonical result.
pub fn run_case(case: &Value) -> PyResult<Value> {
    let pattern = as_str(
        case.get("pattern")
            .ok_or_else(|| PyExc::type_error("missing pattern"))?,
    )?;
    let flags = case.get("flags").map_or(Ok(0), as_int)?;
    let method = as_str(
        case.get("method")
            .ok_or_else(|| PyExc::type_error("missing method"))?,
    )?;
    let args = case
        .get("args")
        .and_then(Value::as_array)
        .ok_or_else(|| PyExc::type_error("missing arguments"))?;
    let re = compile(pattern, flags, method == "fullmatch")?;
    let is_sub = method == "sub" || method == "subn";
    match method {
        "match" | "search" | "fullmatch" | "findall" | "finditer" => arg_count(args, 1, 3)?,
        "split" => arg_count(args, 1, 2)?,
        "sub" | "subn" => arg_count(args, 2, 3)?,
        _ => return Err(PyExc::attribute_error("unknown regex method")),
    }
    let text = as_str(&args[usize::from(is_sub)])?;
    let pos = positions(text);
    let len = pos.len() - 1;
    let norm = |x: i64| -> usize { x.max(0).min(len as i64) as usize };
    let start = if !is_sub && method != "split" && args.len() > 1 {
        norm(as_int(&args[1])?)
    } else {
        0
    };
    let end = if !is_sub && method != "split" && args.len() > 2 {
        norm(as_int(&args[2])?)
    } else {
        len
    };
    let input = &text[..pos[end]];
    if method == "search" || method == "match" || method == "fullmatch" {
        if start > end {
            return Ok(Value::Null);
        }
        let cap = re.captures_from_pos(input, pos[start]).map_err(re_error)?;
        return Ok(match cap {
            Some(c) if method == "search" || c.get(0).unwrap().start() == pos[start] => {
                match_json(&c, &re, &pos)
            }
            _ => Value::Null,
        });
    }
    let limit = if is_sub && args.len() > 2 {
        as_int(&args[2])?
    } else if method == "split" && args.len() > 1 {
        as_int(&args[1])?
    } else {
        0
    };
    if is_sub && args[0].get("$callable").and_then(Value::as_array).is_none() {
        // CPython validates templates even when no substitution is performed.
        replacement(as_str(&args[0])?, None, &re)?;
    }
    if limit < 0 {
        return if method == "sub" {
            Ok(json!(text))
        } else if method == "subn" {
            Ok(json!({"$tuple":[text,0]}))
        } else {
            Ok(json!([text]))
        };
    }
    let mut at = if start <= end {
        pos[start]
    } else {
        return Ok(json!([]));
    };
    let mut nonempty_at_origin = None;
    let mut previous_was_empty = false;
    let mut tail = 0;
    let mut count = 0usize;
    let mut items = Vec::new();
    let mut output = String::new();
    loop {
        if limit > 0 && count >= limit as usize {
            break;
        }
        // After an empty match, Python retries at the same position, forbidding
        // another empty match there while permitting a nonempty alternative.
        // Fancy-regex's \G anchors to captures_from_pos's search origin.
        let active = if previous_was_empty {
            if nonempty_at_origin.is_none() {
                let translated = translate(pattern, flags)?;
                nonempty_at_origin =
                    Some(Regex::new(&format!(r"(?:{translated})(?!\G)")).map_err(re_error)?);
            }
            nonempty_at_origin.as_ref().unwrap()
        } else {
            &re
        };
        let Some(c) = active.captures_from_pos(input, at).map_err(re_error)? else {
            break;
        };
        let m = c.get(0).unwrap();
        match method {
            "finditer" => items.push(match_json(&c, &re, &pos)),
            "findall" => {
                let groups = re.captures_len() - 1;
                items.push(match groups{0=>json!(m.as_str()),1=>json!(c.get(1).map_or("",|x|x.as_str())),_=>json!({"$tuple":(1..=groups).map(|i| c.get(i).map_or("",|x|x.as_str())).collect::<Vec<_>>()})});
            }
            "split" => {
                items.push(json!(&text[tail..m.start()]));
                for i in 1..re.captures_len() {
                    items.push(c.get(i).map_or(Value::Null, |x| json!(x.as_str())));
                }
                tail = m.end();
            }
            "sub" | "subn" => {
                output.push_str(&text[tail..m.start()]);
                let repl = &args[0];
                if let Some(seq) = repl.get("$callable").and_then(Value::as_array) {
                    let v = seq.get(count).ok_or_else(|| PyExc {
                        kind: "StopIteration",
                        message: String::new(),
                    })?;
                    if !v.is_null() {
                        output.push_str(as_str(v)?);
                    }
                } else {
                    output.push_str(&replacement(as_str(repl)?, Some(&c), &re)?);
                }
                tail = m.end();
            }
            _ => unreachable!(),
        }
        count += 1;
        previous_was_empty = m.start() == m.end();
        at = m.end();
        if previous_was_empty && at == input.len() {
            break;
        }
    }
    match method {
        "split" => {
            items.push(json!(&text[tail..]));
            Ok(Value::Array(items))
        }
        "sub" | "subn" => {
            output.push_str(&text[tail..]);
            if method == "subn" {
                Ok(json!({"$tuple":[output,count]}))
            } else {
                Ok(json!(output))
            }
        }
        _ => Ok(Value::Array(items)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_word_boundary_and_spans() {
        let case =
            json!({"pattern":r"\b(?P<word>\w+)\b", "flags":0,"method":"search","args":["😀 ٣²"]});
        let got = run_case(&case).unwrap();
        assert_eq!(
            got,
            json!({"$match":{"span":[2,4],"groups":["٣²"],"spans":[[2,4]],"groupdict":{"word":"٣²"},"lastindex":1,"lastgroup":"word"}})
        );
    }
    #[test]
    fn references_and_substitutions() {
        let case = json!({"pattern":r"(?P<x>\w+)-(\d+)","method":"subn","args":[r"\2:\g<x>","ab-12 cd-3"]});
        assert_eq!(run_case(&case).unwrap(), json!({"$tuple":["12:ab 3:cd",2]}));
    }
    #[test]
    fn empty_matches_after_nonempty_and_final_newline_anchor() {
        let case = json!({"pattern":"a*","method":"findall","args":["aaa b"]});
        assert_eq!(run_case(&case).unwrap(), json!(["aaa", "", "", ""]));
        let case = json!({"pattern":"$","method":"finditer","args":["a\n"]});
        let result = run_case(&case).unwrap();
        assert_eq!(result[0]["$match"]["span"], json!([1, 1]));
        assert_eq!(result[1]["$match"]["span"], json!([2, 2]));
    }
    #[test]
    fn nonempty_alternatives_follow_empty_matches() {
        for (pattern, text) in [
            (".*?", "a"),
            (".*?", "😀"),
            ("(?:|a)", "a"),
            ("(?:|😀)", "😀"),
        ] {
            let findall = json!({"pattern":pattern,"method":"findall","args":[text]});
            assert_eq!(run_case(&findall).unwrap(), json!(["", text, ""]));
            let finditer = json!({"pattern":pattern,"method":"finditer","args":[text]});
            let matches = run_case(&finditer).unwrap();
            let spans: Vec<_> = matches
                .as_array()
                .unwrap()
                .iter()
                .map(|m| m["$match"]["span"].clone())
                .collect();
            assert_eq!(spans, vec![json!([0, 0]), json!([0, 1]), json!([1, 1])]);
            let split = json!({"pattern":pattern,"method":"split","args":[text]});
            assert_eq!(run_case(&split).unwrap(), json!(["", "", "", ""]));
            let sub = json!({"pattern":pattern,"method":"sub","args":["-",text]});
            assert_eq!(run_case(&sub).unwrap(), json!("---"));
            let subn = json!({"pattern":pattern,"method":"subn","args":["-",text]});
            assert_eq!(run_case(&subn).unwrap(), json!({"$tuple":["---",3]}));
        }
        let empty = json!({"pattern":".*?","method":"findall","args":[""]});
        assert_eq!(run_case(&empty).unwrap(), json!([""]));
        let later = json!({"pattern":"(?:|a)","method":"findall","args":["😀"]});
        assert_eq!(run_case(&later).unwrap(), json!(["", ""]));
        let slice = json!({"pattern":".*?","method":"finditer","args":["x😀aZ",1,3]});
        let matches = run_case(&slice).unwrap();
        let spans: Vec<_> = matches
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["$match"]["span"].clone())
            .collect();
        assert_eq!(
            spans,
            vec![
                json!([1, 1]),
                json!([1, 2]),
                json!([2, 2]),
                json!([2, 3]),
                json!([3, 3])
            ]
        );
    }
    #[test]
    fn invalid_replacements_raise_without_matches() {
        for (template, kind) in [
            (r"\2", "PatternError"),
            (r"\g<missing>", "IndexError"),
            (r"\q", "PatternError"),
        ] {
            for count in [0, -1] {
                let case = json!({"pattern":"(a)","method":"sub","args":[template,"z",count]});
                assert_eq!(run_case(&case).unwrap_err().kind, kind);
            }
        }
        let case = json!({"pattern":"a","method":"sub","args":[42,"z"]});
        assert_eq!(run_case(&case).unwrap_err().kind, "TypeError");
    }
    #[test]
    fn match_slices_and_captures() {
        let case = json!({"pattern":"(a(b))","method":"search","args":["😀ab",1,3]});
        assert_eq!(run_case(&case).unwrap()["$match"]["lastindex"], 1);
        let case = json!({"pattern":"(,)","method":"split","args":["a,b,c",1]});
        assert_eq!(run_case(&case).unwrap(), json!(["a", ",", "b,c"]));
    }
}
