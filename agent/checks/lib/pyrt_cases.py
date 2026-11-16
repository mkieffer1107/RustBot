"""pyrt cases: executors (the Python reference semantics) and the synthetic generators.

A case is inputs only; the golden stores {"case": ..., "expect": {"result", "exc"}} computed by run_case().
`rustbot pyrt-check <kind>` receives the case objects and prints {"result", "exc"} per case.
"""
import ast
import os
import random as _random
import re
import sys

from . import canon, paths

STR_METHODS_0 = ["lower", "upper", "title", "capitalize", "casefold", "swapcase", "strip", "lstrip", "rstrip",
                 "isalpha", "isdigit", "isdecimal", "isnumeric", "isalnum", "isspace", "isupper", "islower",
                 "istitle", "isprintable", "isidentifier", "isascii", "splitlines", "split", "rsplit"]
STR_METHODS_1 = ["strip", "lstrip", "rstrip", "split", "rsplit", "partition", "rpartition", "find", "rfind", "index",
                 "rindex", "count", "startswith", "endswith", "removeprefix", "removesuffix", "zfill", "center",
                 "ljust", "rjust", "expandtabs", "splitlines", "join", "__contains__", "__getitem__", "__mul__"]
STR_METHODS_2 = ["replace", "center", "ljust", "rjust", "split", "rsplit", "find", "rfind", "count", "startswith",
                 "endswith"]
STR_ALLOWED = set(STR_METHODS_0 + STR_METHODS_1 + STR_METHODS_2 + ["format_map", "encode", "translate"])


def exc_name(e):
    return type(e).__name__


# ---------------------------------------------------------------- results

def enc_result(x):
    if isinstance(x, re.Match):
        return {"$match": {"span": list(x.span()), "groups": [canon.encode(g) for g in x.groups()],
                           "spans": [list(x.span(i)) if x.span(i) != (-1, -1) else None
                                     for i in range(1, (x.re.groups or 0) + 1)],
                           "groupdict": {k: canon.encode(v) for k, v in x.groupdict().items()},
                           "lastindex": x.lastindex, "lastgroup": x.lastgroup}}
    if isinstance(x, list):
        return [enc_result(v) for v in x] if x and isinstance(x[0], re.Match) else canon.encode(x)
    return canon.encode(x)


# ---------------------------------------------------------------- executors

def _callable_from(spec):
    """{"$callable": [ret, ...]} -> a function returning the recorded values in order."""
    rets = iter(canon.decode(spec["$callable"]))
    return lambda m: next(rets)


def run_re(case):
    flags = case.get("flags", 0)
    pat = re.compile(case["pattern"], flags)
    args = []
    for a in case["args"]:
        args.append(_callable_from(a) if isinstance(a, dict) and "$callable" in a else canon.decode(a))
    m = case["method"]
    r = getattr(pat, m)(*args)
    if m == "finditer":
        r = list(r)
    return enc_result(r)


def run_str(case):
    s = case["self"]
    r = getattr(s, case["method"])(*canon.decode(case["args"]))
    return canon.encode(r)


def run_format(case):
    op = case["op"]
    if op == "percent":
        return canon.encode(case["fmt"] % canon.decode(case["args"]))
    if op == "format":
        return canon.encode(case["template"].format(*canon.decode(case["args"]), **canon.decode(case.get("kwargs", {}))))
    if op == "fstring":
        v = canon.decode(case["value"])
        conv = case.get("conv", "")
        if conv:
            v = {"r": repr, "s": str, "a": ascii}[conv](v)
        return canon.encode(format(v, case.get("spec", "")))
    if op == "repr":
        return canon.encode(repr(canon.decode(case["value"])))
    if op == "str":
        return canon.encode(str(canon.decode(case["value"])))
    raise ValueError("unknown format op " + op)


def run_random(case):
    r = _random.Random(canon.decode(case["seed"]))
    ops = case["ops"] if "ops" in case else [[case["op"], case["args"]]]
    out = []
    for name, args in ops:
        args = canon.decode(args)
        v = getattr(r, name)(*args)
        out.append(canon.encode(args[0] if name == "shuffle" else v))
    return out if "ops" in case else out[0]


RUNNERS = {"re": run_re, "str": run_str, "format": run_format, "random": run_random}


def run_case(kind, case):
    try:
        return {"result": RUNNERS[kind](case), "exc": None}
    except Exception as e:  # noqa: BLE001
        return {"result": None, "exc": exc_name(e)}


# ---------------------------------------------------------------- synthetic generators

