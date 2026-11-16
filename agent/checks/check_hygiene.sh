#!/usr/bin/env bash
# G7: no interpreter crates, no embedded replies, clippy, tests
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
exec $PY -s "$CHECKS/lib/check_hygiene.py"  "$@"
