# Runtime implementation status

This is an initial M2 implementation, not a completed Python runtime. The
translator and bot are not implemented. Runtime probes call the same Rust
functions that translated code will use; no Python interpreter or recorded bot
replies are included.

| Component | Implemented foundation | Remaining work |
| --- | --- | --- |
| Values | Arbitrary precision integers, booleans, floats, code point indexed strings, shared mutable lists/dicts, tuples, sets/frozensets, canonical JSON tags | Full builtin/object/module/callable model, container methods, surrogate code points, identity for inline immutable values (including NaN) |
| Operators | Integer promotion, add/subtract/multiply, signed floor division/modulo, bitwise operators, numeric/lexicographic comparisons, membership/indexing | True division, power, other operators and complete allocation error handling |
| Copies | Memoized deepcopy of mutable containers, repeated set/frozenset references, tuple/list cycles and unchanged immutable tuples | Full custom object and date/time copy semantics; reference cycles are not automatically collected |
| Strings | CPython 3.14 Unicode 16.0 predicates/mappings, contextual final sigma, casefold, split/strip/search/partition/replace, padding, code point indexing and other common methods | `encode`, `translate`, `format_map`; methods currently limit some result allocations to ten million units |
| Random | CPython MT19937, integer/string/bytes/float seeding, word counters, paired state identity/count restore with an eight-state history, choice/shuffle/sample/range, weighted choices; conversation context with global/creation-ordered unseeded streams and count snapshots | Hook/translated-module integration, identity-derived NaN seeds, exact large integer weighted accumulation; raw or expired state provenance is opaque |
| Hashes/sets | CPython 64-bit SipHash13 with zero seed, integer/float/tuple/frozenset hashes, CPython set probing, resize and deletion iteration | Date/time/opaque/NaN hashes, complete public set API (including Python copy/merge semantics); current `PySet::copy` preserves table layout for snapshots |
| Formatting | Python repr/str, format specs, conversions, percent formatting, nested template fields, basic date formatting | Full timezone/date directives, Python Unicode surrogates, complete parity across runtime-generated format inputs; numeric locale assumes C |
| Regex | Native shared `fancy-regex` engine; Python Unicode classes/boundaries, code point offsets, common flags, matching/iteration/split/sub, group and match JSON, empty/nonempty alternation and eager replacement validation | Scoped flags, case-insensitive Unicode edge cases, repeated-group `lastindex`, negated categories inside classes, complete Python syntax validation |
| Date/time/calendar | Typed Gregorian dates, normalized integer/float timedeltas, ISO dates/week dates/datetimes, fixed-offset arithmetic, monthrange and the oracle's injected-clock semantics | Dynamic `Value`/module/CLI dispatch, named/DST timezones, subsecond offsets, strftime/strptime/timestamps, timedelta multiply/divide |
| Other standard library | Unicode tables, inventory math functions with domain errors and integer-preserving ceil/floor, ASCII string constants | Full math parity (including logarithms of integers too large for float conversion), Unicode normalization/combining, remaining inventory APIs |

The checked-in smoke tools exercise a small set of boundary cases and the native
JSONL interface. They never execute a PyBot conversation or build goldens.
Acceptance parity requires generated goldens and all relevant gates. Local
generation and full checks are authorized by root `AGENTS.md`.
