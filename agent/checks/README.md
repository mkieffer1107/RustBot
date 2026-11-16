# Checks: contract between the goldens and RustBot

The checks run PyBot (the oracle) and RustBot on the same inputs and compare. They never need to be edited by the
coding agent. Everything here runs with `PYTHONHASHSEED=0`, `PYBOT_SEED=0`, `PYBOT_CLOCK=2026-06-15T12:00:00` unless a
corpus entry says otherwise.

## Layout
```
agent/checks/
  lib/                 shared Python (oracle loader with seeding + frozen clock, canonical JSON, compare, report)
  corpus/              build_corpus.py and its output dev.jsonl (the hold-out corpus uses another seed and disjoint prompts)
  build_goldens.sh     runs the oracle over the corpus; writes $GOLDENS (default agent/goldens/, git-ignored)
  check_env.sh  check_pyrt.sh  check_modules.sh  check_e2e.sh  check_perf.sh  check_wasm.sh  check_space.sh
  check_hygiene.sh  check_all.sh
  selftest/            proves the checks work: a fake "rustbot" that wraps PyBot must pass, and injected faults must fail
```
Each `check_*.sh` exits 0 on pass, 1 on fail, 2 when the thing it checks does not exist yet (e.g. no binary), and
prints a one-line summary last: `CHECK <name> PASS|FAIL|MISSING <key numbers>`. `check_all.sh` runs them in gate order,
`--quick` uses a 10% sample of the corpus, and the last line is a table of all gates.

## Corpus (`corpus/dev.jsonl`)
One conversation per line: `{"id", "seed", "clock", "turns": [text, ...]}`. Built deterministically from:
- every SmolTalkBench gold prompt (`reference/smoltalkbench/test.parquet`, 1,000) as a single turn, and inside
  multi-turn conversations;
