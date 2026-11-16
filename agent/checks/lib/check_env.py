#!/usr/bin/env python3
"""G0: toolchains present, reference/pybot at the pinned commit, goldens built."""
import glob
import json
import os
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from lib import goldens, paths, report  # noqa: E402


def out(*cmd, cwd=None):
    try:
        return subprocess.run(cmd, capture_output=True, text=True, cwd=cwd, timeout=60).stdout.strip()
    except Exception:  # noqa: BLE001
        return ""


def main():
    name = "env"
    problems, notes = [], {}
    for tool in ("cargo", "rustc", "node", "python3"):
        if shutil.which(tool):
            print(f"  ok   {tool}: {out(tool, '--version').splitlines()[0] if out(tool, '--version') else '?'}")
        else:
            problems.append(f"{tool} not found")
    if shutil.which("rustc"):
        sysroot = out("rustc", "--print", "sysroot")
        if not os.path.isdir(os.path.join(sysroot, "lib", "rustlib", "wasm32-unknown-unknown")):
            problems.append("rust target wasm32-unknown-unknown not installed (rustup target add wasm32-unknown-unknown)")
        else:
            print("  ok   wasm32-unknown-unknown target")
    for opt in ("wasm-bindgen", "wasm-opt", "wasm-pack"):
        print(f"  {'ok  ' if shutil.which(opt) else 'warn'} {opt}{'' if shutil.which(opt) else ' not found (needed for M7)'}")
    py = sys.version_info
    notes["python"] = f"{py.major}.{py.minor}"
    if (py.major, py.minor) < (3, 14):
        problems.append(f"python3 is {py.major}.{py.minor}; PyBot is recorded with CPython 3.14 (set/str hash orders)")
    if not os.path.isdir(os.path.join(paths.PYBOT, "bot")):
        problems.append(f"reference/pybot missing ({paths.PYBOT})")
    else:
        head = out("git", "-C", paths.PYBOT, "rev-parse", "HEAD")
        if not head.startswith(paths.PYBOT_COMMIT):
            problems.append(f"reference/pybot is at {head[:12] or 'unknown'}, expected {paths.PYBOT_COMMIT}")
        else:
            print(f"  ok   reference/pybot at {head[:7]}")
    m = goldens.manifest()
    if m is None:
        return report.missing(name, f"goldens not built ({paths.goldens_dir()}/MANIFEST.json missing): run agent/checks/build_goldens.sh") \
            if not problems else report.fail_line(name, "; ".join(problems) + "; goldens also not built")
    for sub, pat in (("e2e", "*.jsonl"), ("hooks", "*.jsonl*"), ("functions", "*.jsonl*"), ("pyrt", "*.jsonl*"), ("trace", "*.json")):
        n = len(glob.glob(os.path.join(paths.goldens_dir(), sub, pat)))
        if n == 0:
            problems.append(f"goldens/{sub} is empty")
        notes[sub] = n
    if not os.path.exists(paths.corpus_path()):
        problems.append(f"corpus {paths.corpus_path()} not found")
    elif m and "sha256" in str(m.get("corpus_sha256", "sha256")):
        import hashlib
        sha = hashlib.sha256(open(paths.corpus_path(), "rb").read()).hexdigest()
        if sha != m.get("corpus_sha256"):
            problems.append("corpus file differs from the one the goldens were built from (rebuild goldens)")
    if m.get("python") and m["python"].split(".")[:2] != f"{py.major}.{py.minor}".split("."):
        notes["golden_python"] = m["python"]
    for p in problems:
        print(f"  PROBLEM {p}")
    if problems:
        return report.fail_line(name, f"{len(problems)} problems")
    print(f"CHECK {name} PASS turns={m['stats'].get('turns')} hook_calls={m['stats'].get('hook_calls')} " +
          " ".join(f"{k}={v}" for k, v in notes.items()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
