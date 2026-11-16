# RustBot native port

The Cargo workspace contains `pyrt` (Python-compatible primitives), `bot` (the
future generated translations), and `cli` (the `rustbot` executable). Native
parity comes first; WASM will wrap this same `bot` crate in milestone M7.

No PyBot modules have been translated yet. The executable currently supports
runtime probes, `--help`, and `--version`; conversation modes are not implemented.

```sh
# If rustup's tools are not on PATH yet:
. "$HOME/.cargo/env"
cargo build --manifest-path rustbot/Cargo.toml
cargo test --manifest-path rustbot/Cargo.toml -p pyrt
python3 rustbot/tools/smoke_runtime.py
python3 rustbot/tools/progress.py
```

`rustbot/target/debug/rustbot pyrt-check <str|random|format|re>` reads cases on
stdin as JSONL and writes one `{result, exc}` JSON object per case. Runtime
implementation and known gaps are described in [docs/runtime.md](docs/runtime.md).
These small tests do not establish bot parity.

Generate the static source inventory and import graph with
`python3 rustbot/tools/inventory.py`. It also includes runtime traces from
`agent/goldens/trace/` when they are present. Regenerate Unicode tables using
CPython 3.14 and `python3 rustbot/tools/generate_unicode.py`.

`tools/progress.py` reports native source files and functions with generated
translations, and tracks the presence of the WASM wrapper and browser integration
separately. Both builds share the same source translation count. Presence counts
do not assert behavioral parity; acceptance gates are recorded separately.

Corpus generation, full parity, performance, and the acceptance harness self-test
must run on a compute host as described in `agent/checks/README.md`. No native or
browser performance claims have been measured for this port yet.
