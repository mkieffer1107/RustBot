#!/usr/bin/env python3
"""G4b: one Python file = one Rust file, and every translated fn carries a correct `// py:` comment.

    check_structure.py [--partial]      Rust tree: $RUSTBOT_SRC (default rustbot/crates/bot/src)
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from lib import paths, report  # noqa: E402

PKGS = ("compose", "knowledge", "memory", "modules", "understanding")
FN = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:unsafe\s+)?(?:async\s+)?(?:extern\s+\"[^\"]*\"\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)")
PYC = re.compile(r"^\s*//\s*py:\s*(bot/\S+\.py):(\d+)-(\d+)\s+(\S.*?)\s*$")
EXEMPT_DIRS = ("manual", "support")


def mirror(rel):
    """bot/... .py -> path relative to crates/bot/src."""
    parts = rel.split("/")[1:]
    if parts == ["__init__.py"]:
        return "lib.rs"
    if parts == ["runtime.py"]:
        return "runtime.rs"
    if parts == ["__main__.py"]:
        return None  # lives in crates/cli/src/main.rs
    if parts[-1] == "__init__.py":
        return "/".join(parts[:-1] + ["mod.rs"])
    return "/".join(parts)[:-3] + ".rs"


def code_spans(path):
    """{(qualname, first, last)} of every function-like code object in a Python file (as pybot_web computes spans)."""
    code = compile(open(path, encoding="utf-8").read(), path, "exec")
    out, stack = set(), [code]
    while stack:
        c = stack.pop()
        for k in c.co_consts:
            if hasattr(k, "co_code"):
                stack.append(k)
                last = max((l for _, _, l in k.co_lines() if l), default=k.co_firstlineno)
                out.add((k.co_qualname, k.co_firstlineno, last))
    return out


def main():
    partial = "--partial" in sys.argv
    name = "structure"
    src = os.environ.get("RUSTBOT_SRC") or os.path.join(paths.ROOT, "rustbot", "crates", "bot", "src")
    bot = os.path.join(paths.PYBOT, "bot")
    if not os.path.isdir(src):
        return report.missing(name, f"{src} not found")
    py_files = []
    for dp, dirs, fs in os.walk(bot):
        dirs.sort()
        py_files += [os.path.relpath(os.path.join(dp, f), paths.PYBOT) for f in sorted(fs) if f.endswith(".py")]
    want = {}
    for rel in py_files:
        m = mirror(rel)
        if m:
            want[m] = rel
    have = set()
    for dp, dirs, fs in os.walk(src):
        dirs.sort()
        for f in fs:
            if f.endswith(".rs"):
                have.add(os.path.relpath(os.path.join(dp, f), src).replace(os.sep, "/"))
    problems = []
    missing = [m for m in sorted(want) if m not in have]
    if missing and not partial:
        problems.append(f"{len(missing)} Python files have no Rust file, e.g. {missing[0]} (for {want[missing[0]]})")
    extra = [h for h in sorted(have) if h not in want and (h.split("/")[0] in PKGS or "/" not in h) and
             h.split("/")[0] not in EXEMPT_DIRS]
    extra = [h for h in extra if not (h.startswith("manual") or h.startswith("support"))]
    if extra:
        problems.append(f"{len(extra)} extra .rs files not mirroring a Python file, e.g. {extra[0]}")
    spans_cache = {}
    n_fn = n_ok = n_bad = n_manual = 0
    bad_examples = []
    for h in sorted(have):
        if h not in want:
            continue
        rel = want[h]
        if rel not in spans_cache:
            spans_cache[rel] = code_spans(os.path.join(paths.PYBOT, rel))
        lines = open(os.path.join(src, h), encoding="utf-8", errors="replace").read().splitlines()
        for i, line in enumerate(lines):
            m = FN.match(line)
            if not m:
                continue
            n_fn += 1
            j = i - 1
            while j >= 0 and (lines[j].strip().startswith("#[") or lines[j].strip().startswith("///") or not lines[j].strip()):
                j -= 1
            prev = lines[j] if j >= 0 else ""
            if re.match(r"^\s*//\s*manual\b", prev):
                n_manual += 1
                continue
            c = PYC.match(prev)
            why = None
            if not c:
                why = "no `// py: bot/..py:first-last qualname` comment before it"
            elif c.group(1) != rel:
                why = f"comment names {c.group(1)} but this file mirrors {rel}"
            elif (c.group(4), int(c.group(2)), int(c.group(3))) not in spans_cache[rel]:
                why = f"{c.group(4)} {c.group(2)}-{c.group(3)} is not a function span of {rel}"
            if why:
                n_bad += 1
                if len(bad_examples) < 8:
                    bad_examples.append(f"{h}:{i + 1} fn {m.group(1)}: {why}")
            else:
                n_ok += 1
    if n_bad:
        problems.append(f"{n_bad} functions lack a valid // py: comment")
    for e in bad_examples:
        print(f"  {e}")
    for p in problems:
        print(f"  PROBLEM {p}")
    nums = f"python_files={len(want)} rust_files={len(have & set(want))} missing={len(missing)} extra={len(extra)} " \
           f"fns={n_fn} fns_ok={n_ok} fns_bad={n_bad} fns_manual={n_manual}" + (" partial=1" if partial else "")
    print(f"CHECK {name} {'FAIL' if problems else 'PASS'} {nums}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
