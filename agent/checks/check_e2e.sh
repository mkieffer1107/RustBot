#!/usr/bin/env bash
# G3: whole-turn parity on the corpus (reply, module, candidates, memory)
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
exec $PY -s "$CHECKS/lib/check_e2e.py"  "$@"
