//! Development G1 frontend using the production runtime without linking bot.
use std::io::{self, BufRead, Write};

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 || args[0] != "pyrt-check" {
        return Err("usage: runtime_check pyrt-check <kind> < cases.jsonl".into());
    }
    if !matches!(args[1].as_str(), "re" | "str" | "format" | "random") {
        return Err(format!("unknown runtime case kind: {}", args[1]));
    }
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for (index, line) in io::stdin().lock().lines().enumerate() {
        let line = line.map_err(|error| format!("line {}: {error}", index + 1))?;
        let case: serde_json::Value =
            serde_json::from_str(&line).map_err(|error| format!("line {}: {error}", index + 1))?;
        if !case.is_object() {
            return Err(format!(
                "line {}: runtime case must be a JSON object",
                index + 1
            ));
        }
        let result = pyrt::cases::run_case(&args[1], &case);
        serde_json::to_writer(&mut out, &result).map_err(|error| error.to_string())?;
        writeln!(out).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("runtime_check: {error}");
        std::process::exit(2);
    }
}