TRICKY = ["", " ", "hello world", "Hello, World!", "  padded\t\n", "ALL CAPS", "mixed Case Words", "isn't it", "don’t",
          "straße", "ǅungla", "İstanbul", "ΑΣ", "ὈΔΥΣΣΕΎΣ", "ﬁne ﬂow", "naïve café", "éclair", "日本語のテキスト", "😀 smile",
          "👨‍👩‍👧", "a b", "x y z", "line1\nline2\r\nline3\rline4", "٣٤٥", "²³", "½", "Ⅷ", "x²",
          "snake_case_name", "3.14", "-42", "a,b;c d", "tab\tsep\tvalues", "\x00\x1f", "ß", "ǈ", "ŉ", "ΐ", "Ǆ", "ῼ",
          "i̇", "AbC dEf", "The Quick-Brown fox", "o'neil mc-donald", "123abc", "abc123", "  ", "​", "﻿bom"]


def synth_str():
    cases = []
    for s in TRICKY:
        for m in STR_METHODS_0:
            cases.append({"self": s, "method": m, "args": []})
        for m in ("splitlines", "expandtabs"):
            cases.append({"self": s, "method": m, "args": [True if m == "splitlines" else 4]})
        subs = sorted({s[:1], s[-1:], s[1:3], " ", "a", "e", "ß", "l"})
        for sub in subs:
            for m in ("find", "rfind", "index", "count", "startswith", "endswith", "partition", "rpartition", "split",
                      "rsplit", "strip", "lstrip", "rstrip", "removeprefix", "removesuffix"):
                if sub == "" and m in ("split", "rsplit", "partition", "rpartition"):
                    continue
                cases.append({"self": s, "method": m, "args": [sub]})
            cases.append({"self": s, "method": "replace", "args": [sub, "XY"]})
            cases.append({"self": s, "method": "split", "args": [sub, 1]})
            cases.append({"self": s, "method": "rsplit", "args": [sub, 1]})
        for w in (0, 3, 12):
            for m in ("center", "ljust", "rjust"):
                cases.append({"self": s, "method": m, "args": [w, "*"]})
            cases.append({"self": s, "method": "zfill", "args": [w]})
        cases.append({"self": ",", "method": "join", "args": [[s, s[:2]]]})
        for i in (0, -1, 2):
            cases.append({"self": s, "method": "__getitem__", "args": [i]})
        cases.append({"self": s, "method": "__getitem__", "args": [{"$dict": []}]})
        cases.append({"self": s, "method": "find", "args": [s[1:3], 1, 4]})
    return cases


FLOATS = [0.0, -0.0, 0.5, 1.0, 0.1, 0.2 + 0.1, 2.675, 1e15, 1e16, 1e-4, 1e-5, 123456789.123456789, float("inf"),
          float("-inf"), float("nan"), 1 / 3, 2 / 3, 1e22, 1.5e300, 5e-324, 99.995, 0.045, 100.0, 12345.6789, -7.25]
INTS = [0, 1, -1, 7, 42, 255, 1000, 123456789, 10 ** 20, -(10 ** 19), True, False]
STRS = ["", "a", "hello", "naïve", "日本", "x y"]


def _spec_corpus():
    """Format specs, %-templates and str.format templates found by static scan of PyBot (deterministic order)."""
    specs, percents, templates = set(), set(), set()
    for dirpath, dirs, files in os.walk(os.path.join(paths.PYBOT, "bot")):
        dirs.sort()
        for fn in sorted(files):
            if not fn.endswith(".py"):
                continue
            try:
                tree = ast.parse(open(os.path.join(dirpath, fn), encoding="utf-8").read())
            except SyntaxError:
                continue
            for node in ast.walk(tree):
                if isinstance(node, ast.FormattedValue):
                    spec = ""
                    if node.format_spec is not None and all(isinstance(v, ast.Constant) for v in node.format_spec.values):
                        spec = "".join(v.value for v in node.format_spec.values)
                    conv = {-1: "", 115: "s", 114: "r", 97: "a"}[node.conversion]
                    if node.format_spec is None or spec:
                        specs.add((spec, conv))
                elif (isinstance(node, ast.BinOp) and isinstance(node.op, ast.Mod) and isinstance(node.left, ast.Constant)
                      and isinstance(node.left.value, str) and "%" in node.left.value):
                    percents.add(node.left.value)
                elif (isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute) and node.func.attr == "format"
                      and isinstance(node.func.value, ast.Constant) and isinstance(node.func.value.value, str)):
                    templates.add(node.func.value.value)
    return sorted(specs), sorted(percents), sorted(templates)


_DIRECTIVE = re.compile(r"%(?:\((\w+)\))?[-#0 +]*(\*|\d+)?(?:\.(\*|\d+))?[hlL]?([diouxXeEfFgGcrsa%])")


