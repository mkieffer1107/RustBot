"""Run work items against freshly loaded PyBot: one load per (seed, clock) group, one forked child per item."""
import os
import pickle
import sys

from . import oracle


def _child(fn, arg):
    r, w = os.pipe()
    pid = os.fork()
    if pid == 0:
        os.close(r)
        try:
            data = pickle.dumps(("ok", fn(arg)), protocol=4)
        except BaseException:  # noqa: BLE001
            import traceback
            data = pickle.dumps(("err", traceback.format_exc()), protocol=4)
        with os.fdopen(w, "wb") as f:
            f.write(data)
        os._exit(0)
    os.close(w)
    with os.fdopen(r, "rb") as f:
        data = f.read()
    os.waitpid(pid, 0)
    status, payload = pickle.loads(data) if data else ("err", "child died")
    if status == "err":
        raise RuntimeError(payload)
    return payload


def map_items(items, fn, setup=None, after_load=None):
    """items: dicts with "seed" and "clock". Returns [fn(item) ...] in input order; fn runs in a forked child of a
    process that has loaded PyBot for that item's (seed, clock). setup() runs once per load (before importing PyBot)."""
    groups = {}
    for i, it in enumerate(items):
        groups.setdefault((it.get("seed", 0), it.get("clock", "2026-06-15T12:00:00")), []).append(i)
    results = [None] * len(items)
    for (seed, clock), idxs in sorted(groups.items()):
        r, w = os.pipe()
        pid = os.fork()
        if pid == 0:
            os.close(r)
            try:
                oracle.load(seed, clock, setup)
                if after_load:
                    after_load()
                out = [(i, _child(fn, items[i])) for i in idxs]
                data = pickle.dumps(("ok", out), protocol=4)
            except BaseException:  # noqa: BLE001
                import traceback
                data = pickle.dumps(("err", traceback.format_exc()), protocol=4)
            with os.fdopen(w, "wb") as f:
                f.write(data)
            os._exit(0)
        os.close(w)
        with os.fdopen(r, "rb") as f:
            data = f.read()
        os.waitpid(pid, 0)
        status, payload = pickle.loads(data) if data else ("err", "group died")
        if status == "err":
            raise RuntimeError(payload)
        for i, res in payload:
            results[i] = res
    return results
