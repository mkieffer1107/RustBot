#!/usr/bin/env python3
"""G5 part 2: the .wasm gzip size."""
import glob
import gzip
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from lib import paths, report  # noqa: E402

LIMIT = 15 * 1024 * 1024


def main():
    root = paths.wasmbot_dir()
    cands = [p for p in glob.glob(os.path.join(root, "**", "*.wasm"), recursive=True) if "/target/" not in p and "/node_modules/" not in p]
    if not cands:
        return report.missing("wasm-size", f"no .wasm under {root}")
    biggest = max(cands, key=os.path.getsize)
    raw = open(biggest, "rb").read()
    gz = len(gzip.compress(raw, 9, mtime=0))
    ok = gz <= LIMIT
    print(f"  {os.path.relpath(biggest, root)}: raw={len(raw)/1e6:.2f} MB gzip={gz/1e6:.2f} MB (limit 15 MB)")
    print(f"CHECK wasm-size {'PASS' if ok else 'FAIL'} gzip_mb={gz/1e6:.2f}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
