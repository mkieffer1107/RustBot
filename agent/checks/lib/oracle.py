"""Loads PyBot as the oracle: PYTHONHASHSEED=0, the pybot_web seeding scheme, a frozen clock, RNG consumption counts,
and hook-level recording. Nothing here changes what PyBot computes.

Process model: one process imports PyBot for a (seed, clock) pair; every conversation then runs in a forked child, so
each conversation sees exactly a freshly loaded PyBot (module-level state never leaks between conversations).
"""
import os
import sys
import types

from . import paths


def ensure_env():
    """Re-exec with PYTHONHASHSEED=0 (and a pyc cache outside the repo) when it is not set."""
    if os.environ.get("PYTHONHASHSEED") == "0" and os.environ.get("_ORACLE_ENV") == "1":
        return
    env = dict(os.environ, PYTHONHASHSEED="0", _ORACLE_ENV="1", PYTHONSAFEPATH="1", PYTHONIOENCODING="utf-8")
    env.pop("PYTHONDONTWRITEBYTECODE", None)
    env.setdefault("PYTHONPYCACHEPREFIX", os.path.join(os.environ.get("TMPDIR", "/tmp").rstrip("/"), "pybot-pyc"))
    sys.stdout.flush()
    os.execve(sys.executable, [sys.executable, "-s", "-X", "utf8"] + sys.orig_argv[1:], env)


# ---------------------------------------------------------------- clock

class Clock:
    instant = None  # a real datetime.datetime, naive, treated as UTC when a tz is asked for


def parse_clock(text):
    import datetime as real
    try:
        return real.datetime.fromisoformat(text)
    except ValueError:
        return real.datetime.fromisoformat(text + "T00:00:00")


def set_clock(iso):
    Clock.instant = parse_clock(iso)


def install_datetime_shim():
    """Replace sys.modules['datetime'] by a module whose date/datetime read the frozen clock.

    isinstance(x, datetime.date) stays true for real dates and datetimes (metaclass), so objects made by C code
    (datetime.date() returns a plain date) still pass.
    """
    import datetime as real
    if getattr(real, "_ORACLE_SHIM", False):
        return real
    R_DATE, R_DT = real.date, real.datetime

    class DateMeta(type):
        def __instancecheck__(cls, obj):
            return isinstance(obj, R_DATE)

        def __subclasscheck__(cls, sub):
            return issubclass(sub, R_DATE)

    class DateTimeMeta(type):
        def __instancecheck__(cls, obj):
            return isinstance(obj, R_DT)

        def __subclasscheck__(cls, sub):
            return issubclass(sub, R_DT)

    class FrozenDate(R_DATE, metaclass=DateMeta):
        @classmethod
        def today(cls):
            i = Clock.instant
            return cls(i.year, i.month, i.day)

    class FrozenDateTime(R_DT, metaclass=DateTimeMeta):
        @classmethod
        def now(cls, tz=None):
            i = Clock.instant
            if tz is not None:
                i = i.replace(tzinfo=real.timezone.utc).astimezone(tz)
            return cls(i.year, i.month, i.day, i.hour, i.minute, i.second, i.microsecond, tzinfo=i.tzinfo)

        @classmethod
        def today(cls):
            return cls.now()

        @classmethod
        def utcnow(cls):
            return cls.now()

    FrozenDate.__module__ = FrozenDateTime.__module__ = "datetime"
    FrozenDate.__qualname__, FrozenDateTime.__qualname__ = "date", "datetime"
    shim = types.ModuleType("datetime")
    shim.__dict__.update({k: v for k, v in real.__dict__.items() if k not in ("__name__", "__spec__", "__loader__")})
    shim.date, shim.datetime, shim._ORACLE_SHIM = FrozenDate, FrozenDateTime, True
    sys.modules["datetime"] = shim
    return shim


# ---------------------------------------------------------------- random

class RngState:
    seed = 0           # SEED of the pybot_web scheme
    created = 0        # unseeded Random() instances made so far (the n in f"{SEED}:{n}")
    counts = []        # 32-bit MT outputs consumed since seeding, per unseeded instance, by n
    opaque = False     # a setstate() we could not account for
    glob = None        # the global instance
    log = None         # list collecting fresh explicit-seed single-op cases (random cases), or None


def rng_state():
    return {"global": RngState.glob._n, "instances": list(RngState.counts)}


def rng_snapshot():
    """Cheap comparable form of rng_state()."""
    return (RngState.glob._n, tuple(RngState.counts))


_LOGGED = ("random", "getrandbits", "randrange", "randint", "choice", "shuffle", "sample", "choices")


