"""Running RustBot (native binary or the wasm replay script) and reading its JSONL output."""
import json
import os
import subprocess
import sys

from . import paths


def run_jsonl(argv, records, timeout=3600, env=None):
    """Feed records (dicts) as JSONL to a command; returns (returncode, [parsed output objects], stderr text, bad_line)."""
    data = "".join(paths.dump_line(r) + "\n" for r in records).encode("utf-8", "surrogatepass")
    full_env = dict(os.environ, PYBOT_SEED="0", PYBOT_CLOCK=paths.DEFAULT_CLOCK)
    full_env.update(env or {})
    try:
        p = subprocess.run(argv, input=data, capture_output=True, timeout=timeout, env=full_env)
    except subprocess.TimeoutExpired:
        return -9, [], f"timeout after {timeout}s", None
    except OSError as e:
        return -1, [], f"cannot run {argv[0]}: {e}", None
    out, bad = [], None
    for line in p.stdout.decode("utf-8", "replace").split("\n"):
        if not line.strip():
            continue
        try:
            out.append(json.loads(line))
        except ValueError:
            bad = line[:200]
            break
    err = p.stderr.decode("utf-8", "replace")
    return p.returncode, out, err[-1500:], bad


def need_binary():
    """Path of the RustBot binary, or exits through report.missing."""
    from . import report
    b = paths.rustbot_bin()
    if b is None:
        want = os.environ.get("RUSTBOT_BIN") or "rustbot/target/release/rustbot"
        sys.exit(report.missing_exit(f"RustBot binary not found or not executable ({want}); build it or set RUSTBOT_BIN"))
    return b
