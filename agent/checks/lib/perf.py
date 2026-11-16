#!/usr/bin/env python3
"""Performance measurement of a pybot.py-style CLI (PyBot or RustBot), the same way for both.

A session = one process fed one conversation line by line over a pipe. Cold start = process spawn to the first reply
line; per-turn time = time from sending a line to receiving its reply (turns 2.. of each session); peak RSS comes from
/usr/bin/time (-l on macOS, -v on Linux) around the process.
    perf.py measure --out FILE [--python | --cmd BIN [ARGS...]] [--sessions N] [--turns T]
"""
import argparse
import json
import os
import platform
import re
import statistics
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from lib import paths  # noqa: E402


def time_wrapper():
    t = "/usr/bin/time"
    if not os.path.exists(t):
        return None
    return [t, "-l"] if platform.system() == "Darwin" else [t, "-v"]


def parse_rss_mb(stderr):
    m = re.search(r"(\d+)\s+maximum resident set size", stderr)          # macOS, bytes
    if m:
        return int(m.group(1)) / 1e6
    m = re.search(r"Maximum resident set size \(kbytes\):\s*(\d+)", stderr)  # Linux, KiB
    if m:
        return int(m.group(1)) / 1024.0
    return None


def session(cmd, lines, env, wrap):
    full = (wrap or []) + cmd
    t0 = time.perf_counter()
    p = subprocess.Popen(full, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
    cold, turns = None, []
    for i, line in enumerate(lines):
        t1 = time.perf_counter()
        p.stdin.write(line.encode("utf-8") + b"\n")
        p.stdin.flush()
        reply = p.stdout.readline()
        t2 = time.perf_counter()
        if not reply:
            break
        if i == 0:
            cold = t2 - t0
        else:
            turns.append(t2 - t1)
    p.stdin.close()
    p.stdout.read()
    err = p.stderr.read().decode("utf-8", "replace")
    p.wait()
    return cold, turns, parse_rss_mb(err) if wrap else None


def pick_conversations(n, turns):
    allc = paths.load_corpus()
    convs = [c for c in allc if len(c["turns"]) >= min(turns, 6)]
    if len(convs) < n:  # small corpus: pad with the longest remaining conversations
        convs += sorted((c for c in allc if c not in convs), key=lambda c: -len(c["turns"]))[:n - len(convs)]
    return [c["turns"][:turns] for c in convs[:n]]


def int_env(name, default):
    return int(os.environ.get(name, default))


def pct(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(round(p / 100.0 * (len(xs) - 1))))]


def measure(cmd, env, sessions, turns):
    wrap = time_wrapper()
    convs = pick_conversations(sessions, turns)
    session(cmd, ["hello"], env, None)  # warm-up: pyc caches, page cache
    colds, all_turns, rss = [], [], []
    for lines in convs:
        c, ts, r = session(cmd, lines, env, wrap)
        if c is not None:
            colds.append(c)
        all_turns += ts
        if r:
            rss.append(r)
    if not all_turns:
        raise SystemExit("no turns measured: does the command answer lines on stdin?")
    return {"cold_start_s": round(statistics.median(colds), 4), "turn_mean_ms": round(1000 * statistics.mean(all_turns), 3),
            "turn_p50_ms": round(1000 * pct(all_turns, 50), 3), "turn_p95_ms": round(1000 * pct(all_turns, 95), 3),
            "peak_rss_mb": round(max(rss), 1) if rss else None, "sessions": len(convs), "turns_measured": len(all_turns),
            "machine": platform.platform(), "python": platform.python_version(), "cpus": os.cpu_count()}


def python_cmd():
    return [sys.executable, "-s", os.path.join(paths.PYBOT, "pybot.py"), "-s", "0"]


def python_env():
    env = dict(os.environ, PYTHONHASHSEED="0", PYTHONSAFEPATH="1", PYTHONIOENCODING="utf-8")
    env.setdefault("PYTHONPYCACHEPREFIX", os.path.join(os.environ.get("TMPDIR", "/tmp").rstrip("/"), "pybot-pyc"))
    return env


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out")
    ap.add_argument("--python", action="store_true")
    ap.add_argument("--cmd", nargs=argparse.REMAINDER)
    ap.add_argument("--sessions", type=int, default=int(os.environ.get("PERF_SESSIONS", "10")))
    ap.add_argument("--turns", type=int, default=int(os.environ.get("PERF_TURNS", "8")))
    a = ap.parse_args()
    if a.python:
        res = measure(python_cmd(), python_env(), a.sessions, a.turns)
    else:
        res = measure(a.cmd, dict(os.environ), a.sessions, a.turns)
    text = json.dumps(res, indent=1)
    if a.out:
        os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
        open(a.out, "w").write(text + "\n")
    print(text)


if __name__ == "__main__":
    main()
