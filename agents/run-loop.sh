#!/usr/bin/env sh
# govfolio loop runner — start the Ralph loop with one command.
# Usage: ./agents/run-loop.sh [effort] [model]
#   effort: low|medium|high|xhigh|max     (default: max — founder decision, 2026-07-04)
#   model : optional Claude Code model name/alias passed to --model
# Env toggles:
#   GOVFOLIO_SKIP_PERMS=0  -> re-enable permission prompts (default 1: skip, per
#                             founder requirement 2026-07-04; -p sessions stall on
#                             prompts, so skipping is load-bearing for unattended loops)
#   GOVFOLIO_LANES=N       -> parallel lanes (goal 097; default 1 = classic single loop).
#                             Lane 0 (foreground) = full orchestration (agents/PROMPT.md).
#                             Lanes 1..N-1 (background) = factory-only jurisdiction
#                             workers (agents/PROMPT-FACTORY-LANE.md), each in its own
#                             git worktree under GOVFOLIO_LANES_DIR (default
#                             ../govfolio-lanes/lane-<n>), branch lane/<n>, log
#                             agents/loop.lane-<n>.log. They coordinate via the atomic
#                             jurisdiction lease (jurisdiction-lease bin) and share one
#                             durable Bronze parent (GOVFOLIO_BRONZE_ROOT, exported as
#                             this checkout's target/ unless already set).
#   GOVFOLIO_EPOCH=En      -> epoch for factory lanes (default E2). Lane startup
#                             pre-flight: while the epoch gate is red the lane re-checks
#                             hourly (GOVFOLIO_LANE_SLEEP_RED, default 3600s) WITHOUT
#                             spending a claude session. NOTE the gate is expensive by
#                             design (goal 016: it scores rust-builder by running the
#                             REAL fmt/clippy/test/conformance gates — minutes per run,
#                             cold worktree = full workspace compile), which is exactly
#                             why it runs once-until-green here and NOT per iteration;
#                             once green, the in-session workflow step 2 is the
#                             authoritative fail-closed check per iteration.
#
# Safety model with permissions skipped (enforcement moves OFF the harness):
#  1. ISOLATION: run on a dedicated VM/devcontainer, never a daily-use machine.
#  2. CREDENTIALS: this environment holds ONLY a repo-scoped git token + Claude login.
#     No GCP / Stripe / prod secrets — human-only lanes keep those elsewhere by design.
#  3. REMOTE ENFORCEMENT: protect main on the git host (server rejects force/direct pushes).
#  Note: recent Claude Code versions refuse --dangerously-skip-permissions under root/sudo.
#  Windows Ctrl-C caveat: a mid-iteration claude.exe may finish its single -p turn
#  before dying; its lease either releases normally or goes stale (>24h) and is
#  reclaimed — self-healing, no manual cleanup needed.

set -eu
cd "$(dirname "$0")/.."

EFFORT="${1:-max}"
MODEL="${2:-}"
SKIP="${GOVFOLIO_SKIP_PERMS:-1}"
LANES="${GOVFOLIO_LANES:-1}"
ROOT=$(pwd)
LANES_DIR="${GOVFOLIO_LANES_DIR:-$ROOT/../govfolio-lanes}"
EPOCH="${GOVFOLIO_EPOCH:-E2}"
export CLAUDE_CODE_EFFORT_LEVEL="$EFFORT"
# One shared durable-Bronze parent across every lane/worktree (invariant 2;
# see pipeline::conformance::durable_bronze_parent and the JOURNAL 2026-07-09
# front_b Bronze-gap incident).
export GOVFOLIO_BRONZE_ROOT="${GOVFOLIO_BRONZE_ROOT:-$ROOT/target}"
# Lanes need the shared local registry for the lease (leases live in Postgres).
export DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5433/govfolio}"

command -v claude >/dev/null 2>&1 || { echo "ERROR: claude CLI not found (npm i -g @anthropic-ai/claude-code)"; exit 1; }
[ -f agents/PROMPT.md ] || { echo "ERROR: agents/PROMPT.md missing — run from the govfolio repo"; exit 1; }

BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" = "main" ] || [ "$BRANCH" = "master" ]; then
  echo "NOTICE: on $BRANCH — switching to loop/main (agents never commit to main)"
  git checkout -B loop/main
fi

PERM_FLAG=""
[ "$SKIP" = "1" ] && PERM_FLAG="--dangerously-skip-permissions"
VERB_FLAG=""
[ "${GOVFOLIO_VERBOSE:-0}" = "1" ] && VERB_FLAG="--verbose"

