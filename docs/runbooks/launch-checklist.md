# Runbook — launch checklist (v1 go-live)

Tracks everything remaining to launch govfolio.io v1 (design §5.5 SLOs, §5.7 launch
jurisdictions, §7 trust/legal). This is a **living checklist**: each item is tagged with who
can finish it and its current state. It is NOT the go/no-go decision itself — that gate is at
the bottom and is a human lane.

Tags:
- **buildable-done** — mechanically completable by the loop; done (or has a green acceptance).
- **infra-blocked** — needs the cloud substrate applied first (goal 020 HALT: founder runs
  `gcloud auth application-default login` once, then terraform apply). No code gate blocks it;
  the substrate does.
- **human-lane** — the residual human touch with no mechanical guardrail yet (pricing / legal /
  methodology PUBLIC copy, and the final go/no-go). Fail closed until a guardrail exists
  (`/CLAUDE.md` "Residual human touch"; `docs/decisions/automation-policy.md`).

---

## 1. US history backfill (goal 080, design §5.6)

"Backfill = the same pipeline pointed at archives. Launch with US history to the 2012 STOCK
Act era." The us_house adapter's `discover` is now year-parameterized (`discover_year`); the
backfill re-points it at the Clerk's historical `{YYYY}FD.zip` indexes.

- [x] **Dry-run machinery + diff report** — buildable-done.
  `cargo run -p worker --bin backfill -- --adapter us_house --from 2012 --dry-run`.
  Discovers each archive year (full per-year PTR count), dry-processes a bounded per-year
  sample, classifies each sampled filing against current Gold (adds / changes / supersessions /
  unchanged — design §5.6), and writes NOTHING (no Bronze ledger row, no Silver, no Gold; proven
  by `cargo test -p worker --test backfill -- --ignored`, which asserts every table's row count
  is unchanged). Reprocessing supersedes, never mutates (invariant 1): an unchanged reprocess
  inserts nothing; an amendment (a new `DocID` with an `Amended` row) is surfaced as a
  supersession for review. The diff fingerprints reproduce the publish stage's exactly (parity
  test green).
  - **Verified live scope:** 7,544 PTRs discovered 2012→2026. Historical `{YYYY}FD.zip` exists
    and parses back to 2012; **PTR (FilingType `P`) e-filing begins ~2015** — 2012–2014 hold zero
    P rows (a real, valid empty result, not a failure; answers the us-house SAF historical-depth
    open question — a SAF re-freeze to record it is a goal-016 follow-up, since the SAF is a
    frozen eval reference). PTR counts rise to a mid-decade peak (2018 ≈ 830) and taper
    (2026 partial = 274).
- [ ] **Adapter hardening against archive-surfaced edge cases** — buildable follow-up (file as a
  new goal). The bounded dry-run over live 2026 data already fail-closed two real filings the
  five-fixture adapter does not yet handle: (a) a `LOCATION` (`L:`) sub-line appearing INSIDE the
  Transactions region (DocID 20034201) — a parser branch not in the fixtures; (b) paper/scanned
  filings routing to the LLM seam (needs `ANTHROPIC_API_KEY`). Fail-closed per filing is correct
  (invariant 6) — but a real backfill will surface these at scale, so widen fixtures + parser
  FIRST. The dry run is the tool that enumerates them (raise `--limit`, review the FAIL-CLOSED
  list). Recorded in the us-house SAF quirks log (2026-07-06).
- [ ] **Real (write-to-prod) backfill run** — **infra-blocked** (no longer human-lane/HALT on the
  mechanical side — see below). `bin backfill-real` (goal 081) drives the real write chain, and
  the founder diff-review + go/no-go this section used to require is now a mechanical guardrail:
  every archive year is gated through `BACKFILL_BUDGET` (env var, default 500 Gold rows/year —
  goal 081 Task 4), mirroring `scripts/check-tf-plan.sh`'s numeric-count-vs-env-var-budget shape.
  Before each year's real write, it calls the existing dry-run and reads that year's
  `record_delta`; `<= BACKFILL_BUDGET` proceeds autonomously, `> BACKFILL_BUDGET` skips the year
  (logged to `agents/JOURNAL.md`, nothing blocks — a later invocation retries it) instead of
  halting for a human diff review. Historical-roster seeding (Task 1) and backfill-mode alert
  suppression (Task 2, every `outbox_event` pre-dispatched — zero real subscriber alerts for
  historical filings) are wired in the same bin. **Still infra-blocked**: the actual 2012–2026
  production run (goal 081 Task 5) needs the minimal Cloud SQL IAM/connectivity fix goal 081
  scopes (§6 does not cover this — that section is entity/billing, not DB access) and a local
  full-scale rehearsal first; tracked in `agents/goals/081-us-backfill-execution.md`.

