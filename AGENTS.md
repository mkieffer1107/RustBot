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
- **Local execution is authorized.** Build the corpus and goldens, run PyBot, full parity checks,
  `check_all.sh`, and performance checks on this machine. Use bounded worker counts and timeouts so implementation
  can continue alongside long jobs. A separate compute host is optional; SSH is not required.
- This supersedes older compute-host-only wording in supporting documents. Follow the build steps in
  [`agent/checks/README.md`](agent/checks/README.md), "Building the corpus and goldens", locally. Keep the hidden
  hold-out set separate from development as described there.
