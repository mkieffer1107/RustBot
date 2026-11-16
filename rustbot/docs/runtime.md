# Runtime implementation status

`pyrt` supplies native Rust values and operations to statically translated bot
functions. It does not evaluate Python source, bytecode, or an AST at runtime.
No Python interpreter or recorded bot replies are included.

The full corrected dev runtime gate passes **94,123/94,123 cases**: 59,136 regular
expressions, 28,574 string operations, 3,474 formatting cases, and 2,939 random
cases. This was verified through the standalone example calling the production
`pyrt::cases` implementation. The complete CLI, hooks, turns, trace, performance
and browser gates still need their own acceptance results. See
[PROGRESS.md](../../agent/PROGRESS.md) for the current evidence.

The runtime includes:

- Arbitrary precision integers, floats, code-point strings, mutable aliased
  containers, CPython64 zero-seed hashes/set iteration, canonical JSON, and
  memoized container/user-instance copies.
- Native function argument binding, lexical frames, modules/imports, classes,
  exceptions, lazy iterators/comprehensions/generators, object protocols, and
  private source/code identities for UI traces.
- Unicode 16 string mappings/normalization, Python formatting, backtracking
  regular expressions through a compatibility layer over `fancy-regex`, and
  CPython MT19937 with creation-ordered unseeded streams and count snapshots.
- Per-runtime Python regex wrapper caches with configurable LRU/FIFO capacities
  and purge, separate from shared engine programs; category membership stays
  exact under `IGNORECASE`, including mixed character classes and boundaries.
- Frozen date/time/calendar operations and the inventory's native wrappers for
  math, copy, JSON, hashing, difflib, AST arithmetic, importlib/pkgutil,
  functools/itertools, Fraction and Decimal.

The implementation targets the bot's required Python subset, not a general Python runtime.
Known limits beyond the recorded tests include lone Unicode surrogates, custom
`__deepcopy__` callbacks, some rare regex capture/backreference combinations,
general Decimal contexts/nonfinite arithmetic, and broader timezone/stdlib APIs.
Reference cycles are cleaned at runtime namespace teardown; there is no tracing
garbage collector, and temporary cycles can persist until their runtime drops.
User-defined truth conversion currently has an infallible value-level interface,
so propagating errors from arbitrary `__bool__` implementations remains a gap.

Native conversations use a loaded parent and per-record Unix forks to preserve
module/RNG isolation. The shared session resets memory and restores post-import
global random state for the Space semantics. WASM integration is deferred until
native parity is established. Local corpus generation and full checks are
authorized by root `AGENTS.md`. The official Python source remains unchanged;
the scoped, user-authorized recorder identity correction is documented there
and in `agent/PROGRESS.md`.

The corrected recording is under `rustbot/target/official-goldens`; use its
absolute path as `GOLDENS` for checks on this checkout. The previous ignored
`agent/goldens` directory is retained as historical evidence and contains the
earlier recorder's identity defect. Neither dataset is shipped in the bot.
