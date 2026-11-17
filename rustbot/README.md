# RustBot native port

Restarting on 2026-11-17: the files will be translated from Python to Rust by hand. The earlier translator, its
generated `bot` crate and the `cli` were removed; they remain in git history (`95723ca`). Read
[`../agent/FINDINGS.md`](../agent/FINDINGS.md) first: it covers what that attempt measured, the Python behaviour a
byte-identical port must reproduce, and what is worth keeping.

What remains here:

- `crates/pyrt`: verified Python-compatible pieces (regex, Unicode tables, CPython set ordering, `random`, string
  formatting, float repr). It passes the recorded runtime cases. Use it as a library for behaviour the output can
  observe, not as an execution model.
- `vendor/fancy-regex`: the backtracking regex engine `pyrt` builds on.
- `data/trace`: `norm.json` and `lines.json`, which map a reply to its source file and line for the Space trace.
- `tools/`: Unicode and normalisation table generators, the source inventory, corpus measurement, and translation
  progress counting.

```sh
cargo test --manifest-path rustbot/Cargo.toml -p pyrt
```
