# RustBot: port PyBot to Rust and WebAssembly

Read this file first, then `AGENTS.md` (how you work), then `TASKS.md` (what to do next), then `PROGRESS.md` (where the
last session stopped).

## Goal

PyBot (`reference/pybot`, pinned at commit `7d63c7f`) is a rule-based chatbot: 530 Python files, about 636k lines,
standard library only. Port it to Rust so that:

1. **It behaves identically.** For the same conversation, seed and clock, RustBot gives the same reply, the same
   winning module and the same candidate list as PyBot, turn for turn. Identical means byte-identical strings.
2. **It is faster and uses less memory.** Native RustBot beats CPython PyBot on cold start, per-turn latency and peak
   memory (numbers in "Acceptance gates").
3. **It runs in the browser.** The same Rust compiles to `wasm32-unknown-unknown`. It replaces Pyodide behind the
   existing PyBot Space UI (`reference/pybot-space`), so a "RustBot" Space looks and works the same, only faster.

## Repository layout

```
rustbot/        Cargo workspace (native)
  crates/pyrt/      Python-compatible runtime: values, str/list/dict/set semantics, re, random, exceptions, clock
  crates/bot/       the bot itself: generated from reference/pybot/bot by the translator, then optimised
  crates/cli/       `rustbot` binary: same CLI as pybot.py, plus the test modes the checks use
  tools/py2rs/      the translator (Python, uses `ast`): reference/pybot/bot/**.py -> crates/bot/src/**.rs
wasmbot/        wasm-bindgen crate + web/ (the Space: index.html, worker.js, Dockerfile, server.py)
agent/          this spec, the task list, the progress log, and the checks
  checks/           the acceptance checks (do not edit; see AGENTS.md)
reference/      read-only inputs: pybot (the source of truth), pybot-space (the UI), smoltalkbench (prompts)
```

## Structure contract: one Python file = one Rust file, same name, same place (hard rule)

The Space's UI draws PyBot as a treemap of its files (`bot/knowledge/lookup.py`, `bot/compose/fallback.py`, ...) and
highlights the winning module, the files it called, and the file holding the reply's text. RustBot keeps that
picture, so the port mirrors the source tree exactly:

| Python | Rust (`rustbot/crates/bot/src/`) |
|---|---|
| `bot/__init__.py` | `lib.rs` |
| `bot/runtime.py` | `runtime.rs` |
| `bot/<pkg>/__init__.py` | `<pkg>/mod.rs` |
| `bot/<pkg>/<name>.py` | `<pkg>/<name>.rs` (e.g. `knowledge/lookup.rs`, `modules/answers.rs`, `memory/store_replies.rs`) |
| `bot/__main__.py` and `pybot.py` | `rustbot/crates/cli/src/main.rs` |

- Every one of the 527 Python files other than `__main__.py` has its Rust file. No extra modules sit inside `crates/bot/src/<pkg>/`, and no
  Python file is merged into another or split across several. All 528 names are valid Rust identifiers; none is a
  keyword. Shared support code lives in `pyrt` or in `crates/bot/src/manual/` and `crates/bot/src/support/`.
- Each Rust file holds the translation of the matching Python file, in the same order, with the same function names
  (snake_case already). Every translated function has a `// py: bot/<pkg>/<name>.py:<first>-<last> <qualname>`
  comment. Optimisation (M6) may change how a function works, but not which file it lives in or what it's called.
- The modules registry is generated from `bot/modules/*.py` with each module's `PRIORITY`, in runtime order.
- **The trace is part of the contract**, because the UI needs it. For the winning module, RustBot reports the same
  `files` (Python paths, `bot/<pkg>/<name>.py`), the same `calls` (`{file: [[first line, last line, qualname], ...]}`
  in first-call order, at most 40 per file, module-level code excluded), the same `respond` span, and the same
  `source`/`source_line` as `pybot_web.turn`. Lines and qualnames refer to the Python source, so the UI's links into
  the source still work. The translator generates a static table that maps each Rust function to its Python file,
  line span and qualname. A cheap call hook (one array write per call, compiled in by the `trace` cargo feature,
  which is on in the Space build) records the calls. `check_trace.sh` compares the trace with PyBot's.

## Behaviour contract (what "identical" means)

PyBot's whole interface is `bot/runtime.py`: `turn(text, memory) -> {"reply", "module", "candidates"}`. Each turn
runs every module's `observe`, then every `respond` on its own deep copy of memory, then `arbiter.choose`, then every
`after`. A module that raises is skipped. RustBot must reproduce all of it, including:

