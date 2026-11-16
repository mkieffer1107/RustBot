//! Trace shaping and source lookup compatible with the PyBot Space.
//!
//! The caller captures one `Runtime::begin_trace`/`end_trace` pair around each
//! respond invocation, then passes only the winning handler's spans here.
//! Source tables are supplied separately so native and WASM builds can load
//! the same generated norm.json and lines.json without embedding Python.
use pyrt::{PyResult, runtime::SourceSpan};
use serde_json::{Value as Json, json};
use std::collections::{BTreeMap, HashSet, VecDeque};

const MAX_CALLS_PER_FILE: usize = 40;

#[derive(Debug)]
struct SourceFile {
    normalized: String,
    lines: Vec<usize>,
}
/// Immutable lookup tables generated from the pinned source tree.
#[derive(Debug)]
pub struct SourceIndex {
    files: BTreeMap<String, SourceFile>,
}

impl SourceIndex {
    /// Read the Space's norm.json and lines.json objects.
    pub fn from_json(norm: &[u8], lines: &[u8]) -> Result<Self, String> {
        let norm: BTreeMap<String, String> = serde_json::from_slice(norm)
            .map_err(|error| format!("invalid normalized source table: {error}"))?;
        let mut lines: BTreeMap<String, Vec<usize>> = serde_json::from_slice(lines)
            .map_err(|error| format!("invalid source line table: {error}"))?;
        let mut files = BTreeMap::new();
        for (path, normalized) in norm {
            let counts = lines
                .remove(&path)
                .ok_or_else(|| format!("missing source line counts for {path}"))?;
            files.insert(
                path,
                SourceFile {
                    normalized,
                    lines: counts,
                },
            );
        }
        Ok(Self { files })
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Return the uniquely identified source and its one-based line, if any.
    pub fn answer_source(
        &self,
        reply: &str,
        used: &[String],
    ) -> PyResult<(Option<String>, Option<usize>)> {
        let normalized = normalize(reply)?;
        for n in [60, 40] {
            let key = &normalized[..normalized.len().min(n)]; // normalized text is ASCII
            if key.len() < 20 {
                continue;
            }
            let hits: Vec<_> = self
                .files
                .iter()
                .filter(|(_, file)| file.normalized.contains(key))
                .collect();
            let candidates: Vec<_> = if hits.len() == 1 {
                hits
            } else {
                hits.into_iter()
                    .filter(|(path, _)| used.contains(path))
                    .collect()
            };
            if let [(path, file)] = candidates.as_slice() {
                let offset = file.normalized.find(key).expect("candidate contains key");
                let mut seen = 0usize;
                let line = file
                    .lines
                    .iter()
                    .position(|count| {
                        seen = seen.saturating_add(*count);
                        offset < seen
                    })
                    .map(|index| index + 1);
                return Ok((Some((*path).clone()), line));
            }
        }
        Ok((None, None))
    }
}

fn normalize(text: &str) -> PyResult<String> {
    let lowered = pyrt::strings::call_method(text, "lower", &[])?;
    Ok(lowered
        .as_str()?
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect())
}
fn reply_text(reply: &str) -> PyResult<String> {
    let parts = pyrt::strings::call_method(reply, "split", &[])?.sequence()?;
    let words = parts
        .iter()
        .map(|part| part.as_str())
        .collect::<PyResult<Vec<_>>>()?;
    let reply = words.join(" ");
    Ok(if reply.is_empty() {
        "Hello!".to_owned()
    } else {
        reply
    })
}

/// Reproduce the Space's breadth-first search for a handler's own respond.
/// It visits at most 50 distinct callable identities; __wrapped__ precedes
/// live closure cells. Non-function callables can expose __wrapped__ too.
pub fn own_respond(
    runtime: &mut pyrt::runtime::Runtime,
    module: &str,
    handler: &pyrt::Value,
) -> PyResult<Option<(String, u32, u32)>> {
    let wanted = format!("bot/modules/{module}.py");
    let mut queue = VecDeque::from([handler.clone()]);
    let mut seen: Vec<pyrt::Value> = Vec::new();
    while !queue.is_empty() && seen.len() < 50 {
        let candidate = queue.pop_front().unwrap();
        if seen.iter().any(|value| value.is_identical(&candidate)) {
            continue;
        }
        seen.push(candidate.clone());
        let pyrt::Value::Object(object) = &candidate else {
            continue;
        };
        let native = object
            .as_any()
            .downcast_ref::<pyrt::object::NativeFunction>();
        if let Some(span) = native.and_then(|f| f.source_span())
            && span.file == wanted
        {
            return Ok(Some((span.file.to_owned(), span.first, span.last)));
        }
        match runtime.getattr(&candidate, "__wrapped__") {
            Ok(value) if matches!(&value,pyrt::Value::Object(object) if object.is_callable()) => {
                queue.push_back(value)
            }
            Ok(_) => {}
            Err(error) if error.kind == "AttributeError" => {}
            Err(error) => return Err(error),
        }
        if let Some(native) = native {
            for value in native.closure_values(runtime) {
                if matches!(&value,pyrt::Value::Object(object) if object.is_callable()) {
                    queue.push_back(value);
                }
            }
        }
    }
    Ok(None)
}

/// Find the plain `respond` definition in a generated module's static metadata.
/// If a decorator defined in that same module changes the bound handler, use
/// the actual resolved handler span instead (as the Space's closure walk does).
pub fn respond_from_metadata(
    module: &str,
    file: &str,
    functions: &[(&str, u32, u32)],
) -> Option<(String, u32, u32)> {
    if file != format!("bot/modules/{module}.py") {
        return None;
    }
    functions
        .iter()
        .find(|(name, _, _)| *name == "respond")
        .map(|(_, first, last)| (file.to_owned(), *first, *last))
}

/// Shape the winning handler's trace into the check's JSON fields.
/// `respond` must be the winner's own resolved handler span, if known.
pub fn build_trace(
    reply: &str,
    winner: Option<&str>,
    spans: &[SourceSpan],
    respond: Option<(&str, u32, u32)>,
    sources: &SourceIndex,
) -> PyResult<Json> {
    let reply = reply_text(reply)?;
    let mut calls: BTreeMap<String, Vec<Json>> = BTreeMap::new();
    let mut seen = HashSet::new();
    if winner.is_some() {
        for span in spans {
            if !span.file.starts_with("bot/") {
                continue;
            }
            let identity = (span.file, span.first, span.last, span.qualname);
            if !seen.insert(identity) {
                continue;
            }
            let entries = calls.entry(span.file.to_owned()).or_default();
            if span.qualname != "<module>" && entries.len() < MAX_CALLS_PER_FILE {
                entries.push(json!([span.first, span.last, span.qualname]));
            }
        }
    }
    let files: Vec<String> = calls.keys().cloned().collect();
    let (source, line) = sources.answer_source(&reply, &files)?;
    let response = winner
        .and_then(|winner| {
            respond.filter(|(file, _, _)| *file == format!("bot/modules/{winner}.py"))
        })
        .map(|(file, first, last)| json!([file, first, last]));
    Ok(
        json!({"reply":reply,"files":files,"calls":calls,"respond":response,"source":source,"source_line":line}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn index(norm: Json, lines: Json) -> SourceIndex {
        SourceIndex::from_json(
            &serde_json::to_vec(&norm).unwrap(),
            &serde_json::to_vec(&lines).unwrap(),
        )
        .unwrap()
    }
    fn function(
        file: &'static str,
        first: u32,
        closure: std::rc::Rc<pyrt::frame::Frame>,
        freevars: &[&str],
        wrapped: Option<pyrt::Value>,
    ) -> pyrt::Value {
        use pyrt::object::{FunctionOrigin, NativeFunction};
        let attrs = pyrt::Value::dict();
        if let Some(value) = wrapped {
            attrs
                .dict_set(pyrt::Value::from("__wrapped__"), value)
                .unwrap();
        }
        NativeFunction::qualified_with_origin(
            "test".into(),
            "test".into(),
            "test".into(),
            attrs,
            FunctionOrigin {
                span: SourceSpan {
                    file,
                    first,
                    last: first + 1,
                    qualname: "test",
                },
                closure,
                freevars: freevars.iter().map(|s| (*s).to_owned()).collect(),
            },
            |_, _| Ok(pyrt::Value::None),
        )
    }
    #[test]
    fn respond_search_walks_wrapped_then_live_closure_cells() {
        let mut runtime = pyrt::runtime::Runtime::default();
        let module = pyrt::frame::Frame::module(pyrt::Value::dict());
        let captured = pyrt::frame::Frame::function(
            module.clone(),
            &["target"],
            &[],
            &[],
            pyrt::Value::dict(),
        );
        let from_cell = function("bot/modules/demo.py", 10, module.clone(), &[], None);
        let from_wrapped = function("bot/modules/demo.py", 20, module.clone(), &[], None);
        captured.store("target", from_cell).unwrap();
        let wrapper = function(
            "bot/compose/wrapper.py",
            1,
            captured.clone(),
            &["target"],
            Some(from_wrapped),
        );
        assert_eq!(
            own_respond(&mut runtime, "demo", &wrapper).unwrap(),
            Some(("bot/modules/demo.py".into(), 20, 21))
        );
        runtime
            .setattr(&wrapper, "__wrapped__", pyrt::Value::None)
            .unwrap();
        assert_eq!(
            own_respond(&mut runtime, "demo", &wrapper).unwrap(),
            Some(("bot/modules/demo.py".into(), 10, 11))
        );
        let replacement = function("bot/modules/demo.py", 30, module, &[], None);
        captured.store("target", replacement).unwrap();
        assert_eq!(
            own_respond(&mut runtime, "demo", &wrapper).unwrap(),
            Some(("bot/modules/demo.py".into(), 30, 31))
        );
    }
    #[test]
    fn respond_search_stops_after_fifty_distinct_functions() {
        let mut runtime = pyrt::runtime::Runtime::default();
        let frame = pyrt::frame::Frame::module(pyrt::Value::dict());
        let mut chain = function("bot/modules/demo.py", 70, frame.clone(), &[], None);
        for _ in 0..49 {
            chain = function("bot/compose/wrapper.py", 1, frame.clone(), &[], Some(chain));
        }
        assert!(own_respond(&mut runtime, "demo", &chain).unwrap().is_some());
        chain = function("bot/compose/wrapper.py", 1, frame, &[], Some(chain));
        assert_eq!(own_respond(&mut runtime, "demo", &chain).unwrap(), None);
    }
    #[test]
    fn source_lookup_prefers_unique_used_file_and_counts_lines() {
        let key = "thisisaverylonganswerwithmorethantwentyletters";
        let source = index(
            json!({"bot/a.py":format!("first{key}"),"bot/b.py":format!("other{key}")}),
            json!({"bot/a.py":[5,key.len()],"bot/b.py":[5,key.len()]}),
        );
        assert_eq!(
            source
                .answer_source(
                    "This is a very long answer with more than twenty letters",
                    &["bot/b.py".into()]
                )
                .unwrap(),
            (Some("bot/b.py".into()), Some(2))
        );
        assert_eq!(
            source
                .answer_source(
                    "This is a very long answer with more than twenty letters",
                    &[]
                )
                .unwrap(),
            (None, None)
        );
        assert_eq!(
            source.answer_source("short", &["bot/a.py".into()]).unwrap(),
            (None, None)
        );
    }
    #[test]
    fn output_sorts_files_deduplicates_and_caps_calls() {
        let source = index(json!({}), json!({}));
        let mut spans = Vec::new();
        spans.push(SourceSpan {
            file: "bot/z.py",
            first: 1,
            last: 1,
            qualname: "<module>",
        });
        for i in 1..=42 {
            spans.push(SourceSpan {
                file: "bot/a.py",
                first: i,
                last: i,
                qualname: "f",
            });
        }
        spans.push(spans[1]);
        spans.push(SourceSpan {
            file: "outside.py",
            first: 1,
            last: 1,
            qualname: "f",
        });
        let value = build_trace(
            "  hello\u{1c}world  ",
            Some("demo"),
            &spans,
            Some(("bot/modules/demo.py", 2, 5)),
            &source,
        )
        .unwrap();
        assert_eq!(value["reply"], "hello world");
        assert_eq!(value["files"], json!(["bot/a.py", "bot/z.py"]));
        assert_eq!(value["calls"]["bot/a.py"].as_array().unwrap().len(), 40);
        assert_eq!(value["calls"]["bot/z.py"], json!([]));
        assert_eq!(value["respond"], json!(["bot/modules/demo.py", 2, 5]));
        assert_eq!(
            build_trace("", None, &spans, None, &source).unwrap()["reply"],
            "Hello!"
        );
    }
    #[test]
    fn python_lowercase_is_used_before_ascii_filtering() {
        let source = index(
            json!({"bot/a.py":"iandkarelettersinthislongsample"}),
            json!({"bot/a.py":[31]}),
        );
        assert_eq!(
            source
                .answer_source("İ and K are letters in this long sample", &[])
                .unwrap()
                .0,
            Some("bot/a.py".into())
        );
    }
}
