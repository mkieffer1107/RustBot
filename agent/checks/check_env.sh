#!/usr/bin/env bash
# G0: toolchains, pinned reference commit, goldens present
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
exec $PY -s "$CHECKS/lib/check_env.py"  "$@"
