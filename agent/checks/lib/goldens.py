"""Reading goldens: which conversations are covered, per-kind file lists."""
import glob
import json
import os

from . import paths, sampling


def manifest():
    p = os.path.join(paths.goldens_dir(), "MANIFEST.json")
    return json.load(open(p)) if os.path.exists(p) else None


def e2e_path():
    files = sorted(glob.glob(os.path.join(paths.goldens_dir(), "e2e", "*.jsonl")))
    return files[0] if files else None


def quick_percent():
    return int(os.environ.get("CHECK_QUICK_PERCENT", "0") or 0) or 100


def conv_selected(conv_id):
    return sampling.in_sample(conv_id, quick_percent())


def hook_files(module=None, hook=None):
    out = []
    for p in sorted(glob.glob(os.path.join(paths.goldens_dir(), "hooks", "*.jsonl*"))):
        base = os.path.basename(p).split(".jsonl")[0]
        mod, _, h = base.rpartition(".")
        if (module is None or mod == module) and (hook is None or h == hook):
            out.append((mod, h, p))
    return out


def function_files(name=None):
    out = []
    for p in sorted(glob.glob(os.path.join(paths.goldens_dir(), "functions", "*.jsonl*"))):
        fn = os.path.basename(p).split(".jsonl")[0]
        if name is None or fn == name or fn.endswith("." + name):
            out.append((fn, p))
    return out