**Backfill audit trail — decision (goal 080):** no new table. A backfill is the same pipeline
pointed at more years, so its runs already land in `pipeline_run` (per-stage rows keyed by a
deterministic `idempotency_key` = `<regime>:<stage>:<sha|external_id>`), and its Gold inserts
carry the same fingerprints + `raw_document` provenance as live runs. A backfill is therefore
distinguishable and replayable through existing tables; a dedicated `backfill_run` table would be
speculative (CLAUDE.md §2). No migration; `check-migration-safety.sh` stays trivially green.

## 2. Quality SLOs (design §5.5, §7.4)

| SLO | Target | Owner | State |
|---|---|---|---|
| Tier-1 freshness (US House/Senate) | discover→publish **p50 < 10 min** | data plane | **infra-blocked** — needs Scheduler unpaused + Cloud Run worker live (goal 020); the pipeline meets it locally, measurement needs prod |
| Tier-2 freshness (UK/CA/AU) | same day | data plane | infra-blocked (as above) |
| Tier-3 freshness (EU-P/FR/DE) | bulk on publication | data plane | infra-blocked |
| API uptime | (set at go/no-go) | infra | infra-blocked |
| Sampling-audit precision | monthly precision report (design §7.4) | data plane | **buildable-done** — `worker` `sample-audit` bin (goal 070); dashboards infra-blocked |
| Drift freeze/unfreeze | fail-closed on layout drift | data plane | **buildable-done** — sentinel WATCH (goal 017) + publish freeze gate (goal 070) |

- [ ] **SLO dashboards + freshness measurement** — infra-blocked. The signals exist
  (`pipeline_run` timings, `filing.discovered_at` vs `published_at`, sentinel drift reports);
  wiring them to Cloud Monitoring dashboards needs the substrate. Document the queries here when
  the substrate lands.

## 3. Monitoring, status page, budget alerts (goal 020 infra; automation-policy HARD CAP)

- [ ] **Cloud Run health checks** (`/health` on api/web/worker) — infra-blocked. Services defined
  in `infra/cloudrun.tf` (scale-to-zero); deploy + verify per `docs/runbooks/deploy.md` step 3.
- [ ] **Scheduler cadence unpause per tier** — infra-blocked. `infra/scheduler.tf` jobs are
  created PAUSED; unpause each as its adapter passes conformance (deploy.md step 7).
- [ ] **Budget / billing alerts** — infra-blocked + guardrail-bound. Billing changes are
  auto-only within the monthly **HARD CAP** (`docs/decisions/automation-policy.md` §3); a budget
  alert at the cap ceiling must be wired in GCP billing once the project is funded. Over-cap →
  halt.
- [ ] **Public status / coverage page** — partly buildable. The per-jurisdiction **coverage
  dashboard** (covered-since, gaps, freshness — design §7.3) is served from `/v1/jurisdictions`
  (goal 065) and rendered by the web app (goal 040); a dedicated uptime status page is
  infra-blocked (needs the monitoring backend).
- [ ] **Secrets present** — infra-blocked. `DATABASE_URL`, `ANTHROPIC_API_KEY`, `STRIPE_*`,
  `SMTP_*` via Secret Manager only (never in repo/state); shells created by the apply
  (deploy.md steps 5–6).

## 4. Trust surface (design §7.3) — buildable, mostly done

- [x] Record-page audit trail: official-source link + archived copy, verification badge +
  confidence, supersession/correction history, methodology link (goals 040/041/070).
- [x] Public corrections log page (goal 070b).
- [x] Per-regime pre-publication redaction pass (design §7.5; goal 070a) — raw stays sacred.
- [ ] Verify every launch regime's methodology link resolves to a published methodology page
  (blocked on the methodology copy — human-lane, §5 below).

## 5. Legal / methodology PUBLIC pages — human-lane (REQUIRED artifacts; copy NOT written here)

