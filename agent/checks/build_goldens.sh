#!/usr/bin/env bash
# Records PyBot's behaviour over a corpus into goldens.
#   build_goldens.sh [--corpus PATH] [--out DIR] [--sample PCT] [--jobs J]      all shards locally, merge, perf
#   build_goldens.sh --shard i/N [--corpus ...] [--out ...]                      one shard (e.g. one array task); resumable
#   build_goldens.sh merge --shards N [--corpus ...] [--out ...]                 merge finished shards
#   build_goldens.sh perf [--out DIR]                                            CPython perf numbers only
# Writes only to --out (default $GOLDENS, i.e. agent/goldens). Re-running skips finished shards.
set -euo pipefail
CHECKS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$CHECKS/../.." && pwd)"
export PYTHONDONTWRITEBYTECODE=1
OUT="${GOLDENS:-$REPO/agent/goldens}"; CORPUS_ARG="${CORPUS:-$CHECKS/corpus/dev.jsonl}"
SAMPLE=100; JOBS="$( (nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4) | head -n1)"
SHARD=""; SHARDS=""; CMD="all"; EXTRAS=3
if [ $# -gt 0 ] && [[ "$1" =~ ^(merge|perf)$ ]]; then CMD="$1"; shift; fi
while [ $# -gt 0 ]; do
  case "$1" in
    --corpus) CORPUS_ARG="$2"; shift 2;;
    --out) OUT="$2"; shift 2;;
    --sample) SAMPLE="$2"; shift 2;;
    --jobs) JOBS="$2"; shift 2;;
    --shard) SHARD="$2"; shift 2;;
    --shards) SHARDS="$2"; shift 2;;
    --extras-every) EXTRAS="$2"; shift 2;;
    *) echo "unknown argument $1" >&2; exit 2;;
  esac
done
PY="${PYTHON:-python3}"
rec() { "$PY" -s "$CHECKS/lib/record.py" "$@"; }
mkdir -p "$OUT"
if [ -n "$SHARD" ]; then CMD="shard"; fi
case "$CMD" in
  shard) rec shard --corpus "$CORPUS_ARG" --out "$OUT" --sample "$SAMPLE" --shard "$SHARD" --extras-every "$EXTRAS";;
  merge) [ -n "$SHARDS" ] || { echo "merge needs --shards N" >&2; exit 2; }
         rec merge --corpus "$CORPUS_ARG" --out "$OUT" --sample "$SAMPLE" --shards "$SHARDS";;
  perf)  mkdir -p "$OUT/perf"; "$PY" -s "$CHECKS/lib/perf.py" --python --out "$OUT/perf/python.json";;
  all)
    N="${SHARDS:-$JOBS}"
    pids=()
    for ((i = 0; i < N; i++)); do
      rec shard --corpus "$CORPUS_ARG" --out "$OUT" --sample "$SAMPLE" --shard "$i/$N" --extras-every "$EXTRAS" \
        > "$OUT/shard-$i.log" 2>&1 &
      pids+=($!)
    done
    fail=0
    for p in "${pids[@]}"; do wait "$p" || fail=1; done
    [ "$fail" -eq 0 ] || { echo "a shard failed; see $OUT/shard-*.log (re-run to resume)" >&2; exit 1; }
    rec merge --corpus "$CORPUS_ARG" --out "$OUT" --sample "$SAMPLE" --shards "$N"
    if [ "${SKIP_PERF:-0}" != "1" ]; then
      mkdir -p "$OUT/perf"; CORPUS="$CORPUS_ARG" "$PY" -s "$CHECKS/lib/perf.py" --python --out "$OUT/perf/python.json" >/dev/null
    fi
    echo "goldens built in $OUT";;
esac
