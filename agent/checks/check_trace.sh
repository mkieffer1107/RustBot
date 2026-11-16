#!/usr/bin/env bash
# G4b: trace
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
exec $PY -s "$CHECKS/lib/check_trace.py" "$@"
