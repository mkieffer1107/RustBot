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
not establish parity for every Unicode input. Its isolated correction is being
validated separately from the delegate-sharing patch.
