#!/usr/bin/env python3
"""Throwaway fake crates/bot/src with stub .rs files (one per Python file, one stub fn per code object)."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from lib import paths  # noqa: E402
from lib.check_structure import code_spans, mirror  # noqa: E402

dst = sys.argv[1]
n = 0
for dp, dirs, fs in os.walk(os.path.join(paths.PYBOT, "bot")):
    dirs.sort()
    for f in sorted(fs):
        if not f.endswith(".py"):
            continue
        rel = os.path.relpath(os.path.join(dp, f), paths.PYBOT)
        m = mirror(rel)
        if not m:
            continue
        out = os.path.join(dst, m)
        os.makedirs(os.path.dirname(out), exist_ok=True)
        with open(out, "w") as h:
            for q, a, b in sorted(code_spans(os.path.join(paths.PYBOT, rel)), key=lambda t: (t[1], t[0])):
                n += 1
                h.write(f"// py: {rel}:{a}-{b} {q}\npub fn f{n}() {{}}\n")
print(f"fake tree: {n} functions", file=sys.stderr)
