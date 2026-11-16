#!/usr/bin/env bash
# Proves the checks work: a "RustBot" that is PyBot itself passes, injected faults fail (naming conv/turn), goldens are
# deterministic and independent of the shard count, missing pieces exit 2.   Usage: selftest/run.sh [--keep]
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECKS="$(dirname "$HERE")"; REPO="$(cd "$CHECKS/../.." && pwd)"
ST="${SELFTEST_DIR:-$REPO/agent/runs/selftest}"
PER_KIND="${SELFTEST_PER_KIND:-3}"
rm -rf "$ST"; mkdir -p "$ST"
export PYTHONDONTWRITEBYTECODE=1
SLICE="$ST/slice.jsonl"
python3 -s "$HERE/make_slice.py" "${SELFTEST_CORPUS:-$CHECKS/corpus/dev.jsonl}" "$SLICE" "$PER_KIND"
FAKE="$HERE/fake_rustbot.py"
results=(); failures=0
record() { # name expectation actual ok
  results+=("$(printf '%-44s %-14s %-14s %s' "$1" "$2" "$3" "$4")")
  [ "$4" = "ok" ] || failures=$((failures + 1))
}
step() { echo ">>> $*" >&2; }

# ---- goldens: build twice with different shard counts; must be byte-identical
step "building goldens (3 shards)"
SKIP_PERF=1 "$CHECKS/build_goldens.sh" --corpus "$SLICE" --out "$ST/g1" --jobs 3 --extras-every 1 > "$ST/build1.log" 2>&1 \
  && record "build goldens (3 shards)" "exit 0" "exit 0" ok || { record "build goldens (3 shards)" "exit 0" "failed" FAIL; tail -20 "$ST/build1.log"; }
step "building goldens again (2 shards)"
SKIP_PERF=1 "$CHECKS/build_goldens.sh" --corpus "$SLICE" --out "$ST/g2" --jobs 2 --extras-every 1 > "$ST/build2.log" 2>&1 \
  && record "build goldens (2 shards)" "exit 0" "exit 0" ok || record "build goldens (2 shards)" "exit 0" "failed" FAIL
same=ok
for d in e2e hooks functions pyrt trace; do
  diff -r "$ST/g1/$d" "$ST/g2/$d" > "$ST/diff-$d.txt" 2>&1 || same=FAIL
done
sed 's/"shards": [0-9]*//' "$ST/g1/MANIFEST.json" > "$ST/m1"; sed 's/"shards": [0-9]*//' "$ST/g2/MANIFEST.json" > "$ST/m2"
cmp -s "$ST/m1" "$ST/m2" || same=FAIL
record "goldens byte-identical (3 vs 2 shards)" "identical" "$([ $same = ok ] && echo identical || echo DIFFERENT)" "$same"

export GOLDENS="$ST/g1" CORPUS="$SLICE" CHECK_OUT="$ST/out" RUSTBOT_BIN="$FAKE"
mkdir -p "$CHECK_OUT"
run() { # label, expected-exit, script args...
  local label="$1" want="$2"; shift 2
  local log="$ST/$(echo "$label" | tr -c 'A-Za-z0-9' '_').log"
  "$@" > "$log" 2>&1; local rc=$?
  LAST_LOG="$log"; LAST_RC=$rc
  record "$label" "exit $want" "exit $rc" "$([ "$rc" = "$want" ] && echo ok || echo FAIL)"
  [ "$rc" = "$want" ] || tail -5 "$log" >&2
}
names_in_log() { # conv turn [hookprefix]  -> check the log names the divergence
  local label="$1" pat="$2"
  if grep -q -- "$pat" "$LAST_LOG"; then record "$label names the divergence" "'$pat'" found ok
  else record "$label names the divergence" "'$pat'" "not found" FAIL; grep -m3 DIVERGENCE -A1 "$LAST_LOG" >&2; fi
}

step "clean pass"
run "e2e vs fake (no fault)" 0 "$CHECKS/check_e2e.sh"
run "modules vs fake (no fault)" 0 "$CHECKS/check_modules.sh"
CHECK_FUNCTION_PERCENT=3 run "modules --functions vs fake (3% of functions)" 0 "$CHECKS/check_modules.sh" --functions
run "pyrt vs fake (no fault)" 0 "$CHECKS/check_pyrt.sh"
run "modules --module chat vs fake" 0 "$CHECKS/check_modules.sh" --module chat
run "e2e --quick vs fake" 0 "$CHECKS/check_e2e.sh" --quick