def synth_format():
    cases = []
    specs, percents, templates = _spec_corpus()
    panel_by = {"d": INTS, "f": FLOATS + INTS[:4], "s": STRS + [None, 3, 2.5], "x": INTS[:7]}
    for spec, conv in specs:
        kind = "s"
        if spec and spec[-1] in "dn":
            kind = "d"
        elif spec and spec[-1] in "feEgG%F":
            kind = "f"
        elif spec and spec[-1] in "xXob":
            kind = "x"
        vals = panel_by[kind] if spec else INTS[:5] + FLOATS[:6] + STRS[:3] + [None]
        for v in vals[:30]:
            cases.append({"op": "fstring", "value": canon.encode(v), "spec": spec, "conv": conv})
    for fmt in percents:
        dirs = [m for m in _DIRECTIVE.finditer(fmt) if m.group(4) != "%"]
        if len(dirs) > 4 or any(m.group(1) or "*" in (m.group(2) or "") + (m.group(3) or "") for m in dirs):
            continue
        for variant in range(4):
            args = []
            for m in dirs:
                c = m.group(4)
                pool = (INTS if c in "diouxXc" else FLOATS if c in "eEfFgG" else STRS + [None, 3, 2.5])
                if c == "c":
                    pool = [65, "x"]
                args.append(pool[(variant * 3 + len(args)) % len(pool)])
            cases.append({"op": "percent", "fmt": fmt, "args": canon.encode(tuple(args) if len(args) != 1 else args[0])
                          if dirs else canon.encode(())})
    for t in templates:
        n = len(re.findall(r"\{[^{}]*\}", t))
        if n and not re.search(r"\{[a-zA-Z_]", t) and n <= 4:
            for variant in range(3):
                args = [(STRS + INTS[:4] + FLOATS[:4])[(variant * 5 + i * 2) % 14] for i in range(n)]
                cases.append({"op": "format", "template": t, "args": canon.encode(args), "kwargs": {}})
    for f in FLOATS + [float(i) / 7 for i in range(1, 40)] + [10.0 ** k for k in range(-8, 24, 3)]:
        cases.append({"op": "repr", "value": canon.encode(f)})
        cases.append({"op": "str", "value": canon.encode(f)})
    for v in INTS + STRS + [None, (1, "a"), [1, 2.5], {"a": (1,)}, {1, 2}, "it's \"q\"", "tab\t\n\x00é\U0001f600"]:
        cases.append({"op": "repr", "value": canon.encode(v)})
    return cases


def synth_random():
    cases = []
    seeds = [0, 1, 42, 12345, -7, 2 ** 40, "0:0", "0:1", "1729:3", "abc", "naïve", 3.5]
    pool = list("abcdefghijklmnopqrstuvwxyz")
    for s in seeds:
        cases.append({"seed": canon.encode(s), "ops": [["random", []], ["random", []], ["getrandbits", [1]],
                      ["getrandbits", [32]], ["getrandbits", [33]], ["getrandbits", [64]], ["randrange", [10]],
                      ["randrange", [5, 50, 3]], ["randint", [-4, 4]], ["choice", [pool]], ["shuffle", [list(pool)]],
                      ["sample", [pool, 5]], ["random", []], ["randrange", [2 ** 70]], ["choice", [[1, 2, 3]]]]})
        cases.append({"seed": canon.encode(s), "ops": [["choices", [pool, [1] * 26], ]]}) if False else None
    for n in range(0, 80):
        for k in (2, 3, 5, 17, 100):
            cases.append({"seed": n, "op": "randrange", "args": [k]})
        cases.append({"seed": n, "op": "choice", "args": [[1, 2, 3, 4, 5, 6, 7]]})
        cases.append({"seed": n, "op": "sample", "args": [list(range(20)), 3]})
        cases.append({"seed": n, "op": "getrandbits", "args": [1]})
    return [c for c in cases if c]


def str_cases_for(meth, s):
    """Cases for a str method seen on string s in PyBot. Arguments are not visible to the monitor, so they are derived
    from s itself (every case is still checked against CPython's real answer)."""
    cases = [{"self": s, "method": meth, "args": []}]
    subs = sorted({s[:1], s[-1:], s[:3], s[1:4], " ", "e"} - {""})
    if meth in ("strip", "lstrip", "rstrip"):
        cases += [{"self": s, "method": meth, "args": [" .,!?'\"-"]}]
    elif meth in ("split", "rsplit"):
        cases += [{"self": s, "method": meth, "args": [x]} for x in (" ", ",") ]
        cases += [{"self": s, "method": meth, "args": [None, 1]}]
    elif meth in ("partition", "rpartition", "find", "rfind", "index", "rindex", "count", "startswith", "endswith",
                  "removeprefix", "removesuffix"):
        cases += [{"self": s, "method": meth, "args": [x]} for x in subs]
    elif meth == "replace":
        cases += [{"self": s, "method": meth, "args": [x, "_"]} for x in subs[:3]]
    elif meth in ("center", "ljust", "rjust"):
        cases += [{"self": s, "method": meth, "args": [len(s) + 4]}]
    elif meth == "zfill":
        cases += [{"self": s, "method": meth, "args": [len(s) + 2]}]
    elif meth == "splitlines":
        cases += [{"self": s, "method": meth, "args": [True]}]
    elif meth == "join":
        cases = [{"self": s, "method": meth, "args": [[s[:2], s[2:5]]]}]
    return cases
