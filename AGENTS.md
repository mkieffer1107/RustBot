# AGENTS.md

This repository ports PyBot (`reference/pybot`, a 636k-line Python chatbot) to Rust and WebAssembly with
byte-identical behaviour. Any coding agent (Codex, Claude, ...) follows the same documents:

1. [`agent/SPEC.md`](agent/SPEC.md): goal, structure contract (one `.py` = one mirrored `.rs`), behaviour contract,
   recommended approach, acceptance gates.
2. [`agent/AGENTS.md`](agent/AGENTS.md): working rules (never edit `agent/checks`, `reference`, `agent/SPEC.md`; log
   to `agent/PROGRESS.md`; commit small steps).
3. [`agent/TASKS.md`](agent/TASKS.md): milestones M0–M9, each with a "Done when" command.
4. [`agent/checks/README.md`](agent/checks/README.md): what the checks call (`rustbot replay|hook|call|pyrt-check`,
   `--trace`) and the golden/canonical-JSON formats.
5. [`agent/PROGRESS.md`](agent/PROGRESS.md): where the last session stopped.

Setup: `reference/fetch.sh` fetches the pinned PyBot source and SmolTalkBench prompts (git-ignored).

## Where to run what
- **The development machine: no CPU-heavy work.** Edit code, `cargo check`/`cargo build`, unit tests of a single
  crate, and smoke runs of a few turns only. Don't build the corpus or goldens, run PyBot over a corpus, or run
  `check_all.sh`, `check_perf.sh` or full parity checks there.
- **A separate compute host** runs the heavy work: corpus + goldens, full checks, perf, and grading on the hidden
  hold-out set. Steps: [`agent/checks/README.md`](agent/checks/README.md), "Building the corpus and goldens".
