#!/usr/bin/env python3
"""Deterministic small slice of a corpus for the selftest: the first K conversations of every kind."""
import collections
import json
import sys

src, dst, per_kind = sys.argv[1], sys.argv[2], int(sys.argv[3])
seen = collections.Counter()
out = []
for line in open(src, encoding="utf-8"):
    c = json.loads(line)
    kind = c["id"].split("-")[1] if c["id"].count("-") >= 2 else "x"
    if seen[kind] < per_kind and (kind != "long" or len(c["turns"]) <= 5 or seen[kind] < 1):
        seen[kind] += 1
        out.append(line if line.endswith("\n") else line + "\n")
open(dst, "w", encoding="utf-8").writelines(out)
print(f"slice: {len(out)} conversations ({dict(seen)})", file=sys.stderr)
