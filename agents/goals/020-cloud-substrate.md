# 020 — cloud substrate

## Objective
Terraform GCP project: Cloud SQL PG (PITR), GCS bronze+exports (versioned), Cloud Run api/web/worker (scale-to-zero), Cloud Tasks queues per stage, Scheduler cadences, Secret Manager; GitHub Actions deploy on main; Cloudflare DNS/CDN/WAF notes.

## Scope
In: infra/ + deploy workflow + runbook docs/runbooks/deploy.md. Out: alerts infra (030), Stripe (050).

## Context (read first)
- design §3, §8 · CLAUDE.md human-only lanes

## Acceptance criteria
```bash
terraform -chdir=infra fmt -check && terraform -chdir=infra validate
terraform -chdir=infra plan   # human reviews; APPLY IS HUMAN-ONLY
```

## Checklist
- [ ] modules  - [ ] plan clean  - [ ] deploy workflow  - [ ] runbook

## BLOCKED (human)
(apply)
