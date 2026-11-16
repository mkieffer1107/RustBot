# Progress log

Append only. Newest entry at the bottom. Each entry: date and time, milestone, what changed, check numbers, the exact
next step.

## Baseline (2026-11-16, measured on an Apple-silicon laptop with CPython 3.14.6, before any port)
- PyBot `7d63c7f`: import 2.0 s; 10-turn conversation mean 730 ms/turn cold, 404 ms warm, worst 2.3 s; peak RSS 236 MB.
- 109 modules: 97 `respond`, 16 `observe`. Profile: `bot/knowledge/lookup.py` fuzzy matching dominates.

## Check disputes
(none yet)

## Log

### 2026-11-16 11:27 EST — M0 bootstrap / M2 runtime foundation (partial)
- Read all required instructions. Did not run `check_all.sh --quick`: the root AGENTS.md explicitly forbids even full/quick all-gate runs on this development machine. No corpus, goldens, full parity, performance, or harness self-test was run locally.
- Fetched the read-only inputs with `reference/fetch.sh`: PyBot is pinned at `7d63c7f`; SmolTalkBench parquet downloaded. Installed stable Rust 1.100.0, rustfmt/clippy and the wasm32-unknown-unknown target with rustup; no shell configuration changed. Use `PATH=/home/max/.cargo/bin:$PATH` in this environment.
- Created Cargo workspace (`pyrt`, `bot`, `cli`) and translator directory. The bot crate is scaffolding, not a translated bot. `rustbot pyrt-check` supports JSONL `str`, `random`, `format`, `re` probes; interactive/call/hook/replay modes are not implemented.
- Implemented initial arbitrary precision values, code point strings, mutable container aliases, ordered dicts, memoized deepcopy, numeric/sequence operators and canonical JSON. Added CPython64 zero-seed hashes/set probing, MT19937/string seeding/state counters, Unicode string tables/methods, formatting and shared fancy-regex compatibility. Found/fixed shared set and immutable tuple deepcopy, empty repetition, and cased/Case_Ignorable final-sigma issues during independent review.
- Validation: `cargo build` PASS; single-crate `cargo test -p pyrt` 39/39 PASS (32 unit + 7 regressions); `cargo clippy -p pyrt -p cli --all-targets -- -D warnings` PASS; `cargo fmt --all -- --check` PASS. CLI smoke 46/46 and formatting panel 116/116 matched CPython 3.14.8. Subagent targeted panels: strings 1,920/1,920, regex 81/81; Unicode regeneration SHA-256 reproducible. These are bounded runtime probes, not acceptance/corpus parity results.
- `check_env.sh`: MISSING (exit 2), goldens/MANIFEST.json absent; native tools and pinned reference detected, WASM packaging tools absent. `check_structure.sh --partial`: PASS with 1 scaffold file, 526 missing, 0 translated functions; this is not translation progress. The harness self-test must run on the compute host before acceptance results are trusted.
- User requested a pull: origin/main had unrelated rewritten history but an identical tree. Merged origin/main without rewriting local history (`ba5f568`); no upstream file changes or work loss.
- Translation count: native 0/527 mirrored source files; WASM integration not started; unique translated bot files 0/527. About 20 minutes of initial bootstrap/runtime work so far; no useful total ETA before translator/parity throughput is measured. Runtime support files are excluded from this count.
- Known gaps and next work: `rustbot/docs/runtime.md` lists partial semantics. Next fix regex empty/nonempty iteration and scoped/case flags, implement remaining builtins/container methods and injected datetime/calendar/math/global RNG integration, and obtain compute-host corpus/goldens/self-test for G0/G1. M0 and M2 are NOT complete. Do not start WASM or claim native bot parity.

### 2026-11-16 11:29 EST — M1 static inventory / progress reporting (partial)
- Runtime bootstrap is committed as `10c416e` (buildable). Added reproducible AST inventory, import graph, and decorated `respond` source expressions. Static scan covers all 528 source files, 75 AST node types and 2,280,522 AST nodes; independent `ast.walk` counts matched. Runtime trace remains explicitly missing until compute-host goldens are available; aliases and regex features are conservative static estimates.
- Added `rustbot/tools/progress.py`: native generated file/function/span presence, WASM wrapper presence, and combined unique translations without counting shared Rust twice. Bootstrap files and placeholder macros are excluded. Counts measure presence, not acceptance parity. Current native/unique translation count remains 0/527; WASM not started.
- Validation: tooling fixture tests 3/3 PASS, including real trace JSON shapes and progress scaffolding exclusion. Native build remains passing. Updated native README with build/smoke/progress commands and runtime limitations. M1 acceptance still depends on G0 goldens; no milestone marked complete.
- Exact next step: continue M2 regex empty-match correctness and frozen datetime/calendar primitives, then remaining inventory builtins/stdlib; merge runtime traces into inventory when supplied. Run all heavy gates and the harness self-test only on the compute host.
