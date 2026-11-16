//! Unicode 16.0.0 normalization and canonical combining classes.
//!
//! The generated tables come from the same database as CPython 3.14. The
//! runtime performs recursive decomposition, stable canonical ordering, and
//! blocked canonical composition; Hangul uses the Unicode syllable algorithm.

use crate::unicode_normalization_data::{COMBINING, COMPOSITION, DECOMPOSITION};
use crate::{PyExc, PyResult, Value};

const S_BASE: u32 = 0xac00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11a7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

pub fn combining(c: char) -> u8 {
    let codepoint = c as u32;
    let index = COMBINING.partition_point(|entry| entry.1 < codepoint);
    COMBINING
        .get(index)
        .filter(|entry| entry.0 <= codepoint)
        .map_or(0, |entry| entry.2)
}

fn push_decomposed(c: char, compatibility: bool, output: &mut Vec<(char, u8)>) {
    let point = c as u32;
    if (S_BASE..S_BASE + S_COUNT).contains(&point) {
        let syllable = point - S_BASE;
        output.push((char::from_u32(L_BASE + syllable / N_COUNT).unwrap(), 0));
        output.push((
            char::from_u32(V_BASE + syllable % N_COUNT / T_COUNT).unwrap(),
            0,
        ));
        if !syllable.is_multiple_of(T_COUNT) {
            output.push((char::from_u32(T_BASE + syllable % T_COUNT).unwrap(), 0));
        }
        return;
    }
    if let Ok(index) = DECOMPOSITION.binary_search_by_key(&point, |entry| entry.0) {
        let (_, is_compatibility, mapping) = DECOMPOSITION[index];
        if compatibility || !is_compatibility {
            for point in mapping {
                push_decomposed(
                    char::from_u32(*point).expect("Unicode scalar decomposition"),
                    compatibility,
                    output,
                );
            }
            return;
        }
    }
    output.push((c, combining(c)));
}

fn compose_pair(first: char, second: char) -> Option<char> {
    let (first, second) = (first as u32, second as u32);
    if (L_BASE..L_BASE + L_COUNT).contains(&first) && (V_BASE..V_BASE + V_COUNT).contains(&second) {
        return char::from_u32(S_BASE + (first - L_BASE) * N_COUNT + (second - V_BASE) * T_COUNT);
    }
    if (S_BASE..S_BASE + S_COUNT).contains(&first)
        && (first - S_BASE).is_multiple_of(T_COUNT)
        && (T_BASE + 1..T_BASE + T_COUNT).contains(&second)
    {
        return char::from_u32(first + second - T_BASE);
    }
    COMPOSITION
        .binary_search_by_key(&(first, second), |entry| (entry.0, entry.1))
        .ok()
        .and_then(|index| char::from_u32(COMPOSITION[index].2))
}

pub fn normalize(form: &str, text: &str) -> PyResult<String> {
    // CPython returns an empty input before checking the normalization form.
    if text.is_empty() {
        return Ok(String::new());
    }
    let (compatibility, compose) = match form {
        "NFC" => (false, true),
        "NFD" => (false, false),
        "NFKC" => (true, true),
        "NFKD" => (true, false),
        _ => return Err(PyExc::value_error("invalid normalization form")),
    };
    let mut decomposed = Vec::with_capacity(text.chars().count());
    for c in text.chars() {
        push_decomposed(c, compatibility, &mut decomposed);
    }
    // Sort only nonstarter runs. Stable sorting is essential: two marks with
    // the same class retain their original order and block later composition.
    let mut start = 0;
    for index in 0..decomposed.len() {
        if decomposed[index].1 == 0 {
            decomposed[start..index].sort_by_key(|entry| entry.1);
            start = index + 1;
        }
    }
    decomposed[start..].sort_by_key(|entry| entry.1);
    if !compose {
        return Ok(decomposed.iter().map(|entry| entry.0).collect());
    }

    let mut output = Vec::with_capacity(decomposed.len());
    let mut starter = None;
    let mut previous_class = 0;
    for (c, class) in decomposed {
        if let Some(index) = starter
            && (previous_class == 0 || previous_class < class)
            && let Some(composed) = compose_pair(output[index], c)
        {
            output[index] = composed;
            continue;
        }
        if class == 0 {
            starter = Some(output.len());
        }
        previous_class = class;
        output.push(c);
    }
    Ok(output.into_iter().collect())
}

pub fn call(name: &str, args: &[Value]) -> PyResult<Value> {
    match name {
        "normalize" => {
            if args.len() != 2 {
                return Err(PyExc::type_error(format!(
                    "normalize expected 2 arguments, got {}",
                    args.len()
                )));
            }
            let text = |index: usize| {
                args[index].as_str().map_err(|_| {
                    PyExc::type_error(format!(
                        "normalize() argument {} must be str, not {}",
                        index + 1,
                        args[index].type_name()
                    ))
                })
            };
            Ok(Value::from(normalize(text(0)?, text(1)?)?))
        }
        "combining" => {
            if args.len() != 1 {
                return Err(PyExc::type_error(format!(
                    "unicodedata.combining() takes exactly one argument ({} given)",
                    args.len()
                )));
            }
            let text = args[0].as_str().map_err(|_| {
                PyExc::type_error(format!(
                    "combining() argument must be a unicode character, not {}",
                    args[0].type_name()
                ))
            })?;
            let mut chars = text.chars();
            let first = chars.next();
            if first.is_none() || chars.next().is_some() {
                return Err(PyExc::type_error(format!(
                    "combining(): argument must be a unicode character, not a string of length {}",
                    text.chars().count()
                )));
            }
            Ok(Value::from(i64::from(combining(first.unwrap()))))
        }
        _ => Err(PyExc::attribute_error(format!(
            "module 'unicodedata' has no attribute '{name}'"
        ))),
    }
}
