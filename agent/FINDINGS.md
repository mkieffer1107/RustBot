# Findings from the translator-based port (2026-11-16 to 2026-11-17)

The first attempt at RustBot generated Rust automatically from PyBot's Python with a translator (`rustbot/tools/py2rs`)
and a Python-compatible runtime (`rustbot/crates/pyrt`). It was stopped on 2026-11-17 in favour of translating the
files by hand. This file records what was learned, so the hand-written port starts from evidence instead of guesses.
Everything referenced here remains in git history (last translator commit: `95723ca`).

## 1. What the translator approach produced

- **Approach.** One Rust file per Python file (all 527 mirrors), each Python statement emitted as calls into a dynamic
  runtime. Variables were looked up by name in dictionaries, calls went through Python-style argument binding into a
  dict, every integer was a `BigInt`, and every expression became a block of temporaries plus dynamic calls.
  In effect, it was a Python interpreter specialised per file and compiled into Rust.
- **Correctness was good and converging.**
  - Runtime behaviour: 94,123 of 94,123 recorded Python cases matched (regex, `str` methods, formatting, `random`).
  - Functions: 1,782 of 1,787 recorded functions matched on every call (242,643 of 242,808 calls). All 5 failures
    came from one bug (section 3, point 1).
  - Module hooks: 650,793 of 753,393 calls matched. Almost all misses were timeouts on 16 heavy hooks, not wrong
    answers; `detail_question.respond` had 7 real mismatches.
  - Whole conversations: 60–65% of the 6,508 dev turns matched byte for byte (reply, module, candidates, memory and
    trace). The remaining failures were dominated by `answers`, whose knowledge lookup picked a different entry;
    the set-ordering bug is the likely cause.
- **Performance was the blocker.** On the same 8 conversations (20 turns), single process:

  | | CPython 3.14 PyBot | Rust, first optimised build | Rust, best build (slots + fixes) |
  |---|---|---|---|
  | Wall time | 1 min 32 s – 2 min | 25 min 51 s (≈17× slower) | 17 min 10 s (≈10× slower) |
  | Peak memory | 199 MB | 2.17 GB | 2.18 GB |
  | Binary size | n/a | 347 MB | 413 MB |
  | Build time | n/a | 54–83 min on 64 cores | 46 min |

  The approach can't plausibly get "much faster than Python" or produce a small WASM binary. It reproduces
  interpreter work (name lookups, argument binding into dicts, big-integer arithmetic, bound-method objects) without
  CPython's decades of optimisation.

## 2. Where the time went (perf profiles of the native build)

1. **Frame registry pruning: 19%.** Every 256 frame creations, a global list of every frame ever registered was
   walked. Fixed by pruning only after the list doubles.
2. **Generic `Value` equality and memcmp in dict scans: 10% + 4%.** Every name lookup scanned small dicts linearly.
3. **malloc/free: about 15%.**
   - Per call: a `HashSet<String>` for each scope list, two dicts and a `HashSet` in argument binding, and a new
     key string for every variable access.
   - Values: `BigInt` for every integer, and a fresh string allocation for every string literal on every
     evaluation. The 23,010 integer literals were parsed from decimal strings on every evaluation.
4. **Builtin method calls.** Each one allocated a bound-method object and an owned copy of the method name.

The lesson for a hand port: use plain Rust locals, concrete types (`i64`, `&str`/`String`, `Vec`, real structs)
and direct function calls. Keep Python-compatible behaviour only where the output can observe it (section 3).

## 3. Python behaviour a byte-identical port must reproduce (found the hard way)

1. **Set iteration order (CPython, `PYTHONHASHSEED=0`).** The order depends on the hash-table history, not only the
   contents:
   - `set(x)` / `frozenset(x)` from a set or frozenset copies the source table (presize, then a table-order copy);
   - from an exact `dict`, presize once for all keys, then insert;
   - otherwise insert one by one;
   - `frozenset(fs)` returns `fs` itself.

   Inserting element by element grows the table in different steps and changes iteration order. This single bug
   caused all 5 failing functions (e.g. `compose.fallback._the_words_in`, `compose.onscreen._stems`,
   `understanding.topics._person_words`). PyBot iterates word sets in its ranking code, so iteration order changes
   replies. The fix (`set()`/`frozenset()` going through the CPython-faithful update path) is in
   `pyrt/src/builtins.rs` and `pyrt/src/containers.rs`.
2. **Regex.** About 13,600 call sites, about 1,900 with lookaround or backreferences, so the Rust `regex` crate is
   not enough.
   - The `pyrt` regex layer (vendored `fancy-regex` plus Python compatibility) matched all 59,136 recorded regex cases.
   - It needed: Unicode `\w`/`\d`/`\s`/`\b` exactly as CPython's tables; IGNORECASE handling of U+0345 inside
     classes; Python's `re` cache semantics (`lookup.py` sets `re._MAXCACHE = 16384`); empty-match iteration rules.
3. **`str`** is code points: `len`, slicing and regex spans count characters, not bytes. `lower`, `title`, `split`,
   `isalpha`, etc. follow Python's Unicode tables (generated into `pyrt` from CPython 3.14).
4. **`random`.**
   - MT19937 with CPython's string seeding (SHA-512), `getrandbits`, `_randbelow`, `choice` and `shuffle`.
   - The Space's scheme: global seed 0, and every unseeded `random.Random()` seeded with `f"0:{n}"` in creation order.
5. **Clock.** `date.today()` and `datetime.now()` (54 sites) must read an injected clock; the checks freeze it.
6. **Exceptions are behaviour.** A module that raises is skipped by `runtime.turn`, so the port must fail in exactly
   the same places (`KeyError`, `IndexError`, `TypeError`, ...).
7. **Dict order** is insertion order everywhere, including memory, which the checks compare key by key.
8. **The trace.** The Space treemap needs, for the winning module, the Python file names, first-call order of
   functions (up to 40 per file) with Python line spans and qualnames, the `respond` span, and the reply's source
   line. `rustbot/data/trace/{norm.json,lines.json}` and the old `crates/bot/src/support/trace.rs` (in git history)
   implement this. `trace.calls` keys must keep first-call order.

## 4. What to keep for the hand-written port

- **`agent/checks/`** (corpus, oracle, golden recorder, `check_*`, self-test with a fake RustBot). The CLI modes
  `replay`, `hook`, `call` and `pyrt-check` give per-function and per-hook parity, which is how bugs get localised.
- **`agent/checks/corpus/dev.jsonl`:** 2,413 conversations, 6,508 turns.
- **Goldens:** rebuild them with `agent/checks/build_goldens.sh` (shard it; about 6 CPU-hours in total).
- **`rustbot/crates/pyrt`**, as a library of verified Python-compatible pieces rather than as an execution model:
  regex compatibility, Unicode tables, CPython set ordering, Mersenne Twister and seeding, `%`/`format` formatting,
  `repr(float)`. Its dynamic `Value`/`Frame` machinery is what made the port slow; don't build the hand port on it.
- **Structure contract:** one `.rs` per `.py`, same names and folders (`agent/SPEC.md`). The UI relies on it.

## 5. Practical notes

- A 600k-line generated crate takes 45–80 minutes to build even with parallel codegen. Hand-written, typed code
  will be far smaller. Keep each package's Rust in its own crate so builds parallelise.
- `rust-lld` (the default linker) writes through memory maps. On network filesystems that intermittently crashes
  with SIGBUS (`ld terminated with signal 7`); build on local disk.
- Native memory was about 2.2 GB per conversation, so parallel parity runs need about 4–8 GB per worker.
