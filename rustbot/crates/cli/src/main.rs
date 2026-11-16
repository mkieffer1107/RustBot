use std::io::{self, BufRead, Write};
mod isolation;

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
            "RustBot port under development.\nUsage: rustbot pyrt-check <kind> < cases.jsonl\n{}",
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
        Some("call") if args.len() == 2 => call_records(&args[1])?,
        _ => return Err("bot translation is not implemented yet; use --help".into()),
    }
    Ok(())
}

fn call_records(name: &str) -> Result<(), String> {
    use serde_json::{Value as Json, json};
    use std::collections::BTreeMap;

    let records = io::stdin()
        .lock()
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let line = line.map_err(|e| e.to_string())?;
            serde_json::from_str::<Json>(&line).map_err(|e| format!("line {}: {e}", index + 1))
        })
        .collect::<Result<Vec<_>, _>>()?;
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
    let mut results = vec![Json::Null; records.len()];
    for indices in groups.values() {
        let first = &records[indices[0]];
        let loaded = (|| -> pyrt::PyResult<_> {
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
            let mut runtime = bot::new_runtime(seed, clock)?;
            // Once the complete registry exists, match the oracle's full preload.
            if runtime
                .module_names("bot")
                .iter()
                .any(|(module, _)| module == "runtime")
            {
                runtime.import("bot.runtime")?;
            }
            let function = bot::support::resolve(&mut runtime, name)?;
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
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for result in results {
        serde_json::to_writer(&mut out, &result).map_err(|e| e.to_string())?;
        writeln!(out).map_err(|e| e.to_string())?;
    }
    Ok(())
}
