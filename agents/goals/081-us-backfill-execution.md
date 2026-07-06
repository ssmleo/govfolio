# 081 — US backfill: real write-to-prod execution

## Objective
Build the missing real (write-to-prod) US House backfill write path — Runner-driven,
budget-gated, alert-suppressed, historically-rostered — then execute it: 2012-2026 US House
PTRs (7,544 discovered by goal 080's dry run) land in production Gold, replacing goal 080's
founder-go/no-go HALT with a mechanical BACKFILL_BUDGET guardrail, zero real subscriber alerts
fired for historical filings, zero human waiting at any point.

## Scope
In: historical roster seeding (Clerk index only, 2012-2026); backfill-mode alert suppression;
the archive-to-Runner real write bin; the BACKFILL_BUDGET guardrail replacing founder go/no-go;
a local full-scale rehearsal; the minimal production DB connectivity needed for this backfill
specifically; the real production run.
Out: non-US backfills (no other adapter has archive depth); legal/methodology PUBLIC copy;
launch go/no-go (human lane, CLAUDE.md); a congress-legislators/Wikidata cross-source identity
merge for redistricted members (named follow-up, not built here); building/deploying the first
real api/worker Cloud Run image (separate, much larger initiative — launch-checklist.md tracks
it independently of this goal).

## Context (read first)
- agents/goals/080-backfill-launch.md — dry-run half (done), its now-superseded HALT bullet on
  founder go/no-go, its Findings (7,544 PTRs 2012-2026, PTR e-filing starts ~2015, the
  conditional-GET-per-year fix, the two known fail-closed parse cases)
- agents/goals/020-cloud-substrate.md — infra live in prod; its "Still open" note on
  `database-url` being genuinely undesigned (connector/proxy-only, IAM auth, no static password)
- docs/runbooks/deploy.md — guardrails (DESTROY_BUDGET, migration safety), the non-negotiable
  that ad-hoc prod writes never go through the toolbox MCP (local dev DB only — does not apply
  to this goal's own sqlx-driven write path, which is the same kind of production write any
  deployed service would make)
- crates/worker/src/backfill.rs — ArchiveSource/GoldBaseline traits, ClerkArchive, dry_run(),
  DiffReport/YearDiff (record_delta field), the existing bin's hard `--dry-run`-required refusal
- crates/pipeline/src/run.rs — Runner, RunnerBinding, run_live(), the process_remote chain,
  Claim/pipeline_run idempotency (Claim::Replay skips finished stages)
- crates/pipeline/src/stages/publish.rs — publish_filing, insert_outbox, FilingSpec, PublishStats
- crates/pipeline/src/stages/roster.rs — seed_roster, resolve_politician, RosterMember,
  open_review_task_once
- crates/adapters/us_house/src/seed.rs — roster_from_index_xml (already year-generic)
- crates/adapters/us_house/src/index.rs — index_zip_url(year), parse_index_xml
- crates/core/migrations/0005_alerts.sql:49 — outbox_undispatched partial index
- crates/worker/src/alerts/matcher.rs — match_pass, the `dispatched_at is null` poll + stamp
- scripts/check-tf-plan.sh — the numeric-budget-vs-env-var pattern Task 4 mirrors
- docs/runbooks/launch-checklist.md §1 (backfill), §7 (go/no-go — not §6; 080 mis-cited this)
- infra/sql.tf, infra/cloudrun.tf — current Cloud SQL access model + why no real image exists yet

## Research findings (pre-verified 2026-07-06 — do not re-derive)
- **Roster source: Clerk per-year index only** (not a congress-legislators/Wikidata
  cross-reference). `roster_from_index_xml` + `seed_roster` already work on any year's index
  XML — the only missing piece is looping them across 2012-2026 instead of just the current
  year. Known, accepted, bounded limitation: `seed_roster` keys strictly on `(alias, district)`
  with no cross-year identity linking, so a member whose district *number* changed
  (redistricting — the only such cycle inside the PTR era is 2023) gets a second `politician`
  row instead of a merged one. Live-confirmed concrete case: Rep. Pelosi's 2023 filings carry
  both `CA11` and `CA12` in the same year's index. This never causes a wrong or guessed match
  (invariant 3 stays intact — every hit is still genuinely exact) — it only fragments one
  person's history across two profile rows, bounded to a few dozen people at most. Deliberately
  deferred: `politician.wikidata_qid` (schema column already exists, unused) is the named
  eventual merge key, filed as a follow-up, not built here — a future Wikidata-reconciliation
  pass would fix this fragmentation AND any future recurrence (2032's redistricting) in one
  shot, across both the backfill and the already-running live seeding path, which a
  congress-legislators-for-backfill-only fix would not. Do not re-litigate this decision; do not
  add a congress-legislators/Wikidata cross-reference in this goal.
- `YearDiff.record_delta: usize` ("Gold rows the adds + supersessions + changes would insert")
  is already exactly what Task 4 needs — calling the EXISTING
  `dry_run(source, baseline, year, year, usize::MAX)` for one year and reading
  `report.years[0].record_delta` requires no new prediction/classification code.
- `Runner::run_live()` inlines its loop-over-refs body around a call to
  `self.adapter.discover(&self.ctx)` (pinned to the current year). Minimal-diff way to drive
  historical `FilingRef`s through the identical real write chain: extract that loop body into a
  new `pub async fn run_over(&self, refs: &[FilingRef]) -> anyhow::Result<RunReport>`, have
  `run_live` call `discover()` then delegate to it. Zero behavior change for existing callers.
- **Production DB connectivity**: goal 020 deliberately left `database-url` undesigned
  (`sql.tf`: no authorized networks, connector/proxy only, IAM auth, no passwords) and
  `cloudrun.tf` confirms Cloud Run today serves a placeholder "hello world" image — no real
  api/worker container has ever been built or deployed. The IAM DB users terraform already
  created (`google_sql_user.iam_service_accounts`) are scoped to the `api`/`worker` Cloud Run
  service accounts only, not to any human/session identity. Building the full "first real
  production deployment" (Dockerfile, CI image build, Cloud Run Job) is a separate, much larger
  initiative that launch-checklist.md's other infra-blocked items are already waiting on
  regardless of this backfill — out of scope here. The minimal, additive, backfill-scoped fix
  (one new Cloud SQL IAM DB user for the operating identity + Cloud SQL Auth Proxy run locally)
  is in scope, as Task 5b below.
- `docs/runbooks/launch-checklist.md` §1's "founder reviews the diff" line and §7's go/no-go
  preconditions need a follow-up wording pass once this goal ships — do it inside Task 4's own
  commit, not a separate task.

## Acceptance criteria (all must pass)
```bash
cargo test -p pipeline --test roster_historical -- --nocapture         # Task 1
cargo test -p pipeline --test backfill_suppression -- --nocapture      # Task 2
cargo test -p worker --test backfill_real -- --ignored --nocapture     # Task 3
cargo test -p worker --test backfill_budget_gate -- --nocapture        # Task 4
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
# Task 5: real execution, verified operationally (see checklist) — no single test command
```

## Checklist
- [x] **Task 1 — historical roster seeding.** Loop `roster_from_index_xml` + `seed_roster`
  (both unchanged) over each archive year 2012-2026 via `index_zip_url(year)`, sharing ONE fetch
  per year with filing-discovery (extract the index-zip fetch+unzip out of
  `UsHouseAdapter::discover_year` into a small reusable helper — do not double-fetch the same
  archive, invariant 10). Each year's `seed_roster` call fails closed independently (mirror
  `dry_run`'s per-year isolation — an ambiguous-roster bail on one year must not sink the other
  14).
  Acceptance: `cargo test -p pipeline --test roster_historical -- --nocapture` — a real,
  pre-2015 filer no longer in Congress today resolves via `resolve_politician` to
  `Some(politician_id)` against the seeded historical roster, not `None`.
- [x] **Task 2 — backfill-mode alert suppression.** Add `pub backfill: bool` to `FilingSpec` (a
  plain bool, not a `PublishMode` enum — only two states exist; CLAUDE.md simplicity-first).
  Thread into `insert_outbox` (`crates/pipeline/src/stages/publish.rs`): when true, bind
  `dispatched_at = now()` in the same INSERT instead of leaving it NULL. Gold rows and
  review_tasks are unaffected — only outbox dispatch is suppressed, and the row still exists for
  audit.
  Acceptance: `cargo test -p pipeline --test backfill_suppression -- --nocapture` — a
  backfill-mode run over existing us_house fixtures yields `gold_inserted > 0` AND a subsequent
  `match_pass` returns `matched.events == 0`.
- [x] **Task 3 — archive-to-Runner real write bin.** New bin
  `crates/worker/src/bin/backfill-real.rs` (kept separate from the existing `--dry-run`-only
  `bin/backfill.rs`, which must keep refusing to run without `--dry-run`). For a given year
  range: call `UsHouseAdapter::discover_year(year, &ctx)` directly (full list, no
  `--limit`/sampling) to get `Vec<FilingRef>`; drive every one through the real write chain via
  the new `Runner::run_over(&refs)` in backfill mode (Task 2's flag). Reuse
  `Runner`/`RunnerBinding` as-is beyond that one extraction. `pipeline_run` claim/idempotency
  already makes this kill-and-resume safe — no new resume-tracking.
  Acceptance: `cargo test -p worker --test backfill_real -- --ignored --nocapture` over a small
  real year slice — filings land in Gold, a second run is idempotent (0 new rows), every
  `outbox_event` from the run has `dispatched_at` set.
- [x] **Task 4 — BACKFILL_BUDGET-bounded autonomous go-ahead (replaces founder go/no-go).**
  Mirror `scripts/check-tf-plan.sh`'s numeric-count-vs-env-var-budget shape. Chunk by archive
  year. Before Task 3's real write pass for a year, call the existing
  `worker::backfill::dry_run(source, pg_baseline, year, year, usize::MAX)` and read
  `report.years[0].record_delta`. If `<= BACKFILL_BUDGET`, proceed autonomously to the real
  write for that year. If it exceeds budget, skip that year, log it (this goal file's
  `## BLOCKED (human)` section or `agents/JOURNAL.md`, matching existing halt convention), and
  continue — nothing blocks; a later invocation naturally retries the skipped year.
  `BACKFILL_BUDGET` env var, default `500` (Gold-row cap per year) — an explicit starting
  default per goal 080's peak-year finding (~830 filings/2018), widenable later. Update
  `docs/runbooks/launch-checklist.md` §1 + §7 wording in this same commit to reflect
  BACKFILL_BUDGET replacing the founder-diff-review step.
  Acceptance: `cargo test -p worker --test backfill_budget_gate -- --nocapture` — a
  synthetic/mocked high-count year halts cleanly; a low-count year proceeds to a real write.
- [ ] **Task 5 — full execution: local rehearsal, prod connectivity, real production run.**
  - **5a (local rehearsal, zero cloud cost/risk):** run the complete, budget-gated
    `backfill-real` for the full 2012-2026 range against local dev Postgres
    (`pg-local.ps1`, `localhost:5433/govfolio`). First end-to-end run at full scale, not a
    1-year test slice. Verify: total Gold rows roughly match goal 080's dry-run expectations
    (discounting BACKFILL_BUDGET skips and the two known fail-closed 2026 edge cases),
    `pipeline_run` shows the full range claimed+finished, zero real alerts (outbox
    `dispatched_at` set throughout).
  - **5b (minimal prod connectivity, only after 5a is clean):** add ONE new `google_sql_user`
    (`CLOUD_IAM_USER`, not `CLOUD_IAM_SERVICE_ACCOUNT`) for the operating identity actually
    running this (the founder's own authenticated gcloud identity — ADC is already done; confirm
    with `gcloud config get-value account`). Small, additive, non-destructive terraform change
    (1 add, 0 destroy — does not approach DESTROY_BUDGET); apply through the normal guardrail
    (`check-tf-plan.sh`). Run Cloud SQL Auth Proxy locally against `sql_connection_name`
    (recorded terraform output from goal 020) using existing ADC; confirm connectivity before
    proceeding. Write the resulting connection string into the `database-url` secret (gives
    goal 020's deferred loose end a real value for this operator-driven access pattern
    specifically — does NOT resolve the separate Cloud-Run-service DATABASE_URL wiring, which
    stays deferred to whoever builds the first real API/worker image).
  - **5c (the real production run):** run `backfill-real` for the full 2012-2026 range against
    the now-connected production Cloud SQL, budget-gated exactly as Task 4 built it.
    Acceptance: prod `filing`/`disclosure_record` row counts reflect the real backfill
    (spot-checked against goal 080's dry-run per-year counts, less any BACKFILL_BUDGET-skipped
    years), zero real subscriber alerts fired, a second invocation is a no-op. This is the step
    that makes the goal done in the full sense — real historical filings live in prod Gold.

## BLOCKED (human)
(empty — this goal's entire purpose is to remove the founder-go/no-go HALT that goal 080's
"## HALT (human/infra)" bullet named. Per docs/decisions/automation-policy.md's banner and root
CLAUDE.md's allocator line, Task 4's BACKFILL_BUDGET gate is the mechanical guardrail that
supersedes it, matching the DESTROY_BUDGET/HARD CAP precedent already applied to terraform and
billing. Task 5b's terraform change is likewise pre-authorized by that same policy (additive,
within budget). Legal/methodology copy and final launch go/no-go remain separately human-lane
and are out of this goal's scope, not touched here.)