- **Which modules raise.** A Python exception inside a module is behaviour: the module is skipped. The Rust code must
  fail in exactly the same places (an `IndexError`, `KeyError`, `TypeError`, `AttributeError`, `ValueError`,
  `ZeroDivisionError`, `RecursionError` in Python is an `Err(PyExc::...)` at the same point in Rust).
- **Ordering.** Dict iteration is insertion order. Set iteration order follows CPython 3.14 with `PYTHONHASHSEED=0`
  (SipHash-1-3 with a zero key for `str`, CPython's int/tuple hashes, CPython's set table probing and resizing). Every
  check runs PyBot with `PYTHONHASHSEED=0`.
- **Randomness.** Same scheme as the Space's `pybot_web.py`: the global `random` is seeded with `SEED` (default 0);
  every `random.Random()` created without a seed is seeded with the string `f"{SEED}:{n}"`, n counting from 0 in
  creation order. Reproduce CPython's MT19937, seeding from `str` (SHA-512, version 2), `getrandbits`, `_randbelow`,
  `random()`, `choice`, `shuffle`, `sample`, `randint`, `getstate/setstate` exactly.
- **The clock.** `date.today()` and `datetime.now()` (54 call sites) read an injected clock. The checks freeze it
  (`PYBOT_CLOCK`, ISO 8601, default `2026-06-15T12:00:00`). Native and WASM builds take it from the same variable or
  argument, falling back to the system clock only in the interactive CLI and the Space.
- **Strings.** Python `str` is a sequence of code points: `len`, indexing, slicing, `find` and regex spans count code
  points, not bytes. `str.lower/upper/title/capitalize/casefold/split/strip/isalpha/isdigit/...` follow Python's
  Unicode rules. `repr`, `str(float)`, `%`-formatting, f-strings and `format()` follow CPython output exactly.
- **Regular expressions.** About 13,600 `re` call sites, about 1,900 with lookaround or backreferences. Python `re`
  semantics are the contract: backtracking order, `\b`/`\w`/`\d`/`\s` are Unicode-aware for `str` patterns,
  `re.IGNORECASE` folds like Python, `(?P<name>...)`, `\Z`, atomic groups and possessive quantifiers (3.11+),
  `sub` with group references and callables, `split` with groups, `findall` returning tuples. Use `fancy-regex`
  behind a compatibility layer, or write a backtracking engine that follows `sre`; either way the regex parity check
  decides.
- **Output of a turn.** `reply`, `module`, and `candidates` as `[(module, reply), ...]` in order. Memory after the turn
  must also match (canonical JSON, defined in `agent/checks/README.md`). Memory is how a divergence shows up one turn
  before it reaches a reply.

The Space's `files`, `calls` and `respond` trace fields are required (see "Structure contract"). `source` and
`source_line` only need `norm.json` and `lines.json`, which build from the Python source the same way as now.

## How to do it (recommended approach)

636k lines cannot be ported by hand in the time available. Port by **translator first, optimiser second**:

1. **`pyrt`: a small Python runtime in Rust.** A dynamic `Value` enum (`None, Bool, Int(i64, BigInt on overflow),
   Float, Str(Rc<str> + code-point index), List(Rc<RefCell<Vec>>), Tuple(Rc<[Value]>), Dict(Rc<RefCell<IndexMap>>),
   Set(CPython-ordered), Func, Module...`), `PyResult<T> = Result<T, PyExc>`, and the builtins and stdlib pieces PyBot
   uses (`re`, `random`, `datetime`, `calendar`, `math`, `string`, `unicodedata`, `copy.deepcopy`, `json` where used).
   Inventory first: list every builtin, method and stdlib attribute PyBot touches (a static `ast` scan plus a runtime
   trace), and implement exactly that set.
2. **`py2rs`: an `ast`-based translator** for the subset of Python PyBot actually uses (functions, closures, lambdas,
   comprehensions, generators (23 `yield`s), `try/except/finally`, `with`, 8 classes, decorators, `*args/**kwargs`,
   module globals, `getattr` on modules). Generate exactly one Rust file per Python file, at the mirrored path (see
   "Structure contract"), with a `// py: bot/<pkg>/<name>.py:<first>-<last> <qualname>` comment on every function
   and the trace table. Divergences then map straight back to the source. Statically resolve names: imports are acyclic per
   function call, globals are known at load time. Translate module-level constants to Rust `static`s built once
   (`LazyLock`); precompile every literal regex once.
