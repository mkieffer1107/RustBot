#!/usr/bin/env python3
"""A small native CLI/runtime differential smoke test; never loads PyBot or a corpus."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


def cases():
    strings = [
        {"self": "aé😀z", "method": "__getitem__", "args": [-2]},
        {"self": "Straße ΣΟΣ", "method": "casefold", "args": []},
        {"self": "a\u001cb\u00a0c", "method": "split", "args": []},
        {"self": "  a b  ", "method": "rsplit", "args": [None, 1]},
        {"self": "é😀x", "method": "find", "args": ["x", 1, 3]},
        {"self": "abc", "method": "find", "args": ["", 99]},
        {"self": "", "method": "__getitem__", "args": [0]},
        {"self": "x", "method": "split", "args": [""]},
        {"self": "tab\tline\r\nlast\u2028", "method": "splitlines", "args": [True]},
        {"self": "a\t😀\tx", "method": "expandtabs", "args": [4]},
        {"self": "ǅ ΣΟΣ", "method": "swapcase", "args": []},
        {"self": "\u200b", "method": "isprintable", "args": []},
    ]
    randoms = []
    for seed in [0, -7, 2 ** 70, "0:0", "naïve", {"$float": "-3.5"}]:
        randoms.append({"seed": seed, "ops": [
            ["random", []], ["getrandbits", [0]], ["getrandbits", [65]],
            ["randrange", [1, 2 ** 70, 3]], ["randrange", [10, -10, -3]],
            ["randint", [-4, 4]], ["choice", ["é😀xyz"]],
            ["shuffle", [[1, 2, 3, 4, 5]]], ["sample", [list(range(80)), 9]],
            ["getstate", []], ["random", []],
        ]})
    randoms.extend({"seed": 0, "op": op, "args": args} for op, args in [
        ("choice", [[]]), ("randrange", [0]), ("getrandbits", [-1]),
        ("sample", [[1], 2]), ("randint", [3, 1]),
    ])
    formats = [{"op": "repr", "value": value} for value in [
        "it's \"q\"", "tab\t\n\x00é😀\u200b", 10 ** 70,
        {"$float": "-0.0"}, {"$float": "1e-05"},
        {"$tuple": [1, "a"]}, {"$dict": [["$float", "literal"]]},
    ]]
    formats += [
        {"op": "fstring", "value": 42, "spec": "02d", "conv": ""},
        {"op": "fstring", "value": {"$float": "2.675"}, "spec": ".2f", "conv": ""},
        {"op": "fstring", "value": "é", "spec": ">5", "conv": "r"},
        {"op": "percent", "fmt": "%s=%04d", "args": {"$tuple": ["x", 7]}},
        {"op": "format", "template": "{name!r}: {0:.2f}", "args": [{"$float": "1.25"}], "kwargs": {"name": "é"}},
    ]
    regexes = [
        {"pattern": pattern, "flags": flags, "method": method, "args": args}
        for pattern, flags, method, args in [
            (r"(?P<word>\w+)\s+(?P=word)", 0, "search", ["é é 😀"]),
            (r"(?<=a)b", 0, "findall", ["ab cab"]),
            (r"\w+", 0, "findall", ["a\u0301 é ² 😀"]),
            (r"\d", 0, "findall", ["3٣²"]),
            (r"\s", 0, "findall", ["x\u001c\u00a0\t"]),
            (r"(a)?(b)", 0, "search", ["é😀b"]),
            (r"a\Z", 0, "search", ["a\n"]),
            (r"(,)", 0, "split", ["a,b,c", 1]),
            (r"(?P<x>\w+)", 0, "sub", [r"[\g<x>]", "é x"]),
            (r"x", 0, "subn", [{"$callable": ["a", "b"]}, "xx"]),
            (r"[", 0, "search", ["x"]),
        ]
    ]
    return {"str": strings, "random": randoms, "format": formats, "re": regexes}


def main():
    if os.environ.get("PYTHONHASHSEED") != "0":
        os.execvpe(sys.executable, [sys.executable, __file__, *sys.argv[1:]], {**os.environ, "PYTHONHASHSEED": "0"})
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "rustbot/target/debug/rustbot")
    parser.add_argument("--kind", choices=["str", "random", "format", "re"])
    args = parser.parse_args()
    sys.dont_write_bytecode = True
    sys.path.insert(0, str(ROOT / "agent/checks"))
    from lib.pyrt_cases import run_case
    failures = total = 0
    for kind, panel in cases().items():
        if args.kind and kind != args.kind:
            continue
        result = subprocess.run([str(args.binary.resolve()), "pyrt-check", kind],
                                input="".join(json.dumps(c, ensure_ascii=False) + "\n" for c in panel),
                                text=True, capture_output=True, timeout=30, check=True)
        actual = [json.loads(line) for line in result.stdout.split("\n") if line]
        if len(actual) != len(panel):
            raise RuntimeError(f"{kind}: expected {len(panel)} output lines, got {len(actual)}")
        bad = 0
        for case, found in zip(panel, actual):
            expected = run_case(kind, case)
            if found != expected:
                bad += 1
                print(json.dumps({"kind": kind, "case": case, "expected": expected, "actual": found}, ensure_ascii=False))
        total += len(panel)
        failures += bad
        print(f"{kind}: {len(panel) - bad}/{len(panel)} matched")
    print(f"runtime smoke: {total - failures}/{total} matched")
    return bool(failures)


if __name__ == "__main__":
    sys.exit(main())
