#!/usr/bin/env bash
# Fetch the read-only inputs at pinned versions. Run from anywhere.
set -euo pipefail
cd "$(dirname "$0")"
PYBOT_COMMIT=7d63c7fbebb44c7e466e5c8b5c1f341ab767929e
if [ ! -d pybot/.git ]; then git clone -q https://github.com/huggingface/pybot pybot; fi
git -C pybot fetch -q --depth 1 origin "$PYBOT_COMMIT" 2>/dev/null || true
git -C pybot checkout -q "$PYBOT_COMMIT"
mkdir -p smoltalkbench
[ -f smoltalkbench/test.parquet ] || curl -sfL \
  https://huggingface.co/datasets/HuggingFaceSwarms/SmolTalkBench/resolve/main/gold/test-00000-of-00001.parquet \
  -o smoltalkbench/test.parquet
echo "pybot $(git -C pybot rev-parse --short HEAD); smoltalkbench $(wc -c < smoltalkbench/test.parquet) bytes"
