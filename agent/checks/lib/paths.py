"""Locations, environment defaults and JSONL helpers shared by every check."""
import gzip
import json
import os
import sys

CHECKS = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
AGENT = os.path.dirname(CHECKS)
ROOT = os.path.dirname(AGENT)
PYBOT = os.path.join(ROOT, "reference", "pybot")
SMOLTALK = os.path.join(ROOT, "reference", "smoltalkbench", "test.parquet")
PYBOT_COMMIT = "7d63c7f"
DEFAULT_CLOCK = "2026-06-15T12:00:00"
DEFAULT_SEED = 0


def goldens_dir():
    return os.environ.get("GOLDENS") or os.path.join(AGENT, "goldens")


def check_out(*parts):
    """Scratch/output dir for check runs (never under agent/checks or agent/goldens: those are read-only on the cluster)."""
    base = os.environ.get("CHECK_OUT") or os.path.join(AGENT, "runs", "checks")
    path = os.path.join(base, *parts)
    os.makedirs(os.path.dirname(path) if parts and "." in os.path.basename(path) else path, exist_ok=True)
    return path


def wasmbot_dir():
    return os.environ.get("WASMBOT_DIR") or os.path.join(ROOT, "wasmbot")


def corpus_path(name="dev"):
    return os.environ.get("CORPUS") or os.path.join(CHECKS, "corpus", name + ".jsonl")


def rustbot_bin():
    """The RustBot binary (RUSTBOT_BIN or rustbot/target/release/rustbot), or None when it does not exist."""
    path = os.environ.get("RUSTBOT_BIN") or os.path.join(ROOT, "rustbot", "target", "release", "rustbot")
    return path if os.path.isfile(path) and os.access(path, os.X_OK) else None


def open_text(path, mode="rt"):
    """Open a text file; .gz files are read and written deterministically (mtime 0)."""
    if path.endswith(".gz"):
        if "w" in mode:
            raw = open(path, mode.replace("t", "b"))
            return _io_wrap(gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0, compresslevel=6), raw)
        return gzip.open(path, mode, encoding="utf-8")
    return open(path, mode, encoding="utf-8")


def _io_wrap(gz, raw):
    import io

    class Both(io.TextIOWrapper):
        def close(self):
            if not self.closed:
                super().close()
                raw.close()
    return Both(gz, encoding="utf-8", newline="\n")


def find_golden(*parts):
    """Path of a golden file given without extension handling: prefers .jsonl.gz, then .jsonl; None if absent."""
    base = os.path.join(goldens_dir(), *parts)
    for cand in (base + ".gz", base):
        if os.path.isfile(cand):
            return cand
    return None


def read_jsonl(path):
    with open_text(path) as handle:
        for line in handle:
            line = line.strip()
            if line:
                yield json.loads(line)


def dump_line(obj):
    """One canonical-JSON-compatible line (no sort, compact, UTF-8; surrogates fall back to \\u escapes)."""
    try:
        text = json.dumps(obj, ensure_ascii=False, separators=(",", ":"))
        text.encode("utf-8")
    except UnicodeEncodeError:
        text = json.dumps(obj, ensure_ascii=True, separators=(",", ":"))
    return text


def eprint(*args):
    print(*args, file=sys.stderr, flush=True)


def load_corpus(path=None):
    return list(read_jsonl(path or corpus_path()))
