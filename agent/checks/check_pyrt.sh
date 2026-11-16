#!/usr/bin/env bash
# G1: regex/str/format/random parity (rustbot pyrt-check)
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
exec $PY -s "$CHECKS/lib/check_pyrt.py"  "$@"
