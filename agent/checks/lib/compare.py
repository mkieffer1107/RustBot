"""Strict comparison of decoded canonical JSON, and the path of the first difference."""


def first_diff(a, b, path=""):
    """None when a and b are identical (types, dict key ORDER, list order), else (path, a_part, b_part)."""
    if type(a) is not type(b):
        return (path or "$", a, b)
    if isinstance(a, dict):
        ka, kb = list(a), list(b)
        if ka != kb:
            only_a = [k for k in ka if k not in b]
            only_b = [k for k in kb if k not in a]
            if only_a or only_b:
                return (path or "$", {"missing_in_rust": only_a}, {"extra_in_rust": only_b})
            return (path + ".<key order>", ka, kb)
        for k in ka:
            d = first_diff(a[k], b[k], f"{path}.{k}")
            if d:
                return d
        return None
    if isinstance(a, list):
        for i, (x, y) in enumerate(zip(a, b)):
            d = first_diff(x, y, f"{path}[{i}]")
            if d:
                return d
        if len(a) != len(b):
            return (path + ".<len>", len(a), len(b))
        return None
    return None if a == b else (path or "$", a, b)


def clip(x, n=120):
    s = x if isinstance(x, str) else __import__("json").dumps(x, ensure_ascii=False)
    s = s if isinstance(x, str) is False else repr(x)
    return s if len(s) <= n else s[: n - 3] + "..."


def describe(d):
    path, py, rs = d
    return f"{path or '$'}: python={clip(py)} rust={clip(rs)}"
