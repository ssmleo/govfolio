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
- [x] modules  - [ ] plan clean (HALT below)  - [x] deploy workflow  - [x] runbook

## Guardrail (auto, fail closed)
apply: auto within `DESTROY_BUDGET` via `scripts/check-tf-plan.sh`; over budget halts to a work item.

## HALT (interactive auth) — 2026-07-05
`terraform plan` cannot run: host has gcloud user auth but no Application Default
Credentials, and minting them is browser-interactive (founder-only). Plan attempt
(after `init -backend=false`) halts at backend init; binding the real backend
(`init -backend-config="bucket=govfolio-terraform-state"`) fails with the exact error:

```
Error: storage.NewClient() failed: dialing: credentials: could not find default credentials. See https://cloud.google.com/docs/authentication/external/set-up-adc for more information
```

Founder must run, once, interactively:

```bash
gcloud auth application-default login
```

then follow the bootstrap sequence in `docs/runbooks/deploy.md` (state bucket → init
backend → plan → `check-tf-plan.sh` → first apply → secret versions → WIF repo vars).
No service-account keys were (or may be) created as a workaround; no plan was faked.
Everything runnable without auth is green: `fmt -check`, `init -backend=false`,
`validate` (provider hashicorp/google v6.50.0, terraform 1.15.7) — locally and in CI.
`plan clean` stays unticked until the founder unblocks ADC.
