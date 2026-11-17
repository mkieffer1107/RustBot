# RustBot native port

The Cargo workspace contains `pyrt` (Python-compatible values and operations),
`bot` (generated native functions), and `cli` (the `rustbot` executable). Native
parity comes first; WASM will wrap this same `bot` crate in milestone M7.

All 527 source mirrors are generated and pass the compiler and structure checks,
including all 11,359 function/lambda spans. The full runtime check passes
94,123/94,123 corrected dev cases. One complete three-turn conversation matches
the raw Python reference. Full-corpus parity, performance, and the browser build
are still pending. See [the progress log](../agent/PROGRESS.md) for exact build and
acceptance results; generated-file counts do not establish behavioral parity.

```sh
# If rustup's tools are not on PATH yet:
. "$HOME/.cargo/env"
# Verification build: optimized runtime, generated bot code at opt-level 0:
# Incremental caching is enabled for later verification builds.
CARGO_BUILD_JOBS=2 cargo build --manifest-path rustbot/Cargo.toml --profile parity -p cli
cargo test --manifest-path rustbot/Cargo.toml -p pyrt
python3 rustbot/tools/smoke_runtime.py --binary rustbot/target/parity/rustbot
python3 rustbot/tools/progress.py
```

The binary implements `pyrt-check <str|random|format|re>`, `call <dotted.name>`,
`hook <module> <hook>`, and `replay [--trace]` JSONL protocols. The `parity` build
is at `rustbot/target/parity/rustbot`; use `RUSTBOT_BIN` to select it for the
protected checks. Final performance acceptance uses the normal release build.
`load-info` reports live loader names, priorities, hooks and RNG counters for
comparison with Python. These conversation/loader adapters are still undergoing
whole-program validation.

On this checkout, select the corrected recording explicitly for acceptance checks:

```sh
export GOLDENS="$PWD/rustbot/target/official-goldens"
export RUSTBOT_BIN="$PWD/rustbot/target/parity/rustbot"
agent/checks/check_pyrt.sh
```

The previous `agent/goldens` directory is preserved as historical evidence; it
contains the recorder identity defect described in the progress log. Goldens
are generated development artifacts and are never shipped in RustBot.

Terminal modes accept `-p TEXT`, `-i`, `-s N`, `-v`, and piped input. The shared
session owns conversation memory; `PYBOT_CLOCK` injects a frozen clock, with a
system-clock fallback for terminal use. Runtime implementation and known gaps
are described in [docs/runtime.md](docs/runtime.md).

Generate the static source inventory and import graph with
`python3 rustbot/tools/inventory.py`. It also includes runtime traces from
`$GOLDENS/trace/` when they are present (defaulting to `agent/goldens/trace/`).
Regenerate Unicode tables using
CPython 3.14 and `python3 rustbot/tools/generate_unicode.py`.

`tools/progress.py` reports native source files and functions with generated
translations, and tracks the presence of the WASM wrapper and browser integration
separately. Both builds share the same source translation count. Presence counts
do not assert behavioral parity; acceptance gates are recorded separately.

Corpus generation, full parity, performance, and the acceptance harness self-test
may run locally with bounded worker counts. Follow the steps in `agent/checks/README.md`; its older compute-host-only
wording is superseded by root `AGENTS.md`. Diagnostic native measurements are in
[docs/perf.md](docs/perf.md); final performance and browser gates remain pending.
