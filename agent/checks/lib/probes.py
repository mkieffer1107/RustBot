"""Instrumentation used while recording goldens and measuring corpus coverage (never changes PyBot's behaviour)."""
import os
import re
import sys

from . import canon, paths, pyrt_cases
from . import oracle

BOT_DIR = os.path.join(paths.PYBOT, "bot") + os.sep
mon = sys.monitoring
RE_METHODS = ("match", "search", "fullmatch", "findall", "finditer", "sub", "subn", "split")


# ---------------------------------------------------------------- regex recording

class ReLog:
    cases = []            # recorded re cases (inputs only)
    seen = {}             # (pattern, flags, method) -> count
    keys = set()
    cap = 4
    enabled = False
    patterns = {}         # (pattern, flags) -> True, every pattern compiled by bot code (incl. at import)


def _record_re(pattern, flags, method, args):
    key = (pattern, flags, method)
    n = ReLog.seen.get(key, 0)
    if n >= ReLog.cap or not isinstance(pattern, str):
        return
    try:
        if any(isinstance(a, str) and len(a) > 4000 for a in args):
            return
        enc = [a if isinstance(a, dict) and "$callable" in a else canon.encode(a) for a in args]
        sig = repr(enc)
        if (key, sig) in ReLog.keys:
            return
        ReLog.keys.add((key, sig))
        ReLog.seen[key] = n + 1
        ReLog.cases.append({"pattern": pattern, "flags": int(flags), "method": method, "args": enc})
    except Exception:  # noqa: BLE001
        pass


class RecPattern:
    """Stands in for a compiled pattern made by bot code; forwards everything to the real one and records calls."""
    __slots__ = ("_p",)

    def __init__(self, p):
        object.__setattr__(self, "_p", p)

    pattern = property(lambda self: self._p.pattern)
    flags = property(lambda self: self._p.flags)
    groups = property(lambda self: self._p.groups)
    groupindex = property(lambda self: self._p.groupindex)

    def __repr__(self):
        return repr(self._p)

    def __copy__(self):
        return self

    def __deepcopy__(self, memo):
        return self

    def __eq__(self, other):
        return self._p == (other._p if isinstance(other, RecPattern) else other)

    def __hash__(self):
        return hash(self._p)

    def __getattr__(self, name):
        return getattr(self._p, name)


class RecMatch:
    """Forward a match while preserving its compiled pattern identity."""
    __slots__ = ("_match", "_owner")

    def __init__(self, match, owner):
        self._match = match
        self._owner = owner

    @property
    def __class__(self):
        return self._match.__class__

    @property
    def re(self):
        return self._owner

    def __getattr__(self, name):
        return getattr(self._match, name)

    def __getitem__(self, key):
        return self._match[key]

    def __repr__(self):
        return repr(self._match)

    def __copy__(self):
        return self

    def __deepcopy__(self, memo):
        return self


def _call_recorded(p, name, args, kwargs=None, owner=None, record=True):
    kwargs = kwargs or {}
    should_record = record and ReLog.enabled and not kwargs
    rets, call_args = None, args
    replacement = args[0] if args else kwargs.get("repl")
    if name in ("sub", "subn") and callable(replacement):
        rets, fn = ([] if should_record else None), replacement

        def wrapped(m):
            r = fn(RecMatch(m, owner) if owner is not None else m)
            if rets is not None:
                rets.append(r)
            return r
        if args:
            call_args = (wrapped,) + tuple(args[1:])
        else:
            kwargs = {**kwargs, "repl": wrapped}
    result = getattr(p, name)(*call_args, **kwargs)
    if should_record:
        if name == "finditer":
            result = iter(list(result))
        if rets is not None:
            _record_re(p.pattern, p.flags, name, [{"$callable": canon.encode(rets)}] + list(args[1:]))
        else:
            _record_re(p.pattern, p.flags, name, list(args))
    if owner is not None:
        if name in ("match", "search", "fullmatch") and result is not None:
            return RecMatch(result, owner)
        if name == "finditer":
            return (RecMatch(match, owner) for match in result)
    return result


def _make_method(name):
    def method(self, *args, **kwargs):
        return _call_recorded(self._p, name, args, kwargs, self)
    method.__name__ = name
    return method


for _m in RE_METHODS:
    setattr(RecPattern, _m, _make_method(_m))


