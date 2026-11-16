#!/usr/bin/env python3
"""G4b: `rustbot replay --trace` reproduces pybot_web's trace (files, calls, respond, source, source_line, reply)."""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from lib import compare, goldens, paths, report, rust  # noqa: E402


def main():
    name = "trace"
    os.environ["CHECK_NAME"] = name
    gpath = goldens.e2e_path()
    if gpath is None:
        return report.missing(name, f"no e2e goldens in {paths.goldens_dir()}/e2e (run build_goldens.sh)")
    binary = paths.rustbot_bin()
    if binary is None:
        return report.missing(name, "RustBot binary not found (rustbot/target/release/rustbot or $RUSTBOT_BIN)")
    gold_by, order = {}, []
    for rec in paths.read_jsonl(gpath):
        if goldens.conv_selected(rec["conv"]):
            if rec["conv"] not in gold_by:
                order.append(rec["conv"])
            gold_by.setdefault(rec["conv"], []).append(rec)
    corpus = {c["id"]: c for c in paths.load_corpus()}
    todo = [corpus[c] for c in order if c in corpus]
    rc, out, err, bad = rust.run_jsonl([binary, "replay", "--trace"], todo)
    if not out and rc != 0:
        return report.missing(name, f"`replay --trace` is not available (exit {rc}): {err.strip()[-200:]}")
    if out and not any("trace" in r for r in out):
        return report.missing(name, "`replay --trace` output has no \"trace\" field yet")
    rep = report.Report(name, 10)
    gold = [t for c in order for t in gold_by[c]]
    bad_convs = set()
    for i, g in enumerate(gold):
        where = f"conv={g['conv']} turn={g['turn']} module={g['module']}"
        r = out[i] if i < len(out) else None
        if r is None or "trace" not in r:
            rep.fail(where, "no trace in RustBot output")
            bad_convs.add(g["conv"])
            continue
        d = None
        for f in ("reply", "files", "calls", "respond", "source", "source_line"):
            d = compare.first_diff(g["trace"][f], r["trace"].get(f, "<missing>"), "trace." + f)
            if d:
                break
        if d:
            rep.fail(where, compare.describe(d))
            bad_convs.add(g["conv"])
        else:
            rep.ok()
    return rep.finish("turns", conversations=len(order), bad_conversations=len(bad_convs))


if __name__ == "__main__":
    sys.exit(main())
