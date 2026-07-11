#!/usr/bin/env bash
# govfolio Codex loop runner - feature parity with agents/run-loop.sh.
# Usage: ./agents/run-loop-codex.sh [effort] [model]
#
# GOVFOLIO_CODEX_PREFLIGHT_ONLY=1 validates the primary checkout and every
# configured, existing lane worktree without invoking Codex or mutating state.

set -Eeuo pipefail
cd "$(dirname "$0")/.."

REQUESTED_EFFORT="${1:-xhigh}"
MODEL="${2:-}"
SKIP="${GOVFOLIO_SKIP_PERMS:-1}"
LANES="${GOVFOLIO_LANES:-1}"
FACTORY_ONLY="${GOVFOLIO_CODEX_FACTORY_ONLY:-0}"
PREFLIGHT_ONLY="${GOVFOLIO_CODEX_PREFLIGHT_ONLY:-0}"
VERBOSE="${GOVFOLIO_VERBOSE:-0}"
ROOT="$(pwd)"
LANES_DIR="${GOVFOLIO_LANES_DIR:-$ROOT/../govfolio-codex-lanes}"
EPOCH="${GOVFOLIO_EPOCH:-E2}"

case "$REQUESTED_EFFORT" in
  max) EFFORT="xhigh" ;;
  minimal|low|medium|high|xhigh) EFFORT="$REQUESTED_EFFORT" ;;
  *) echo "ERROR: effort must be minimal, low, medium, high, xhigh, or max" >&2; exit 2 ;;
esac
case "$SKIP" in 0|1) ;; *) echo "ERROR: GOVFOLIO_SKIP_PERMS must be 0 or 1" >&2; exit 2 ;; esac
case "$FACTORY_ONLY" in 0|1) ;; *) echo "ERROR: GOVFOLIO_CODEX_FACTORY_ONLY must be 0 or 1" >&2; exit 2 ;; esac
case "$PREFLIGHT_ONLY" in 0|1) ;; *) echo "ERROR: GOVFOLIO_CODEX_PREFLIGHT_ONLY must be 0 or 1" >&2; exit 2 ;; esac
case "$VERBOSE" in 0|1) ;; *) echo "ERROR: GOVFOLIO_VERBOSE must be 0 or 1" >&2; exit 2 ;; esac
case "$LANES" in ''|*[!0-9]*|0) echo "ERROR: GOVFOLIO_LANES must be a positive integer" >&2; exit 2 ;; esac

command -v git >/dev/null 2>&1 || { echo "ERROR: git not found" >&2; exit 1; }
command -v node >/dev/null 2>&1 || { echo "ERROR: node not found" >&2; exit 1; }
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || {
  echo "ERROR: not inside a Git worktree" >&2
  exit 1
}

require_contract_assets() {
  local worktree="$1"
  local path
  for path in \
    AGENTS.md \
    agents/skill-routing.json \
    scripts/agents/codex-contract-lib.mjs \
    scripts/agents/render-codex-contract.mjs \
    scripts/agents/validate-codex-contract.mjs; do
    [ -f "$worktree/$path" ] || {
      echo "ERROR: tracked Codex contract asset missing in $worktree: $path" >&2
      return 1
    }
  done
}

validate_contract() {
  local worktree="$1"
  require_contract_assets "$worktree"
  node "$worktree/scripts/agents/render-codex-contract.mjs" --check --repo-root "$worktree"
  node "$worktree/scripts/agents/validate-codex-contract.mjs" --repo-root "$worktree"
}

if [ "$FACTORY_ONLY" = "1" ]; then
  first_lane=1
  last_lane="$LANES"
else
  first_lane=1
  last_lane=$((LANES - 1))
fi

if [ "$PREFLIGHT_ONLY" = "1" ]; then
  validate_contract "$ROOT"
  n="$first_lane"
  while [ "$n" -le "$last_lane" ]; do
    wt="$LANES_DIR/lane-$n"
    [ -d "$wt" ] || {
      echo "ERROR: configured Codex lane worktree missing: $wt" >&2
      exit 1
    }
    validate_contract "$wt"
    n=$((n + 1))
  done
  echo "Codex contract preflight passed for the primary checkout and configured lane worktrees."
  exit 0
