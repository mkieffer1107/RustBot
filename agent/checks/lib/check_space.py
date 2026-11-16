#!/usr/bin/env python3
"""G6: serve wasmbot/web, drive headless Chromium through the worker protocol, compare replies to native RustBot."""
import json
import os
import socket
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from lib import paths, report, rust  # noqa: E402

TURNS = ["hello", "my name is Anna", "what is my name?", "how long does it take to bake a cake", "thanks"]


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


def collapse(s):
    return " ".join(str(s).split()) or "Hello!"


def main():
    name = "space"
    os.environ["CHECK_NAME"] = name
    web = os.path.join(paths.wasmbot_dir(), "web")
    server = os.path.join(web, "server.py")
    if not os.path.exists(server):
        return report.missing(name, f"{server} not found")
    binary = paths.rustbot_bin()
    if binary is None:
        return report.missing(name, "native RustBot binary not found (needed to compare replies)")
    if subprocess.run(["which", "node"], capture_output=True).returncode != 0:
        return report.missing(name, "node not found")
    port = free_port()
    srv = subprocess.Popen([sys.executable, server], cwd=web, env=dict(os.environ, PORT=str(port), HOST="127.0.0.1"),
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        for _ in range(50):
            try:
                socket.create_connection(("127.0.0.1", port), 0.2).close()
                break
            except OSError:
                time.sleep(0.1)
        drive = os.path.join(paths.CHECKS, "lib", "space_drive.mjs")
        p = subprocess.run(["node", drive, f"http://127.0.0.1:{port}/", json.dumps(TURNS)], capture_output=True, text=True,
                           timeout=900, cwd=paths.check_out("space"))
    finally:
        srv.terminate()
    if p.returncode == 3:
        return report.missing(name, "headless Chromium or playwright-core/puppeteer-core not available: " + p.stderr.strip())
    if p.returncode != 0 or not p.stdout.strip():
        return report.fail_line(name, f"browser run failed: {(p.stderr or p.stdout).strip()[-300:]}")
    res = json.loads(p.stdout.strip().splitlines()[-1])
    if "error" in res:
        return report.fail_line(name, f"worker error: {res['error']}")
    conv = {"id": "space", "seed": 0, "clock": paths.DEFAULT_CLOCK, "turns": TURNS}
    rc, out, err, bad = rust.run_jsonl([binary, "replay"], [conv])
    rep = report.Report(name, 10)
    for i, r in enumerate(res["replies"]):
        want = out[i] if i < len(out) else None
        where = f"turn={i} text={TURNS[i]!r}"
        if want is None:
            rep.fail(where, "native RustBot gave no reference reply")
        elif r.get("reply") != collapse(want["reply"]) or r.get("module") != want["module"] or \
                list(r.get("candidates", [])) != [c[0] for c in want["candidates"]]:
            rep.fail(where, f"browser={json.dumps({k: r.get(k) for k in ('reply', 'module')})[:160]} native={json.dumps([want['reply'], want['module']])[:160]}")
        else:
            rep.ok()
    ready_s = res["ready_s"]
    turn_ms = sorted(r["wall_ms"] for r in res["replies"])
    note = {"ready_s": round(ready_s, 2), "turn_ms_p50": round(turn_ms[len(turn_ms) // 2], 1)}
    base = paths.find_golden("perf", "pyodide.json")
    if base:
        b = json.load(open(base))
        for k, mine, theirs in (("ready", ready_s, b["ready_s"]), ("turn", note["turn_ms_p50"], b["turn_ms"])):
            ok = mine <= 0.25 * theirs
            note[f"{k}_vs_pyodide"] = f"{mine / theirs:.3f}x"
            if not ok:
                rep.fail(f"timing {k}", f"{mine} > 0.25 x {theirs}")
    else:
        note["pyodide_baseline"] = "none(timing ratios not enforced)"
    return rep.finish("turns", **note)


if __name__ == "__main__":
    sys.exit(main())
