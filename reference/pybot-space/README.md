---
title: PyBot
emoji: 🐍
colorFrom: gray
colorTo: gray
sdk: docker
app_port: 7860
pinned: false
short_description: Can Python be a chatbot?
---

# PyBot: Can Python be a chatbot?

A chat demo that runs [PyBot](https://github.com/huggingface/pybot), a chatbot written in plain Python, entirely in your
browser: [Pyodide](https://pyodide.org) (CPython compiled to WebAssembly) runs the bot in a Web Worker, and this Space
only serves files. Each reply shows which of PyBot's modules and files produced it; click a file to read it on GitHub.

## Files

- `index.html`: the page (chat, the reply's trace, and a treemap of PyBot's files; Plotly from jsDelivr).
- `worker.js`: loads Pyodide 314.0.7 from jsDelivr and the bot bundle, then answers turns. Like transformers.js does
  for model files, it keeps the bundle in Cache Storage, so a second visit doesn't download it again.
- `pybot_web.py`: the glue: fixed random seeds as in the benchmark's eval server (so replies match the leaderboard run),
  the trace via `sys.monitoring` (each function the winner runs, with its line range, and the lines of the winner's
  own `respond()`), and the file and line that hold a reply's text.
- `pybot-<sha12>-<hash>.zip`: `bot/**.pyc` compiled for Pyodide's CPython 3.14 (loading skips compiling the source),
  plus `norm.json` (each file's text, letters and digits only) for finding the file that holds a reply, and
  `lines.json` (how many of those each line has) for turning that into a line number.
  It is built from the same code as `bot/` in the GitHub repo.
- `tree.json`: PyBot's files and line counts for the treemap, and the GitHub repo and commit its links open. Links go
  to exact lines (the reply's text, the winner's `respond()`, the first function the winner ran in a file), so `ref` is
  a commit, not `main`: line numbers would drift as soon as `main` changes.
- `Dockerfile` and `server.py`: only serve the files above (`http.server` plus cache headers: the page, scripts and
  data are revalidated on every load so updates show up on a normal reload; the bundle, renamed on every build, is
  cached for good). A static Space would redirect the large files to a
  CDN without CORS headers, which blocks the worker's fetch of the bundle and the fonts.

## Rebuild from a newer bot

```sh
git clone https://github.com/huggingface/pybot && (cd pybot && zip -qr ../bot-src.zip bot -i '*.py')
npm install pyodide@314.0.7   # the same version as worker.js: bytecode only loads in the Python that compiled it
node build/build.mjs bot-src.zip build/build_bundle.py pybot-<sha12>.zip   # then add the first 8 hex of its sha256
```

Then regenerate `tree.json` (`[path, line count]` for every `bot/**.py`, and `ref` set to the GitHub commit you
cloned) and update `BUNDLE` in `worker.js` (a new name makes browsers download the new bundle). To test locally, serve this folder (`python -m http.server`) and open it.
