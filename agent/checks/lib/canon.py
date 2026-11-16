"""Canonical JSON for Python values (see agent/checks/README.md).

encode(x) turns a Python value into plain JSON-able data (dict/list/str/int/bool/None) with tagged objects;
decode(j) is the inverse. Everything is order preserving: dict insertion order, set iteration order.
"""
import datetime as _dt
import re

_REAL_DATE = _dt.date
_REAL_DATETIME = _dt.datetime
_ADDR = re.compile(r"0x[0-9a-fA-F]{4,}")
MAX_DEPTH = 60


class Opaque:
    """Stands for a value that was encoded as {"$repr", "$type"} and cannot be rebuilt."""

    def __init__(self, text, type_name):
        self.text, self.type_name = text, type_name

    def __repr__(self):
        return self.text

    def __eq__(self, other):
        return isinstance(other, Opaque) and (self.text, self.type_name) == (other.text, other.type_name)

    def __hash__(self):
        return hash((self.text, self.type_name))


def _plain_keys(d):
    return all(type(k) is str and not k.startswith("$") for k in d)


def encode(x, _depth=0):
    if _depth > MAX_DEPTH:
        return {"$repr": "<too deep>", "$type": type(x).__name__}
    t = type(x)
    if x is None or t is bool or t is str:
        return x
    if t is int:
        return x
    if t is float:
        return {"$float": repr(x)}
    if t is list:
        return [encode(v, _depth + 1) for v in x]
    if t is dict:
        if _plain_keys(x):
            return {k: encode(v, _depth + 1) for k, v in x.items()}
        return {"$dict": [[encode(k, _depth + 1), encode(v, _depth + 1)] for k, v in x.items()]}
    if t is tuple:
        return {"$tuple": [encode(v, _depth + 1) for v in x]}
    if t is set:
        return {"$set": [encode(v, _depth + 1) for v in x]}
    if t is frozenset:
        return {"$frozenset": [encode(v, _depth + 1) for v in x]}
    # subclasses and the rest
    if isinstance(x, _REAL_DATETIME):
        return {"$datetime": x.isoformat()}
    if isinstance(x, _REAL_DATE):
        return {"$date": x.isoformat()}
    if isinstance(x, bool):
        return bool(x)
    if isinstance(x, int):
        return int(x)
    if isinstance(x, float):
        return {"$float": repr(float(x))}
    if isinstance(x, str):
        return str(x)
    if isinstance(x, dict):
        return encode(dict(x), _depth)
    if isinstance(x, list):
        return [encode(v, _depth + 1) for v in x]
    if isinstance(x, tuple):
        return {"$tuple": [encode(v, _depth + 1) for v in x]}
    if isinstance(x, frozenset):
        return {"$frozenset": [encode(v, _depth + 1) for v in x]}
    if isinstance(x, set):
        return {"$set": [encode(v, _depth + 1) for v in x]}
    try:
        text = _ADDR.sub("0x0", repr(x))
    except Exception:  # noqa: BLE001
        text = "<unrepresentable>"
    return {"$repr": text, "$type": t.__name__}


def is_clean(j):
    """True when an encoded value holds no $repr (it can be decoded back exactly)."""
    if isinstance(j, dict):
        if "$repr" in j and "$type" in j:
            return False
        return all(is_clean(v) for v in j.values())
    if isinstance(j, list):
        return all(is_clean(v) for v in j)
    return True


_TAGS = {"$float", "$tuple", "$set", "$frozenset", "$dict", "$date", "$datetime", "$repr"}


def decode(j):
    if isinstance(j, list):
        return [decode(v) for v in j]
    if not isinstance(j, dict):
        return j
    if len(j) in (1, 2) and any(k in _TAGS for k in j):
        if "$float" in j:
            return float(j["$float"])
        if "$tuple" in j:
            return tuple(decode(v) for v in j["$tuple"])
        if "$set" in j:
            return set(decode(v) for v in j["$set"])
        if "$frozenset" in j:
            return frozenset(decode(v) for v in j["$frozenset"])
        if "$dict" in j:
            return {decode(k): decode(v) for k, v in j["$dict"]}
        if "$date" in j:
            return _REAL_DATE.fromisoformat(j["$date"])
        if "$datetime" in j:
            return _REAL_DATETIME.fromisoformat(j["$datetime"])
        if "$repr" in j:
            return Opaque(j["$repr"], j.get("$type", ""))
    return {k: decode(v) for k, v in j.items()}
