#!/usr/bin/env sh
# govfolio loop runner — start the Ralph loop with one command.
# Usage: ./agents/run-loop.sh [effort] [model]
#   effort: low|medium|high|xhigh|max     (default: max — founder decision, 2026-07-04)
#   model : optional Claude Code model name/alias passed to --model
# Env toggles:
#   GOVFOLIO_SKIP_PERMS=0  -> re-enable permission prompts (default 1: skip, per
#                             founder requirement 2026-07-04; -p sessions stall on
#                             prompts, so skipping is load-bearing for unattended loops)
#
# Safety model with permissions skipped (enforcement moves OFF the harness):
#  1. ISOLATION: run on a dedicated VM/devcontainer, never a daily-use machine.
#  2. CREDENTIALS: this environment holds ONLY a repo-scoped git token + Claude login.
#     No GCP / Stripe / prod secrets — human-only lanes keep those elsewhere by design.
#  3. REMOTE ENFORCEMENT: protect main on the git host (server rejects force/direct pushes).
#  Note: recent Claude Code versions refuse --dangerously-skip-permissions under root/sudo.

set -eu
cd "$(dirname "$0")/.."

EFFORT="${1:-max}"
MODEL="${2:-}"
SKIP="${GOVFOLIO_SKIP_PERMS:-1}"
export CLAUDE_CODE_EFFORT_LEVEL="$EFFORT"

command -v claude >/dev/null 2>&1 || { echo "ERROR: claude CLI not found (npm i -g @anthropic-ai/claude-code)"; exit 1; }
[ -f agents/PROMPT.md ] || { echo "ERROR: agents/PROMPT.md missing — run from the govfolio repo"; exit 1; }

BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" = "main" ] || [ "$BRANCH" = "master" ]; then
  echo "NOTICE: on $BRANCH — switching to loop/main (agents never commit to main)"
  git checkout -B loop/main
fi

PERM_FLAG=""
[ "$SKIP" = "1" ] && PERM_FLAG="--dangerously-skip-permissions"

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
echo " Stop: Ctrl-C during the 5s gap. State/memory: the repo (JOURNAL.md, goals)."
echo "=============================================================="

i=0
while :; do
  i=$((i+1))
  echo ""
  echo "---- iteration $i | $(date -u +%FT%TZ) | effort=$EFFORT ----"
  if [ -n "$MODEL" ]; then
    cat agents/PROMPT.md | claude -p $PERM_FLAG --model "$MODEL"
  else
    cat agents/PROMPT.md | claude -p $PERM_FLAG
  fi
  sleep 5
done
