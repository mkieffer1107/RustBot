use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
mod isolation;
mod terminal;

fn main() {
    if let Err(message) = run() {
        eprintln!("rustbot: {message}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") | Some("-V") => println!("rustbot {}", env!("CARGO_PKG_VERSION")),
        Some("--help") | Some("-h") => println!(
            "RustBot port under development.\nUsage: rustbot [-p TEXT] [-i] [-s N] [-v]\n       rustbot replay [--trace] [--seed N] [--clock ISO] < conversations.jsonl\n       rustbot hook MODULE HOOK < hooks.jsonl\n       rustbot call DOTTED.NAME < functions.jsonl\n       rustbot pyrt-check KIND < cases.jsonl\n{}",
            bot::PORT_STATUS
        ),
        Some("pyrt-check") if args.len() == 2 => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            for (index, line) in io::stdin().lock().lines().enumerate() {
                let line = line.map_err(|e| e.to_string())?;
                let case: serde_json::Value =
                    serde_json::from_str(&line).map_err(|e| format!("line {}: {e}", index + 1))?;
                let result = pyrt::cases::run_case(&args[1], &case);
                serde_json::to_writer(&mut out, &result).map_err(|e| e.to_string())?;
                writeln!(out).map_err(|e| e.to_string())?;
            }
        }
        Some("call") if args.len() == 2 => call_records(&args[1], None)?,
        Some("hook") if args.len() == 3 => call_records(&args[1], Some(&args[2]))?,
        Some("replay") => replay_records(&args[1..])?,
        _ => terminal::run(&args)?,
    }
    Ok(())
}

fn call_records(name: &str, hook: Option<&str>) -> Result<(), String> {
    let records = read_records()?;
    let groups = group_records(&records);
    let mut results = vec![Json::Null; records.len()];
    for indices in groups.values() {
        let first = &records[indices[0]];
        let loaded = (|| -> pyrt::PyResult<_> {
            let mut runtime = load_runtime(first)?;
            // The partial development registry can exercise already translated leaves.
            // The complete registry preloads bot.runtime before resolving each child call.
            let full = runtime
                .module_names("bot")
                .iter()
                .any(|(module, _)| module == "runtime");
            if full {
                runtime.import("bot.runtime")?;
            }
            let function = if let Some(hook) = hook {
                Some(bot::support::resolve_hook(&mut runtime, name, hook)?)
            } else if full {
                None
            } else {
                Some(bot::support::resolve(&mut runtime, name)?)
            };
            Ok((runtime, function))
        })();
        let (mut runtime, function) = match loaded {
            Ok(loaded) => loaded,
            Err(exc) => {
                for &index in indices {
                    results[index] = json!({"ret":null,"exc":exc.kind});
                }
                continue;
            }
        };
        for &index in indices {
            results[index] = isolation::run_json(|| {
                let function = function
                    .clone()
                    .map(Ok)
                    .unwrap_or_else(|| bot::support::resolve(&mut runtime, name))
                    .map_err(|e| e.to_string())?;
                if hook.is_some() {
                    let record = &records[index];
                    let memory =
                        pyrt::from_canon_json(record.get("memory_in").ok_or("missing memory_in")?)
                            .map_err(|e| e.to_string())?;
                    let mut values =
                        pyrt::from_canon_json(record.get("args").ok_or("missing args")?)
                            .and_then(|value| value.sequence())
                            .map_err(|e| e.to_string())?;
                    if values.is_empty() {
                        return Err("hook args must include text".into());
                    }
                    values.insert(1, memory.clone());
                    let rng = record
                        .get("rng_in")
                        .cloned()
                        .unwrap_or_else(|| json!({"global":0,"instances":[]}));
                    runtime.random.restore(&rng).map_err(|e| e.to_string())?;
                    let result = runtime.call(&function, pyrt::object::CallArgs::new(values));
                    let (ret, exc) = match result {
                        Ok(value) => (
                            pyrt::canon_json(&value).map_err(|e| e.to_string())?,
                            Json::Null,
                        ),
                        Err(exc) => (Json::Null, Json::String(exc.kind.into())),
                    };
                    return Ok(
                        json!({"ret":ret,"exc":exc,"memory_out":pyrt::canon_json(&memory).map_err(|e|e.to_string())?,"rng_out":runtime.random.snapshot()}),
                    );
                }
                let result = (|| -> pyrt::PyResult<_> {
                    let record = &records[index];
                    let values = pyrt::from_canon_json(
                        record
                            .get("args")
                            .ok_or_else(|| pyrt::PyExc::key_error("args"))?,
                    )?
                    .sequence()?;
                    let mut args = pyrt::object::CallArgs::new(values);
                    if let Some(kwargs) = record.get("kwargs") {
                        args.extend_keywords(&pyrt::from_canon_json(kwargs)?)?;
                    }
                    let value = runtime.call(&function, args)?;
                    pyrt::canon_json(&value)
                })();
                Ok(match result {
                    Ok(value) => json!({"ret":value,"exc":null}),
                    Err(exc) => json!({"ret":null,"exc":exc.kind}),
                })
            })?;
        }
    }
    emit_records(&results)
}

fn read_records() -> Result<Vec<Json>, String> {
    io::stdin()
        .lock()
        .lines()
        .enumerate()
        .filter_map(|(index, line)| match line {
            Ok(line) if line.trim().is_empty() => None,
            line => Some((index, line)),
        })
        .map(|(index, line)| {
            let line = line.map_err(|e| e.to_string())?;
            serde_json::from_str::<Json>(&line).map_err(|e| format!("line {}: {e}", index + 1))
        })
        .collect()
}

