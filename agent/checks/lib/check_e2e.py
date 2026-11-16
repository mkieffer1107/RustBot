#!/usr/bin/env python3
"""G3 / G5: replay the corpus through RustBot (native or wasm) and compare every turn with the goldens."""
import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from lib import compare, goldens, paths, report, rust  # noqa: E402

FIELDS = ("conv", "turn", "reply", "module", "candidates", "memory")


def compare_turns(rep, gold, got, label_unit="turns"):
    """gold/got: lists of per-turn records. Returns the set of conversations with a divergence."""
    bad_convs = set()
    for i, g in enumerate(gold):
        where = f"conv={g['conv']} turn={g['turn']} text={compare.clip(g['text'], 60)!r}"
        if i >= len(got):
            rep.fail(where, "no output from RustBot for this turn")
            bad_convs.add(g["conv"])
            continue
        r = got[i]
        diff = None
        for f in FIELDS:
            if f not in r:
                diff = (f, g.get(f), "<missing field>")
                break
            d = compare.first_diff(g[f], r[f], f)
            if d:
                diff = d
                break
        if diff:
            rep.fail(where, compare.describe(diff))
            bad_convs.add(g["conv"])
        else:
            rep.ok()
    return bad_convs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runner", choices=("native", "wasm"), default="native")
    ap.add_argument("--name", default=None)
    ap.add_argument("--show", type=int, default=10)
    a = ap.parse_args()
    name = a.name or ("e2e" if a.runner == "native" else "wasm")
    os.environ["CHECK_NAME"] = name
    gpath = goldens.e2e_path()
    if gpath is None:
        return report.missing(name, f"no e2e goldens in {paths.goldens_dir()}/e2e (run agent/checks/build_goldens.sh)")
    if a.runner == "native":
        binary = paths.rustbot_bin()
        if binary is None:
            return report.missing(name, "RustBot binary not found (rustbot/target/release/rustbot or $RUSTBOT_BIN)")
        argv = [binary, "replay"]
    else:
        script = os.path.join(paths.wasmbot_dir(), "node", "replay.mjs")
        if not os.path.exists(script):
            return report.missing(name, f"{script} not found")
        argv = ["node", script]
    gold_by_conv, order = {}, []
    for rec in paths.read_jsonl(gpath):
        if goldens.conv_selected(rec["conv"]):
            if rec["conv"] not in gold_by_conv:
                order.append(rec["conv"])
            gold_by_conv.setdefault(rec["conv"], []).append(rec)
    convs = {c["id"]: c for c in paths.load_corpus()}
    todo = [convs[c] for c in order if c in convs]
    if len(todo) != len(order):
        print(f"  warning: {len(order) - len(todo)} golden conversations are not in the corpus file", file=sys.stderr)
    rep = report.Report(name, a.show)
    rc, out, err, bad = rust.run_jsonl(argv, todo)
    outdir = paths.check_out(name)
    with open(os.path.join(outdir, "turns.jsonl"), "w", encoding="utf-8") as f:
        for r in out:
            f.write(paths.dump_line(r) + "\n")
    if bad:
        print(f"  output is not JSON: {bad!r}", flush=True)
    if rc != 0:
        print(f"  command exited with {rc}: {err.strip()[-600:]}", flush=True)
    gold = [t for c in order for t in gold_by_conv[c]]
    bad_convs = compare_turns(rep, gold, out)
    if len(out) > len(gold):
        rep.fail("output", f"{len(out) - len(gold)} extra turn records")
    return rep.finish("turns", conversations=len(order), bad_conversations=len(bad_convs))


if __name__ == "__main__":
    sys.exit(main())
