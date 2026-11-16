use std::io::{self, BufRead, Write};

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
        _ => return Err("bot translation is not implemented yet; use --help".into()),
    }
    Ok(())
}
