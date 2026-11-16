#!/usr/bin/env python3
"""A stand-in for the RustBot binary that runs PyBot itself (through lib/oracle.py). It lets the selftest prove that the
checks pass on a perfect implementation and fail on injected faults (FAKE_FAULT=reply|module|candidate_order|memory|
exception|rng). Modes: replay, hook, call, pyrt-check, and the pybot.py-style stdin / -p mode."""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from lib import oracle  # noqa: E402

oracle.ensure_env()
from lib import canon, forkrun, paths, pyrt_cases  # noqa: E402

FAULT = os.environ.get("FAKE_FAULT", "")
FAULT_AT = int(os.environ.get("FAKE_FAULT_AT", "3"))          # conversation index (replay mode)
FAULT_MODULE = os.environ.get("FAKE_FAULT_MODULE", "chat")      # hook mode: module.hook whose first call is corrupted
FAULT_HOOK = os.environ.get("FAKE_FAULT_HOOK", "respond")
FAULT_LOG = os.environ.get("FAKE_FAULT_LOG")


def log_fault(**kw):
    if FAULT_LOG:
        with open(FAULT_LOG, "a") as f:
            f.write(json.dumps(dict(kw, fault=FAULT)) + "\n")


def read_lines():
    return [json.loads(l) for l in sys.stdin if l.strip()]


def emit(obj):
    sys.stdout.write(paths.dump_line(obj) + "\n")


# ------------------------------------------------------------------ replay

def run_conv(conv):
    memory, turns = {}, []
    for i, text in enumerate(conv["turns"]):
        res = oracle.run_turn(text, memory)
        turns.append(oracle.turn_record(conv, i, text, res, memory))
    return turns


def cmd_replay(argv):
    seed = int(os.environ.get("PYBOT_SEED", "0"))
    clock = os.environ.get("PYBOT_CLOCK", paths.DEFAULT_CLOCK)
    convs = read_lines()
    for c in convs:
        c.setdefault("seed", seed)
        c.setdefault("clock", clock)
    trace = "--trace" in argv
    results = forkrun.map_items(convs, run_conv, after_load=oracle.install_trace if trace else None)
    injected = False
    for ci, turns in enumerate(results):
        for t in turns:
            if FAULT and not injected and ci >= FAULT_AT:
                injected = inject_turn(t)
            emit(t)


def inject_turn(t):
    k = FAULT
    if k == "reply":
        t["reply"] += " X"
    elif k == "module":
        t["module"] = "zz_" + (t["module"] or "none")
    elif k == "candidate_order":
        if len(t["candidates"]) < 2:
            return False
        t["candidates"][0], t["candidates"][1] = t["candidates"][1], t["candidates"][0]
    elif k == "memory":
        t["memory"]["_fault"] = 1
    elif k == "trace":
        if "trace" not in t or not t["trace"]["files"]:
            return False
        t["trace"]["files"] = t["trace"]["files"][:-1]
    elif k == "exception":
        if len(t["candidates"]) < 2:
            return False
        t["candidates"].pop(0)           # as if the first proposing module had raised
    else:
        return False
    log_fault(conv=t["conv"], turn=t["turn"])
    return True


# ------------------------------------------------------------------ hook

def run_hook(rec, mod, hook):
    memory = canon.decode(rec["memory_in"])
    args = canon.decode(rec["args"])
    oracle.restore_rng(rec.get("rng_in", {"global": 0, "instances": []}))
    fn = oracle.Loaded.hooks[(mod, hook)]
    ret, exc = None, None
    try:
        ret = fn(args[0], memory, *args[1:])
    except Exception as e:  # noqa: BLE001
        exc = type(e).__name__
    return {"ret": None if exc else canon.encode(ret), "exc": exc, "memory_out": canon.encode(memory),
            "rng_out": oracle.rng_state()}


def cmd_hook(argv):
    mod, hook = argv[0], argv[1]
    recs = read_lines()
    outs = forkrun.map_items(recs, lambda r: run_hook(r, mod, hook))
    if FAULT and recs and mod == FAULT_MODULE and hook == FAULT_HOOK:
        o, r = outs[0], recs[0]
        if FAULT == "exception":
            o["exc"], o["ret"] = "ValueError", None
        elif FAULT == "rng":
            o["rng_out"]["global"] += 1
        elif FAULT == "memory":
            o["memory_out"]["_fault"] = 1
        if FAULT in ("exception", "rng", "memory"):
            log_fault(conv=r["conv"], turn=r["turn"], hook=f"{mod}.{hook}")
    for o in outs:
        emit(o)


# ------------------------------------------------------------------ call

def run_call(item, name):
    import importlib
    modname, _, attr = name.rpartition(".")
    fn = getattr(importlib.import_module(modname), attr)
    args = canon.decode(item["args"])
    try:
        return {"ret": canon.encode(fn(*args)), "exc": None}
    except Exception as e:  # noqa: BLE001
        return {"ret": None, "exc": type(e).__name__}


def cmd_call(argv):
    name = argv[0]
    items = read_lines()
    for n, o in enumerate(forkrun.map_items(items, lambda it: run_call(it, name))):
        if FAULT == "function" and n == 0 and name == os.environ.get("FAKE_FAULT_FN"):
            o = {"ret": "WRONG", "exc": None}
            log_fault(fn=name, index=0)
        emit(o)


def cmd_pyrt(argv):
    kind = argv[0]
    for n, case in enumerate(read_lines()):
        out = pyrt_cases.run_case(kind, case)
        if FAULT == "pyrt" and n == 0:
            out = {"result": "WRONG", "exc": None}
            log_fault(kind=kind, id=0)
        emit(out)


# ------------------------------------------------------------------ pybot.py-style CLI

def cmd_cli(argv):
    seed, prompt, i = 0, None, 0
    while i < len(argv):
        if argv[i] in ("-s", "--seed"):
            seed = int(argv[i + 1]); i += 1
        elif argv[i] in ("-p", "--prompt"):
            prompt = argv[i + 1]; i += 1
        i += 1
    oracle.load(seed, os.environ.get("PYBOT_CLOCK", paths.DEFAULT_CLOCK))
    memory = {}
    lines = [prompt] if prompt is not None else (l.rstrip("\r\n") for l in sys.stdin)
    for line in lines:
        try:
            reply = " ".join(str(oracle.run_turn(line, memory)["reply"]).split()) or "Hello!"
        except Exception:  # noqa: BLE001
            reply = "Hello!"
        print(reply, flush=True)


def main():
    argv = sys.argv[1:]
    modes = {"replay": cmd_replay, "hook": cmd_hook, "call": cmd_call, "pyrt-check": cmd_pyrt}
    if argv and argv[0] in modes:
        modes[argv[0]](argv[1:])
    else:
        cmd_cli(argv)
    sys.stdout.flush()


if __name__ == "__main__":
    main()
