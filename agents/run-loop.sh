#!/usr/bin/env sh
# Compatibility launcher for the pre-built Rust supervisor (goal 108).
# Usage remains: ./agents/run-loop.sh [effort] [model]
set -eu
cd "$(dirname "$0")/.."

EFFORT="${1:-max}"
MODEL="${2:-}"
export CLAUDE_CODE_EFFORT_LEVEL="$EFFORT"
[ -z "$MODEL" ] || export GOVFOLIO_LOOP_MODEL="$MODEL"

# One host supervisor owns lane 0 plus explicitly configured factory worktrees.
# GOVFOLIO_LANES includes orchestrator-0; each factory worktree remains required.
lanes="${GOVFOLIO_LANES:-1}"
if [ "$lanes" -lt 1 ]; then
  echo "ERROR: GOVFOLIO_LANES must be at least 1" >&2
  exit 1
fi
if [ -z "${GOVFOLIO_FACTORY_LANES:-}" ]; then
  GOVFOLIO_FACTORY_LANES=$((lanes - 1))
  export GOVFOLIO_FACTORY_LANES
fi

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
  echo "Build explicitly: cargo build -p loop-supervisor --bin govfolio-loop" >&2
  exit 1
fi

exec "$bin" run
