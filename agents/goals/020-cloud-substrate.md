# 020 — cloud substrate

## Objective
Terraform GCP project: Cloud SQL PG (PITR), GCS bronze+exports (versioned), Cloud Run api/web/worker (scale-to-zero), Cloud Tasks queues per stage, Scheduler cadences, Secret Manager; GitHub Actions deploy on main; Cloudflare DNS/CDN/WAF notes.

## Scope
In: infra/ + deploy workflow + runbook docs/runbooks/deploy.md. Out: alerts infra (030), Stripe (050).

## Context (read first)
- design §3, §8 · `docs/decisions/automation-policy.md` · `docs/runbooks/deploy.md`

## Acceptance criteria
```bash
terraform -chdir=infra fmt -check && terraform -chdir=infra validate
terraform -chdir=infra plan -out=tfplan
terraform -chdir=infra show -json tfplan > tfplan.json
DESTROY_BUDGET=2 scripts/check-tf-plan.sh tfplan.json   # within budget -> auto-apply; over -> halt
```

## Checklist
- [ ] modules  - [ ] plan clean  - [ ] deploy workflow  - [ ] runbook

## Guardrail (auto, fail closed)
apply: auto within `DESTROY_BUDGET` via `scripts/check-tf-plan.sh`; over budget halts to a work item.