- follow-ups that exercise memory and reference resolution (names, ages, places, "how long does that take", "what's my
  name?", corrections, "/reset"-like restarts are NOT included: one conversation = one memory);
- perturbations: case, punctuation, whitespace, common typos and chat-speak;
- lines taken from PyBot's own source where a module matches literal phrases (so most modules fire at least once).
- several seeds and at least three clock values (a weekday, a weekend, 29 February of a leap year, and a 31 December).

The builder reports line coverage of `reference/pybot/bot` achieved by the corpus (via `sys.monitoring`/`coverage`) and
which modules never produced a candidate. Target: every module in `bot/modules/` wins or proposes in at least one turn.

## Canonical JSON for Python values
Plain JSON for `None`, `bool`, `str`, `list`, and `dict` with only `str` keys. Everything else is tagged so the
encoding is exact and order-preserving:
- `int` -> JSON integer (any size); `float` -> `{"$float": repr(x)}` (so `1.0`, `inf`, `nan`, `-0.0` survive);
- `tuple` -> `{"$tuple": [...]}`; `set` -> `{"$set": [... in iteration order]}`; `frozenset` -> `{"$frozenset": [...]}`;
- `dict` with any non-`str` key -> `{"$dict": [[k, v], ...]}` in insertion order;
- dates and datetimes -> `{"$date": iso}`, `{"$datetime": iso}`; anything else -> `{"$repr": repr(x), "$type": name}`.
RustBot must produce the same encoding from its own values (`pyrt::canon_json`).

## What the goldens hold (`$GOLDENS/`)
- `e2e/<corpus>.jsonl`: per turn `{"conv", "turn", "text", "reply", "module", "candidates": [[module, reply], ...],
  "memory": <canon>}`.
- `hooks/<module>.<hook>.jsonl`: every observe/respond/after/choose call: `{"conv", "turn", "args": <canon>,
  "memory_in": <canon>, "ret": <canon>|null, "exc": "<ExceptionType>"|null, "memory_out": <canon>,
  "rng_in", "rng_out"}`. RNG state is recorded compactly and exactly as, per `Random` instance (the global one and
  each `Random()` in creation order), the number of 32-bit MT outputs consumed since seeding; the recorder counts
  them with an instrumented `Random` subclass installed before PyBot is imported. RustBot restores a state by seeding
  and advancing. If a `setstate` breaks the counting, the record says `"rng": "opaque"` and only the result is compared.
- `functions/<dotted.name>.jsonl`: calls of selected pure leaf functions (args and result, canon JSON), capped per
  function, for M3.
- `pyrt/*.jsonl`: regex cases (every pattern compiled at run time, with flags, method, input and result), str method
  cases, format cases, random cases.
- `trace/`: which builtins/methods/stdlib attributes were called (feeds the inventory).
- `perf/python.json`: CPython numbers on this machine (cold start, per-turn p50/p95/mean, peak RSS).

## RustBot side (what the checks call)
Native binary: `rustbot/target/release/rustbot` (override with `$RUSTBOT_BIN`).
- `rustbot replay [--seed N] [--clock ISO] < conversations.jsonl > turns.jsonl`: same record format as
  `e2e/*.jsonl` (minus nothing).
- `rustbot hook <module> <hook> < hooks.jsonl > out.jsonl`: for each recorded call, restore `memory_in` and rng state,
  run the hook, print `{"ret", "exc", "memory_out", "rng_out"}`.
- `rustbot call <dotted.name> < functions.jsonl > out.jsonl`: `{"ret", "exc"}` per call.
- `rustbot pyrt-check <kind> < cases.jsonl > out.jsonl`: `{"result", "exc"}` per case.
- `rustbot` interactive / `-p TEXT` / `-s N` / `-v` / stdin lines: same as `pybot.py`.

WASM: `node wasmbot/node/replay.mjs < conversations.jsonl > turns.jsonl` (same format).
Space: `wasmbot/web/` served by its `server.py`; the page posts the same worker messages as the original.

## Implementation details (as built; these refine the sections above)
- `turn` indices are 0-based. Hook goldens are stored gzipped (`hooks/*.jsonl.gz`); RustBot never reads goldens
  directly, the checks feed it plain JSONL on stdin.
- Hook and function records carry `seed` and `clock`; `rustbot hook` and `rustbot call` must honour both.
- RNG state format: `{"global": n, "instances": [n0, n1, ...]}` (32-bit MT outputs consumed per instance, instances
  in creation order), optionally `"opaque": true` when a `setstate` broke the counting.
- pyrt goldens are `{"id", "case", "expect"}`; `rustbot pyrt-check <kind>` receives only the `case` objects. A regex
  `sub` with a callable replacement is recorded as `{"$callable": [return values in call order]}`.
- Each conversation runs on a freshly loaded PyBot (fork of a loaded parent, grouped by seed and clock). This differs
  from the Space, where `reset()` restores only the global RNG.
- `datetime.now(tz)` treats the frozen instant as UTC.
- The oracle runs Python with `-s` and `PYTHONSAFEPATH=1` instead of `-I`, because `-I` ignores `PYTHONHASHSEED`.
- Traces come from the raw-text run; the whitespace-normalised reply is stored inside `trace`, as `pybot_web` does.
- The hygiene check skips recorded replies that also appear verbatim in PyBot's source.
- Prompts: SmolTalkBench prompts are split by a stable hash into `dev` (~70%) and `holdout` (~30%, grading only;
  `build_corpus.py build --split holdout`).
- G6's time ratios need a Pyodide baseline at `$GOLDENS/perf/pyodide.json`. Nothing builds it yet, so until it
  exists `check_space.sh` checks replies only.
- File count: 528 Python files = 527 mirrored `.rs` files + `bot/__main__.py` (which maps to the CLI).

## Status (2026-11-16)
Written and syntax-checked (`bash -n`, `py_compile`); a 3-conversation smoke run passed e2e, trace and pyrt against
the fake RustBot. Not yet run: `corpus/dev.jsonl`, the goldens, and `selftest/run.sh` (run it before trusting a FAIL or PASS).

## Building the corpus and goldens
CPU-heavy (hours of CPython): run it on a compute host, not the development machine. Needs CPython 3.14,
`PYTHONHASHSEED=0` (the scripts set it), `pyarrow` for the corpus builder, and `reference/fetch.sh` done first.
1. Corpus: `python3 agent/checks/corpus/build_corpus.py mine --jobs N` (slow; mines trigger phrases from the modules
   into `corpus/mined.json`), then `build_corpus.py build` (`corpus/dev.jsonl`), then
   `build_corpus.py measure --jobs N` (`corpus/dev.coverage.json`). Commit the three files.
2. Goldens: `agent/checks/build_goldens.sh --shard i/N` for each i (independent, one CPU each, resumable), then
   `build_goldens.sh merge --shards N`, then `build_goldens.sh perf` (CPython numbers for G4, so run it on the machine
   the Rust perf will be measured on). Or `build_goldens.sh` alone runs all shards locally in parallel.
3. Self-test: `agent/checks/selftest/run.sh`. It must pass before any check result is trusted.
4. Hold-out (for grading; keep it away from whoever does the port):
   `build_corpus.py build --seed 1729 --split holdout --out H/holdout.jsonl`, then
   `build_goldens.sh --corpus H/holdout.jsonl --out H/goldens`. Grade with
   `CORPUS=H/holdout.jsonl GOLDENS=H/goldens agent/checks/check_e2e.sh` (and `check_trace.sh`, `check_perf.sh`).
