# Tasks

Work top to bottom. A milestone is done when its "Done when" command exits 0. Record each finished milestone, with
the commit, in `PROGRESS.md`. Milestones M3–M5 can run in parallel on separate git worktrees once M2 is done.

## M0 — Bootstrap (gate G0)
- Read `SPEC.md`, `AGENTS.md`, `agent/checks/README.md`.
- Build the corpus and goldens (PyBot recorded on the dev corpus at every layer) on a compute host, not the
  development machine: `agent/checks/README.md`, "Building the corpus and goldens". Run
  `agent/checks/selftest/run.sh` once there and record its result in `PROGRESS.md`. Commit the corpus files; keep the
  goldens out of git (`agent/goldens/` is ignored).
- Create the Cargo workspace skeleton (`rustbot/Cargo.toml`, crates `pyrt`, `bot`, `cli`) and `rustbot/tools/py2rs/`.
- **Done when:** `agent/checks/check_env.sh`

## M1 — Inventory
- Write `rustbot/tools/inventory.py`: from `ast` plus the runtime trace in `goldens/trace/`, list every builtin,
  method, stdlib function, syntax node type and regex feature PyBot uses, with counts and first file:line.
  Output `rustbot/docs/inventory.md` and `inventory.json`.
- Write `rustbot/docs/import_graph.md`: per module, which files it pulls in; which `respond`s are decorated and by what.
- **Done when:** `inventory.json` exists, lists every `ast` node type found in `reference/pybot/bot` (the check
  compares against its own scan), and `check_env.sh` still passes.

## M2 — `pyrt` core (gate G1)
- `Value`, `PyExc`, `PyResult`; `str` with code-point semantics and every `str` method in the inventory; list, tuple,
  dict (insertion order), set (CPython 3.14 order, `PYTHONHASHSEED=0`), int (with big-int overflow), float, bool.
- `repr`/`str`/`format`/`%`/f-string formatting identical to CPython.
- `re` compatibility layer (compile cache, every flag and method in the inventory).
- `random` (MT19937, str seeding, the seeding scheme in SPEC.md), `datetime`/`date`/`calendar` with injected clock,
  `copy.deepcopy`, `math`, `string`, `unicodedata` subset.
- Canonical JSON encoder for `Value` (format in `agent/checks/README.md`).
- `cli` modes: `rustbot pyrt-check <cases.jsonl>`.
- **Done when:** `agent/checks/check_pyrt.sh`

## M3 — Translator, leaves first (gate G2 partial)
- `py2rs`: translate each Python file to its mirrored Rust file (`bot/knowledge/lookup.py` ->
  `crates/bot/src/knowledge/lookup.rs`; full table in SPEC.md, "Structure contract"), same function names, a
  `// py:` span comment per function, and the generated trace table. Start with `bot/understanding/` (25k lines, imported by
  almost everything), then `bot/compose/`, `bot/memory/`, `bot/knowledge/`.
- `rustbot call` mode (see checks README) for per-function parity on leaf helpers recorded in `goldens/functions/`.
- **Done when:** `agent/checks/check_modules.sh --functions` passes for every recorded function in `understanding/`
  and `compose/`, and `agent/checks/check_structure.sh --partial` passes (every file generated so far sits at its
  mirrored path with spans).

## M4 — All hooks (gate G2)
- Translate all of `bot/` including `bot/modules/` and `runtime.py`'s loader (priority order, filename order).
- **Done when:** `agent/checks/check_modules.sh` (every observe/respond/after/choose call).

## M5 — Whole turns (gate G3)
- `rustbot replay` mode. Same CLI flags as `pybot.py` (`-p`, `-i`, `-s`, `-v`, stdin lines).
- `rustbot replay --trace`: the winner's `files`, `calls`, `respond`, `source`, `source_line`, as `pybot_web.turn`.
- **Done when:** `agent/checks/check_e2e.sh` reports 100% on the dev corpus, and `check_structure.sh` and
  `check_trace.sh` pass.

## M6 — Performance (gate G4)
- Profile (`samply record`, `perf`), then optimise; re-run `check_e2e.sh` after every change. Candidates are listed
  in SPEC.md ("Only then optimise"). Record each optimisation and its measured effect in `rustbot/docs/perf.md`.
- Optimisations keep every function in its mirrored file under its own name (`check_structure.sh` must still pass).
- **Done when:** `agent/checks/check_perf.sh` passes the "must" thresholds. Keep going toward the targets while time
  remains.

## M7 — WASM (gate G5)
- `wasmbot`: `wasm-bindgen` crate over `bot`; `load`, `reset`, `turn`; clock and seed passed in. Release profile with
  `opt-level = "z"` or `"s"` (measure both), `lto = true`, `codegen-units = 1`, `panic = "abort"`; `wasm-opt -Oz`.
- `wasmbot/node/replay.mjs`: same JSONL contract as `rustbot replay`.
- **Done when:** `agent/checks/check_wasm.sh`

## M8 — The RustBot Space (gate G6)
- `wasmbot/web/`: copy `reference/pybot-space/{index.html,server.py,Dockerfile,fonts}`, rename to RustBot, write a
  new `worker.js` with the same message protocol that loads the `.wasm` instead of Pyodide; `norm.json`/`lines.json`
  built by `wasmbot/build_assets.py` the same way as `build_bundle.py`.
- The worker's `reply` message carries the trace from the `trace` feature, so the treemap works unchanged.
- **Done when:** `agent/checks/check_space.sh`

## M9 — Finish (gate G7)
- `agent/checks/check_hygiene.sh`, then `agent/checks/check_all.sh`.
- `rustbot/README.md` with measured numbers (native and browser) against PyBot.
