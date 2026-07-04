#!/usr/bin/env sh
# Halt terraform apply if it would destroy/replace more than DESTROY_BUDGET resources.
set -eu
PLAN_JSON="${1:-tfplan.json}"
BUDGET="${DESTROY_BUDGET:-2}"
[ -f "$PLAN_JSON" ] || { echo "no plan json ($PLAN_JSON); run: terraform show -json tfplan > $PLAN_JSON"; exit 1; }
DELETES=$(grep -o '"actions":\[[^]]*"delete"[^]]*\]' "$PLAN_JSON" | wc -l | tr -d ' ')
if [ "$DELETES" -gt "$BUDGET" ]; then
  echo "BLOCKED: plan destroys/replaces $DELETES resources (budget $BUDGET) -> auto-halt to work item."
  exit 1
fi
echo "tf plan within destroy budget ($DELETES <= $BUDGET): safe to auto-apply"
