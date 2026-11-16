#!/usr/bin/env python3
"""G4: RustBot vs CPython PyBot on this machine (cold start, mean/p95 turn, peak RSS)."""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from lib import paths, perf, report  # noqa: E402

LIMITS = {"cold_start_s": 0.25, "turn_mean_ms": 0.5, "turn_p95_ms": 0.5, "peak_rss_mb": 0.5}
TARGETS = {"turn_mean_ms": 0.1}


def main():
    name = "perf"
    binary = paths.rustbot_bin()
    if binary is None:
        return report.missing(name, "RustBot binary not found (rustbot/target/release/rustbot or $RUSTBOT_BIN)")
    out = paths.check_out("perf")
    if "--use-golden" in sys.argv:
        gp = paths.find_golden("perf", "python.json")
        if not gp:
            return report.missing(name, "no perf/python.json golden")
        py = json.load(open(gp))
        print("  CPython numbers: recorded golden (not re-measured on this machine)")
    else:
        sessions, turns = perf.int_env("PERF_SESSIONS", 10), perf.int_env("PERF_TURNS", 8)
        print("  measuring CPython PyBot on this machine ...", flush=True)
        py = perf.measure(perf.python_cmd(), perf.python_env(), sessions, turns)
        json.dump(py, open(os.path.join(out, "python.json"), "w"), indent=1)
    sessions, turns = perf.int_env("PERF_SESSIONS", 10), perf.int_env("PERF_TURNS", 8)
    print("  measuring RustBot ...", flush=True)
    env = dict(os.environ, PYBOT_CLOCK=paths.DEFAULT_CLOCK)
    rs = perf.measure([binary, "-s", "0"], env, sessions, turns)
    json.dump(rs, open(os.path.join(out, "rustbot.json"), "w"), indent=1)
    ok, nums = True, {}
    for k, lim in LIMITS.items():
        if py.get(k) in (None, 0) or rs.get(k) is None:
            print(f"  {k}: not measurable (python={py.get(k)} rust={rs.get(k)})")
            ok = False
            continue
        ratio = rs[k] / py[k]
        good = ratio <= lim
        ok &= good
        nums[k.replace("_ms", "").replace("_s", "").replace("_mb", "")] = f"{ratio:.3f}x({'ok' if good else 'over ' + str(lim)})"
        print(f"  {k}: python={py[k]} rust={rs[k]} ratio={ratio:.3f} limit<={lim}  {'ok' if good else 'FAIL'}")
    tgt = rs["turn_mean_ms"] and py["turn_mean_ms"] / rs["turn_mean_ms"]
    print(f"  target: >=10x faster mean turn: {tgt:.1f}x; <=50 MB RSS: {rs['peak_rss_mb']} MB")
    print(f"CHECK {name} {'PASS' if ok else 'FAIL'} " + " ".join(f"{k}={v}" for k, v in nums.items()))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
