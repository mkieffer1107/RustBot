#!/usr/bin/env bash
# G6: the Space in headless Chromium
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
exec $PY -s "$CHECKS/lib/check_space.py"  "$@"
