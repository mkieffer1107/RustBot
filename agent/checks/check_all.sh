#!/usr/bin/env bash
# Runs every gate in order and prints a table. --quick: 10% deterministic sample of the corpus.
set -uo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
names=(G0:env G1:pyrt G2:modules G3:e2e G4:perf G4b:structure G4b:trace G5:wasm G6:space G7:hygiene)
rows=(); worst=0
for g in "${names[@]}"; do
  gate="${g%%:*}"; chk="${g##*:}"
  echo "=== $gate $chk"
  log="$CHECK_OUT/$chk.log"
  if [ "$chk" = "modules" ] && [ "${CHECK_QUICK_PERCENT:-}" = "10" ]; then extra=(--quick); else extra=(); fi
  [ "${CHECK_QUICK_PERCENT:-}" = "10" ] && extra=(--quick) || extra=()
  "$CHECKS/check_$chk.sh" ${extra[@]+"${extra[@]}"} 2>&1 | tee "$log" >/dev/null
  rc=${PIPESTATUS[0]}
  tail -n 12 "$log" | grep -v '^CHECK' | head -n 12
  line="$(grep '^CHECK ' "$log" | tail -n 1)"
  [ -z "$line" ] && line="CHECK $chk FAIL no summary line (crashed?)"
  echo "$line"
  rows+=("$gate|$chk|$rc|$line")
  [ "$rc" -gt "$worst" ] && worst=$rc
done
echo
echo "GATE  CHECK     RESULT   SUMMARY"
for r in "${rows[@]}"; do
  IFS='|' read -r gate chk rc line <<<"$r"
  case "$rc" in 0) res=PASS;; 2) res=MISSING;; *) res=FAIL;; esac
  printf '%-5s %-9s %-8s %s\n' "$gate" "$chk" "$res" "${line#CHECK $chk $res }"
done
[ "$worst" -eq 0 ] && echo "ALL GATES PASS" || echo "NOT DONE: some gates FAIL or are MISSING"
exit "$worst"
