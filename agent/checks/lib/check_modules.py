#!/usr/bin/env python3
"""G2: every recorded observe/respond/after/choose call (and, with --functions, leaf functions) against RustBot."""
import argparse
import json
import os
import sys
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from lib import compare, goldens, paths, report, rust, sampling  # noqa: E402

INPUT_FIELDS = ("conv", "turn", "seed", "clock", "args", "memory_in", "rng_in")


def check_hook(binary, mod, hook, path):
    recs = [r for r in paths.read_jsonl(path) if goldens.conv_selected(r["conv"])]
    rc, out, err, bad = rust.run_jsonl([binary, "hook", mod, hook], [{k: r[k] for k in INPUT_FIELDS if k in r} for r in recs])
    fails = []
    for i, g in enumerate(recs):
        where = f"{mod}.{hook} conv={g['conv']} turn={g['turn']}"
        if i >= len(out):
            fails.append((where, f"no output ({'exit %d: ' % rc if rc else ''}{(err or bad or '').strip()[-200:]})", g))
            continue
        r = out[i]
        diff = None
        for f in ("exc", "ret", "memory_out", "rng_out"):
            if f == "rng_out" and g["rng_out"].get("opaque"):
                continue
            if f == "ret" and g["exc"]:
                continue
            if f not in r:
                diff = (f, "<recorded>", "<missing field>")
                break
            d = compare.first_diff(g[f], r[f], f)
            if d:
                diff = d
                break
        if diff:
            fails.append((where, compare.describe(diff), g))
    return len(recs), fails


def check_function(binary, name, path):
    recs = list(paths.read_jsonl(path))
    rc, out, err, bad = rust.run_jsonl([binary, "call", name], [{k: r[k] for k in ("args", "seed", "clock")} for r in recs])
    fails = []
    for i, g in enumerate(recs):
        where = f"{name}#{i}"
        if i >= len(out):
            fails.append((where, f"no output ({(err or bad or '').strip()[-200:]})", g))
            continue
        r = out[i]
        for f in ("exc", "ret"):
            if f == "ret" and g["exc"]:
                continue
            d = compare.first_diff(g[f], r.get(f, "<missing>"), f)
            if d:
                fails.append((where, f"args={compare.clip(g['args'], 80)} " + compare.describe(d), g))
                break
    return len(recs), fails


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--functions", action="store_true", help="per-function parity of leaf helpers only")
    ap.add_argument("--module")
    ap.add_argument("--hook")
    ap.add_argument("--function")
    ap.add_argument("--first-failure", action="store_true")
    ap.add_argument("--show", type=int, default=10)
    a = ap.parse_args()
    name = "functions" if a.functions else "modules"
    os.environ["CHECK_NAME"] = name
    binary = paths.rustbot_bin()
    if binary is None:
        return report.missing(name, "RustBot binary not found (rustbot/target/release/rustbot or $RUSTBOT_BIN)")
    if a.functions:
        pct = int(os.environ.get("CHECK_FUNCTION_PERCENT", "0") or 0) or goldens.quick_percent()
        jobs = [("fn", fn, None, p) for fn, p in goldens.function_files(a.function)
                if a.function or sampling.in_sample(fn, pct)]
    else:
        jobs = [("hook", m, h, p) for m, h, p in goldens.hook_files(a.module, a.hook)]
    if not jobs:
        return report.missing(name, f"no {'function' if a.functions else 'hook'} goldens matching in {paths.goldens_dir()}")
    rep = report.Report(name, 1 if a.first_failure else a.show)
    workers = 1 if a.first_failure else min(8, os.cpu_count() or 2)

    def run(job):
        kind, x, h, p = job
        return (x if kind == "fn" else f"{x}.{h}"), (check_function(binary, x, p) if kind == "fn" else check_hook(binary, x, h, p))

    bad_units, total_calls = [], 0
    with ThreadPoolExecutor(workers) as ex:
        for label, (n, fails) in ex.map(run, jobs):
            total_calls += n
            rep.ok(n - len(fails))
            if fails:
                bad_units.append(label)
            for where, detail, g in fails:
                rep.fail(where, detail)
                if a.first_failure:
                    print("  args:", compare.clip(g.get("args"), 300))
                    if "memory_in" in g:
                        print("  memory_in:", compare.clip(g["memory_in"], 400))
                    return rep.finish("calls", hooks=len(jobs), failing_hooks=len(bad_units))
    if bad_units:
        print("  failing: " + ", ".join(bad_units[:25]) + (" ..." if len(bad_units) > 25 else ""))
    return rep.finish("calls", units=len(jobs), failing_units=len(bad_units))


if __name__ == "__main__":
    sys.exit(main())
