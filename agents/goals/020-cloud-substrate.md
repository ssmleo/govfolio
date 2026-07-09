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
- [x] modules  - [x] plan clean  - [x] deploy workflow  - [x] runbook  - [x] first apply (below)

## Guardrail (auto, fail closed)
apply: auto within `DESTROY_BUDGET` via `scripts/check-tf-plan.sh`; over budget halts to a work item.

## Resolved 2026-07-06 — first apply complete
Founder ran ADC (`gcloud auth application-default login`) and confirmed. Bootstrap
sequence executed per `docs/runbooks/deploy.md`: billing linked to project (was
disabled — founder approved linking account `01D5E1-FC1345-98F417`), state bucket
`gs://govfolio-terraform-state` created + versioned, backend bound, `plan` → 82 to
add/0 destroy → `check-tf-plan.sh` (0 <= budget 2) → apply.

Apply ran in two passes: first pass created 80/82 resources then hit a transient GCP
propagation race (`role "cloudsqliamserviceaccount" does not exist` — the Cloud SQL
IAM-auth service role isn't queryable for a short window right after instance create);
second `plan`→`check`→`apply` pass (2 add, 3 cosmetic Cloud Run scaling-default
changes, 0 destroy) created the two IAM DB users cleanly. Live now: Cloud SQL PG16
`govfolio-pg` (PITR on), GCS `govfolio-bronze`/`govfolio-exports` (versioned), Cloud
Run api/web/worker, Artifact Registry, Cloud Tasks x5, Scheduler jobs (paused), Secret
Manager shells, WIF pool+provider. Outputs recorded in terraform state (`sql_connection_name`,
`cloud_run_urls`, `deploy_service_account`, `terraform_service_account`, `wif_provider`).

GitHub repo vars for WIF deploy auth set 2026-07-06 (`GCP_WIF_PROVIDER`, `GCP_DEPLOY_SA`,
`GCP_TF_SA`, `GCP_STATE_BUCKET` — values pulled straight from terraform output, no
ambiguity). `deploy.yml` now runs its real plan/apply path on `main` instead of no-op'ing.

**Still open (human lane, not blocking this goal):**
- `openfigi-api-key` secret VALUE — zero code consumers yet (future entity-matching
  work); no key in agent possession; founder adds via `gcloud secrets versions add`
  when the feature lands.
- `database-url` secret VALUE — genuinely undesigned, not just unset: Cloud SQL has no
  authorized networks (connector/proxy-only, per `sql.tf` comment) and IAM DB auth means
  no static password is correct here. `cloudrun.tf` defers this deliberately ("lands with
  the real images" — see its top comment) because the real answer (Cloud SQL Auth Proxy
  sidecar vs. Cloud Run's native `--add-cloudsql-instances` unix-socket mount, exact
  `postgres://user@/db?host=/cloudsql/...` + auto-IAM-authn wiring) is an architecture
  decision for whoever builds the first real API/worker image, not a value to fabricate
  now. Flag as an open design question for that goal, not a HALT on this one.
- Scheduler jobs created PAUSED — unpause per tier as adapters clear conformance.
This unblocks goal 080's real (write-to-prod) backfill step, which was HALTed on this
exact substrate.