fi

# From this point onward the runner may inspect build tools or mutate runtime state.
command -v codex >/dev/null 2>&1 || {
  echo "ERROR: Codex CLI not found (install it, then run: codex login)" >&2
  exit 1
}
command -v cargo >/dev/null 2>&1 || { echo "ERROR: cargo not found" >&2; exit 1; }
command -v tee >/dev/null 2>&1 || { echo "ERROR: tee not found" >&2; exit 1; }
[ -f agents/PROMPT.md ] || { echo "ERROR: agents/PROMPT.md missing" >&2; exit 1; }
[ -f agents/PROMPT-FACTORY-LANE.md ] || { echo "ERROR: agents/PROMPT-FACTORY-LANE.md missing" >&2; exit 1; }
validate_contract "$ROOT"

export GOVFOLIO_BRONZE_ROOT="${GOVFOLIO_BRONZE_ROOT:-$ROOT/target}"
export DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5433/govfolio}"
export GOVFOLIO_LANE_ID="${GOVFOLIO_LANE_ID:-codex-lane-0}"
if [ ! -d "$GOVFOLIO_BRONZE_ROOT" ]; then
  mkdir -p "$GOVFOLIO_BRONZE_ROOT" || {
    echo "ERROR: cannot create GOVFOLIO_BRONZE_ROOT: $GOVFOLIO_BRONZE_ROOT" >&2
    exit 1
  }
fi

# Codex root flags must precede `exec`; exec flags follow it.
CODEX_ARGS=()
if [ "$SKIP" = "0" ]; then
  CODEX_ARGS+=(
    -a never
    --add-dir "$GOVFOLIO_BRONZE_ROOT"
  )
fi
CODEX_ARGS+=(
  exec
  --ephemeral
  --color never
  --config "model_reasoning_effort=\"$EFFORT\""
  --config 'agents.max_depth=2'
  --config 'agents.max_threads=6'
)
if [ "$SKIP" = "1" ]; then
  CODEX_ARGS+=(--dangerously-bypass-approvals-and-sandbox)
else
  CODEX_ARGS+=(
    --sandbox workspace-write
    --config 'sandbox_workspace_write.network_access=true'
  )
fi
[ "$VERBOSE" = "1" ] && CODEX_ARGS+=(--json)
[ -n "$MODEL" ] && CODEX_ARGS+=(--model "$MODEL")
CODEX_ARGS+=(-)

# This is the only function permitted to invoke Codex. Validation is deliberately
# adjacent to the raw call so no dispatch can bypass the governed projection.
codex_with_contract() {
  local worktree="$1"
  local prompt="$2"
  require_contract_assets "$worktree"
  node "$worktree/scripts/agents/render-codex-contract.mjs" --check --repo-root "$worktree"
  node "$worktree/scripts/agents/validate-codex-contract.mjs" --repo-root "$worktree"
  (
    cd "$worktree"
    printf '%s' "$prompt" | codex "${CODEX_ARGS[@]}"
  )
}

