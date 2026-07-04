# Runbook — cloud computing & deploy

**Read this before any deploy or infra work**: `terraform`, Cloud Run / GKE deploys,
Cloud SQL / prod migrations, GCS, Secret Manager, billing-affecting changes, or driving the
`gcloud` / `cloud-run` MCP tools. Not needed for pure data-plane or web code.

Authority for gate behavior is `docs/decisions/automation-policy.md` (founder decision
2026-07-04). Where the CLAUDE.md "Human-only lanes" text still reads *stop and ask* for
`terraform apply` / prod migrations, the automation policy supersedes it: those gates are
**automated behind mechanical, fail-closed guardrails** — no human gate, but never proceed
on ambiguity (halt → file a goal).

## Tooling inventory

| Tool | Invocation | Use |
|------|-----------|-----|
| `gcloud` MCP | `@google-cloud/gcloud-mcp` (stdio) | Any gcloud surface; permissions = active gcloud account |
| `cloud-run` MCP | `@google-cloud/cloud-run-mcp` (stdio) | Deploy/list/log Cloud Run services |
| `toolbox` MCP | `toolbox --prebuilt postgres --stdio` | Query pg (local dev = `localhost:5433/govfolio`); `execute_sql` runs arbitrary SQL |
| `gcloud` CLI | `gcloud …` | Auth, project config, anything the MCP can't reach |
| `terraform` | `terraform -chdir=infra …` | All infra; remote state + locking + versioning |

GCP domain skills live under `.agents/skills/` — load the matching one before acting:
`google-agents-cli-deploy` (Cloud Run / GKE / Agent Runtime / CI-CD / secrets / rollback,
with `references/{cloud-run,gke,terraform-patterns,cicd-pipeline}.md`), `cloud-run-basics`,
`alloydb-basics`, `bigquery-basics`, and the `gke-*` skills.

## Auth prerequisites (do once per machine/session)

```bash
gcloud auth login                         # user creds (interactive browser)
gcloud auth application-default login     # ADC for libraries / terraform
gcloud config set project <PROJECT_ID>
```

The `gcloud` MCP hard-fails at startup with `gcloud executable not found` until the gcloud
SDK is on the launching process's PATH. If gcloud was installed after the agent host
started, **restart the host** (stale PATH snapshot) — the SDK on User PATH is not enough.
`cloud-run` MCP connects without gcloud but its tools are inert until auth is done.

## Guardrails (fail closed — halt to work item, never proceed on ambiguity)

**1. Prod migrations — auto-apply only if expand-only.** Destructive DDL is rejected.
```bash
scripts/check-migration-safety.sh crates/core/migrations   # DROP/TRUNCATE/ALTER…DROP -> exit 1
# on pass: mandatory pre-apply snapshot, then apply. On fail: convert to a reviewed work item.
```

**2. `terraform apply` — auto only within `DESTROY_BUDGET` (default 2) destroys/replaces.**
```bash
terraform -chdir=infra plan -out=tfplan
terraform -chdir=infra show -json tfplan > tfplan.json
DESTROY_BUDGET=2 scripts/check-tf-plan.sh tfplan.json       # deletes > budget -> exit 1 (halt)
terraform -chdir=infra apply tfplan                          # only if the check passed
```

**3. Billing / money — auto only within the hard cap** (monthly ceiling + per-action limit).
Over cap → halt. Any billing-affecting change (new services, larger tiers, egress) counts.

A halt files a goal and the loop continues other work. Ambiguity is a halt, not a guess.

## Deploy flow (target infra — goal `agents/goals/020-cloud-substrate.md`, design §3/§8)

Terraform GCP: Cloud SQL PG (PITR), GCS bronze+exports (versioned), Cloud Run api/web/worker
(scale-to-zero), Cloud Tasks per stage, Scheduler cadences, Secret Manager. GitHub Actions
deploys on `main`; Cloudflare fronts DNS/CDN/WAF.

1. `terraform -chdir=infra fmt -check && terraform -chdir=infra validate`
2. plan → `check-tf-plan.sh` → apply (within budget)
3. Cloud Run deploy via `cloud-run` MCP or `gcloud run deploy`; verify `/health` + logs
4. Secrets via Secret Manager only — never commit or echo secret values

## Non-negotiables

- **Never** hand-edit remote terraform state; **never** disable state locking/versioning.
- Prod DB obeys the domain invariants (supersede-never-update, raw immutable): no ad-hoc
  `execute_sql` writes against prod through the toolbox MCP. Local dev DB only.
- Least privilege: deploy service accounts get exactly their scopes; no owner roles.
- Politeness (invariant #10) carries to any egress: identified UA, conditional GETs,
  per-source min-interval, concurrency 1 default.
