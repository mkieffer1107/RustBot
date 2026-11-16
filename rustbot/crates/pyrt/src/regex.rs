//! Initial CPython `re.Pattern` compatibility layer.
use crate::{PyExc, PyResult, unicode_data as unicode};
use fancy_regex::{Captures, Regex};
use serde_json::{Value, json};
use std::{
    cell::{OnceCell, RefCell},
    collections::VecDeque,
    rc::Rc,
    sync::OnceLock,
};

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
// Recognize a deliberately small fixed-width lookbehind subset. Fancy-regex
// permits unequal-width literal alternatives, whereas CPython rejects them.
// Return None for complex expressions so they remain the engine's concern.
fn simple_lookbehind_widths(
    chars: impl Iterator<Item = char>,
    verbose: bool,
) -> Option<Vec<usize>> {
    let mut chars = chars.peekable();
    let mut widths = vec![0usize];
    while let Some(c) = chars.next() {
        if verbose && (c.is_ascii_whitespace() || c == '#') {
            return None;
        }
        match c {
            ')' => return Some(widths),
            '|' => widths.push(0),
            '\\' => {
                let escaped = chars.next()?;
                if escaped.is_ascii_digit()
                    || matches!(escaped, 'A' | 'Z' | 'z' | 'b' | 'B' | 'N' | 'u' | 'U' | 'x')
                {
                    return None;
                }
                *widths.last_mut()? += 1;
            }
            '[' => {
                let mut first = true;
                let mut closed = false;
                while let Some(item) = chars.next() {
                    if item == '\\' {
                        chars.next()?;
                        first = false;
                    } else if item == ']' && !first {
                        closed = true;
                        break;
                    } else if item != '^' {
                        first = false;
                    }
                }
                if !closed {
                    return None;
                }
                *widths.last_mut()? += 1;
            }
            '(' | '*' | '+' | '?' | '{' | '}' | '^' | '$' => return None,
            _ => *widths.last_mut()? += 1,
        }
    }
    None
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| unicode::property(c, 1 << 10))
        && chars.all(|c| unicode::property(c, 1 << 11))
}

fn class_contains_i(source: &str) -> bool {
    let chars: Vec<_> = source.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if matches!(chars[i], 'i' | 'I' | 'İ' | 'ı') {
            return true;
        }
        if i + 2 < chars.len() && chars[i + 1] == '-' {
            for ch in ['i', 'I', 'İ', 'ı'] {
                if chars[i] <= ch && ch <= chars[i + 2] {
                    return true;
                }
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    false
}

fn validate_class_ranges(source: &str) -> PyResult<()> {
    // Preserve the distinction between a literal hyphen and an escaped one,
    // and between character categories and single-character range endpoints.
    let mut chars = source.chars().peekable();
    let mut tokens: Vec<(char, bool, bool)> = Vec::new();
    if chars.peek() == Some(&'^') {
        chars.next();
    }
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                tokens.push((next, "dDsSwW".contains(next), true));
            }
        } else {
            tokens.push((c, false, false));
        }
    }
    let mut i = 0;
    while i + 2 < tokens.len() {
        let left = tokens[i];
        let middle = tokens[i + 1];
        let right = tokens[i + 2];
        if middle.0 == '-' && !middle.2 {
            if left.1 || right.1 || left.0 > right.0 {
                return Err(pattern_error("bad character range"));
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    Ok(())
}

fn ascii_class_extras(source: &str) -> String {
    let mut chars = source.chars().peekable();
    let mut tokens = Vec::new();
    if chars.peek() == Some(&'^') {
        chars.next();
    }
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                tokens.push((next, "dDsSwW".contains(next), true));
            }
        } else {
            tokens.push((c, false, false));
        }
    }
    let mut extras = String::new();
    let mut i = 0;
    while i < tokens.len() {
        if i + 2 < tokens.len()
            && tokens[i + 1].0 == '-'
            && !tokens[i + 1].2
            && !tokens[i].1
            && !tokens[i + 2].1
        {
            for n in b'A'..=b'z' {
                let x = n as char;
                if x.is_ascii_alphabetic() && tokens[i].0 <= x && x <= tokens[i + 2].0 {
                    extras.push(x.to_ascii_uppercase());
                    extras.push(x.to_ascii_lowercase());
                }
            }
            i += 3;
        } else {
            let (c, category, _) = tokens[i];
            if !category && c.is_ascii_alphabetic() {
                extras.push(c.to_ascii_uppercase());
                extras.push(c.to_ascii_lowercase());
            }
            i += 1;
        }
    }
    extras
}