# One factory lane: own worktree + branch, epoch-gate zero-spend pre-flight,
# then PROMPT-FACTORY-LANE.md through a fresh claude -p session, forever.
run_factory_lane() { # $1 = lane number
  n="$1"
  wt="$LANES_DIR/lane-$n"
  log="$ROOT/agents/loop.lane-$n.log"
  if [ ! -d "$wt" ]; then
    mkdir -p "$LANES_DIR"
    git worktree add -B "lane/$n" "$wt" HEAD \
      || { echo "lane-$n: worktree add failed (branch lane/$n checked out elsewhere?)"; return 1; }
  fi
  [ -f "$wt/agents/PROMPT-FACTORY-LANE.md" ] \
    || { echo "lane-$n: stale worktree $wt — merge main into lane/$n first"; return 1; }
  # Startup pre-flight: hold here (hourly re-check, zero claude spend) until
  # the epoch gate is green. Not per-iteration — see the GOVFOLIO_EPOCH note
  # in the header; in-session workflow step 2 owns the per-iteration check.
  until (cd "$wt" && cargo run -q -p pipeline --bin epoch-gate -- "$EPOCH" >/dev/null 2>&1); do
    echo "$(date -u +%FT%TZ) lane-$n: epoch gate $EPOCH red — sleeping $((${GOVFOLIO_LANE_SLEEP_RED:-3600}))s (zero claude spend)" >>"$log"
    sleep "${GOVFOLIO_LANE_SLEEP_RED:-3600}"
  done
  echo "$(date -u +%FT%TZ) lane-$n: epoch gate $EPOCH green — starting sessions" >>"$log"
  while :; do
    if [ -n "$MODEL" ]; then
      (cd "$wt" && GOVFOLIO_LANE_ID="lane-$n" \
        cat agents/PROMPT-FACTORY-LANE.md | claude -p $PERM_FLAG $VERB_FLAG --model "$MODEL") >>"$log" 2>&1 || true
    else
      (cd "$wt" && GOVFOLIO_LANE_ID="lane-$n" \
        cat agents/PROMPT-FACTORY-LANE.md | claude -p $PERM_FLAG $VERB_FLAG) >>"$log" 2>&1 || true
    fi
    sleep "${GOVFOLIO_LANE_SLEEP:-30}"
  done
}

echo "=============================================================="
echo " govfolio loop | effort=$EFFORT (env var: survives every fresh session)"
[ -n "$MODEL" ] && echo " model=$MODEL (passed via --model)"
if [ "$SKIP" = "1" ]; then
  echo " PERMISSIONS: SKIPPED (autonomous). Harness backstop is OFF —"
  echo "   isolation + credential hygiene + protected main are your enforcement."
  echo "   Human-only lanes remain PROMPT-enforced (agents stop and ask in-goal)."
else
  echo " PERMISSIONS: prompting (GOVFOLIO_SKIP_PERMS=0)"
fi
[ "$EFFORT" = "max" ] && echo " WARNING: max = no token ceiling. Unattended = fastest possible spend."
if [ "$LANES" -gt 1 ]; then
  echo " LANES: $LANES (lane 0 = orchestration here; lanes 1..$((LANES-1)) = factory,"
  echo "   epoch $EPOCH, worktrees $LANES_DIR/lane-<n>, logs agents/loop.lane-<n>.log)"
fi
echo " Monitor: ./agents/monitor.sh in another terminal. Raw log: agents/loop.log"
echo " Stop: Ctrl-C during the 5s gap. State/memory: the repo (JOURNAL.md, goals)."
echo "=============================================================="

LANE_PIDS=""
if [ "$LANES" -gt 1 ]; then
  trap '[ -n "$LANE_PIDS" ] && { echo "reaping lanes:$LANE_PIDS"; kill $LANE_PIDS 2>/dev/null || true; }; wait; exit 0' INT TERM
  n=1
  while [ "$n" -lt "$LANES" ]; do
    run_factory_lane "$n" &
    LANE_PIDS="$LANE_PIDS $!"
    n=$((n+1))
  done
fi

i=0
while :; do
  i=$((i+1))
  echo ""
  echo "---- iteration $i | $(date -u +%FT%TZ) | effort=$EFFORT ----"
  if [ -n "$MODEL" ]; then
    cat agents/PROMPT.md | claude -p $PERM_FLAG $VERB_FLAG --model "$MODEL" 2>&1 | tee -a agents/loop.log
  else
    cat agents/PROMPT.md | claude -p $PERM_FLAG $VERB_FLAG 2>&1 | tee -a agents/loop.log
  fi
  sleep 5
done