def install_re():
    """Patch `re` so patterns compiled by bot code (and module-level re.sub etc. called from bot code) are recorded.
    Call before importing PyBot."""
    real_compile, proxies, keep = re.compile, {}, []

    def owner_for(p):
        prox = proxies.get(id(p))
        if prox is None:
            prox = proxies[id(p)] = RecPattern(p)
            keep.append(p)
        return prox

    def compile_(pattern, flags=0):
        if isinstance(pattern, RecPattern):
            return pattern
        p = real_compile(pattern, flags)
        if sys._getframe(1).f_code.co_filename.startswith(BOT_DIR):
            if isinstance(pattern, str):
                ReLog.patterns[(pattern, int(flags))] = True
            return owner_for(p)
        return p

    flag_pos = {"sub": 3, "subn": 3, "split": 2}   # index in args after the pattern

    def module_fn(name):
        fpos = flag_pos.get(name, 1)

        def f(pattern, *args, **kwargs):
            flags = kwargs.pop("flags", 0)
            if len(args) > fpos:
                flags, args = args[fpos], args[:fpos]
            owner = pattern if isinstance(pattern, RecPattern) else None
            p = owner._p if owner is not None else real_compile(pattern, flags)
            record = sys._getframe(1).f_code.co_filename.startswith(BOT_DIR)
            if owner is None and record and isinstance(pattern, (str, bytes)):
                owner = owner_for(p)
            return _call_recorded(p, name, args, kwargs, owner, record)
        f.__name__ = name
        return f

    re.compile = compile_
    for name in RE_METHODS:
        setattr(re, name, module_fn(name))


# ---------------------------------------------------------------- call probe (trace + str/format samples)

class CallLog:
    trace = {}         # key -> [count, "file:line"]
    str_cases = []
    floats = []
    per_method = {}
    seen = set()
    cap_per_method = 40


def _key(c, arg0=None):
    t = type(c).__name__
    name = getattr(c, "__name__", None)
    if t == "builtin_function_or_method":
        owner = getattr(c, "__self__", None)
        if owner is None or isinstance(owner, type(sys)):
            return f"{getattr(owner, '__name__', 'builtins')}.{name}", None
        if isinstance(owner, type):
            return f"{owner.__name__}.{name}", None
        return f"{type(owner).__name__}.{name}", owner
    if t in ("method_descriptor", "wrapper_descriptor", "method-wrapper"):
        oc = getattr(c, "__objclass__", None)
        # a method call `s.m(...)` reaches the monitor as (descriptor, self): the real arguments are not visible
        return f"{getattr(oc, '__name__', '?')}.{name}", (arg0 if oc is str and isinstance(arg0, str) else None)
    if isinstance(c, type):
        return f"class:{c.__name__}", None
    mod = getattr(c, "__module__", None)
    if mod and not str(mod).startswith("bot") and name:
        return f"{mod}.{getattr(c, '__qualname__', name)}", None
    return None, None


def install_call_probe():
    tool = 3
    mon.use_tool_id(tool, "oracle-calls")
    missing = mon.MISSING
    cap = CallLog.cap_per_method
    sites = {}

    def on_call(code, offset, c, arg0):
        fn = code.co_filename
        if not fn.startswith(BOT_DIR):
            return mon.DISABLE
        try:
            key, owner = _key(c, arg0)
            if key is None:
                return mon.DISABLE
            site = (code, offset)
            k = sites.get(site, 0) + 1
            sites[site] = k
            ent = CallLog.trace.get(key)
            if ent is None:
                CallLog.trace[key] = [1, f"{fn[len(BOT_DIR) - 4:]}:{code.co_firstlineno}"]
            else:
                ent[0] += 1
            if isinstance(owner, str) and len(owner) < 400:
                meth = c.__name__
                if meth in pyrt_cases.STR_ALLOWED and CallLog.per_method.get(meth, 0) < cap:
                    sig = (meth, owner)
                    if sig not in CallLog.seen:
                        CallLog.seen.add(sig)
                        CallLog.per_method[meth] = CallLog.per_method.get(meth, 0) + 1
                        CallLog.str_cases.extend(pyrt_cases.str_cases_for(meth, owner))
            elif key in ("class:str", "builtins.repr", "builtins.round", "builtins.format") and type(arg0) is float:
                if len(CallLog.floats) < 400:
                    CallLog.floats.append(arg0)
            if k >= 3:
                return mon.DISABLE
        except Exception:  # noqa: BLE001
            pass
        return None

    mon.register_callback(tool, mon.events.CALL, on_call)
    mon.set_events(tool, mon.events.CALL)


# ---------------------------------------------------------------- line coverage

class Cov:
    hit = set()


def install_coverage():
    tool = 1
    mon.use_tool_id(tool, "oracle-cov")
    hit = Cov.hit

    def on_line(code, line):
        if code.co_filename.startswith(BOT_DIR):
            hit.add((code.co_filename, line))
        return mon.DISABLE

    mon.register_callback(tool, mon.events.LINE, on_line)
    mon.set_events(tool, mon.events.LINE)


