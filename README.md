# RustBot

PyBot ([huggingface/pybot](https://github.com/huggingface/pybot), a 636k-line rule-based Python chatbot) ported to
Rust and WebAssembly: the same replies, faster, in less memory, and in the browser without Pyodide.

| Folder | What |
|---|---|
| `rustbot/` | the native port: Cargo workspace (`pyrt` runtime, generated `bot`, `cli`) and the `py2rs` translator |
| `wasmbot/` | the WASM build and the RustBot Space (same UI as the PyBot Space, new worker) |
| `agent/` | what the autonomous port needs: `SPEC.md` (goal, approach, gates), `TASKS.md` (milestones), `AGENTS.md` (rules), `PROGRESS.md` (log), `checks/` (acceptance tests) |
| `reference/` | read-only inputs: `fetch.sh` pins PyBot `7d63c7f` and SmolTalkBench; `pybot-space/` is the original Space UI |

Start with [agent/SPEC.md](agent/SPEC.md). The checks and how to build their inputs are in [agent/checks/README.md](agent/checks/README.md).
