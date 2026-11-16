#!/usr/bin/env python3
"""G1: regex / str / format / random cases against `rustbot pyrt-check <kind>`."""
import glob
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from lib import compare, paths, report, rust  # noqa: E402


def main():
    name = "pyrt"
    os.environ["CHECK_NAME"] = name
    files = sorted(glob.glob(os.path.join(paths.goldens_dir(), "pyrt", "*.jsonl*")))
    only = [x for x in sys.argv[1:] if not x.startswith("-")]
    files = [f for f in files if not only or os.path.basename(f).split(".jsonl")[0] in only]
    if not files:
        return report.missing(name, f"no pyrt goldens in {paths.goldens_dir()}/pyrt (run build_goldens.sh)")
    binary = paths.rustbot_bin()
    if binary is None:
        return report.missing(name, "RustBot binary not found (rustbot/target/release/rustbot or $RUSTBOT_BIN)")
    rep = report.Report(name, 12)
    per_kind = {}
    for f in files:
        kind = os.path.basename(f).split(".jsonl")[0]
        recs = list(paths.read_jsonl(f))
        rc, out, err, bad = rust.run_jsonl([binary, "pyrt-check", kind], [r["case"] for r in recs])
        fails = 0
        for i, g in enumerate(recs):
            where = f"pyrt/{kind} #{g['id']}"
            case = g["case"]
            brief = compare.clip({k: v for k, v in case.items()}, 110)
            if i >= len(out):
                rep.fail(where, f"no output ({(err or bad or '').strip()[-160:]}) case={brief}")
                fails += 1
                continue
            d = None
            for fld in ("exc", "result"):
                if g["expect"]["exc"] and fld == "result":
                    continue
                d = compare.first_diff(g["expect"][fld], out[i].get(fld, "<missing>"), fld)
                if d:
                    break
            if d:
                rep.fail(where, f"case={brief} :: " + compare.describe(d))
                fails += 1
            else:
                rep.ok()
        per_kind[kind] = (len(recs), fails)
    for k, (n, f) in per_kind.items():
        print(f"  {k}: {n - f}/{n}")
    return rep.finish("cases")


if __name__ == "__main__":
    sys.exit(main())
