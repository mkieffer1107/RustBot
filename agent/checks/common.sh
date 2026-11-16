# Sourced by every check script. Paths come from the script location (the repo may live anywhere, e.g. /work/repo).
CHECKS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$CHECKS/../.." && pwd)"
export PYTHONDONTWRITEBYTECODE=1
export GOLDENS="${GOLDENS:-$REPO/agent/goldens}"
export CHECK_OUT="${CHECK_OUT:-$REPO/agent/runs/checks}"
export CORPUS="${CORPUS:-$CHECKS/corpus/dev.jsonl}"
export WASMBOT_DIR="${WASMBOT_DIR:-$REPO/wasmbot}"
[ -n "${RUSTBOT_BIN:-}" ] && export RUSTBOT_BIN
export PYBOT_SEED=0 PYBOT_CLOCK="${PYBOT_CLOCK:-2026-06-15T12:00:00}"
mkdir -p "$CHECK_OUT" 2>/dev/null || true
PY="${PYTHON:-python3}"
# --quick on any script (or CHECK_QUICK=1) = deterministic 10% sample of the conversations
args=()
for a in "$@"; do
  if [ "$a" = "--quick" ]; then export CHECK_QUICK_PERCENT=10; else args+=("$a"); fi
done
[ "${CHECK_QUICK:-0}" = "1" ] && export CHECK_QUICK_PERCENT=10
set -- ${args[@]+"${args[@]}"}