3. **Reach parity in layers**, smallest first: `pyrt` unit parity (regex, str, random, formatting) -> per-function
   parity for leaf helpers -> per-module parity for each `observe`/`respond`/`after`/`choose` -> whole-turn parity.
   The checks record PyBot's behaviour at each layer (see `agent/checks/`). Fix the translator, not the generated
   code, whenever a fix applies to more than one site. Hand-edits to generated files go in `crates/bot/src/manual/` or
   are protected by a `// manual` marker the translator respects.
4. **Only then optimise**, guided by profiles (`samply`, `cargo flamegraph`, `perf`), re-running parity after every
   change. The obvious wins, measured on CPython: `bot/knowledge/lookup.py` fuzzy matching dominates turn time
   (millions of `str.lower` calls, half a million regex cache lookups per 10 turns); the runtime deep-copies memory
   for each of 97 handlers every turn (use copy-on-write / persistent maps so a copy is O(1)); regexes are rebuilt
   from strings at run time (hoist and cache them); big literal tables are rebuilt at import (make them `static`,
   `phf` or sorted arrays). Replace `Value` with concrete Rust types in hot paths once types are known.
5. **WASM last.** `wasmbot` wraps the same `bot` crate with `wasm-bindgen`: `load() -> {load_s, modules, handlers}`,
   `reset()`, `turn(text) -> JSON` with the same fields as `pybot_web.turn`. A new `worker.js` speaks the same message
   protocol as the original (`progress`, `ready`, `reply`, `error`), so `index.html` needs only branding changes.

Things that will bite:
- PyBot is non-local: modules import helpers from all four packages, and many `respond`s are wrapped by decorators
  from other files. Build the import graph first.
- A failing translation that raises where Python didn't (or vice versa) silently changes the candidate list. Per-module
  parity catches it; end-to-end parity alone hides it.
- Python `int` is unbounded; `bool` is an `int`; `True + 1 == 2`; `1 == 1.0` and `hash(1) == hash(1.0)` (dict keys).
- `isinstance` is used 4,400 times; module code inspects types of memory values.
- `sorted` is stable and compares tuples element-wise; mixed-type comparisons raise `TypeError`.
- Do not embed a Python interpreter (no pyo3, RustPython, Pyodide or CPython in the build) and do not ship recorded
  replies. The hold-out check uses conversations you never see.

## Acceptance gates

Run `agent/checks/check_all.sh` (local, dev corpus). Grading runs the same checks on a hold-out corpus built from prompts the dev corpus never uses (see `agent/checks/README.md`).
"All conversations" means every conversation in the corpus, and every turn in each.

| Gate | Checks | Pass when |
|---|---|---|
| G0 environment | `check_env.sh` | toolchains present; `reference/pybot` at `7d63c7f`; goldens built |
| G1 runtime parity | `check_pyrt.sh` | 100% of recorded regex, str-method, format and random cases match |
| G2 module parity | `check_modules.sh` | every observe/respond/after/choose call in the corpus matches (return value or exception type, and memory after), for all 97+16+N hooks |
| G3 turn parity | `check_e2e.sh` | 100% of turns match (reply, module, candidates, memory) on dev; the same on the hold-out corpus |
| G4 native performance | `check_perf.sh` | vs CPython 3.14 on the same node, warm, same corpus: cold start ≤ 0.25×, mean turn ≤ 0.5×, p95 turn ≤ 0.5×, peak RSS ≤ 0.5× (must); target ≥ 10× faster mean turn and ≤ 50 MB RSS |
| G4b structure + trace | `check_structure.sh`, `check_trace.sh` | every `bot/**.py` has its mirrored `.rs` (and nothing extra in the package folders); every function in it carries a `// py:` span; the winner's `files`, `calls`, `respond`, `source`, `source_line` match PyBot's for 100% of dev turns (`rustbot replay --trace`) |
| G5 WASM parity | `check_wasm.sh` | `wasmbot` in Node replays the dev corpus with 100% turn parity; `.wasm` ≤ 15 MB gzipped |
| G6 Space | `check_space.sh` | `wasmbot/web` serves; headless Chromium loads it, gets `ready`, sends 5 turns, every `reply` matches native; time-to-ready ≤ 0.25× of the Pyodide original, per-turn ≤ 0.25× |
| G7 hygiene | `check_hygiene.sh` | no pyo3/rustpython/cpython crates; no recorded replies embedded; `cargo clippy -D warnings` on hand-written crates; `cargo test` passes |

G3 on the hold-out set, G4 and G4b are the definition of done. G5 and G6 are done for the browser release.