fn group_records(records: &[Json]) -> BTreeMap<String, Vec<usize>> {
    let mut groups = BTreeMap::<String, Vec<usize>>::new();
    for (index, record) in records.iter().enumerate() {
        let key = json!([
            record.get("seed").unwrap_or(&json!(0)),
            record
                .get("clock")
                .and_then(Json::as_str)
                .unwrap_or("2026-06-15T12:00:00")
        ])
        .to_string();
        groups.entry(key).or_default().push(index);
    }
    groups
}

fn load_runtime(first: &Json) -> pyrt::PyResult<pyrt::runtime::Runtime> {
    let seed = first
        .get("seed")
        .map(pyrt::from_canon_json)
        .transpose()?
        .unwrap_or(pyrt::Value::from(0_i64))
        .as_int()?;
    let clock = first
        .get("clock")
        .and_then(Json::as_str)
        .unwrap_or("2026-06-15T12:00:00");
    bot::new_runtime(seed, clock)
}

fn emit_records(results: &[Json]) -> Result<(), String> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for result in results {
        serde_json::to_writer(&mut out, &result).map_err(|e| e.to_string())?;
        writeln!(out).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn replay_records(options: &[String]) -> Result<(), String> {
    let mut trace = false;
    let mut seed = std::env::var("PYBOT_SEED").unwrap_or_else(|_| "0".into());
    let mut clock = std::env::var("PYBOT_CLOCK").unwrap_or_else(|_| "2026-06-15T12:00:00".into());
    let mut options = options.iter();
    while let Some(option) = options.next() {
        match option.as_str() {
            "--trace" => trace = true,
            "--seed" => seed = options.next().ok_or("--seed needs an integer")?.clone(),
            "--clock" => {
                clock = options
                    .next()
                    .ok_or("--clock needs an ISO timestamp")?
                    .clone()
            }
            _ => return Err(format!("unknown replay option: {option}")),
        }
    }
    let seed: Json = serde_json::from_str(&seed).map_err(|e| format!("invalid seed: {e}"))?;
    let mut records = read_records()?;
    for record in &mut records {
        let record = record
            .as_object_mut()
            .ok_or("conversation must be a JSON object")?;
        record.entry("seed").or_insert_with(|| seed.clone());
        record
            .entry("clock")
            .or_insert_with(|| Json::String(clock.clone()));
    }
    let sources = trace
        .then(|| {
            bot::support::trace::SourceIndex::from_json(
                include_bytes!("../../../data/trace/norm.json"),
                include_bytes!("../../../data/trace/lines.json"),
            )
        })
        .transpose()?;
    let mut results = vec![Vec::new(); records.len()];
    for indices in group_records(&records).values() {
        let mut runtime = load_runtime(&records[indices[0]]).map_err(|e| e.to_string())?;
        runtime.import("bot.runtime").map_err(|e| e.to_string())?;
        for &index in indices {
            let result = isolation::run_json(|| {
                let mut session = bot::support::engine::Session::from_runtime(runtime.clone())
                    .map_err(|e| e.to_string())?;
                if trace {
                    session.enable_trace_auto().map_err(|e| e.to_string())?;
                }
                let record = &records[index];
                let turns = record
                    .get("turns")
                    .and_then(Json::as_array)
                    .ok_or("conversation needs turns array")?;
                let id = record.get("id").ok_or("conversation needs id")?;
                let mut output = Vec::with_capacity(turns.len());
                for (turn, text) in turns.iter().enumerate() {
                    let text = text.as_str().ok_or("turn text must be a string")?;
                    let (decision, traced) = match &sources {
                        Some(sources) => {
                            let (decision, trace) = session
                                .turn_with_trace(text, sources)
                                .map_err(|e| e.to_string())?;
                            (decision, Some(trace))
                        }
                        None => (session.turn(text).map_err(|e| e.to_string())?, None),
                    };
                    let mut row = turn_record(id, turn, text, &decision, &session.memory)
                        .map_err(|e| e.to_string())?;
                    if let Some(trace) = traced {
                        row["trace"] = trace;
                    }
                    output.push(row);
                }
                Ok(Json::Array(output))
            })?;
            results[index] = result
                .as_array()
                .ok_or("isolated conversation did not return turns")?
                .clone();
        }
    }
    emit_records(&results.into_iter().flatten().collect::<Vec<_>>())
}

fn turn_record(
    id: &Json,
    turn: usize,
    text: &str,
    decision: &pyrt::Value,
    memory: &pyrt::Value,
) -> pyrt::PyResult<Json> {
    let get = |key: &str| decision.dict_get(&pyrt::Value::from(key));
    let candidates = get("candidates")?
        .sequence()?
        .into_iter()
        .map(|candidate| {
            Ok(json!([
                pyrt::canon_json(&candidate.dict_get(&pyrt::Value::from("module"))?)?,
                pyrt::canon_json(&candidate.dict_get(&pyrt::Value::from("reply"))?)?,
            ]))
        })
        .collect::<pyrt::PyResult<Vec<_>>>()?;
    Ok(json!({"conv":id,"turn":turn,"text":text,
        "reply":pyrt::canon_json(&get("reply")?)?, "module":pyrt::canon_json(&get("module")?)?,
        "candidates":candidates,"memory":pyrt::canon_json(memory)?}))
}