/// Convert character classes for Python semantics while leaving regex syntax intact.
fn translate(pattern: &str, flags: i64) -> PyResult<(String, i64)> {
    if flags & 4 != 0 {
        return Err(PyExc::value_error(
            "LOCALE cannot be used with a str pattern",
        ));
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
    let mut global_unicode = flags & 32 != 0;
    let mut p = pattern;
    let mut inline = String::new();
    for (bit, ch) in [(2, 'i'), (8, 'm'), (16, 's'), (64, 'x')] {
        if flags & bit != 0 {
            inline.push(ch);
        }
    }
    // Python allows adjacent global flag groups, and ignores leading
    // whitespace and comments when verbose mode is already enabled.
    loop {
        if inline.contains('x') {
            loop {
                p = p.trim_start_matches(|c| {
                    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}')
                });
                if p.starts_with('#') {
                    p = p.find('\n').map_or("", |end| &p[end + 1..]);
                } else {
                    break;
                }
            }
        }
        if !p.starts_with("(?") {
            break;
        }
        let Some(end) = p.find(')') else {
            break;
        };
        let cand = &p[2..end];
        if cand.is_empty() || !cand.chars().all(|c| "aiLmsux".contains(c)) {
            break;
        }
        if cand.contains('a') && cand.contains('u') {
            return Err(pattern_error("ASCII and UNICODE flags are incompatible"));
        }
        if (cand.contains('a') && global_unicode) || (cand.contains('u') && ascii) {
            return Err(PyExc::value_error(
                "ASCII and UNICODE flags are incompatible",
            ));
        }
        for c in cand.chars() {
            match c {
                'a' => ascii = true,
                'u' => global_unicode = true,
                'i' | 'm' | 's' | 'x' => {
                    if !inline.contains(c) {
                        inline.push(c);
                    }
                }
                _ => return Err(pattern_error("invalid inline flags")),
            }
        }
        p = &p[end + 1..];
    }
    let mut effective_flags = flags;
    for (bit, ch) in [(2, 'i'), (8, 'm'), (16, 's'), (64, 'x')] {
        if inline.contains(ch) {
            effective_flags |= bit;
        }
    }
    if ascii {
        effective_flags |= 256;
    } else {
        effective_flags |= 32;
    }
    let mut out = String::new();
    if !inline.is_empty() {
        out.push_str("(?");
        out.push_str(&inline);
        out.push(')');
    }
    let mut chars = p.chars().peekable();
    let mut multiline = inline.contains('m');
    let mut verbose = inline.contains('x');
    let mut ignorecase = inline.contains('i');
    let mut class_source = String::new();
    let mut modes = Vec::new();
    let mut group_names: Vec<(String, usize)> = Vec::new();
    let mut lookbehinds: Vec<(usize, usize)> = Vec::new();
    let mut open_named_groups: Vec<(String, usize)> = Vec::new();
    let mut open_numbered_groups: Vec<(usize, usize)> = Vec::new();
    let mut group_count = 0usize;
    let mut in_class = false;
    let mut class_first = false;
    while let Some(c) = chars.next() {
        if !in_class && verbose && c == '#' {
            // Remove the comment so wrappers appended by the runtime cannot
            // accidentally become part of a trailing verbose-mode comment.
            for comment in chars.by_ref() {
                if comment == '\n' {
                    out.push('\n');
                    break;
                }
            }
            continue;
        }
        if !in_class && c == '(' && chars.peek() == Some(&'?') {
            let mut behind = chars.clone();
            if behind.next() == Some('?')
                && behind.next() == Some('<')
                && matches!(behind.next(), Some('=' | '!'))
                && let Some(widths) = simple_lookbehind_widths(behind, verbose)
                && widths.iter().any(|width| *width != widths[0])
            {
                return Err(pattern_error("look-behind requires fixed-width pattern"));
            }
            let mut syntax = chars.clone();
            syntax.next();
            match syntax.next() {
                Some('<') if !matches!(syntax.next(), Some('=' | '!')) => {
                    return Err(pattern_error("unknown extension"));
                }
                Some('\'') | Some('|') => return Err(pattern_error("unknown extension")),
                _ => {}
            }
            let mut conditional = chars.clone();
            conditional.next();
            if conditional.next() == Some('(') {
                let name: String = conditional.by_ref().take_while(|c| *c != ')').collect();
                if !(is_identifier(&name)
                    || (!name.is_empty() && name.chars().all(|c| c.is_ascii_digit())))
                {
                    return Err(pattern_error("invalid conditional group"));
                }
                modes.push((ascii, multiline, verbose, ignorecase));
                chars.next();
                chars.next();
                for _ in name.chars() {
                    chars.next();
                }
                if chars.next() != Some(')') {
                    return Err(pattern_error("unterminated conditional group"));
                }
                if name.chars().all(|c| c.is_ascii_digit()) {
                    out.push_str(&format!("(?({name})"));
                } else {
                    out.push_str(&format!("(?(<{name}>)"));
                }
                continue;
            }
            let mut look = chars.clone();
            look.next();
            if look.next() == Some('P') && matches!(look.next(), Some('<' | '=')) {
                let mut header = chars.clone();
                header.next();
                header.next();
                let delimiter = header.next().unwrap();
                let stop = if delimiter == '<' { '>' } else { ')' };
                let name: String = header.take_while(|c| *c != stop).collect();
                if !is_identifier(&name) {
                    return Err(pattern_error("invalid group name"));
                }
                if delimiter == '<' {
                    if group_names.iter().any(|(existing, _)| existing == &name) {
                        return Err(pattern_error("duplicate group name"));
                    }
                    group_count += 1;
                    group_names.push((name.clone(), group_count));
                    open_named_groups.push((name, modes.len()));
                    open_numbered_groups.push((group_count, modes.len()));
                    modes.push((ascii, multiline, verbose, ignorecase));
                } else {
                    let index = group_names
                        .iter()
                        .find(|(existing, _)| existing == &name)
                        .map(|(_, index)| *index);
                    if index.is_none()
                        || open_named_groups.iter().any(|(open, _)| open == &name)
                        || index.is_some_and(|index| {
                            lookbehinds.iter().any(|(_, start)| index > *start)
                        })
                    {
                        return Err(pattern_error("invalid group reference"));
                    }
                }
                out.push('(');
                for item in chars.by_ref() {
                    out.push(item);
                    if item == stop {
                        break;
                    }
                }
                continue;
            }
            let mut look = chars.clone();
            look.next();
            if look.peek() == Some(&'#') {
                out.push_str("(?#");
                chars.next();
                chars.next();
                let mut closed = false;
                for comment in chars.by_ref() {
                    out.push(comment);
                    if comment == ')' {
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    return Err(pattern_error("unterminated comment"));
                }
                continue;
            }
            let spec: String = look
                .by_ref()
                .take_while(|c| "aiLmsux-".contains(*c))
                .collect();
            let delimiter = look.next();
            // take_while consumes the first delimiter from this lookahead.
            let mut inspect = chars.clone();
            inspect.next();
            for _ in spec.chars() {
                inspect.next();
            }
            let delimiter = inspect.next().or(delimiter);
            if !spec.is_empty() && matches!(delimiter, Some(':' | ')')) {
                let (on, off) = spec.split_once('-').unwrap_or((&spec, ""));
                if on.contains('L')
                    || off.chars().any(|x| "aLu".contains(x))
                    || (on.contains('a') && on.contains('u'))
                    || (spec.contains('-') && off.is_empty())
                    || on.chars().any(|x| off.contains(x))
                {
                    return Err(pattern_error("invalid inline flags"));
                }
                if delimiter == Some(')') {
                    return Err(pattern_error("global flags not at the start"));
                }
                modes.push((ascii, multiline, verbose, ignorecase));
                if on.contains('i') {
                    ignorecase = true;
                }
                if off.contains('i') {
                    ignorecase = false;
                }
                if on.contains('a') {
                    ascii = true;
                }
                if on.contains('u') {
                    ascii = false;
                }
                if on.contains('m') {
                    multiline = true;
                }
                if off.contains('m') {
                    multiline = false;
                }
                if on.contains('x') {
                    verbose = true;
                }
                if off.contains('x') {
                    verbose = false;
                }
                let on_rust: String = on.chars().filter(|x| "imsx".contains(*x)).collect();
                let off_rust: String = off.chars().filter(|x| "imsx".contains(*x)).collect();
                out.push_str("(?");
                out.push_str(&on_rust);
                if !off_rust.is_empty() {
                    out.push('-');
                    out.push_str(&off_rust);
                }
                out.push(':');
                for _ in 0..spec.chars().count() + 2 {
                    chars.next();
                }
                continue;
            }
        }
        if !in_class && c == '(' {
            let mut prefix = chars.clone();
            if prefix.next() == Some('?')
                && prefix.next() == Some('<')
                && matches!(prefix.next(), Some('=' | '!'))
            {
                lookbehinds.push((modes.len(), group_count));
            }
            if chars.peek() != Some(&'?') {
                group_count += 1;
                open_numbered_groups.push((group_count, modes.len()));
            }
            modes.push((ascii, multiline, verbose, ignorecase));
        }
        if !in_class
            && c == ')'
            && let Some((a, m, x, ic)) = modes.pop()
        {
            ascii = a;
            multiline = m;
            verbose = x;
            ignorecase = ic;
            if open_named_groups
                .last()
                .is_some_and(|(_, depth)| *depth == modes.len())
            {
                open_named_groups.pop();
            }
            if open_numbered_groups
                .last()
                .is_some_and(|(_, depth)| *depth == modes.len())
            {
                open_numbered_groups.pop();
            }
            if lookbehinds
                .last()
                .is_some_and(|(depth, _)| *depth == modes.len())
            {
                lookbehinds.pop();
            }
        }
        if in_class && class_first && c == '-' && chars.peek() == Some(&'-') {
            // Python treats [--a] as a range, while regex-syntax uses -- as set difference.
            out.push_str(r"\-");
            class_source.push('-');
            class_first = false;
            continue;
        }
        if in_class && matches!(c, '&' | '~' | '|') && chars.peek() == Some(&c) {
            class_source.push(c);
            class_source.push(c);
            out.push('\\');
            out.push(c);
            out.push('\\');
            out.push(c);
            chars.next();
            class_first = false;
            continue;
        }
        if c == '\\' {
            let Some(n) = chars.next() else {
                return Err(pattern_error("bad escape"));
            };
            if in_class && !matches!(n, 'x' | 'u' | 'U' | '0'..='9') {
                if let Some(control) = match n {
                    'a' => Some('\u{7}'),
                    'b' => Some('\u{8}'),
                    't' => Some('\t'),
                    'n' => Some('\n'),
                    'v' => Some('\u{b}'),
                    'f' => Some('\u{c}'),
                    'r' => Some('\r'),
                    _ => None,
                } {
                    class_source.push(control);
                } else {
                    class_source.push('\\');
                    class_source.push(n);
                }
            }
            match n {
                'w' | 'd' | 's' | 'W' | 'D' | 'S' => {
                    let neg = n.is_ascii_uppercase();
                    let body = class_body(n.to_ascii_lowercase(), ascii);
                    if in_class {
                        if neg {
                            out.push_str("[^");
                        }
                        out.push_str(body);
                        if neg {
                            out.push(']');
                        }
                    } else {
                        if ascii && ignorecase {
                            out.push_str("(?-i:");
                        }
                        out.push('[');
                        if neg {
                            out.push('^');
                        }
                        out.push_str(body);
                        out.push(']');
                        if ascii && ignorecase {
                            out.push(')');
                        }
                    }
                }
                'b' | 'B' if !in_class => {
                    if ascii && ignorecase {
                        out.push_str("(?-i:");
                    }
                    if n == 'B' {
                        out.push_str("(?!(?:");
                        out.push_str(boundary(ascii));
                        out.push_str("))");
                    } else {
                        out.push_str(boundary(ascii));
                    }
                    if ascii && ignorecase {
                        out.push(')');
                    }
                }
                'b' if in_class => out.push_str(r"\x{8}"),
                'A' if in_class => return Err(pattern_error("bad escape in character class")),
                'Z' | 'z' if !in_class => out.push_str(r"\z"),
                'a' => out.push_str(r"\x{7}"),
                'v' => out.push_str(r"\x{b}"),
                'f' => out.push_str(r"\x{c}"),
                'x' | 'u' | 'U' => {
                    let count = match n {
                        'x' => 2,
                        'u' => 4,
                        _ => 8,
                    };
                    let digits: String = chars.by_ref().take(count).collect();
                    if digits.len() != count || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
                        return Err(pattern_error("incomplete Unicode escape"));
                    }
                    let value = u32::from_str_radix(&digits, 16).map_err(re_error)?;
                    let ch = char::from_u32(value)
                        .ok_or_else(|| pattern_error("invalid Unicode escape"))?;
                    if in_class {
                        class_source.push(ch);
                    }
                    if !in_class && ignorecase && !ascii && matches!(ch, 'i' | 'I' | 'İ' | 'ı') {
                        out.push_str("[iIİı]");
                    } else if !in_class && ignorecase && ascii && ch.is_ascii_alphabetic() {
                        out.push_str(&format!(
                            "(?-i:[{}{}])",
                            ch.to_ascii_lowercase(),
                            ch.to_ascii_uppercase()
                        ));
                    } else if !in_class && ignorecase && ascii && !ch.is_ascii() {
                        out.push_str(&format!(r"(?-i:\x{{{value:x}}})"));
                    } else {
                        out.push_str(&format!(r"\x{{{value:x}}}"));
                    }
                }
                '0'..='9' => {
                    let mut look = chars.clone();
                    let a = look.next();
                    let b = look.next();
                    let octal = n == '0'
                        || (in_class && n <= '7')
                        || (n <= '7'
                            && a.is_some_and(|x| ('0'..='7').contains(&x))
                            && b.is_some_and(|x| ('0'..='7').contains(&x)));
                    if octal {
                        let mut value = n.to_digit(8).unwrap();
                        for _ in 0..2 {
                            if chars.peek().is_some_and(|x| ('0'..='7').contains(x)) {
                                value = value * 8 + chars.next().unwrap().to_digit(8).unwrap();
                            } else {
                                break;
                            }
                        }
                        if value > 255 {
                            return Err(pattern_error("octal escape outside range"));
                        }
                        let ch = char::from_u32(value).unwrap();
                        if in_class {
                            class_source.push(ch);
                        }
                        if !in_class && ignorecase && !ascii && matches!(ch, 'i' | 'I' | 'İ' | 'ı')
                        {
                            out.push_str("[iIİı]");
                        } else if !in_class && ignorecase && ascii && ch.is_ascii_alphabetic() {
                            out.push_str(&format!(
                                "(?-i:[{}{}])",
                                ch.to_ascii_lowercase(),
                                ch.to_ascii_uppercase()
                            ));
                        } else if !in_class && ignorecase && ascii && !ch.is_ascii() {
                            out.push_str(&format!(r"(?-i:\x{{{value:x}}})"));
                        } else {
                            out.push_str(&format!(r"\x{{{value:x}}}"));
                        }
                    } else if in_class {
                        return Err(pattern_error("invalid group reference in character class"));
                    } else {
                        let mut index = n.to_digit(10).unwrap() as usize;
                        let second = if chars.peek().is_some_and(char::is_ascii_digit) {
                            chars.next()
                        } else {
                            None
                        };
                        if let Some(digit) = second {
                            index = index * 10 + digit.to_digit(10).unwrap() as usize;
                        }
                        if index > group_count
                            || open_numbered_groups.iter().any(|(open, _)| *open == index)
                            || lookbehinds.iter().any(|(_, start)| index > *start)
                        {
                            return Err(pattern_error("invalid group reference"));
                        }
                        out.push('\\');
                        out.push(n);
                        if let Some(digit) = second {
                            out.push(digit);
                        }
                    }
                }
                'N' => {
                    return Err(PyExc::not_implemented_error(
                        "named Unicode escapes are not implemented",
                    ));
                }
                n if n.is_ascii_alphabetic() && !matches!(n, 'A' | 'n' | 'r' | 't' | 'x') => {
                    return Err(pattern_error(format!("bad escape \\{n}")));
                }
                _ => {
                    out.push('\\');
                    out.push(n);
                }
            }
            class_first = false;
        } else {
            let closes_class = c == ']' && in_class && !class_first;
            if c == '[' && !in_class {
                if ascii && ignorecase {
                    out.push_str("(?-i:");
                }
                in_class = true;
                class_first = true;
                class_source.clear();
            } else if c == ']' && in_class && !class_first {
                validate_class_ranges(&class_source)?;
                if ignorecase && !ascii && class_contains_i(&class_source) {
                    out.push_str("iIİı");
                }
                if ignorecase && ascii {
                    out.push_str(&ascii_class_extras(&class_source));
                }
                in_class = false;
            } else if c != '^' || !class_source.is_empty() {
                class_first = false;
            }
            if in_class && c != '[' {
                class_source.push(c);
            }
            if c == '[' && in_class && !class_first {
                out.push_str(r"\[");
            } else if c == '$' && !in_class && !multiline {
                out.push_str(r"(?=\n?\z)");
            } else if !in_class && ignorecase && !ascii && matches!(c, 'i' | 'I' | 'İ' | 'ı') {
                out.push_str("[iIİı]");
            } else if !in_class && ignorecase && ascii && c.is_ascii_alphabetic() {
                out.push_str(&format!(
                    "(?-i:[{}{}])",
                    c.to_ascii_lowercase(),
                    c.to_ascii_uppercase()
                ));
            } else if !in_class && ignorecase && ascii && !c.is_ascii() {
                out.push_str(&format!("(?-i:{c})"));
            } else {
                out.push(c);
            }
            if closes_class && ascii && ignorecase {
                out.push(')');
            }
        }
    }
    Ok((out, effective_flags))
}
/// Capture nesting and textual closing order help reconstruct CPython's
/// lastindex when captures end at the same position or inside lookaround.
/// The engine does not expose capture-completion events, so repeated
/// alternatives can still be ambiguous.
#[derive(Debug)]
struct GroupOrder {
    parents: Vec<Option<usize>>,
    rank: Vec<usize>,
}
fn group_order(pattern: &str, groups: usize) -> Option<GroupOrder> {
    let cs: Vec<char> = pattern.chars().collect();
    let mut parents = vec![None];
    let mut rank = vec![0];
    let mut stack: Vec<Option<usize>> = Vec::new();
    let (mut i, mut class_depth, mut ordinal) = (0, 0, 0);
    while i < cs.len() {
        let c = cs[i];
        if c == '\\' {
            i += 2;
            continue;
        }
        if c == '[' {
            class_depth += 1;
            i += 1;
            continue;
        }
        if c == ']' && class_depth > 0 {
            class_depth -= 1;
            i += 1;
            continue;
        }
        if class_depth > 0 {
            i += 1;
            continue;
        }
        if c == '(' && cs.get(i + 1) == Some(&'?') && cs.get(i + 2) == Some(&'#') {
            i += 3;
            while i < cs.len() && cs[i] != ')' {
                i += 1;
            }
            i += 1;
            continue;
        }
        if c == '(' {
            let capture = cs.get(i + 1) != Some(&'?')
                || (cs.get(i + 2) == Some(&'P') && cs.get(i + 3) == Some(&'<'));
            if capture {
                let group = parents.len();
                parents.push(stack.iter().rev().find_map(|x| *x));
                rank.push(0);
                stack.push(Some(group));
            } else {
                stack.push(None);
                // A conditional's parenthesized group reference is metadata,
                // not another capture or enclosing regex group.
                if cs.get(i + 1) == Some(&'?') && cs.get(i + 2) == Some(&'(') {
                    i += 3;
                    while i < cs.len() && cs[i] != ')' {
                        i += 1;
                    }
                }
            }
        } else if c == ')'
            && let Some(Some(group)) = stack.pop()
        {
            ordinal += 1;
            rank[group] = ordinal;
        }
        i += 1;
    }
    if parents.len() == groups && stack.is_empty() {
        Some(GroupOrder { parents, rank })
    } else {
        None
    }
}
/// A compiled Python string pattern and its Python-visible metadata.
/// Instances are shared by a bounded per-thread cache, which works on native
/// and on single-threaded WebAssembly without global synchronization.
#[derive(Debug)]
pub struct CompiledPattern {
    pattern: String,
    flags: i64,
    translated: String,
    regex: Regex,
    order: Option<GroupOrder>,
    fullmatch: OnceCell<Regex>,
    nonempty_at_origin: OnceCell<Regex>,
}
impl CompiledPattern {
    pub fn pattern(&self) -> &str {
        &self.pattern
    }
    pub fn flags(&self) -> i64 {
        self.flags
    }
    pub fn group_count(&self) -> usize {
        self.regex.captures_len() - 1
    }
    pub fn group_names(&self) -> Vec<Option<String>> {
        self.regex
            .capture_names()
            .skip(1)
            .map(|name| name.map(str::to_owned))
            .collect()
    }
    fn fullmatch_regex(&self) -> PyResult<&Regex> {
        if self.fullmatch.get().is_none() {
            let regex = Regex::new(&format!(r"(?:{})\z", self.translated)).map_err(re_error)?;
            let _ = self.fullmatch.set(regex);
        }
        Ok(self.fullmatch.get().unwrap())
    }
    fn nonempty_regex(&self) -> PyResult<&Regex> {
        if self.nonempty_at_origin.get().is_none() {
            let regex = Regex::new(&format!(r"(?:{})(?!\G)", self.translated)).map_err(re_error)?;
            let _ = self.nonempty_at_origin.set(regex);
        }
        Ok(self.nonempty_at_origin.get().unwrap())
    }
}
const PATTERN_CACHE_SIZE: usize = 64;
type CacheEntry = ((String, i64), Rc<CompiledPattern>);
thread_local! { static PATTERN_CACHE: RefCell<VecDeque<CacheEntry>> = const { RefCell::new(VecDeque::new()) }; }
/// Compile a Python pattern, or reuse an equivalent cached compiled pattern.
pub fn compile_pattern(pattern: &str, flags: i64) -> PyResult<Rc<CompiledPattern>> {
    let hit = PATTERN_CACHE.with_borrow_mut(|cache| {
        let index = cache
            .iter()
            .position(|(key, _)| key.0 == pattern && key.1 == flags)?;
        let entry = cache.remove(index).unwrap();
        let result = Rc::clone(&entry.1);
        cache.push_back(entry);
        Some(result)
    });
    if let Some(hit) = hit {
        return Ok(hit);
    }
    let (translated, effective_flags) = translate(pattern, flags)?;
    let regex = Regex::new(&translated).map_err(re_error)?;
    let order = if regex.captures_len() > 1 {
        group_order(&translated, regex.captures_len())
    } else {
        None
    };
    let compiled = Rc::new(CompiledPattern {
        pattern: pattern.to_owned(),
        flags: effective_flags,
        translated,
        regex,
        order,
        fullmatch: OnceCell::new(),
        nonempty_at_origin: OnceCell::new(),
    });
    PATTERN_CACHE.with_borrow_mut(|cache| {
        if cache.len() == PATTERN_CACHE_SIZE {
            cache.pop_front();
        }
        cache.push_back(((pattern.to_owned(), flags), Rc::clone(&compiled)));
    });
    Ok(compiled)
}
fn ancestor(order: &GroupOrder, parent: usize, mut child: usize) -> bool {
    while let Some(next) = order.parents[child] {
        if next == parent {
            return true;
        }
        child = next;
    }
    false
}
fn match_json(c: &Captures<'_>, re: &Regex, pos: &[usize], order: Option<&GroupOrder>) -> Value {
    let m = c.get(0).unwrap();
    let mut groups = Vec::new();
    let mut spans = Vec::new();
    let mut last = None;
    let mut last_end = 0;
    for i in 1..re.captures_len() {
        if let Some(x) = c.get(i) {
            groups.push(json!(x.as_str()));
            spans.push(json!([codepoint(x.start(), pos), codepoint(x.end(), pos)]));
            let later = match (last, order) {
                (None, _) => true,
                (Some(previous), Some(order)) if ancestor(order, i, previous) => true,
                (Some(previous), Some(order)) if ancestor(order, previous, i) => false,
                (Some(previous), Some(order)) => {
                    x.end() > last_end
                        || (x.end() == last_end && order.rank[i] > order.rank[previous])
                }
                (Some(_), None) => x.end() > last_end,
            };
            if later {
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
                let group = if !name.is_empty() && name.chars().all(|x| x.is_ascii_digit()) {
                    let i = name
                        .parse::<usize>()
                        .map_err(|_| pattern_error("invalid group reference"))?;
                    if i >= re.captures_len() {
                        return Err(pattern_error("invalid group reference"));
                    }
                    c.and_then(|captures| captures.get(i))
                } else {
                    if !is_identifier(&name) {
                        return Err(pattern_error("invalid group name"));
                    }
                    if !re.capture_names().flatten().any(|n| n == name) {
                        return Err(PyExc::index_error("unknown group name"));
                    }
                    c.and_then(|captures| captures.name(&name))
                };
                if let Some(g) = group {
                    out.push_str(g.as_str());
                }
            }
            '0'..='9' => {
                let mut look = chars.clone();
                let a = look.next();
                let b = look.next();
                let is_octal = n == '0'
                    || (n <= '7'
                        && a.is_some_and(|x| ('0'..='7').contains(&x))
                        && b.is_some_and(|x| ('0'..='7').contains(&x)));
                if is_octal {
                    let mut octal = n.to_digit(8).unwrap();
                    for _ in 0..2 {
                        if chars.peek().is_some_and(|x| ('0'..='7').contains(x)) {
                            octal = octal * 8 + chars.next().unwrap().to_digit(8).unwrap();
                        } else {
                            break;
                        }
                    }
                    if octal > 255 {
                        return Err(pattern_error("octal escape outside range"));
                    }
                    out.push(char::from_u32(octal).unwrap());
                } else {
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
/// Substitute matches with a callback invoked lazily in match order.
/// The callback receives canonical `$match` JSON; returning None inserts an
/// empty string, while an error stops iteration immediately.
pub fn sub_with(
    compiled: &CompiledPattern,
    text: &str,
    count: i64,
    mut replace: impl FnMut(Value) -> PyResult<Option<String>>,
) -> PyResult<(String, usize)> {
    if count < 0 {
        return Ok((text.to_owned(), 0));
    }
    let positions = positions(text);
    let mut output = String::new();
    let (mut at, mut tail, mut total) = (0, 0, 0usize);
    let mut previous_was_empty = false;
    loop {
        if count > 0 && total >= usize::try_from(count).unwrap_or(usize::MAX) {
            break;
        }
        let re = if previous_was_empty {
            compiled.nonempty_regex()?
        } else {
            &compiled.regex
        };
        let Some(captures) = re.captures_from_pos(text, at).map_err(re_error)? else {
            break;
        };
        let matched = captures.get(0).unwrap();
        output.push_str(&text[tail..matched.start()]);
        let match_value = match_json(
            &captures,
            &compiled.regex,
            &positions,
            compiled.order.as_ref(),
        );
        if let Some(replacement) = replace(match_value)? {
            output.push_str(&replacement);
        }
        tail = matched.end();
        total += 1;
        previous_was_empty = matched.start() == matched.end();
        at = matched.end();
        if previous_was_empty && at == text.len() {
            break;
        }
    }
    output.push_str(&text[tail..]);
    Ok((output, total))
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
    let compiled = compile_pattern(pattern, flags)?;
    run_compiled(&compiled, method, args)
}
/// Execute a method using a retained compiled pattern, even after cache eviction.
/// Arguments and result use the same canonical JSON representation as run_case.
pub fn run_compiled(compiled: &CompiledPattern, method: &str, args: &[Value]) -> PyResult<Value> {
    let re = if method == "fullmatch" {
        compiled.fullmatch_regex()?
    } else {
        &compiled.regex
    };
    let order = compiled.order.as_ref();
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
                match_json(&c, re, &pos, order)
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
        replacement(as_str(&args[0])?, None, re)?;
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
    let mut previous_was_empty = false;
    let mut tail = 0;
    let mut count = 0usize;
    let mut items = Vec::new();
    let mut output = String::new();
    loop {
        if limit > 0 && count >= usize::try_from(limit).unwrap_or(usize::MAX) {
            break;
        }
        // After an empty match, Python retries at the same position, forbidding
        // another empty match there while permitting a nonempty alternative.
        // Fancy-regex's \G anchors to captures_from_pos's search origin.
        let active = if previous_was_empty {
            compiled.nonempty_regex()?
        } else {
            re
        };
        let Some(c) = active.captures_from_pos(input, at).map_err(re_error)? else {
            break;
        };
        let m = c.get(0).unwrap();
        match method {
            "finditer" => items.push(match_json(&c, re, &pos, order)),
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
                    output.push_str(&replacement(as_str(repl)?, Some(&c), re)?);
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
    fn scoped_flags_negated_categories_and_unknown_escapes() {
        let cases = [
            (
                json!({"pattern":r"(?a:\w+)(?u:\w+)","method":"findall","args":["abé"]}),
                json!(["abé"]),
            ),
            (
                json!({"pattern":r"(?a:\w+)","method":"findall","args":["abé"]}),
                json!(["ab"]),
            ),
            (
                json!({"pattern":r"(?m:a$)(?-m:a$)","method":"findall","args":["a\na"]}),
                json!([]),
            ),
            (
                json!({"pattern":r"(?i:a)(?-i:a)","method":"findall","args":["Aa AA aa"]}),
                json!(["Aa", "aa"]),
            ),
            (
                json!({"pattern":r"[\D_]+","method":"findall","args":["a3_٤😀"]}),
                json!(["a", "_", "😀"]),
            ),
            (
                json!({"pattern":r"[^\W_]+","method":"findall","args":["é_2 !"]}),
                json!(["é", "2"]),
            ),
            (
                json!({"pattern":r"\u0041\U0001f600","method":"findall","args":["A😀"]}),
                json!(["A😀"]),
            ),
        ];
        for (case, expected) in cases {
            assert_eq!(run_case(&case).unwrap(), expected, "{case}");
        }
        for pattern in [r"\p", r"\e", r"\h"] {
            assert_eq!(
                run_case(&json!({"pattern":pattern,"method":"search","args":["x"]}))
                    .unwrap_err()
                    .kind,
                "PatternError"
            );
        }
    }
    #[test]
    fn class_literals_and_verbose_trailing_comments() {
        for (pattern, expected) in [
            ("[a&&b]", "a&b"),
            ("[a~~b]", "a~b"),
            ("[a||b]", "a|b"),
            ("[a[b]", "a[b"),
        ] {
            let case = json!({"pattern":pattern,"method":"findall","args":[expected]});
            let values: Vec<_> = expected.chars().map(|c| json!(c.to_string())).collect();
            assert_eq!(run_case(&case).unwrap(), json!(values));
        }
        let case = json!({"pattern":"(?x) .*? # trailing", "method":"findall","args":["a"]});
        assert_eq!(run_case(&case).unwrap(), json!(["", "a", ""]));
        let case = json!({"pattern":"(?x) a # trailing", "method":"fullmatch","args":["a"]});
        assert_eq!(run_case(&case).unwrap()["$match"]["span"], json!([0, 1]));
    }
    #[test]
    fn callback_substitution_is_lazy_and_count_limited() {
        let compiled = compile_pattern(".*?", 0).unwrap();
        let mut spans = Vec::new();
        let result = sub_with(&compiled, "😀a", 3, |matched| {
            spans.push(matched["$match"]["span"].clone());
            Ok(Some("-".to_owned()))
        })
        .unwrap();
        assert_eq!(result, ("---a".to_owned(), 3));
        assert_eq!(spans, vec![json!([0, 0]), json!([0, 1]), json!([1, 1])]);
        let mut calls = 0;
        let err = sub_with(&compiled, "abc", 0, |_| {
            calls += 1;
            Err(PyExc::value_error("stop"))
        })
        .unwrap_err();
        assert_eq!(err.kind, "ValueError");
        assert_eq!(calls, 1);
        let result = sub_with(&compiled, "a", -1, |_| panic!("callback should not run")).unwrap();
        assert_eq!(result, ("a".to_owned(), 0));
    }
    #[test]
    fn compiled_pattern_cache_and_metadata() {
        let first = compile_pattern("(?i)(?P<word>a)(b)", 0).unwrap();
        let again = compile_pattern("(?i)(?P<word>a)(b)", 0).unwrap();
        assert!(Rc::ptr_eq(&first, &again));
        assert_eq!(first.pattern(), "(?i)(?P<word>a)(b)");
        assert_eq!(first.flags(), 34);
        assert_eq!(first.group_count(), 2);
        assert_eq!(first.group_names(), vec![Some("word".to_owned()), None]);
        let ascii = compile_pattern("(?a)hi", 0).unwrap();
        assert_eq!(ascii.flags(), 256);
        for index in 0..=PATTERN_CACHE_SIZE {
            compile_pattern(&format!("cache-{index}"), 0).unwrap();
        }
        let after = compile_pattern("(?i)(?P<word>a)(b)", 0).unwrap();
        assert!(!Rc::ptr_eq(&first, &after));
        assert_eq!(first.group_count(), 2);
        let result = run_compiled(&first, "search", &[json!("AB")]).unwrap();
        assert_eq!(result["$match"]["span"], json!([0, 2]));
    }
    #[test]
    fn python_group_name_validation() {
        for pattern in [
            r"(?P<1a>x)",
            r"(?P<a>x)(?P<a>x)",
            r"(?P<+a>x)",
            r"(?P<x>(?P=x))",
            r"(?P=x)(?P<x>a)",
        ] {
            assert_eq!(
                compile_pattern(pattern, 0).unwrap_err().kind,
                "PatternError"
            );
        }
        let pattern = compile_pattern(r"(?P<名>x)", 0).unwrap();
        assert_eq!(pattern.group_names(), vec![Some("名".to_owned())]);
    }
    #[test]
    fn lookbehind_literal_alternatives_require_equal_width() {
        let bad = json!({"pattern":r"(?<=ab|c)b","method":"search","args":["cb"]});
        assert_eq!(run_case(&bad).unwrap_err().kind, "PatternError");
        let good = json!({"pattern":r"(?<=ab|cd)e","method":"search","args":["cde"]});
        assert_eq!(run_case(&good).unwrap()["$match"]["span"], json!([2, 3]));
        let verbose = json!({"pattern":r"(?x)(?<=a |b)c","method":"search","args":["bc"]});
        assert_eq!(run_case(&verbose).unwrap()["$match"]["span"], json!([1, 2]));
    }
    #[test]
    fn character_class_ranges_follow_python_rules() {
        for pattern in [r"[\w-a]", r"[a-\w]", r"[a--b]", r"[\d-\w]"] {
            let case = json!({"pattern":pattern,"method":"findall","args":["a-b9"]});
            assert_eq!(
                run_case(&case).unwrap_err().kind,
                "PatternError",
                "{pattern}"
            );
        }
        for pattern in [r"[--a]", r"[\--a]"] {
            let case = json!({"pattern":pattern,"flags":258,"method":"findall","args":["a-b9"]});
            assert_eq!(
                run_case(&case).unwrap(),
                json!(["a", "-", "b", "9"]),
                "{pattern}"
            );
        }
    }
    #[test]
    fn adjacent_global_flags_and_verbose_prefix() {
        for pattern in ["(?i)(?m)a", "(?x) # note\n (?i)a"] {
            let case = json!({"pattern":pattern,"method":"search","args":["A"]});
            assert_eq!(run_case(&case).unwrap()["$match"]["span"], json!([0, 1]));
        }
        let case = json!({"pattern":"(?au)a","method":"search","args":["a"]});
        assert_eq!(run_case(&case).unwrap_err().kind, "PatternError");
        let case = json!({"pattern":"(?a)(?u)a","method":"search","args":["a"]});
        assert_eq!(run_case(&case).unwrap_err().kind, "ValueError");
    }
    #[test]
    fn case_insensitive_unicode_and_ascii_modes() {
        for pattern in ["i", "[i]", "[a-z]", r"\u0049", r"[\x49]"] {
            let case = json!({"pattern":pattern,"flags":2,"method":"findall","args":["iIİı"]});
            assert_eq!(
                run_case(&case).unwrap(),
                json!(["i", "I", "İ", "ı"]),
                "{pattern}"
            );
            let ascii = json!({"pattern":pattern,"flags":258,"method":"findall","args":["iIİı"]});
            assert_eq!(run_case(&ascii).unwrap(), json!(["i", "I"]), "{pattern}");
        }
        let case = json!({"pattern":"[a-z]","flags":258,"method":"findall","args":["ſKkK"]});
        assert_eq!(run_case(&case).unwrap(), json!(["k", "K"]));
        let case = json!({"pattern":"[^a-z]","flags":2,"method":"findall","args":["İıſK😀"]});
        assert_eq!(run_case(&case).unwrap(), json!(["😀"]));
        let categories =
            json!({"pattern":r"\b\w+\b","flags":258,"method":"findall","args":["ſK kK"]});
        assert_eq!(run_case(&categories).unwrap(), json!(["kK"]));
        let case = json!({"pattern":"(?P<identifier>i)","flags":2,"method":"search","args":["İ"]});
        assert_eq!(
            run_case(&case).unwrap()["$match"]["groupdict"]["identifier"],
            "İ"
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
        for template in [r"\g<-1>", r"\g<+1>", r"\g<1a>", r"\g<>", r"\g<١>"] {
            let case = json!({"pattern":"(a)","method":"sub","args":[template,"z"]});
            assert_eq!(run_case(&case).unwrap_err().kind, "PatternError");
        }
        let case = json!({"pattern":"a","method":"sub","args":[42,"z"]});
        assert_eq!(run_case(&case).unwrap_err().kind, "TypeError");
    }
    #[test]
    fn lastindex_accounts_for_group_completion_order() {
        for (pattern, text, last) in [
            ("(a)(b?)", "a", 2),
            ("((a))", "a", 1),
            ("(a(?=(b)))", "ab", 1),
            ("(?=(a+))(a)", "a", 2),
            ("(?:(a)|(b))+", "ba", 1),
            ("(?:(a)|(b))+", "ab", 2),
        ] {
            let case = json!({"pattern":pattern,"method":"search","args":[text]});
            assert_eq!(
                run_case(&case).unwrap()["$match"]["lastindex"],
                last,
                "{pattern}"
            );
        }
    }
    #[test]
    fn python_named_and_numbered_conditionals() {
        for pattern in [r"(?P<identifier>a)?(?(identifier)i|x)", r"(a)?(?(1)i|x)"] {
            let case = json!({"pattern":pattern,"flags":2,"method":"search","args":["aİ"]});
            assert_eq!(run_case(&case).unwrap()["$match"]["span"], json!([0, 2]));
            let case = json!({"pattern":pattern,"method":"search","args":["x"]});
            assert_eq!(run_case(&case).unwrap()["$match"]["span"], json!([0, 1]));
        }
    }
    #[test]
    fn pattern_octal_and_backreferences() {
        for (pattern, text, expected) in [
            (r"\0", "\0", "\0"),
            (r"\111", "I", "I"),
            (r"[\1]", "\u{1}", "\u{1}"),
        ] {
            let case = json!({"pattern":pattern,"method":"findall","args":[text]});
            assert_eq!(run_case(&case).unwrap(), json!([expected]));
        }
        for pattern in [r"\400", r"[\8]", r"\1", r"(a\1)", r"\1(a)", r"(a)(\2)"] {
            let case = json!({"pattern":pattern,"method":"search","args":["a"]});
            assert_eq!(run_case(&case).unwrap_err().kind, "PatternError");
        }
    }
    #[test]
    fn replacement_octal_and_group_references() {
        let case = json!({"pattern":"(a)","method":"sub","args":[r"\111\1\0","a"]});
        assert_eq!(run_case(&case).unwrap(), json!("Ia\u{0}"));
        let case = json!({"pattern":"a","method":"sub","args":[r"\400","z"]});
        assert_eq!(run_case(&case).unwrap_err().kind, "PatternError");
    }
    #[test]
    fn match_slices_and_captures() {
        let case = json!({"pattern":"(a(b))","method":"search","args":["😀ab",1,3]});
        assert_eq!(run_case(&case).unwrap()["$match"]["lastindex"], 1);
        let case = json!({"pattern":"(,)","method":"split","args":["a,b,c",1]});
        assert_eq!(run_case(&case).unwrap(), json!(["a", ",", "b,c"]));
    }
}
