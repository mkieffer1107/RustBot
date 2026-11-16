#!/usr/bin/env bash
# G5: wasmbot replay in node reproduces every turn; .wasm gzip size <= 15 MB
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
rc=0
$PY -s "$CHECKS/lib/check_e2e.py" --runner wasm --name wasm "$@" || rc=$?
if [ "$rc" -ne 2 ]; then
  rc2=0; $PY -s "$CHECKS/lib/check_wasm_size.py" || rc2=$?
  [ "$rc" -eq 0 ] && rc=$rc2
else
  $PY -s "$CHECKS/lib/check_wasm_size.py" || true
fi
exit "$rc"
