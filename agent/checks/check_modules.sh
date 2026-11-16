#!/usr/bin/env bash
# G2: every observe/respond/after/choose call (--functions, --module NAME, --first-failure)
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
exec $PY -s "$CHECKS/lib/check_modules.py"  "$@"
