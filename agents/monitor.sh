#!/usr/bin/env sh
# govfolio monitor — read-only dashboard over the loop's artifacts. Safe to run
# alongside the loop: it only reads. Refresh: 15s. Stop: Ctrl-C.
set -eu
cd "$(dirname "$0")/.."
while :; do
  clear
  echo "== govfolio monitor | $(date -u +%FT%TZ) | branch: $(git rev-parse --abbrev-ref HEAD) =="
  echo
  echo "-- JOURNAL (last 10 iterations) --"
  tail -n 10 agents/JOURNAL.md 2>/dev/null || echo "(no iterations yet)"
  echo
  echo "-- COMMITS (last 10) --"
  git log --oneline -10
  echo
  echo "-- WAITING ON YOU (BLOCKED human gates) --"
  grep -rn -A2 "BLOCKED (human)" agents/goals/ 2>/dev/null | grep -v "(empty)" | head -20 || echo "(none)"
  echo
  echo "-- GOAL QUEUE (next 8) --"
  grep -m 8 "^- \[" agents/goals/000-INDEX.md
  echo
  echo "-- LANES (live jurisdiction leases; goal 097) --"
  cargo run -q -p worker --bin jurisdiction-lease -- status 2>/dev/null || echo "(unavailable: DATABASE_URL/pg down or bin not built)"
  for lanelog in agents/loop.lane-*.log; do
    [ -f "$lanelog" ] || continue
    echo "--- $lanelog (last 3) ---"
    tail -n 3 "$lanelog"
  done
  sleep 15
done
