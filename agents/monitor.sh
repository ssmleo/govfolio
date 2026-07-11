#!/usr/bin/env sh
# govfolio monitor — thin refresh loop over the read-only `loop-board` bin.
# Safe to run alongside the loop: loop-board only reads (DB/git/files/procs).
# Refresh: GOVFOLIO_MONITOR_REFRESH seconds (default 15). Stop: Ctrl-C.
set -eu
cd "$(dirname "$0")/.."
REFRESH="${GOVFOLIO_MONITOR_REFRESH:-15}"
export DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5433/govfolio}"
while :; do
  clear
  cargo run -q -p worker --bin loop-board || echo "(loop-board failed — see stderr)"
  sleep "$REFRESH"
done
