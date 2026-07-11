#!/usr/bin/env sh
# Read-only dual dashboard: fenced supervisor state plus semantic loop board.
set -eu
cd "$(dirname "$0")/.."

repo=$(pwd)
target_dir="${CARGO_TARGET_DIR:-$repo/target}"
case "$target_dir" in
  /* | [A-Za-z]:*) ;;
  *) target_dir="$repo/$target_dir" ;;
esac

supervisor="$target_dir/debug/govfolio-loop"
board="$target_dir/debug/loop-board"
[ -x "$supervisor" ] || supervisor="$supervisor.exe"
[ -x "$board" ] || board="$board.exe"
for bin in "$supervisor" "$board"; do
  [ -x "$bin" ] || {
    echo "ERROR: pre-built monitor dependency missing at $bin" >&2
    echo "Build explicitly: cargo build -p loop-supervisor --bin govfolio-loop && cargo build -p worker --bin loop-board" >&2
    exit 1
  }
done

export DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5433/govfolio}"
interval="${GOVFOLIO_MONITOR_REFRESH:-${GOVFOLIO_MONITOR_INTERVAL:-15}}"
while :; do
  clear 2>/dev/null || true
  "$supervisor" status
  printf '\n'
  "$board" || echo "(loop-board failed — see stderr)"
  sleep "$interval"
done