run_factory_lane() {
  local n="$1"
  local wt="$LANES_DIR/lane-$n"
  local branch="codex/lane/$n"
  local lane_id="codex-lane-$n"
  local log="$ROOT/agents/codex-loop.lane-$n.log"
  local gate_out prompt status

  if [ ! -d "$wt" ]; then
    [ -d "$LANES_DIR" ] || mkdir -p "$LANES_DIR"
    if git show-ref --verify --quiet "refs/heads/$branch"; then
      git worktree add "$wt" "$branch" || {
        echo "$lane_id: worktree add failed (branch checked out elsewhere?)" >&2
        return 1
      }
    else
      git worktree add -b "$branch" "$wt" HEAD || {
        echo "$lane_id: worktree add failed" >&2
        return 1
      }
    fi
  fi
  [ -f "$wt/agents/PROMPT-FACTORY-LANE.md" ] || {
    echo "$lane_id: stale worktree $wt - merge the current base into $branch first" >&2
    return 1
  }
  validate_contract "$wt"

  while :; do
    if gate_out="$(cd "$wt" && cargo run -q -p pipeline --bin epoch-gate -- "$EPOCH" 2>&1)"; then
      break
    fi
    {
      echo "$(date -u +%FT%TZ) $lane_id: epoch gate $EPOCH NOT GREEN - sleeping ${GOVFOLIO_LANE_SLEEP_RED:-3600}s (zero Codex spend). Output tail:"
      printf '%s\n' "$gate_out" | tail -n 5 | sed 's/^/    /'
    } >>"$log"
    sleep "${GOVFOLIO_LANE_SLEEP_RED:-3600}"
  done

  echo "$(date -u +%FT%TZ) $lane_id: epoch gate $EPOCH green - starting Codex sessions" >>"$log"
  while :; do
    prompt="$(cat "$wt/agents/PROMPT-FACTORY-LANE.md")"
    if GOVFOLIO_LANE_ID="$lane_id" codex_with_contract "$wt" "$prompt" >>"$log" 2>&1; then
      status=0
    else
      status=$?
    fi
    if [ "$status" -ne 0 ]; then
      echo "$(date -u +%FT%TZ) WARN: $lane_id Codex iteration exited $status" >>"$log"
    fi
    sleep "${GOVFOLIO_LANE_SLEEP:-30}"
  done
}

echo "=============================================================="
echo " govfolio Codex loop | requested effort=$REQUESTED_EFFORT | Codex effort=$EFFORT"
[ -n "$MODEL" ] && echo " model=$MODEL (passed via --model)"
if [ "$SKIP" = "1" ]; then
  echo " PERMISSIONS + SANDBOX: BYPASSED. Run only in an isolated environment."
else
  echo " SANDBOX: workspace-write; network enabled; Bronze root added as writable."
fi
if [ "$FACTORY_ONLY" = "1" ]; then
  echo " MODE: factory-only coexistence; $LANES Codex factory lane(s), no lane 0."
else
  echo " MODE: full parity; lane 0 = Codex orchestration in this checkout."
  [ "$LANES" -gt 1 ] && echo "   Factory lanes: $((LANES - 1))"
fi
echo " FACTORY: epoch=$EPOCH | worktrees=$LANES_DIR/lane-<n>"
echo " SHARED: Bronze=$GOVFOLIO_BRONZE_ROOT | registry=$DATABASE_URL"
echo "=============================================================="

LANE_PIDS=()
cleanup() {
  local exit_status="${1:-0}"
  local pid
  echo
  echo "Stopping Codex loop."
  for pid in "${LANE_PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
  exit "$exit_status"
}
trap 'cleanup 0' INT TERM

if [ "$FACTORY_ONLY" = "0" ]; then
  BRANCH="$(git rev-parse --abbrev-ref HEAD)"
  if [ "$BRANCH" = "main" ] || [ "$BRANCH" = "master" ]; then
    echo "NOTICE: on $BRANCH - switching to codex/loop-main"
    git checkout -B codex/loop-main
  fi
fi

n="$first_lane"
while [ "$n" -le "$last_lane" ]; do
  run_factory_lane "$n" &
  LANE_PIDS+=("$!")
  n=$((n + 1))
done

if [ "$FACTORY_ONLY" = "1" ]; then
  if wait -n; then status=1; else status=$?; fi
  echo "ERROR: a Codex factory lane exited unexpectedly (status $status)" >&2
  cleanup "$status"
fi

i=0
while :; do
  i=$((i + 1))
  prompt="$(cat agents/PROMPT.md)"
  echo
  echo "---- Codex lane 0 iteration $i | $(date -u +%FT%TZ) | effort=$EFFORT ----"
  if codex_with_contract "$ROOT" "$prompt" 2>&1 | tee -a agents/codex-loop.log; then
    status=0
  else
    status=$?
  fi
  if [ "$status" -ne 0 ]; then
    echo "WARN: Codex lane 0 iteration $i exited $status; retrying in 5s" \
      | tee -a agents/codex-loop.log >&2
  fi
  sleep 5
done