Residual human touch: legal/brand exposure, no mechanical guardrail (`/CLAUDE.md`). These are
**required launch artifacts** — listed, not drafted. The loop must NOT write their public copy.
Each needs a human author + legal review before go-live (design §7.5):

- [ ] **Privacy policy** — GDPR/DSAR basis for publishing public-role disclosures; the redaction
  scope; contact for data requests (design §7.5 GDPR).
- [ ] **Terms of service** — free snapshots **CC BY** (attribution); API under commercial terms;
  "not investment advice" disclaimer (design §7.5 licensing + not-advice).
- [ ] **Methodology pages** (per launch regime) — how we source, extract, band values, resolve
  entities, and what our documented limits are. The trust surface links to these (§4). One per
  launch jurisdiction (US House, US Senate, UK, CA, AU, EU-P, FR, DE — design §5.7). Grounded in
  each `docs/regimes/*.md` SAF, but the PUBLIC copy is human-authored.
- [ ] **Corrections policy** — neutral as-filed language, correction response SLA, right-of-reply
  (design §7.5 defamation). The mechanism (public corrections log) is built (§4); the POLICY copy
  is human-lane.
- [ ] **Takedown / redaction contact** — a reachable channel for redaction/right-of-reply
  requests (design §7.5); wire it to the review queue.
- [ ] **Pricing copy** — the paid tiers/prices (goal 050 built the metering + Stripe seam;
  the price VALUES + public pricing copy are human-lane, `/CLAUDE.md`). Tier structure +
  price values decided 2026-07-05: Free ($0) / Individual ($15mo, $120yr) / API ($49mo,
  $499yr) / Enterprise (custom quote). Stripe direct confirmed over a Merchant of Record.
  Rationale, comp benchmarks, and VAT/GST posture: `docs/decisions/incorporation/README.md`
  and `stripe-config-and-vat-posture.md` in that folder. Public pricing-page copy itself
  still human-lane, not written here.

## 6. Entity formation & billing readiness — human-lane

Founder decision 2026-07-05: incorporate as a Wyoming single-member LLC, keep the
already-shipped Stripe-direct billing (not a Merchant of Record). Full rationale,
comp research, and adversarially-verified Brazilian tax-risk findings (2026-07-06):
`docs/decisions/incorporation/README.md`. None of the items below are loop-executable —
entity filing, EIN, and bank-account opening all require the founder's own
signature/passport/tax-ID as a hard external constraint, independent of automation
policy.

- [ ] **Accountant / cross-border tax lawyer consult** — before filing anything.
  `docs/decisions/incorporation/accountant-briefing.md`.
- [ ] **File WY Articles of Organization + registered agent.**
- [ ] **Apply for EIN** (no SSN/ITIN route).
  `docs/decisions/incorporation/ein-prep.md`.
- [ ] **Operating Agreement** — draft ready, needs review + signing.
  `docs/decisions/incorporation/operating-agreement-draft.md`.
- [ ] **US business bank account** (Mercury primary, Wise backup).
- [ ] **Annual compliance calendar** — WY Annual Report + Form 5472/pro-forma 1120
  ($25k penalty risk, uncapped continuing failure, if missed/late — CPA-prepared, not
  best-effort).
- [ ] **Media liability / E&O insurance quote.**
  `docs/decisions/incorporation/insurance-shortlist.md`.
- [ ] **Configure Stripe pricing tiers + confirm VAT/GST posture.**
  `docs/decisions/incorporation/stripe-config-and-vat-posture.md`.

## 7. Go / no-go gate — human-lane

- [ ] **Launch go/no-go** — human decision. Preconditions: §1 real backfill run complete
  (mechanically gated by `BACKFILL_BUDGET` per year, goal 081 Task 4 — no founder diff review
  needed; any skipped-for-budget years retried and cleared) and the minimal prod DB connectivity
  it needs stood up; §2 SLO dashboards live and Tier-1 p50 < 10 min observed in prod; §3
  monitoring + budget alerts green; §5 all legal/methodology pages published + legal-reviewed;
  §6 entity formed + bank account + Stripe tiers configured. Automated against acceptance where
  one exists (automation-policy: "LAUNCH: automated against acceptance commands"), but the final
  public-facing legal/pricing sign-off and the go/no-go call remain the residual human lane.
