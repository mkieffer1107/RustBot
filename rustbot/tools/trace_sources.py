#!/usr/bin/env python3
"""Build the PyBot Space's norm.json and lines.json from the pinned source.

The output directory is explicit so these generated resources can be packaged
by either the native runner or the browser; the source tree is never modified.
"""
import argparse
import json
import os
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]
KEEP = re.compile(r"[^a-z0-9]")


def normalize(text):
    return KEEP.sub("", text.lower())


def generate(source_root: Path, output_root: Path):
    source_root = source_root.resolve()
    output_root = output_root.resolve()
    bot = source_root / "bot"
    if not bot.is_dir():
        raise ValueError(f"missing bot source directory: {bot}")
    protected = (source_root, (ROOT / "reference").resolve(), (ROOT / "agent/checks").resolve())
    if any(output_root.is_relative_to(path) for path in protected):
        raise ValueError("output directory must not be inside a read-only source or checks tree")
    norm, lines = {}, {}
    for directory, _, names in sorted(os.walk(bot)):
        for name in sorted(names):
            if not name.endswith(".py"):
                continue
            path = Path(directory) / name
            relative = path.relative_to(source_root).as_posix()
            with path.open(encoding="utf8", errors="replace") as handle:
                text = handle.read()
            norm[relative] = normalize(text)
            lines[relative] = [len(normalize(line)) for line in text.splitlines(keepends=True)]
    output_root.mkdir(parents=True, exist_ok=True)
    for name, value in (("norm.json", norm), ("lines.json", lines)):
        temporary = output_root / (name + ".tmp")
        with temporary.open("w", encoding="utf8", newline="\n") as handle:
            json.dump(value, handle, separators=(",", ":"))
        os.replace(temporary, output_root / name)
    return len(norm)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, default=ROOT / "reference/pybot")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    print(f"Generated source trace tables for {generate(args.source_root, args.output)} files")


if __name__ == "__main__":
    main()
