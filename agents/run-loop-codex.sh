#!/usr/bin/env sh
# Codex compatibility launcher. The Rust supervisor exclusively owns provider
# invocation, process trees, fencing, retries, and failover (goal 108).
set -eu
cd "$(dirname "$0")/.."

ROOT=$(pwd)
LANES="${GOVFOLIO_LANES:-1}"
LANES_DIR="${GOVFOLIO_LANES_DIR:-$ROOT/../govfolio-codex-lanes}"
PREFLIGHT_ONLY="${GOVFOLIO_CODEX_PREFLIGHT_ONLY:-0}"

case "$LANES" in
  ''|*[!0-9]*|0) echo "ERROR: GOVFOLIO_LANES must be a positive integer" >&2; exit 2 ;;
esac
case "$PREFLIGHT_ONLY" in
  0|1) ;;
  *) echo "ERROR: GOVFOLIO_CODEX_PREFLIGHT_ONLY must be 0 or 1" >&2; exit 2 ;;
esac

validate_contract() {
  worktree=$1
  for path in AGENTS.md agents/skill-routing.json \
    scripts/agents/render-codex-contract.mjs \
    scripts/agents/validate-codex-contract.mjs; do
    [ -f "$worktree/$path" ] || {
      echo "ERROR: tracked Codex contract asset missing in $worktree: $path" >&2
      return 1
    }
  done
  node "$worktree/scripts/agents/render-codex-contract.mjs" --check --repo-root "$worktree"
  node "$worktree/scripts/agents/validate-codex-contract.mjs" --repo-root "$worktree"
}

validate_contract "$ROOT"
n=1
while [ "$n" -lt "$LANES" ]; do
  worktree="$LANES_DIR/lane-$n"
  [ -d "$worktree" ] || {
    echo "ERROR: configured Codex lane worktree missing: $worktree" >&2
    exit 1
  }
  validate_contract "$worktree"
  n=$((n + 1))
done

if [ "$PREFLIGHT_ONLY" = "1" ]; then
  echo "Codex contract preflight passed for the primary checkout and configured lane worktrees."
  exit 0
fi

export GOVFOLIO_LOOP_PROVIDER=codex
exec "$ROOT/agents/run-loop.sh" "$@"