fault() { # fault, which-check(e2e|modules|functions|pyrt), expect-exit
  local f="$1" chk="$2" want="$3"
  export FAKE_FAULT="$f" FAKE_FAULT_LOG="$ST/fault-$f-$chk.json"; rm -f "$FAKE_FAULT_LOG"
  case "$chk" in
    modules) run "fault=$f -> check_modules" "$want" "$CHECKS/check_modules.sh";;
    functions) run "fault=$f -> check_modules --functions" "$want" "$CHECKS/check_modules.sh" --functions --function "$FAKE_FAULT_FN";;
    *) run "fault=$f -> check_$chk" "$want" "$CHECKS/check_$chk.sh";;
  esac
  if [ "$want" = 1 ]; then
    if [ -s "$FAKE_FAULT_LOG" ]; then
      read -r conv turn hook < <(python3 -c "import json,sys; d=json.loads(open('$FAKE_FAULT_LOG').readline()); print(d.get('conv','-'), d.get('turn','-'), d.get('hook', d.get('fn', d.get('kind','-'))))")
      case "$chk" in
        e2e|trace) names_in_log "fault=$f/$chk" "conv=$conv turn=$turn";;
        modules) names_in_log "fault=$f/modules" "$hook conv=$conv turn=$turn";;
        functions) names_in_log "fault=$f/functions" "$hook#0";;
        pyrt) names_in_log "fault=$f/pyrt" "pyrt/$hook #0";;
      esac
    else record "fault=$f/$chk injected" "fault log" "empty (not injected)" FAIL; fi
  fi
  unset FAKE_FAULT FAKE_FAULT_LOG
}
export FAKE_FAULT_FN="$(ls "$ST/g1/functions" | head -1 | sed 's/\.jsonl.*//')"
step "fault injection"
fault reply e2e 1;            fault reply modules 0
fault module e2e 1;           fault module modules 0
fault candidate_order e2e 1;  fault candidate_order modules 0
fault memory e2e 1;           fault memory modules 1
fault exception modules 1;    fault exception e2e 1
fault rng modules 1;          fault rng e2e 0
fault pyrt pyrt 1
fault function functions 1

step "trace check"
run "check_trace vs fake (no --trace support yet is MISSING; fake supports it)" 0 "$CHECKS/check_trace.sh"
fault trace trace 1

step "structure check on a throwaway fake tree"
T="$ST/tree"; python3 -s "$HERE/make_fake_tree.py" "$T" 2> "$ST/tree.log"
RUSTBOT_SRC="$T" run "check_structure on complete fake tree" 0 "$CHECKS/check_structure.sh"
first="$(cd "$T" && ls compose/*.rs | head -1)"; cp "$T/$first" "$ST/saved.rs"; rm "$T/$first"
RUSTBOT_SRC="$T" run "structure: a file removed" 1 "$CHECKS/check_structure.sh"
names_in_log "structure/missing file" "have no Rust file"
RUSTBOT_SRC="$T" run "structure --partial: a file removed" 0 "$CHECKS/check_structure.sh" --partial
cp "$ST/saved.rs" "$T/$first"
echo "pub fn stray() {}" > "$T/compose/zz_extra.rs"
RUSTBOT_SRC="$T" run "structure: extra module in compose/" 1 "$CHECKS/check_structure.sh"
names_in_log "structure/extra file" "extra .rs files"
RUSTBOT_SRC="$T" run "structure --partial: extra module in compose/" 1 "$CHECKS/check_structure.sh" --partial
rm "$T/compose/zz_extra.rs"
sed -i.bak '1s/-\([0-9]*\) /-\19 /' "$T/$first"; rm -f "$T/$first.bak"
RUSTBOT_SRC="$T" run "structure: wrong line span in // py:" 1 "$CHECKS/check_structure.sh"
names_in_log "structure/bad span" "is not a function span"
cp "$ST/saved.rs" "$T/$first"; sed -i.bak '1d' "$T/$first"; rm -f "$T/$first.bak"
RUSTBOT_SRC="$T" run "structure: // py: comment deleted" 1 "$CHECKS/check_structure.sh"
names_in_log "structure/no comment" "no \`// py:"
RUSTBOT_SRC="$ST/no_such_dir" run "structure: no Rust tree yet" 2 "$CHECKS/check_structure.sh"

step "missing pieces exit 2"
for c in e2e modules pyrt perf wasm space trace; do
  RUSTBOT_BIN=/nonexistent WASMBOT_DIR="$ST/nowasm" run "check_$c without RustBot" 2 "$CHECKS/check_$c.sh"
  grep -q "^CHECK .* MISSING" "$LAST_LOG" && record "check_$c prints CHECK ... MISSING" "last line" ok ok || record "check_$c prints CHECK ... MISSING" "last line" "absent" FAIL
done
GOLDENS="$ST/empty" run "check_e2e without goldens" 2 "$CHECKS/check_e2e.sh"

step "perf check logic (fake is as slow as CPython, so it must FAIL, not crash)"
PERF_SESSIONS=2 PERF_TURNS=3 run "check_perf vs fake (ratios ~1)" 1 "$CHECKS/check_perf.sh"
grep -q "^CHECK perf FAIL" "$LAST_LOG" && record "check_perf prints CHECK perf FAIL" "last line" ok ok || record "check_perf prints CHECK perf FAIL" "last line" absent FAIL

echo
echo "SELFTEST RESULTS"
printf '%-44s %-14s %-14s %s\n' "check" "expected" "actual" "status"
printf '%s\n' "${results[@]}"
echo "failures: $failures"
if [ "$failures" -eq 0 ]; then echo "SELFTEST PASS"; exit 0; else echo "SELFTEST FAIL"; exit 1; fi
