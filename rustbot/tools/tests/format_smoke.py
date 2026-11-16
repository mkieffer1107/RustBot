#!/usr/bin/env python3
"""Small standalone CPython boundary panel; never builds a corpus or goldens."""
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "agent" / "checks"))
from lib import canon
from lib.pyrt_cases import run_format


def cases():
    for value in [None, True, False, 10**80, -10**80, 0.0, -0.0, 1e-5, 1e16,
                  float("inf"), float("nan"), "it's fine", "both'\"",
                  "\0\x1f\t\né😀\u200b\u00a0", (1,), [1, "a"],
                  {"a": (1,)}, {1, 2}, frozenset(["a", "b"])]:
        for op in ("repr", "str"):
            yield {"op": op, "value": canon.encode(value)}
    for value, spec in [
        (-42, "08d"), (1234, "08,"), (1234, "010,"), (255, "#06x"),
        (123456789, "_"), (0, "#b"), (-5, "=+8d"), (65, "c"),
        (2.675, ".2f"), (99.995, ".2f"), (0.045, ".2f"),
        (12345.6789, ",.2f"), (0.00001, "g"), (999999.9, "g"),
        (1.0, ".2"), (1.0, "#.0e"), (1.0, "#g"), (1.0, "#.3g"),
        (-0.0001, "z.2f"), (-0.0, "+010.2f"), (0.125, ".1%"),
        (float("inf"), "+10F"), (float("nan"), " 10g"),
        (1e-4, ".3g"), (1e-5, ".3g"), (1e15, "g"),
        (5e-324, ".6e"), (1.5e300, ".2f"),
        ("é😀", "*^7.1s"), ("x", "05"), ("x", ">05"),
        (1, ".2d"), ("x", ","), (None, "5"), (1, "%A"),
    ]:
        yield {"op": "fstring", "value": canon.encode(value), "spec": spec}
    for fmt, args in [
        ("%s", None), ("%r %a", ("é", "é")), ("%04d", -3),
        ("%05.3d", 3), ("%.0d", 0), ("%#08x", 255), ("%#o", 8),
        ("%d", 2.75), ("%d", float("inf")), ("%d", float("nan")),
        ("%x", 2.0), ("%c", "é"), ("%c", 65), ("%c", 1.0),
        ("%*.*f", (8, 2, 1.5)), ("%*s", (-5, "é")),
        ("%.*s", (-1, "abc")), ("%*s", ("x", "a")),
        ("%(x)s %(n)04d", {"x": "é", "n": 7}),
        ("%%", ()), ("%s %s", "x"), ("%s", (1, 2)),
        ("literal", {}), ("literal", []), ("%", ()), ("%q", ()),
        ("%q", 1), ("%g", 999999.9), ("%.2f", 2.675),
    ]:
        yield {"op": "percent", "fmt": fmt, "args": canon.encode(args)}
    for template, args, kwargs in [
        ("{{{0!r}}} {x!a:>8}", ["q"], {"x": "é"}),
        ("{:{}}", ["x", 4], {}), ("{0:.{1}f}", [2.675, 2], {}),
        ("{0[0]} {0[1]}", [["a", "b"]], {}),
        ("{a[x]}", [], {"a": {"x": "b"}}),
        ("{} {0}", [1], {}), ("{0} {}", [1], {}),
        ("{missing}", [], {}), ("{1}", [0], {}),
        ("{{}}", [], {}), ("{", [], {}), ("}", [], {}),
        ("{0!x}", [1], {}), ("{0.real}", [1], {}),
    ]:
        yield {"op": "format", "template": template,
               "args": canon.encode(args), "kwargs": canon.encode(kwargs)}


def main():
    if os.environ.get("PYTHONHASHSEED") != "0":
        os.execvpe(sys.executable, [sys.executable, __file__, *sys.argv[1:]],
                   {**os.environ, "PYTHONHASHSEED": "0"})
    panel = list(cases())
    binary = sys.argv[1] if len(sys.argv) > 1 else str(ROOT / "rustbot" / "target" / "debug" / "rustbot")
    process = subprocess.run([binary, "pyrt-check", "format"],
                             input="".join(json.dumps(c) + "\n" for c in panel),
                             text=True, capture_output=True, check=True, timeout=30)
    actual = [json.loads(line) for line in process.stdout.split("\n") if line]
    assert len(actual) == len(panel), (len(actual), len(panel), process.stderr)
    failures = []
    for case, result in zip(panel, actual):
        try:
            expected = {"result": run_format(case), "exc": None}
        except Exception as exc:
            expected = {"result": None, "exc": type(exc).__name__}
        if expected != result:
            failures.append((case, expected, result))
    for case, expected, result in failures:
        print(json.dumps({"case": case, "expected": expected, "actual": result}, ensure_ascii=False))
    print(f"format smoke: {len(panel) - len(failures)}/{len(panel)} match CPython {sys.version.split()[0]}")
    return bool(failures)


if __name__ == "__main__":
    sys.exit(main())
