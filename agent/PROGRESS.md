# Progress log

Append only. Newest entry at the bottom. Each entry: date and time, milestone, what changed, check numbers, the exact
next step.

## Baseline (2026-11-16, measured on an Apple-silicon laptop with CPython 3.14.6, before any port)
- PyBot `7d63c7f`: import 2.0 s; 10-turn conversation mean 730 ms/turn cold, 404 ms warm, worst 2.3 s; peak RSS 236 MB.
- 109 modules: 97 `respond`, 16 `observe`. Profile: `bot/knowledge/lookup.py` fuzzy matching dominates.

## Check disputes
(none yet)

## Log
