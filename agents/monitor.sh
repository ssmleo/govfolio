#!/usr/bin/env sh
# Read-only SQLite/receipt monitor. Never invokes cargo and never scrapes logs.
set -eu
cd "$(dirname "$0")/.."

repo=$(pwd)
target_dir="${CARGO_TARGET_DIR:-$repo/target}"
case "$target_dir" in
  /* | [A-Za-z]:*) ;;
  *) target_dir="$repo/$target_dir" ;;
esac
bin="$target_dir/debug/govfolio-loop"
[ -x "$bin" ] || bin="$bin.exe"
if [ ! -x "$bin" ]; then
  echo "ERROR: pre-built supervisor missing at $bin" >&2
  exit 1
fi

interval="${GOVFOLIO_MONITOR_INTERVAL:-15}"
while :; do
  clear 2>/dev/null || true
  "$bin" status
  sleep "$interval"
done
