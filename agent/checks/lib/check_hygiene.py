#!/usr/bin/env python3
"""G7: no embedded interpreter, no embedded recorded replies, clippy -D warnings, cargo test."""
import glob
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from lib import goldens, paths, report  # noqa: E402

BAD_CRATES = re.compile(r"\b(pyo3|rustpython[\w-]*|cpython|pyembed|python3-sys|inkwell-python|pyo3-ffi)\b", re.I)
TEXT_EXT = (".rs", ".toml", ".json", ".txt", ".md", ".csv", ".tsv", ".js", ".mjs", ".ts", ".py", ".html", ".yaml", ".yml", ".lock")


def norm(s):
    return re.sub(r"[^a-z0-9]", "", s.lower())


def sh(cmd, cwd, timeout=3600):
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout)
    return p.returncode, p.stdout + p.stderr


def main():
    name = "hygiene"
    os.environ["CHECK_NAME"] = name
    rb = os.path.join(paths.ROOT, "rustbot")
    if not os.path.exists(os.path.join(rb, "Cargo.toml")):
        return report.missing(name, "rustbot/Cargo.toml not found")
    problems, notes = [], {}
    # 1. forbidden crates
    rc, tree = sh(["cargo", "tree", "--workspace", "--prefix", "none", "-e", "normal,build,dev", "--offline"], rb)
    if rc != 0:
        rc, tree = sh(["cargo", "tree", "--workspace", "--prefix", "none", "-e", "normal,build,dev"], rb)
    if rc != 0:
        problems.append("cargo tree failed: " + tree.strip()[-200:])
    else:
        hits = sorted({m.group(0).lower() for m in BAD_CRATES.finditer(tree)})
        if hits:
            problems.append("forbidden crates in the dependency tree: " + ", ".join(hits))
        notes["crates"] = len({l.split()[0] for l in tree.splitlines() if l.strip()})
    # 2. embedded recorded replies (strings >= 60 chars that PyBot's source does not itself contain)
    gpath = goldens.e2e_path()
    if gpath:
        src = []
        for dp, _, fs in os.walk(os.path.join(paths.PYBOT, "bot")):
            src += [open(os.path.join(dp, f), encoding="utf-8").read() for f in fs if f.endswith(".py")]
        py_norm = norm("\n".join(src))
        replies = set()
        for rec in paths.read_jsonl(gpath):
            for r in [rec["reply"]] + [c[1] for c in rec["candidates"]]:
                if len(r) >= 60 and norm(r) not in py_norm:
                    replies.add(r)
        rust_text, binaries = [], []
        for root in (rb, paths.wasmbot_dir()):
            for dp, dirs, fs in os.walk(root):
                dirs[:] = [d for d in dirs if d not in ("target", "node_modules", ".git", "pkg")]
                for f in fs:
                    p = os.path.join(dp, f)
                    if f.endswith(TEXT_EXT) and os.path.getsize(p) < 200_000_000:
                        rust_text.append(open(p, encoding="utf-8", errors="ignore").read())
        blob = norm("\n".join(rust_text))
        found = [r for r in replies if norm(r) in blob]
        for b in glob.glob(os.path.join(rb, "target", "release", "rustbot")) + glob.glob(os.path.join(paths.wasmbot_dir(), "**", "*.wasm"), recursive=True):
            data = open(b, "rb").read()
            found += [r for r in replies if r.encode("utf-8") in data]
        notes["recorded_replies_checked"] = len(replies)
        if found:
            problems.append(f"{len(set(found))} recorded reply strings are embedded, e.g. {sorted(set(found))[0][:80]!r}")
    # 3. clippy on hand-written crates, 4. tests
    pk = [p for p in ("pyrt", "cli") if os.path.isdir(os.path.join(rb, "crates", p))]
    if pk:
        args = ["cargo", "clippy"] + sum((["-p", p] for p in pk), []) + ["--all-targets", "--", "-D", "warnings"]
        rc, o = sh(args, rb)
        if rc != 0:
            problems.append("clippy -D warnings failed: " + o.strip()[-400:])
    else:
        problems.append("crates/pyrt and crates/cli not found")
    rc, o = sh(["cargo", "test", "--workspace"], rb)
    if rc != 0:
        problems.append("cargo test failed: " + o.strip()[-400:])
    for p in problems:
        print(f"  PROBLEM {p}")
    if problems:
        return report.fail_line(name, f"{len(problems)} problems")
    print(f"CHECK {name} PASS " + " ".join(f"{k}={v}" for k, v in notes.items()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