def install_random(seed):
    import random as _r
    RngState.seed, RngState.created, RngState.counts, RngState.opaque = seed, 0, [], False
    Base = _r.Random

    class CountingRandom(Base):
        def __init__(self, x=None):
            self._n, self._idx, self._saved, self._depth, self._seed_arg = 0, None, [], 0, None
            super().__init__(x)

        def seed(self, a=None, version=2):
            if a is None:
                n = RngState.created
                RngState.created += 1
                RngState.counts.append(0)
                self._idx, self._seed_arg = n, None
                a = f"{RngState.seed}:{n}"
            else:
                self._idx, self._seed_arg = None, a
            super().seed(a, version)
            self._n = 0
            if self._idx is not None:
                RngState.counts[self._idx] = 0

        def _add(self, k):
            self._n += k
            if self._idx is not None:
                RngState.counts[self._idx] = self._n

        def random(self):
            r = super().random()
            self._add(2)
            return r

        def getrandbits(self, k):
            r = super().getrandbits(k)
            self._add(0 if k == 0 else (k + 31) // 32)
            return r

        def getstate(self):
            st = super().getstate()
            self._saved = (self._saved + [(st, self._n)])[-8:]
            return st

        def setstate(self, state):
            super().setstate(state)
            for s, n in self._saved:
                if s is state:
                    self._n = n
                    if self._idx is not None:
                        RngState.counts[self._idx] = n
                    return
            RngState.opaque = True

    def traced(name):
        base = getattr(Base, name)

        def f(self, *a, **k):
            if (self._depth or RngState.log is None or self._seed_arg is None or self._n != 0
                    or self._idx is not None):
                self._depth += 1
                try:
                    return base(self, *a, **k)
                finally:
                    self._depth -= 1
            from .canon import encode
            before = encode(list(a)) if name in ("shuffle",) or True else None
            self._depth += 1
            try:
                r = base(self, *a, **k)
            finally:
                self._depth -= 1
            if name == "shuffle":
                r = a[0]
            if not k and len(repr(before)) < 4000:
                RngState.log.append({"seed": encode(self._seed_arg), "op": name, "args": before, "result": encode(r)})
            return r
        f.__name__ = name
        return f

    for name in _LOGGED:
        if name in ("random", "getrandbits"):
            continue  # counted above, cheap; logged through the generic ops below
        setattr(CountingRandom, name, traced(name))
    _r.Random = CountingRandom
    inst = CountingRandom(0)
    RngState.glob = inst
    _r._inst = inst
    for name in ("seed", "random", "uniform", "triangular", "randint", "choice", "randrange", "sample", "shuffle",
                 "choices", "normalvariate", "lognormvariate", "expovariate", "vonmisesvariate", "gammavariate",
                 "gauss", "betavariate", "paretovariate", "weibullvariate", "getstate", "setstate", "getrandbits",
                 "randbytes", "binomialvariate"):
        if hasattr(inst, name):
            setattr(_r, name, getattr(inst, name))
    inst.seed(seed)
    return CountingRandom


def restore_rng(rng):
    """Put the global and unseeded instances at the recorded consumption counts (used by fake_rustbot)."""
    import random as _r
    base = _r.Random.__mro__[1]
    inst = RngState.glob
    targets = [(inst, f"{RngState.seed}" if False else RngState.seed, rng.get("global", 0))]
    for i, c in enumerate(rng.get("instances", [])):
        targets.append((None, i, c))
    # global
    Base = base
    Base.seed(inst, RngState.seed, 2)
    inst.gauss_next = None
    for _ in range(rng.get("global", 0)):
        Base.getrandbits(inst, 32)
    inst._n = rng.get("global", 0)
    # unseeded instances: only those that exist in this process can be touched; later ones are made on demand
    for i, c in enumerate(rng.get("instances", [])):
        if i >= len(RngState.counts):
            RngState.counts.append(0)
        RngState.counts[i] = c
        RngState.created = max(RngState.created, i + 1)
    for obj in _LIVE:
        if obj._idx is not None and obj._idx < len(rng.get("instances", [])):
            Base.seed(obj, f"{RngState.seed}:{obj._idx}", 2)
            for _ in range(rng["instances"][obj._idx]):
                Base.getrandbits(obj, 32)
            obj._n = rng["instances"][obj._idx]


_LIVE = []  # unseeded instances alive at load time (module-level Random()); filled by load()


# ---------------------------------------------------------------- loading

class Loaded:
    rt = None
    hooks = {}       # (module, hook) -> callable (the original, unwrapped)
    modules = {}     # module name -> module object
    load_s = 0.0
    clock_read_at_import = False


def load(seed, clock, extra_before_import=None):
    """Import PyBot's runtime under the oracle's seeding and clock. Returns seconds taken."""
    import time
    set_clock(clock)
    install_datetime_shim()
    install_random(seed)
    if extra_before_import:
        extra_before_import()
    sys.path.insert(0, paths.PYBOT)
    t0 = time.monotonic()
    import bot.runtime as rt
    Loaded.load_s = time.monotonic() - t0
    Loaded.rt = rt
    Loaded.modules = {name: m for name, _, m in rt._MODULES}
    for kind, lst in (("observe", rt._OBSERVERS), ("respond", rt._HANDLERS), ("after", rt._AFTER)):
        for name, _, fn in lst:
            Loaded.hooks[(name, kind)] = fn
    if rt._ARBITER is not None:
        Loaded.hooks[("arbiter", "choose")] = rt._ARBITER
    import gc
    gc.collect()
    for o in gc.get_objects():
        if type(o) is sys.modules["random"].Random and o._idx is not None:
            _LIVE.append(o)
    return Loaded.load_s


# ---------------------------------------------------------------- hook recording

def install_recorders(sink, ctx):
    """Wrap each module's observe/respond/after and arbiter.choose. sink(module, hook, record) gets one dict per call.

    ctx is a dict with "conv", "turn", "seed", "clock", updated by the driver."""
    from .canon import encode
    rt = Loaded.rt

    def wrap(name, hook, fn):
        def run(text, memory, *rest):
            memory_in = encode(memory)
            args_in = encode([text, *rest])
            rng_in = rng_state()
            if RngState.opaque:
                rng_in["opaque"] = True
            ret, exc = None, None
            try:
                ret = fn(text, memory, *rest)
            except BaseException as e:  # noqa: BLE001
                exc = type(e).__name__
                if not isinstance(e, Exception):
                    raise
                raise
            finally:
                rng_out = rng_state()
                rec = {"conv": ctx["conv"], "turn": ctx["turn"], "seed": ctx["seed"], "clock": ctx["clock"],
                       "args": args_in, "memory_in": memory_in, "ret": None if exc else encode(ret), "exc": exc,
                       "memory_out": encode(memory), "rng_in": rng_in, "rng_out": rng_out}
                if RngState.opaque:
                    rec["rng_out"]["opaque"] = True
                sink(name, hook, rec)
            return ret
        run.__wrapped__ = fn
        return run

    rt._OBSERVERS = [(n, p, wrap(n, "observe", f)) for n, p, f in rt._OBSERVERS]
    rt._HANDLERS = [(n, p, wrap(n, "respond", f)) for n, p, f in rt._HANDLERS]
    rt._AFTER = [(n, p, wrap(n, "after", f)) for n, p, f in rt._AFTER]
    if rt._ARBITER is not None:
        rt._ARBITER = wrap("arbiter", "choose", rt._ARBITER)


class Trace:
    web = None
    on = False
    last = None


def install_trace():
    """Reuse reference/pybot-space/pybot_web.py's own tracing (sys.monitoring PY_START per handler, respond-span search,
    answer_source with norm.json/lines.json built the way build_bundle.py builds them). Call after load()."""
    import importlib.util
    import json as _json
    spec = importlib.util.spec_from_file_location(
        "pybot_web", os.path.join(paths.ROOT, "reference", "pybot-space", "pybot_web.py"))
    pw = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(pw)
    pw.ROOT = paths.PYBOT
    pw._rt = Loaded.rt
    norm, lines = {}, {}
    keep = lambda s: __import__("re").sub(r"[^a-z0-9]", "", s.lower())  # noqa: E731 (as in build_bundle.py)
    for dirpath, _, names in sorted(os.walk(os.path.join(paths.PYBOT, "bot"))):
        for name in sorted(names):
            if name.endswith(".py"):
                p = os.path.join(dirpath, name)
                rel = os.path.relpath(p, paths.PYBOT)
                with open(p, encoding="utf-8", errors="replace") as h:
                    text = h.read()
                norm[rel] = keep(text)
                lines[rel] = [len(keep(l)) for l in text.splitlines(keepends=True)]
    pw._sources, pw._lines = norm, lines
    pw._watch_handlers()
    Trace.web, Trace.on = pw, True


def run_turn(text, memory):
    pw = Trace.web
    if Trace.on:
        pw._files.clear()
    result = Loaded.rt.turn(text, memory)
    if Trace.on:
        reply = " ".join(str(result["reply"]).split()) or "Hello!"
        module = result.get("module")
        calls = pw._files.get(module, {}) if module else {}
        files = sorted(calls)
        source, source_line = pw.answer_source(reply, files)
        Trace.last = {"reply": reply, "files": files, "calls": calls,
                      "respond": pw._responds.get(module) if module else None,
                      "source": source, "source_line": source_line}
    return result


def turn_record(conv, idx, text, result, memory):
    from .canon import encode
    if Trace.on:
        return {"conv": conv["id"], "turn": idx, "text": text, "reply": result["reply"], "module": result["module"],
                "candidates": [[c["module"], c["reply"]] for c in result["candidates"]], "memory": encode(memory),
                "trace": Trace.last}
    return {"conv": conv["id"], "turn": idx, "text": text, "reply": result["reply"], "module": result["module"],
            "candidates": [[c["module"], c["reply"]] for c in result["candidates"]], "memory": encode(memory)}
