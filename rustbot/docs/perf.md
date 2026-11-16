# Native resource investigation

These measurements diagnose the verification build. They are not a G4 result:
the normal release profile and the complete parity and performance gates remain
unverified. The untouched Python checkout is the behavior reference.

## Full generated bot, before delegate sharing

On 2026-11-16, the complete 527-file native bot built in the `parity` profile
in 557.390 seconds, with peak compiler RSS of 19,963,288 KiB. This profile
disables LTO and optimizes the runtime separately from generated control flow.

`rustbot load-info` initialized the bot in 131.253 seconds. The command took
134.977 seconds overall and reached 17,857,440 KiB peak RSS. Its ordered loader
metadata matched Python exactly: 109 modules, 16 observers, 97 responders,
2 after hooks, an arbiter, and no random outputs consumed during loading.
These resource figures are far outside the acceptance thresholds.

A subsequent three-turn native replay exceeded its 560-second diagnostic
deadline without returning a completed conversation. The raw Python reference
completed those turns in 7.82 seconds. There is no native reply parity result
from that run. Debugger samples during initialization showed Unicode regex
class compilation and Aho-Corasick prefilter allocation. All owned diagnostic
processes were terminated and reaped.

Evidence, generated locally and excluded from Git, is under `rustbot/target/`:
`full-native-parity-fixed-build.log`, `native-loader-fixed*.json/log`, and
`native-raw-smoke-*`.

## Compiled delegate sharing

The candidate vendored `fancy-regex` 0.16.2 patch shares identical live
regex-automata delegates through `Arc`. A thread-local cache retains only weak
references and is bounded to 1,024 entries and 4 MiB of pattern text. Its key
includes the pattern, all syntax settings, and both delegate size limits.
Matching algorithms, validation limits, and prefilter settings are unchanged.
See [the patch provenance](../vendor/fancy-regex/RUSTBOT-PATCH.md).

In an isolated debug experiment holding 64 word-boundary expressions alive,
sharing reduced elapsed time from 4.945 to 1.214 seconds and peak RSS from
117,576 to 43,868 KiB. A separate 64-expression panel without boundaries changed
from 1.708 to 0.702 seconds and 28,636 to 18,500 KiB. All 128 recorded outputs
matched. These bounded panels do not predict full-bot resource use.

The complete upstream test suite passes with RustBot's pinned regex-automata
0.4.18: 254 tests, including 23 doctests. The production runtime also passes
all 205 unit/integration tests and the original 94,125 recorded G1 cases.
Its full native verification build completed in 776.082 seconds, peaking at
18,876,696 KiB compiler RSS, while the separate Python recording was running.
The loader's ordered metadata and RNG state still match Python exactly.
Loading now takes 50.463 seconds; the command takes 51.753 seconds overall and
peaks at 6,398,716 KiB RSS. This is a measured improvement over the earlier
native build, while remaining far outside the required Python-relative limits.
Evidence is in `native-loader-shared*` under `rustbot/target/`.

A separate 576-case Unicode differential panel exposed a pre-existing
`IGNORECASE` category mismatch involving U+0345. Passing recorded G1 cases does
not establish parity for every Unicode input.

## Unicode category correction and representation

The category correction disables case folding for category atoms and word
boundaries. Mixed classes combine folded literals/ranges with exact category
sets before applying negation. Compact property expressions replace expanded
word, digit and whitespace ranges. The direct regex-syntax dependency is pinned
to 0.8.11, and a complete normalized-range test compares its Unicode 16 classes
with the generated CPython 3.14 tables.

Validation passes 576 raw-CPython differential cases, 207 production runtime
tests, Clippy, and all 94,125 original recorded G1 cases. The complete native
verification build took 774.112 seconds and peaked at 19,602,584 KiB compiler RSS.
Ordered loader metadata and RNG state still match Python exactly. Loading takes
28.361 seconds; the command takes 29.099 seconds and peaks at 1,760,164 KiB RSS.
Artifacts are `unicode-native-build.log` and `native-loader-unicode*` under
`rustbot/target/`. These diagnostic improvements still do not meet G4.

## Python wrapper caches and one-pass dictionary lookup

The Python-level regex cache now belongs to each runtime's `re` module, with
the configurable LRU and FIFO behavior used by CPython. This honors the bot's
16,384-pattern setting and preserves pattern ownership across purge, matching
and separate runtimes. It shares engine programs independently of Python object
identity.

Post-fork debugger samples at 5, 20 and 60 seconds identified dictionary scans
and key copying in `knowledge.lookup._domain_rarity`. Dictionary methods now
share a single checked key lookup; they no longer copy every key and scan again.
The result distinguishes a missing key from a `KeyError` raised by equality.

The corrected official recording passes G0 and all 94,123 G1 cases; the runtime
passes 219 tests and Clippy. The full native verification build took 541.824
seconds and peaked at 23,138,388 KiB compiler RSS. Ordered loader metadata and
RNG state match Python exactly. Loading takes 19.631 seconds; the command takes
20.337 seconds and peaks at 1,764,876 KiB RSS. Evidence is in
`cache-dict-native-build.log` and `native-loader-cache-dict*` under
`rustbot/target/`. Whole-turn parity and G4 remain unverified.

## Indexing dictionaries with string keys

Ordered dictionary entries now have a lazy string-key index once they contain
at least 32 entries. Mixed/custom-key dictionaries keep the existing lookup
path. Mutable access invalidates the index, while checked inserts maintain it.
The threshold avoids most of the measured overhead for small frame dictionaries.

The change passes 225 production runtime tests, Clippy, all 94,123 corrected G1
cases, and an independent full bot/CLI typecheck including tracing. The complete
native build took 521.253 seconds and peaked at 23,285,344 KiB compiler RSS.
Ordered loader metadata and RNG state still match Python. Loading takes 14.828
seconds; the command takes 15.524 seconds and peaks at 1,770,688 KiB RSS.

The same raw three-turn conversation matches every reply, winner, candidate and
memory value in 235.104 seconds, compared with 441.390 seconds before indexing.
Peak direct-child rusage is 2,081,588 KiB. A separate traced run took 216.769
seconds; every per-file call list, respond span and source location matches,
while the `trace.calls` file-key ordering still needs correction. These are
diagnostic runs of one conversation, not full G3/G4 acceptance or repeated
benchmarks. Artifacts use `dict-index-native-build.log`, `native-loader-dict-index*`
and `native-raw-smoke-dict-index*` under `rustbot/target/`.
