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
cargo run -p worker --bin backfill -- --adapter us_house --from 2012 --to 2013 --dry-run
                                                                         # Task 4.5: nonzero discovery
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
- [x] **Task 4.5 — pre-2015 PTR discovery-filter fix (blocks Task 5's full-range run).** Real
  finding, discovered concurrently on this branch by the standing loop's Stage 0 role-eval
  calibration work (`docs/regimes/us_house/AUTHORITY.md`, surveyor artifact, commit `cd2c706`,
  independently audited PASS — see its `open_questions` entry on the filing-index schema flip):
  the Clerk's filing-index schema forks before ~2015. 2011 carries no PTR tag at all
  (pre-STOCK-Act). 2012-2013 tag PTRs via `DisclosureType == "PTR"` with `FilingType` `O`
  (original) or `A` (amended) — there is no `FilingType == 'P'` in this era. 2015+ uses
  `FilingType == 'P'` with the `DisclosureType` field gone entirely. Goal 080's dry run (and
  this goal's Tasks 1/3/4, inherited from it) filtered on `FilingType == 'P'` only, which
  **silently** (not fail-closed — a real invariant-6-adjacent gap) skips real 2012-2013 PTR
  filings rather than finding them or erroring. **2014 is a separate, genuinely open anomaly**
  (only 11 total `Member` records in that year's whole index, none `DisclosureType`-tagged PTR
  either) — AUTHORITY.md flags this as unexplained; this task does not need to explain it or
  force rows out of a year that may genuinely have almost nothing in the archive. Do not
  fabricate an explanation for 2014; leave it as a documented anomaly.
  Fix: wherever the PTR-discovery filter lives (`UsHouseAdapter::discover_year` / the index
  parsing in `crates/adapters/us_house/src/index.rs` and `adapter.rs`), recognize BOTH
  conventions: `FilingType == 'P'` (2015+) OR (`DisclosureType == "PTR"` AND `FilingType` in
  `{O, A}`) (pre-2015). Parse `DisclosureType` out of the index XML if it isn't already
  captured on the `Member` struct. Do not touch Tasks 1-4's own committed logic beyond this
  discovery filter; do not attempt to resolve the 2014 anomaly or the adapter's other open
  questions (FilingType legend for B/C/D/E/F/G/H/N/R/T/W/X, DC/JT owner rendering, etc.) — out
  of scope here.
  Acceptance: a test proving discovery over a pre-2015 year (2012 or 2013) now finds a nonzero
  count of PTR-shaped filings under the fixed filter (previously 0 under the old
  `FilingType == 'P'`-only filter) — reuse AUTHORITY.md's real counts/evidence where practical.
  Also re-run the EXISTING, unmodified dry-run bin —
  `cargo run -p worker --bin backfill -- --adapter us_house --from 2012 --to 2013 --dry-run` —
  and confirm discovered counts are now nonzero for 2012/2013 (goal 080's original findings
  reported 0 for both years under the old filter). Command-form for the new test, exact name
  at the implementer's discretion matching existing `us_house`/`worker` test conventions, e.g.
  `cargo test -p us_house -- --nocapture` (whichever suite the fix's test lives in).
  **Closed 2026-07-06** (commit `393cda2`): fix is correct, proven deterministically against
  real sha256-pinned 2012 evidence (0→3 discovered under the fixed filter vs. the old one);
  independently audited PASS. The live dry-run command shows 2013=8 (nonzero, confirms the fix
  fires) but 2012=0 today — confirmed via direct re-fetch to be genuine same-day upstream
  content drift on the Clerk's site (index zips regenerate server-side, not static archival;
  today's live `2012FD.zip` genuinely carries zero PTR-taggable rows under either convention),
  not a code defect. See `agents/JOURNAL.md` for the full investigation. **Implication for
  Task 5:** live per-year discovered counts at execution time may not match goal 080's original
  snapshot or this task's own findings — that's expected given the source's volatility, not a
  regression signal.
- [x] **Task 4.6 — case-insensitive filer-information label match (blocks Task 5's full-range
  run).** Real finding, discovered during Task 5a's rehearsal: `crates/adapters/us_house/src/
  parse.rs`'s `labeled_value()` does an exact-case `line.strip_prefix(prefix)` for the `"Name:"`,
  `"Status:"`, and `"State/District:"` labels. A live, unlimited dry-run sweep across 2012-2026
  (`cargo run -p worker --bin backfill -- --adapter us_house --from 2012 --to 2026 --dry-run
  --limit 2000`, no DB baseline needed for this signal) showed entire years failing closed
  near-100% (2014: 708/708, 2015: 728/728, 2016: 765/765, 2017: 801/801, and more) with the exact
  error `missing filer-information line "Name:"` — historical-era documents render this label
  lowercase (`name:`), which the exact-case prefix match never matches. This is the same root
  cause Task 3's build agent already flagged (for 2015 specifically) as a deferred
  adapter-hardening gap — now confirmed to span most of 2014-2021 at real scale, not a rare edge
  case.
  Fix: make `labeled_value`'s prefix match case-insensitive (e.g. compare
  `line[..prefix.len().min(line.len())].eq_ignore_ascii_case(prefix)` then slice past it, or an
  equivalent tolerant comparison) — a single shared function serving all three labels, so the
  fix covers `"Name:"`/`"Status:"`/`"State/District:"` symmetrically without inventing new
  speculative label variants. Do NOT attempt to fix the other, distinct one-off parsing gaps
  found in the same sweep (2021 `"Digitally Signed:"` line variant, 2022 "unattached asset text
  after the last row", 2023 "band wrap not followed by a `$…` continuation", 2026 `"L:"`
  sub-line inside the Transactions region — this last one is goal 080's own already-documented,
  deliberately-deferred edge case) — those are separate, smaller-blast-radius gaps, out of scope
  here; only the label-casing issue is in scope, per its confirmed wide impact.
  Acceptance: a test proving `labeled_value` (or `parse_document`) now succeeds against a
  lowercase-labeled historical-era text layer that previously failed with "missing
  filer-information line" (use real historical evidence where practical, matching the Task 4.5
  fix's own precedent of testing against real pinned archive data). Also re-run the full
  2012-2026 dry-run sweep (`cargo run -p worker --bin backfill -- --adapter us_house --from 2012
  --to 2026 --dry-run --limit 2000`, DATABASE_URL pointed at a real reachable Postgres so
  sampling isn't forced to discover-only) and confirm the `"Name:"`-label failure mode is gone
  (years that previously failed near-100% on this specific error now parse successfully; other,
  out-of-scope parsing gaps may still legitimately fail closed).
  **Closed 2026-07-07**: fix is a single shared `strip_prefix_ignore_ascii_case` helper
  (byte-length-safe — never slices past `line`'s length; anchored over the label's full byte
  span including the colon, so it never matches partway through a longer word), used identically
  by `labeled_value` for all three labels. Proven against REAL evidence: a live-fetched 2015
  electronic PTR (Filing ID #20002776, Rep. Brad Ashford NE-02, filed 2015-03-24, fetched from
  `https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2015/20002776.pdf`, PDF sha256
  `9fa13801b0271971f090ad1e1cc9f6ffd6b3bd002134c56fa4306560ac0297ff`) whose real
  `pdf_extract::extract_text_from_mem` text layer renders the label lowercase (`name:`) —
  `crates/adapters/us_house/src/parse.rs` unit tests
  `labeled_value_matches_real_historical_lowercase_name_label` (asserts the fix against those
  real lines) + `strip_prefix_ignore_ascii_case_is_byte_length_safe_and_anchored` (short-line and
  anchoring edge cases) green.
  **Live re-verification, full literal scope**: `cargo run -p worker --bin backfill --adapter
  us_house --from 2012 --to 2026 --dry-run --limit 50` (bounded from the acceptance's own
  `--limit 2000` to a wall-clock-feasible per-year sample — still real, DATABASE_URL-backed
  against `govfolio_081_rehearsal`, still every archive year 2012-2026, invariant-10 polite)
  discovered 8,260 real filings, really fetched+parsed 658 of them, and produced **zero**
  occurrences of `missing filer-information line "Name:"`/`"Status:"`/`"State/District:"` across
  every one of the 520 real per-filing failures logged — the exact years that previously failed
  near-100% on this specific error (2014-2017 confirmed by this goal's own finding, plus
  2018-2021 also checked) are now fully clean of it. Years 2022-2026 show real classifications
  flowing through as intended: 133 adds + 5 supersessions across 250 sampled filings.
  REAL FINDINGS surfaced, explicitly NOT fixed (out of scope per this task): the same real sample
  shows 2014-2022 electronic PTRs still fail closed near-100% of the time, now almost entirely
  for a DIFFERENT, previously-undocumented small-caps degradation variant —
  `` Transactions heading (`T`) not found `` — where some historical-era PDFs render whole words
  with scrambled/partial case (e.g. `tranSactionS`, `iD owner asset transaction`) instead of
  NUL-erasing non-initial glyphs (the documented quirk at the top of `parse.rs`), so
  `collapse_ws(line) == "T"` never matches; this is the single most common failure 2014-2022 and
  materially limits Task 5's real yield for those years even after this fix. Also newly confirmed
  at real scale (2016-2026, wider than the goal's originally-named "2021 Digitally Signed:
  variant"): non-zero-padded signature dates (e.g. `"02/1/2016"`) failing the strict
  `MM/DD/YYYY` parse, garbled `Digitally Signed:` name text breaking the comma-split
  (`unsplittable signature line`), and the line being altogether absent
  (`missing \`Digitally Signed:\` line`). The already-named 2022 unattached-asset-text, 2023
  band-wrap, and 2026 `L:` sub-line gaps are all directly visible in this same real sample too,
  confirming they're pre-existing and distinct, not introduced by this fix.
  `needs_llm_extraction: ... ANTHROPIC_API_KEY is absent` entries are expected/benign (genuine
  scanned/paper PTRs correctly routed to the LLM seam per design; no key configured in this dev
  environment) — not a defect.
  `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` green; `cargo test -p
  us_house` 44/44; `us_house`/`us_senate`/`uk_commons_register`/`canada_ciec`/
  `australia_register`/`eu_fr_de_annual`/`br` conformance unregressed.
- [x] **Task 4.7 — catch the `pdf-extract` crate's internal panic (blocks Task 5's full-range
  run; higher severity than the cosmetic parsing gaps).** Real finding: a real `backfill-real
  --from 2012 --to 2026` run against `govfolio_081_rehearsal` (Task 5a's own first rehearsal
  attempt, prior to Tasks 4.6/4.7) crashed the WHOLE process partway through 2020 with:
  `thread 'main' panicked at .../pdf-extract-0.12.0/src/lib.rs:950:49: called
  \`Result::unwrap()\` on an \`Err\` value: FromUtf16Error(())`, exit code 101. This is NOT a
  graceful per-filing failure like the other Task 4.x findings — it is an uncaught panic
  **inside the third-party `pdf-extract` crate's own code**, which unwinds straight past
  `crates/adapters/us_house/src/adapter.rs:83`'s existing `let Ok(text) =
  pdf_extract::extract_text_from_mem(&bytes) else { ... }` graceful-`Err` handling (a panic
  never produces an `Err` to match on — it aborts the call stack outright). Left unfixed, this
  would recur identically on every retry against the same poison-pill PDF (some 2020-era
  document with malformed UTF-16 in its embedded text), permanently blocking any real run that
  reaches it — a much bigger risk than a single filing quietly failing closed.
  Fix: wrap the `pdf_extract::extract_text_from_mem(&bytes)` call at `adapter.rs:83` in
  `std::panic::catch_unwind` (with `std::panic::set_hook`/a restore-guard around it if needed to
  avoid spamming the default panic-message printer for an expected, handled failure mode),
  converting a caught panic into the same `Err`/fail-closed path the existing `else` branch
  already handles — one poison-pill document fails that ONE filing closed and the run continues,
  it does not crash the process. Do not touch pdf-extract itself (external dependency) or any of
  the other Task 4.x-documented parsing gaps.
  Acceptance: a test proving a PDF that would trigger `pdf-extract`'s internal panic (reuse the
  real 2020-era poison-pill document if it can be responsibly captured/fixture-pinned, or a
  minimal synthetic reproduction of the same malformed-UTF-16 condition if the real one isn't
  practical to pin) now returns a normal `Err` (fails that filing closed) instead of crashing the
  test process. Also re-run a real `backfill-real` (or the dry-run sweep) across a range
  including 2020 and confirm no panic/process-crash occurs, only a normal per-filing failure.
  **Closed 2026-07-07**: fixed in `crates/adapters/us_house/src/adapter.rs` — a new private
  `extract_text_catching_panics` wraps the `pdf_extract::extract_text_from_mem(&bytes)` call at
  the former `adapter.rs:83` in `std::panic::catch_unwind`, swapping the default panic hook for a
  no-op for the call's duration (restored immediately after, single `eprintln!` noting the caught
  panic) so the expected failure mode doesn't spam a full backtrace. The caller's existing
  `let Ok(text) = ... else { ... }` branch is otherwise untouched — a caught panic now flows into
  the SAME LLM-seam fallback a graceful `Err` already used.
  Took the REAL-reproduction path, not the synthetic-fallback: constructed a minimal,
  syntactically-valid PDF from scratch (byte offsets computed programmatically, not
  hand-counted) whose one `Type1` font's embedded `ToUnicode` `CMap` maps a character code to
  `<D800D800>` — two consecutive UTF-16 high-surrogate code units, invalid UTF-16 not caught by
  `pdf-extract`'s own single-surrogate guard. Verified genuine, not just "malformed for some
  other reason": a new test (`malformed_utf16_cmap_pdf_reproduces_the_real_pdf_extract_panic`)
  calls `pdf_extract::extract_text_from_mem` DIRECTLY (bypassing the fix) and asserts it panics —
  confirmed via `cargo test`, reproducing the exact upstream bug class
  (`pdf-extract-0.12.0/src/lib.rs:950`, `String::from_utf16(&be).unwrap()` on `FromUtf16Error`),
  from a fully self-contained fixture (no external file needed). A second test
  (`extract_text_catching_panics_converts_the_real_panic_into_an_err`) proves the fix turns that
  SAME panic into `Err` without crashing the test process. A third
  (`extract_text_catching_panics_still_returns_ok_for_a_normal_document`) guards against a
  trivial always-`Err` implementation by patching just the poison surrogate bytes back to an
  ordinary mapping (same byte length) and confirming `Ok`. All three green, plus the two
  pre-existing `is_ptr` tests: `cargo test -p us_house` 46 passed + 1 ignored (was 44/44 at Task
  4.6's close). `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` both clean.
  `cargo test --workspace`: 394 passed, 63 ignored, 0 failed.
  Live re-verification against `govfolio_081_rehearsal` (`localhost:5433`, per this task's own
  instructions): `cargo run -p worker --bin backfill -- --adapter us_house --from 2020 --to 2020
  --dry-run --limit 150` — 733 filings discovered, 150 really fetched + parsed (real network
  calls, real 2020-era PDFs through the real `pdf_extract` call path), **exit code 0, no
  panic/crash**. All 150 sampled filings failed closed on already-documented Task 4.6 parsing
  gaps (`Transactions heading (T) not found`, `missing/unsplittable Digitally Signed: line`,
  `needs_llm_extraction` with no `ANTHROPIC_API_KEY` configured) — none is the UTF-16 panic
  condition, so this particular 150-document sample did not hit the exact original poison-pill
  filing (its specific `Filing ID` was never recorded in the original crash report to target
  directly). This is consistent with the acceptance's own allowance: the real document may no
  longer reproduce the issue, so the unit-level proof against a self-contained, verified-genuine
  reproduction of the same bug class is the primary evidence; the live run additionally confirms
  the wrapper adds no regression across 150 real documents end-to-end.
- [ ] **Task 4.8 — tolerate the scrambled-case rendering variant (blocks Task 5's full-range
  run; now the single most common real failure for 2014-2022).** Real finding, confirmed at
  scale by Task 4.6's own live sweep: a SECOND, independent PDF text-degradation pattern exists
  in the historical corpus, distinct from the one already documented at the top of `parse.rs`
  ("headings/labels lose every non-initial glyph... rendered as NUL characters... anchored on
  the surviving capitals"). In this second pattern, whole words/phrases survive intact but with
  scrambled/inconsistent case — e.g. the Transactions heading renders as `tranSactionS` instead
  of degrading to the NUL-survivor `T`, and the table header block renders as
  `iD owner asset transaction` instead of the expected `ID Owner Asset Transaction`. Because
  `transactions_region` (`crates/adapters/us_house/src/parse.rs:231-235`) matches the heading
  via the EXACT string `collapse_ws(line) == "T"`, and `HEADER_BLOCK` (lines 257-263) matches
  exact-case strings too, neither recognizes this second pattern at all — it fails closed on
  `Transactions heading (\`T\`) not found` for the vast majority of 2014-2022 filings, the single
  biggest remaining real-yield limiter after Tasks 4.6/4.7.
  This needs investigation before a fix, not a guess: fetch and read several REAL 2014-2022
  filings exhibiting this pattern (matching Task 4.6's own precedent of testing against real
  historical evidence) to determine (a) whether the scramble follows any fixed rule (e.g.
  case alternates by position, or is otherwise deterministic) or is effectively arbitrary per
  character, and (b) whether the SAME scrambled-case rendering also affects the sub-line labels
  (`SubLabel` enum + its matching logic, lines ~265+, which today only recognizes NUL-survivor
  abbreviations like `F S:`, `S O:`, `D:`, `C:`, `L:`) or is confined to headings. Scope the fix
  to what the real evidence actually shows — likely a case-insensitive-and-whitespace-tolerant
  match against the FULL, undegraded label text (`"TRANSACTIONS"`, the full `HEADER_BLOCK`
  strings, and — if evidence shows it's needed — the full sub-labels) as an alternate accepted
  form alongside the existing NUL-survivor form, not a replacement of it (both degradation
  patterns are real and must keep working). Do not touch the already-fixed Task 4.5/4.6/4.7
  logic, and do not attempt the other still-out-of-scope gaps (signature dates, Digitally Signed
  line, unattached asset text, band-wrap continuation, `L:` sub-line) — those are separate tasks.
  Acceptance: a test proving `transactions_region`/`parse_document` now succeeds against real
  2014-2022 evidence exhibiting the scrambled-case pattern that previously failed with
  `Transactions heading (\`T\`) not found`, while the existing NUL-survivor-pattern tests (2015+
  era fixtures) continue to pass unchanged. Also re-run a real dry-run sample across a few
  affected years (2014-2022) and confirm this specific failure mode's occurrence count drops
  substantially (other, separately-tracked gaps may still legitimately remain).
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