def executable_lines():
    """{file: set(lines)} of every line that holds code in bot/**.py."""
    result = {}
    for dp, dirs, files in os.walk(BOT_DIR):
        dirs.sort()
        for f in sorted(files):
            if f.endswith(".py"):
                path = os.path.join(dp, f)
                code = compile(open(path, encoding="utf-8").read(), path, "exec")
                lines, stack = set(), [code]
                while stack:
                    c = stack.pop()
                    lines.update(l for _, _, l in c.co_lines() if l)
                    stack += [k for k in c.co_consts if hasattr(k, "co_lines")]
                result[path] = lines
    return result


# ---------------------------------------------------------------- pure function recording

class FuncLog:
    records = []       # {"fn", "args", "ret", "exc", "seed", "clock"}
    impure = set()
    calls = {}         # code -> calls watched so far
    kept = {}          # fn name -> kept count
    names = {}         # code -> dotted name
    sigs = set()
    pending = {}
    ctx = {}
    per_fn_keep = 12
    per_fn_watch = 60


def candidate_functions():
    """{code: dotted name} for plain module-level functions of bot.understanding and bot.compose."""
    import types
    out = {}
    for modname in sorted(sys.modules):
        if not (modname.startswith("bot.understanding.") or modname.startswith("bot.compose.")):
            continue
        mod = sys.modules[modname]
        for name, fn in sorted(vars(mod).items()):
            if (isinstance(fn, types.FunctionType) and fn.__module__ == modname and not hasattr(fn, "__wrapped__")
                    and fn.__code__.co_name == fn.__name__ and fn.__qualname__ == fn.__name__):
                code = fn.__code__
                flags = code.co_flags
                if flags & (0x20 | 0x80 | 0x100 | 0x200):  # generator, coroutine, async gen
                    continue
                if code.co_kwonlyargcount or flags & 0x8 or flags & 0x4:  # keyword-only, **kw, *args
                    continue
                out.setdefault(code, f"{modname}.{name}")
    return out


def install_function_recorder(ctx):
    FuncLog.ctx = ctx
    FuncLog.names = names = candidate_functions()
    tool = 2
    mon.use_tool_id(tool, "oracle-funcs")
    ev = mon.events
    max_enc = 4000

    def on_start(code, offset):
        n = FuncLog.calls.get(code, 0) + 1
        FuncLog.calls[code] = n
        if n > FuncLog.per_fn_watch or FuncLog.kept.get(names[code], 0) >= FuncLog.per_fn_keep:
            mon.set_local_events(tool, code, 0)
            return
        frame = sys._getframe(1)
        loc = frame.f_locals
        args = [loc[v] for v in code.co_varnames[:code.co_argcount]]
        try:
            before = canon.encode(args)
        except Exception:  # noqa: BLE001
            return
        text = repr(before)
        if len(text) > max_enc or not canon.is_clean(before):
            return
        FuncLog.pending[id(frame)] = (code, args, before, text, oracle.rng_snapshot())

    def finish(code, retval, exc):
        frame = sys._getframe(2)  # the monitored function (finish <- on_return <- frame)
        ent = FuncLog.pending.pop(id(frame), None)
        if ent is None or ent[0] is not code:
            return
        _, args, before, text, rng0 = ent
        name = names[code]
        if name in FuncLog.impure:
            return
        try:
            if repr(canon.encode(args)) != text or oracle.rng_snapshot() != rng0:
                FuncLog.impure.add(name)
                mon.set_local_events(tool, code, 0)
                return
            ret = canon.encode(retval) if exc is None else None
            if exc is None and (len(repr(ret)) > max_enc or not canon.is_clean(ret)):
                return
        except Exception:  # noqa: BLE001
            return
        sig = (name, text)
        if sig in FuncLog.sigs:
            return
        FuncLog.sigs.add(sig)
        FuncLog.kept[name] = FuncLog.kept.get(name, 0) + 1
        FuncLog.records.append({"fn": name, "args": before, "ret": ret, "exc": exc and type(exc).__name__,
                                "seed": FuncLog.ctx["seed"], "clock": FuncLog.ctx["clock"]})

    def on_return(code, offset, retval):
        finish(code, retval, None)

    def on_unwind(code, offset, exc):
        if code not in names:
            return None  # DISABLE is not allowed for PY_UNWIND (it would raise inside the unwinding frame)
        finish(code, None, exc)

    mon.register_callback(tool, ev.PY_START, on_start)
    mon.register_callback(tool, ev.PY_RETURN, on_return)
    mon.register_callback(tool, ev.PY_UNWIND, on_unwind)
    for code in names:
        mon.set_local_events(tool, code, ev.PY_START | ev.PY_RETURN)
    mon.set_events(tool, ev.PY_UNWIND)
