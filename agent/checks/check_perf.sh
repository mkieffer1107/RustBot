#!/usr/bin/env bash
# G4: native performance vs CPython PyBot (use --use-golden to skip re-measuring CPython)
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
exec $PY -s "$CHECKS/lib/check_perf.py"  "$@"
