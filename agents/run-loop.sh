#!/usr/bin/env sh
# govfolio loop runner — start the Ralph loop with one command.
# Usage: ./agents/run-loop.sh [effort] [model]
#   effort: low|medium|high|xhigh|max   (default: max — founder decision, 2026-07-04)
#   model : optional Claude Code model name/alias passed to --model
#           (not hardcoded: verify Fable 5's alias in your CLI via /model first)
#
# Why each line exists:
# - env var is THE only way max persists across the loop's fresh sessions
#   (settings files reject max; /effort max dies with its session).
# - branch guard: governance says agents never commit to main directly.
# - sleep 5: your Ctrl-C window between iterations.
# - max warning: uncapped token spend must be visible, never silent.

set -eu
cd "$(dirname "$0")/.."   # anchor at repo root so PROMPT.md resolves from anywhere

EFFORT="${1:-max}"
MODEL="${2:-}"
export CLAUDE_CODE_EFFORT_LEVEL="$EFFORT"

command -v claude >/dev/null 2>&1 || { echo "ERROR: claude CLI not found (npm i -g @anthropic-ai/claude-code)"; exit 1; }
[ -f agents/PROMPT.md ] || { echo "ERROR: agents/PROMPT.md missing — run from the govfolio repo"; exit 1; }

BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" = "main" ] || [ "$BRANCH" = "master" ]; then
  echo "NOTICE: on $BRANCH — switching to loop/main (agents never commit to main)"
  git checkout -B loop/main
fi

echo "=============================================================="
echo " govfolio loop | effort=$EFFORT (env var: survives every fresh session)"
[ -n "$MODEL" ] && echo " model=$MODEL (passed via --model)"
[ "$EFFORT" = "max" ] && echo " WARNING: max = no token ceiling. Unattended = fastest possible spend."
echo " Stop: Ctrl-C during the 5s gap. State/memory: the repo (JOURNAL.md, goals)."
echo "=============================================================="

i=0
while :; do
  i=$((i+1))
  echo ""
  echo "---- iteration $i | $(date -u +%FT%TZ) | effort=$EFFORT ----"
  if [ -n "$MODEL" ]; then
    cat agents/PROMPT.md | claude -p --model "$MODEL"
  else
    cat agents/PROMPT.md | claude -p
  fi
  sleep 5
done
