//! Inspect the live loader registry without invoking handlers or mutating memory.
use pyrt::{Value, canon_json};
use serde_json::{Value as Json, json};

pub fn run() -> Result<(), String> {
    let seed = std::env::var("PYBOT_SEED").unwrap_or_else(|_| "0".into());
    let seed: Json = serde_json::from_str(&seed).map_err(|e| format!("invalid seed: {e}"))?;
    let clock = std::env::var("PYBOT_CLOCK").unwrap_or_else(|_| "2026-06-15T12:00:00".into());
    let started = std::time::Instant::now();
    let mut runtime =
        crate::load_runtime(&json!({"seed":seed,"clock":clock})).map_err(|e| e.to_string())?;
    let module = runtime.import("bot.runtime").map_err(|e| e.to_string())?;
    let mut result = json!({"load_s":started.elapsed().as_secs_f64()});
    for (label, attribute) in [
        ("modules", "_MODULES"),
        ("handlers", "_HANDLERS"),
        ("observers", "_OBSERVERS"),
        ("after", "_AFTER"),
    ] {
        let entries = runtime
            .getattr(&module, attribute)
            .and_then(|value| value.sequence())
            .map_err(|e| e.to_string())?;
        let entries = entries
            .into_iter()
            .map(|entry| {
                let fields = entry.sequence().map_err(|e| e.to_string())?;
                if fields.len() != 3 {
                    return Err(format!("invalid {attribute} entry"));
                }
                Ok(json!([
                    canon_json(&fields[0]).map_err(|e| e.to_string())?,
                    canon_json(&fields[1]).map_err(|e| e.to_string())?,
                ]))
            })
            .collect::<Result<Vec<_>, String>>()?;
        result[label] = Json::Array(entries);
    }
    result["arbiter"] = Json::Bool(!matches!(
        runtime
            .getattr(&module, "_ARBITER")
            .map_err(|e| e.to_string())?,
        Value::None
    ));
    result["rng_after_load"] = runtime.random.snapshot();
    println!("{result}");
    Ok(())
}
