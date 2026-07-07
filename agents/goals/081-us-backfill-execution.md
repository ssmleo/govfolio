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
- [x] **Task 4.8 — tolerate the scrambled-case rendering variant (blocks Task 5's full-range
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
  **Closed 2026-07-07**: investigated first, against real, independently live-fetched and
  sha256-pinned 2014-2022 electronic PTRs (Filing ID #20000077, 2014,
  `https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2014/20000077.pdf`, sha256
  `ea936ce15201393a2fbfc61c9e9670e016fd5c6b0010aae8b750e34ebc924691`; #20001787, 2014, sha256
  `29bfb95acf4679614ded1fb085743c9eb4220bb9964169b850307f584b06d11c`; #20016985, 2020, sha256
  `ce68b1f8b7def98256506531edd2c98557a0844e481ce0126a4cfec510202d6a`; #20020448, 2022, sha256
  `8f7c44affce207b7cc84cc2c74fb514eb37a33d118377f9c974e8710075f27fa`; plus a broader
  unpinned sample across 2014/2016/2018/2020/2022). Findings: (a) the scramble is NOT a fixed
  positional rule — different specific letters are affected inconsistently per
  document/year (`tranSactionS`, `iD owner asset transaction`, `FIlINg sTATus:`,
  `SubHOlDINg OF:`, `DESCRIPTIoN:`) — effectively arbitrary, not decodable by position or
  character; (b) the SAME pattern DOES affect `SubLabel` sub-lines, directly confirmed for
  `FilingStatus`/`SubholdingOf`/`Description` — but as the FULL undegraded label word in
  scrambled case (`FILINg STATUS:`), not the abbreviated NUL-survivor form (`F S:`) the existing
  matcher recognized.
  Fix (`crates/adapters/us_house/src/parse.rs`, all additive alongside the existing
  NUL-survivor forms, none weakened): `transactions_region`'s heading match (new
  `is_transactions_heading` helper) now ALSO accepts
  `collapse_ws(line).eq_ignore_ascii_case("TRANSACTIONS")`; `HEADER_BLOCK` matching in
  `scan_rows` is now case-insensitive (was exact-case — a strict superset, so every
  existing exact-case fixture still matches); `match_sublabel` gained a third form,
  `full_text_label`, matching the FULL label text case-insensitively —
  directly evidenced for `FilingStatus`/`SubholdingOf`/`Description`, extended to
  `Comments`/`Location` by the same font-level mechanism using the full label text already
  documented in `docs/regimes/us-house.md` (not a new speculative variant).
  4 new tests, all real-evidence-cited: `transactions_region_accepts_real_scrambled_case_heading`,
  `scan_rows_accepts_real_scrambled_case_header_block`,
  `match_sublabel_accepts_real_scrambled_case_full_text_form`, and an end-to-end
  `parse_document_succeeds_against_real_2014_2022_scrambled_case_evidence` (real scrambled
  heading/header-block/FILING-STATUS lines from Filing ID #20016985, spliced with clean
  synthetic row/filer-info grammar to isolate this fix from the separate `gfedc` artifact below
  that document's own real row independently hits). `cargo test -p us_house`: 49 passed + 1
  ignored (was 46+1 at Task 4.7's close) — every pre-existing NUL-survivor-pattern test
  unchanged and green. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` both
  clean, workspace-wide.
  Live re-verification against `govfolio_081_rehearsal`: `cargo run -p worker --bin backfill --
  adapter us_house --from <year> --to <year> --dry-run --limit 40` for 2014, 2018, and 2022 (120
  real sampled filings total) — **zero** occurrences of
  `` Transactions heading (`T`) not found `` (was the single most common failure 2014-2022 per
  Task 4.6's own live sweep, "fails closed near-100% of the time"). 2022 additionally shows real
  classifications flowing through again (9 adds of 40 sampled) now that this blocker is gone.
  REAL FINDINGS surfaced, explicitly NOT fixed (separate, out of scope per this task): (1) the
  Transactions FOOTNOTE (`* For the complete list…`) is absent entirely — not scrambled-case,
  genuinely missing — from at least some 2014-era documents (directly confirmed: neither
  #20000077's nor #20001787's real extracted text contains any case-variant of that string
  anywhere), a template/form-version difference across years, now the dominant 2014 failure;
  (2) a `gfedc`-shaped token trails the amount band on many 2018-2022 rows whenever the Cap.
  Gains checkbox column is present, breaking band parsing (`band "$X - $Y gfedc" outside the
  grammar`) — looks like a PDF form-field default-value artifact bleeding into the text layer,
  unrelated to case degradation; (3) row-level type tokens and asset/ticker text can also render
  in the same scrambled case (e.g. a lowercase `s` type token on a real 2014 row, `(gOOgl)` for
  a ticker) — the ticker case is harmless (raw is sacred, stored verbatim regardless), but a
  lowercase type token would fail `find_anchor`'s exact-uppercase match, a genuinely separate
  row-grammar gap. These, plus the already-named non-zero-padded dates / `Digitally Signed:`
  variants / unattached-asset-text/band-wrap/`L:` gaps, remain real, legitimate,
  separately-tracked blockers for Task 5's full yield — flagged here for whoever picks up the
  next narrowing pass, not fixed in this task.
- [x] **Task 4.9 — tolerate an absent Transactions footnote (blocks Task 5's full-range run; now
  the dominant 2014 failure).** Real finding from Task 4.8's own live re-verification: some real
  2014-era filings (confirmed directly against Filing IDs #20000077 and #20001787's actual
  extracted text) never contain the `"* For the complete list..."` footnote line at all —
  genuinely absent, not scrambled-case — so `transactions_region`
  (`crates/adapters/us_house/src/parse.rs`, the `end` boundary search) fails closed with
  `Transactions footnote (* For the complete list…) not found`. This looks like a different
  form-template/version used in earlier years rather than a rendering-degradation quirk.
  Needs real-evidence investigation first (same discipline as Tasks 4.6/4.8): fetch and read the
  cited real 2014 documents (and a few more from that era) to determine what ACTUALLY marks the
  end of the Transactions region when this footnote is missing — likely the start of the next
  real section (compare how `vehicle_region`'s own end-boundary already tries multiple
  alternative markers: `"C" | "I P O" | "C S"`) or simply the end of the document. Do not guess
  the alternative marker without checking real text.
  Fix `transactions_region`'s end-boundary detection to accept the existing footnote match OR a
  confirmed-real alternative ending marker, additively (never break years where the footnote IS
  present). Do not touch Task 4.5-4.8's already-closed logic, and do not attempt the other
  still-separately-tracked gaps (the `gfedc` band artifact, scrambled row-level type tokens,
  non-zero-padded dates, `Digitally Signed:` variants, unattached-asset-text/band-wrap/`L:`
  gaps) — those remain out of scope here.
  Acceptance: a test proving `transactions_region`/`parse_document` now succeeds against the
  real 2014 evidence lacking the footnote, with existing footnote-present fixtures (2015+)
  continuing to pass unchanged. Re-run a real dry-run sample against 2014 (and nearby years) and
  confirm this specific failure mode's occurrence count drops substantially.
  **Closed 2026-07-07**: investigated first, against real, independently live-fetched and
  sha256-pinned 2014 electronic PTRs — six of them, not just the two named above: Filing IDs
  #20000077 (sha256 ea936ce15201393a2fbfc61c9e9670e016fd5c6b0010aae8b750e34ebc924691),
  #20000710 (sha256 80a4bc944f3e59d85c59d59647e292144b37ca2985789beb5b063739a48b0963),
  #20000800 (sha256 40babda90c0d13a76da969956206164657a5d7004c8e49809fdfecf8f024ac9c),
  #20000998 (sha256 49ff83fd5abb33ffc234cf748065c3bb64c053926f6a85da60e3c92fa8554c62),
  #20001787 (sha256 29bfb95acf4679614ded1fb085743c9eb4220bb9964169b850307f584b06d11c),
  #20001934 (sha256 035ddd992057a2e608b3a0720eff31ee9b0a2fd6d7e813172150502fca9f9dfb) — none
  contain any case-variant of "complete list" anywhere in their real extracted text (grep-
  confirmed against the live `pdf_extract::extract_text_from_mem` output of each). In 5 of the 6,
  the real next line after the last row/sub-line is the "Comments" section heading (rendered
  `commentS`, scrambled-case, no colon); in the 6th (#20000077) that section renders no text at
  all and the real next line is "Initial Public Offerings" (`initial Public offeringS`) directly.
  Confirms the hypothesis: when the footnote is absent, the boundary is the next real section
  heading — exactly the vocabulary `vehicle_region`'s own end-boundary already recognizes
  (`"I V D" | "C" | "I P O" | "C S"`), just directly evidenced here for the Task 4.8 scrambled-
  case degradation pattern too.
  Fix (`crates/adapters/us_house/src/parse.rs`, additive, `transactions_region`'s own `end`
  search only): the boundary now matches the existing footnote line OR a new
  `is_next_section_heading` helper, checked in the same `.position()` scan so the FIRST match
  wins — footnote-present documents are unaffected (the footnote always precedes any of these
  section headings in the anatomy, so it's still found first every time). The helper recognizes
  both real degradation forms: the NUL-survivor abbreviation (`"I V D"`, `"C"`, `"I P O"`,
  `"C S"`) and the scrambled-case full-word form (`"INVESTMENT VEHICLE DETAILS"`, `"COMMENTS"`,
  `"INITIAL PUBLIC OFFERINGS"`, `"CERTIFICATION AND SIGNATURE"`, matched case-insensitively,
  whitespace-collapsed). Only "Comments"/"Initial Public Offerings" were directly observed
  footnote-absent in this sample (none of the 6 had subholdings); "Investment Vehicle
  Details"/"Certification and Signature" are included for the same structural reason
  `vehicle_region` already relies on them — fixed, always-present anatomy sections, not a guess.
  3 new tests: `transactions_region_accepts_a_genuinely_absent_footnote` (both real endings —
  via "Comments" and directly via "Initial Public Offerings" — plus the NUL-survivor form),
  `transactions_region_prefers_the_footnote_when_both_are_present` (regression guard: a footnote
  followed by a later section heading still stops at the footnote), and an end-to-end
  `parse_document_succeeds_against_real_2014_evidence_lacking_the_footnote` (real heading +
  real Comments/IPO ending from Filing ID #20001787, spliced with clean synthetic row/header-
  block grammar to isolate this fix from two separate, out-of-scope gaps this same
  investigation surfaced but did not fix — see below). `cargo test -p us_house`: 52 passed + 1
  ignored (was 49+1 at Task 4.8's close), every pre-existing test unchanged and green.
  `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` both clean, workspace-wide;
  `cargo test --workspace` green (0 failed).
  Live re-verification against `govfolio_081_rehearsal`: `cargo run -p worker --bin backfill --
  adapter us_house --from 2014 --to 2014 --dry-run --limit 60` and the same for `--from 2013
  --to 2013` — **zero** occurrences of `Transactions footnote (* For the complete list…) not
  found` or any "complete list" text in either sample's failure log (was, per Task 4.8's own
  close note, "now the dominant 2014 failure"). All 60 sampled 2014 filings still fail closed,
  but now on different, already-separately-tracked errors instead.
  REAL FINDINGS surfaced, explicitly NOT fixed (separate, out of scope per this task,
  discovered by this same investigation): (1) the 2014-era table header block is a genuinely
  DIFFERENT, shorter shape than the 5-line `HEADER_BLOCK` this code recognizes — real 2014 text
  renders only 3 lines (`iD owner asset transaction` / `type Date notification` / `Date
  amount`, no "Cap. Gains > $200?" continuation at all, a column the 2014-era paper form may
  genuinely lack) — now the single most common 2014 failure (`unrecognized table header block`,
  ~50% of the 60-sample); (2) non-zero-padded signature dates (`"05/6/2014"`, `"10/3/2014"`) are
  the second most common, already named in Task 4.6's findings, still unfixed; (3) a `gfedcb`
  checkbox-widget artifact appears bleeding into some certification-section text (distinct from
  Task 4.10's own named `gfedc`-after-band artifact — this one is elsewhere in the document, not
  investigated further here); (4) the already-named `Digitally Signed:` variants
  (unsplittable/missing) and `needs_llm_extraction` (expected, no `ANTHROPIC_API_KEY`) both
  recur as before. These remain real, legitimate, separately-tracked blockers for Task 5's full
  yield — flagged here for whoever picks up the next narrowing pass (the header-block-shape
  finding in particular looks like it could be its own follow-up task), not fixed in this task.
- [x] **Task 4.10 — tolerate the `gfedc` band-parsing artifact (blocks Task 5's full-range run;
  now the dominant 2018-2022 failure).** Real finding from Task 4.8's own live re-verification: a
  `gfedc`-shaped token trails the amount band on many 2018-2022 rows whenever the Cap. Gains
  checkbox column is present in the source PDF, breaking band parsing (e.g. `band "$X - $Y gfedc"
  outside the grammar`) — looks like a PDF form-field/checkbox-widget artifact bleeding into the
  extracted text, not a case-degradation issue.
  Needs real-evidence investigation first: fetch and read real 2018-2022 filings exhibiting this
  (Task 4.8's build agent already has candidates in mind from its own sweep) to confirm the
  artifact's exact shape and whether it is consistent enough to strip/ignore safely (e.g. always
  trailing the recognized `$X - $Y` band, always the literal token `gfedc` or a small known set
  of similar checkbox-widget tokens) without accidentally absorbing real data.
  Fix the band-parsing logic to tolerate and discard this trailing artifact additively (existing
  bands without the artifact must keep parsing exactly as before). Do not touch Task 4.5-4.9's
  already-closed logic, and do not attempt the other still-separately-tracked gaps (scrambled
  row-level type tokens, non-zero-padded dates, `Digitally Signed:` variants,
  unattached-asset-text/band-wrap/`L:` gaps) — those remain out of scope here.
  Acceptance: a test proving band parsing now succeeds against real 2018-2022 evidence carrying
  the `gfedc` artifact, with existing artifact-free band fixtures continuing to pass unchanged.
  Re-run a real dry-run sample against 2018-2022 and confirm this specific failure mode's
  occurrence count drops substantially.
  **Closed 2026-07-07**: investigated first, against real, independently live-fetched and
  sha256-pinned 2018-2022 electronic PTRs — five of them: Filing IDs #20016985 (2020, sha256
  ce68b1f8b7def98256506531edd2c98557a0844e481ce0126a4cfec510202d6a), #20009788 (2018, sha256
  38bb4d144e279c9ff999e6330e7ab90f2b5af86c6a705167da87fdd891e1755e), #20016326 (2020, sha256
  50218765b6aed95559b71d556e36e2e59b772c6195f39a443716c3cc57a4ef25), #20019793 (2021, sha256
  90663eab7fd7922e6d9533db8e220ca7f5f288047d76d7624650676f120575f2), #20020251 (2022, sha256
  94542d4fec1917c208a02da0a5dd40b8a38414e4e5940defc29be68d86c98040) — plus a broader unpinned
  sample across 2018/2019/2020/2021/2022 via the dry-run bin. Findings: the artifact is always a
  standalone, whitespace-separated token trailing the FINAL closing amount (whether a single-line
  band or the continuation line of a wrapped band), always exactly one of two literal
  case-sensitive forms — `gfedc` or `gfedcb` (one letter longer) — never embedded mid-band or
  attached without a preceding space. Same font-level mechanism that renders `nmlkj`/`nmlkji` for
  the IPO Yes/No radio and a leading `gfedcb` before the certification paragraph elsewhere in
  these same real documents (Task 4.9's own separately-flagged, still out-of-scope artifact) —
  unrelated to case degradation, a PDF checkbox-widget glyph-name leak specific to the "Cap.
  Gains > $200?" column.
  Fix (`crates/adapters/us_house/src/parse.rs`, `scan_rows` only, additive): a new
  `strip_band_artifact` helper discards a trailing `BAND_ARTIFACT_TOKENS` (`["gfedc", "gfedcb"]`)
  token from the joined amount string — applied AFTER the existing wrap-join logic (so it covers
  both single-line and wrapped bands uniformly) and BEFORE the `tables::band_bounds` grammar
  check. Guards against absorbing real data: only strips when the token is a standalone trailing
  token preceded by whitespace, never a partial/embedded match. An artifact-free band passes
  through byte-for-byte unchanged.
  4 new tests, all real-evidence-cited: `strip_band_artifact_discards_known_trailing_tokens_only`
  (unit-level, both known forms plus a non-stripping embedded-match guard),
  `scan_rows_accepts_real_gfedc_artifact_evidence` (real single-line AND wrapped-band rows from
  Filing ID #20016985), `scan_rows_accepts_real_gfedcb_variant_evidence` (real wrapped-band row
  from Filing ID #20016326), and an end-to-end
  `parse_document_succeeds_against_real_2018_2022_gfedc_artifact_evidence` (real heading/header
  block/rows from #20016985, doc id/filer info/signature clean synthetic grammar, matching this
  suite's existing splicing convention). Red-green proven: with the fix's call site temporarily
  disabled, the 3 non-unit new tests fail exactly as expected (`band "..." outside the grammar`);
  restored, all 4 pass. `cargo test -p us_house`: 57 passed + 1 ignored (was 52+1 at Task 4.9's
  close), every pre-existing test unchanged and green. `cargo fmt --check` and `cargo clippy
  --all-targets -- -D warnings` both clean, workspace-wide (`cargo test --workspace`: 412 passed,
  63 ignored, 0 failed).
  Live re-verification against `govfolio_081_rehearsal`: `cargo run -p worker --bin backfill --
  adapter us_house --from <year> --to <year> --dry-run --limit 40..60` for 2018, 2019, 2020, 2021,
  and 2022 (260 real sampled filings total) — **zero** `band "..." outside the grammar` rejections
  caused by the `gfedc`/`gfedcb` artifact in any of the five years (was, per Task 4.8's own close
  note, "the single biggest remaining real-yield limiter"/"now the dominant 2018-2022 failure" —
  roughly a third of the 2020 sample alone previously rejected on this exact error). The only
  surviving `gfedc`/`gfedcb` mentions in the post-fix logs are inside a DIFFERENT,
  already-separately-tracked error (`unknown transaction type token` — a scrambled-case lowercase
  row-level type token, explicitly out of scope per this task) whose debug-printed raw line
  happens to still echo the artifact because that row hard-rejects earlier in `find_anchor`,
  before this fix's strip step is ever reached — not a regression or a miss. 2022 additionally
  shows real classifications flowing through (11 adds of 40 sampled) now that this blocker is
  gone. REAL FINDINGS surfaced, explicitly NOT fixed (separate, out of scope per this task,
  already named by Tasks 4.6/4.8/4.9): scrambled-case lowercase row-level type tokens
  (`unknown transaction type token Some("s")`/`Some("(partial)")`), non-zero-padded/garbled
  signature dates and `Digitally Signed:` variants, `needs_llm_extraction` (expected, no
  `ANTHROPIC_API_KEY` configured), and an `L:` sub-line inside the Transactions region. These
  remain real, legitimate, separately-tracked blockers for Task 5's full yield, not fixed here.
- [x] **Task 4.11 — signature/certification-section cluster (blocks Task 5's full-range run).**
  Three real, related gaps clustered around the certification/signature area of the document
  (`extract_signed_date` and its surrounding text), accumulated across Tasks 4.6/4.9's findings:
  (a) non-zero-padded signature dates (e.g. `"05/6/2014"`, `"02/1/2016"`) failing the strict
  `MM/DD/YYYY` parse; (b) garbled/missing `Digitally Signed:` line variants (`unsplittable
  signature line`, `missing \`Digitally Signed:\` line`, seen 2016-2026); (c) a `gfedcb`
  checkbox-widget artifact bleeding into certification-section text (Task 4.9's own finding,
  distinct from Task 4.10's already-fixed band-artifact occurrence — same underlying PDF
  glyph-name-leak mechanism, different location in the document, needs its own fix at the
  signature-line call site).
  Investigate real evidence first (same discipline as prior Task 4.x fixes): fetch several real
  filings spanning 2014-2026 that exhibit each variant, confirm the exact shapes, then fix
  `extract_signed_date` (and whatever else in that area) additively — tolerate non-zero-padded
  dates, tolerate/strip the `gfedcb` artifact, and widen the `Digitally Signed:` line match to
  cover confirmed-real variants — without breaking the existing well-formed cases. Do not touch
  Tasks 4.5-4.10's already-closed logic, and do not attempt Task 4.12's row-grammar cluster
  (dispatched separately, possibly concurrently in the same file — keep this diff scoped to the
  signature/certification area only to minimize collision risk).
  Acceptance: tests proving each of the three sub-issues resolves against real evidence, with
  all existing tests continuing to pass. Re-run a real dry-run sample across a few affected years
  and confirm these specific failure modes' occurrence counts drop substantially.
  **Closed 2026-07-07**: investigated first, against real, independently live-fetched and
  sha256-pinned electronic PTRs spanning 2014-2022 (a live `--dry-run --limit 60` sweep of
  2014/2016/2018/2020/2022 against `govfolio_081_rehearsal` surfaced every variant at real scale;
  individual PDFs were then re-fetched directly from
  `https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/{year}/{doc_id}.pdf` and their
  `pdf_extract::extract_text_from_mem` text inspected verbatim — sha256 verified against each
  dry-run log line before use). Filing IDs cited below: #20000800 (2014, sha256
  40babda90c0d13a76da969956206164657a5d7004c8e49809fdfecf8f024ac9c), #20001787 (2014, sha256
  29bfb95acf4679614ded1fb085743c9eb4220bb9964169b850307f584b06d11c), #20004485 (2016, sha256
  58e99632e80ebe5418c206bdfa970056f6e3ff7f11217e71bc02208d7cd7dbf5), #20020708 (2022, sha256
  825a86bbd6895fc3e9d71913185bd1c2cc8a2840ca9809de26386b537cd580cb), #20001674 (2014, sha256
  65803f4906c94339619c94cc75550d610084d5898e458f53187c37d7c8352b6a), #20000708 (2014, sha256
  bfa02ca731327086bd2fe6d8d61408ebecb57d69652d1341e7adb39a8f19704a).
  Findings, per sub-issue: (a) confirmed exactly as named — real signature lines like `"Digitally
  Signed: Hon. Brad Ashford , 02/1/2016"` carry a non-zero-padded day (or month); the strict
  `is_date10` (exactly 10 bytes, `MM/DD/YYYY`) hard-rejects every one, even though
  `normalize::parse_source_date` already tolerates non-padded `%m/%d/%Y` downstream via chrono's
  own lenient numeric parsing — the ONLY blocker is this Silver-extraction-stage shape check.
  (b) two DISTINCT real shapes, not one: (b1) the `Digitally Signed:` label glued directly onto
  the end of a `Filing ID #NNNNN` footer line with no line break (`" Filing ID
  #20020708Digitally Signed: Hon. Jake Auchincloss , 04/10/2022"`, real #20020708) — the old
  `starts_with` prefix match missed it entirely, the same page-footer-glue pattern
  `extract_doc_id` already tolerates via `.find`; (b2) the label genuinely absent from the
  extracted text everywhere (checked directly — no NUL-survivor form either, confirmed via
  `grep -i digitally/signed` against real #20001674/#20000708/#20004720/#20016417/#20009092's
  full text: zero matches), while the signer name + date survive verbatim as the document's own
  last non-empty line (e.g. `"Mr. Vern Buchanan , 09/15/2014"`, real #20001674). (c) confirmed
  real and reproducible: a `gfedcb` token leads the certification paragraph's opening line,
  immediately after the "Certification and Signature" heading and before "I CERTIFY..." (`"gfedcb
  I CERTIFY that the statements..."`, real #20000708/#20004485/#20016099) — the same underlying
  PDF form-field glyph-name-leak mechanism as Task 4.10's `BAND_ARTIFACT_TOKENS`, at a different
  location; not directly observed attached to the `Digitally Signed:` line/value itself in this
  session's sample, but the same font-level mechanism, so the fix strips it defensively wherever
  it appears in signature-area text rather than only where directly observed.
  Fix (`crates/adapters/us_house/src/parse.rs`, `extract_signed_date` and three new private
  helpers only — no other function touched, all additive): `extract_signed_date` now (1) searches
  for `"Digitally Signed:"` anywhere in a line (`.contains`, not `.starts_with`) — fixes (b1); (2)
  when the label is absent from every line, falls back to scanning from the END of the document
  for the first line whose comma-split tail is itself a lenient date shape — anchored on date
  shape, not merely "has a comma", so it cannot mistake the certification paragraph's own prose
  commas for the signature line — fixes (b2); (3) validates the date via a new `is_lenient_date`
  (structural `M/D/YYYY`, 1-2 digit month/day, 4-digit year — a strict superset of `is_date10`,
  every existing zero-padded date still matches; calendar-range validity stays
  `normalize::parse_source_date`'s job downstream, unchanged) — fixes (a); (4) strips a new
  `SIGNATURE_AREA_ARTIFACT_TOKENS` (`["gfedc", "gfedcb"]`, a separate constant from Task 4.10's
  own `BAND_ARTIFACT_TOKENS` — that closed task's logic is untouched) leading-or-trailing token
  from candidate signature text via a new `strip_signature_area_artifact` helper — defends (c).
  `is_date10` itself, `BAND_ARTIFACT_TOKENS`/`strip_band_artifact`, `scan_rows`,
  `transactions_region`, `HEADER_BLOCK`, and `find_anchor` are all byte-for-byte untouched.
  8 new tests, all real-evidence-cited where the sub-issue calls for it:
  `is_lenient_date_tolerates_non_zero_padded_month_and_day` (unit),
  `extract_signed_date_accepts_real_non_zero_padded_date_evidence` (real #20004485),
  `extract_signed_date_accepts_real_filing_id_glued_line_evidence` (real #20020708),
  `extract_signed_date_falls_back_to_the_last_line_when_the_label_is_genuinely_absent` (real
  #20001674), `strip_signature_area_artifact_discards_a_leading_or_trailing_token_only` (unit,
  mirrors `strip_band_artifact`'s own Task 4.10 test shape),
  `extract_signed_date_fallback_tolerates_the_real_gfedcb_certification_paragraph_artifact` (real
  #20000708), and two end-to-end `parse_document_succeeds_against_real_..._evidence` tests
  (non-zero-padded date via #20004485; missing-label fallback + gfedcb artifact via #20000708).
  `cargo test -p us_house`: 65 passed + 1 ignored (was 57+1 at Task 4.10's close), every
  pre-existing test unchanged and green. `cargo fmt --check` and `cargo clippy --all-targets --
  -D warnings` both clean, workspace-wide; `cargo test --workspace`: 420 passed, 63 ignored, 0
  failed (was 412 at Task 4.10's close).
  Live re-verification against `govfolio_081_rehearsal`: `cargo run -p worker --bin backfill --
  adapter us_house --from <year> --to <year> --dry-run --limit 60` for 2014, 2016, 2020, and 2026
  (240 real sampled filings total), before vs. after the fix. `signature date "..." is not
  MM/DD/YYYY` (sub-issue a): 2014 8→0, 2016 7→0, 2020 0→0 (none in this sample), 2026 0→0 — fully
  eliminated in every sampled year. `missing \`Digitally Signed:\` line` (sub-issue b2): 2014
  2→0, 2016 3→0, 2020 18→11 (the remaining 11 directly re-fetched and confirmed a DIFFERENT,
  doubly-degraded real variant — the label is absent AND the trailing date's digits/comma are
  ALSO eaten, e.g. `"on. Donald Sternoff beyer r 0/0/2020"` with no comma anywhere — genuinely
  unrecoverable, correctly still hard-rejects; not a miss by this fix). `unsplittable signature
  line` (sub-issue b1, genuinely-garbled subset): IDENTICAL sets before/after in every year (2014
  2/2, 2016 2/2, 2020 14/14) — zero regression, these remain correctly fail-closed (no comma
  survives to split on; confirmed unrecoverable by direct re-fetch). REAL FINDING surfaced,
  explicitly NOT fixed (separate, out of scope, Task 4.12 territory): most of the newly-rescued
  2014 filings (all 8 non-padded-date + both missing-label cases) immediately hit the SEPARATE,
  already-documented 2014 shorter-3-line-header-block shape (`unrecognized table header block`)
  one call deeper in `parse_document` — expected, since `extract_signed_date` runs BEFORE
  `scan_rows`/`transactions_region` and previously never let these documents get that far; overall
  per-year failed-count is therefore roughly unchanged even though the specific signature/
  certification failure modes this task targeted are gone, exactly matching this task's own
  acceptance wording ("other, separately-tracked gaps... may legitimately remain").
- [x] **Task 4.12 — row-grammar cluster (blocks Task 5's full-range run).** Four real, related
  gaps in row/transaction-level parsing, accumulated across goal 080's original findings plus
  Tasks 4.8/4.9's own discoveries: (a) scrambled-case lowercase row-level type tokens (e.g.
  `unknown transaction type token Some("s")`, `Some("(partial)")`) failing `find_anchor`'s
  exact-uppercase match; (b) 2022's "unattached asset text after the last row"; (c) 2023's "band
  wrap not followed by a `$…` continuation"; (d) 2026's `L:` sub-line inside the Transactions
  region (goal 080's own original known issue) — plus, discovered by Task 4.9's investigation,
  2014's genuinely different, shorter 3-line table header-block shape (no Cap. Gains > $200?
  columns), which needs its own additive `HEADER_BLOCK`-equivalent variant.
  Investigate real evidence first for each sub-issue (same discipline as prior fixes) before
  changing anything — these are graphically distinct row-grammar issues, not one root cause; fix
  each on its own merits, additively, without weakening the existing well-formed row grammar. Do
  not touch Tasks 4.5-4.11's already-closed logic, and coordinate scope with Task 4.11 if
  dispatched concurrently in the same file (row-grammar vs. signature/certification-area diffs
  should not overlap).
  Acceptance: tests proving each of the (up to) five sub-issues resolves against real evidence,
  with all existing tests continuing to pass. Re-run a real dry-run sample across the affected
  years (2014, 2022, 2023, 2026 at minimum) and confirm these specific failure modes' occurrence
  counts drop substantially.
  **Closed 2026-07-07**: investigated real evidence first for each sub-issue independently (live
  `--dry-run --limit 60` sweeps for 2014/2018/2020/2022/2023/2026 against `govfolio_081_rehearsal`,
  individual real PDFs re-fetched directly from `https://disclosures-clerk.house.gov/public_disc/
  ptr-pdfs/{year}/{doc_id}.pdf` and sha256-verified before use). All FIVE named sub-issues
  confirmed as genuinely distinct root causes and fixed, all in
  `crates/adapters/us_house/src/parse.rs`, additively alongside every existing exact-case/
  NUL-survivor/scrambled-case form (none weakened):
  (a) **scrambled-case row-level type token**: `find_anchor`'s match arms now accept `"P"`/`"S"`/
  `"E"`/`"(partial)"` case-insensitively and normalize to the canonical uppercase form
  `normalize::normalize_row` expects (a strict superset — an already exact-case token round-trips
  unchanged). Real evidence: Filing IDs #20016288 (2020, sha256
  7774958acf4269ed3270638a520b2f61fe6b908d62d053ae226425788c2f86f7, lowercase `"s"` alone),
  #20009743/#20010366 (2018, sha256 2300e59b82f02b7d23b4df3457a603cfd5e83819c0280fa5462775c81bccfa61
  and 19d5e4e536fddb558d55a5813af1a06d77b107b44a24c7e388b634d3d7b27d83, `"s (partial)"`), #20000077
  (2014, sha256 ea936ce15201393a2fbfc61c9e9670e016fd5c6b0010aae8b750e34ebc924691, `"s"` alone — the
  SAME document also evidencing (e) below).
  (b) **"unattached asset text after the last row" (2022)**: root cause is a sub-line's own
  free-text VALUE (Comments/Description) itself wrapping onto further physical lines with no
  repeated label — the old grammar had no continuation-join for sub-line values (unlike the
  pre-anchor asset-name `pending` join and the amount band-wrap join, which already existed). Fixed
  via a new `mid_sublabel` loop-state in `scan_rows`: immediately after a `match_sublabel` hit, a
  plain orphan line is joined onto that same sub-line's value (space-joined) instead of falling
  into `pending`/failing closed; cleared on a blank line, a page-boundary footer, a header-block
  reprint, or the next anchor, so ordinary single-line values (the overwhelming common case, always
  followed by a blank line in every real document sampled) are unaffected. Real evidence: Filing
  IDs #20021740 (2022, sha256 6ba941b0a3d5047c95d1eb4724322ce46ec3cde0ff153b3aa591f7d6c06d697f,
  Comments — the task's own named case), #20022126 (2023, Comments), #20034044 (2026, Comments),
  #20034201 (2026, sha256 6372d59d32a7e54b69e4b456c315670456aa625572a5c78e5c29b72a81de2d43,
  Description).
  (c) **"band wrap not followed by a `$…` continuation" (2023)**: root cause is a page break
  landing between a wrapped band's hyphen and its `$…` continuation, with blank lines / the
  per-page `Filing ID #` footer / a repeated header-block reprint in between (the old code only
  ever peeked exactly one line ahead). Fixed via a new `is_page_boundary_furniture` helper plus a
  skip-forward loop before the "does it start with `$`" check — any run of known page-boundary
  furniture is skipped additively; a continuation on the very next line (the common case) is
  unaffected. Real evidence: Filing IDs #20023082 (2023, sha256
  35f8d99e4c84d26ddebb219499c9f41bbff56dcdc0ef893e4962623642e0316e, blank + header-block reprint),
  #20023623 (2023, sha256 f0d3292c5db2b46013ef382bf0fc3e673144ba7820f225c845beea57ff812463, the
  `Filing ID #` footer directly then a header-block reprint, no blank at all before the footer).
  (d) **`L:` sub-line inside the Transactions region (2026, goal 080's own original known issue)**:
  root cause is a Location sub-line attached directly to a transaction row — not only to an
  Investment Vehicle Details bullet, the only place it was previously recognized — in both the
  NUL-survivor (`L:`) and scrambled-case full-word (`LoCaTIoN:`) forms. Fixed by no longer
  hard-bailing on `SubLabel::Location` inside `scan_rows`: it now populates a new
  `RowDraft.location_raw` field, which `parse_document` feeds into the EXISTING
  `vehicle_location_raw` Gold field (no schema change — that field already means "the location tied
  to this row's holding/vehicle"), taking precedence over, and falling back to, the vehicle-bullet
  join exactly as before. Real evidence: Filing IDs #20020708 (2022, sha256
  825a86bbd6895fc3e9d71913185bd1c2cc8a2840ca9809de26386b537cd580cb), #20022368/#20022428/#20024042
  (2023), #20016088 (2020, sha256 1ea1a47b83870a9f3ff0bf3310f56ee285dacc13530afbbf48509a6bca57f34c,
  scrambled full-text form `"LoCaTIoN: Malvern, Pa, US"`), #20034201/#20033744 and many more (2026).
  (e) **2014's shorter 3-line table-header-block shape** (discovered by Task 4.9's investigation): a
  genuinely different, shorter shape — no "Cap. Gains > $200?" columns at all — `"ID Owner Asset
  Transaction"` / `"Type Date Notification"` / `"Date Amount"` only. Fixed via an additive
  `HEADER_BLOCK_SHORT` const plus a restructured detection in `scan_rows` that checks the 3rd line
  against BOTH the modern `"Date Amount Cap."` and the short `"Date Amount"` before falling into
  the standard 5-line loop — the modern 5-line block keeps matching byte-for-byte as before. Real
  evidence: Filing ID #20000077 (2014, see above — scrambled-case `"iD owner asset transaction"` /
  `"type Date notification"` / `"Date amount"`, directly fetched and dumped this session); the
  exact-case rendering of the same shorter shape independently corroborated via the live dry-run's
  own fail-closed report for Filing ID #20002042 (2014, sha256
  c0921665f767172970259a4b2fb7e727af03a64aec0136ff8bb92909db84dd3b).
  11 new tests, all real-evidence-cited: `find_anchor_accepts_real_scrambled_case_type_tokens`,
  `scan_rows_joins_a_real_wrapped_comments_sub_line_continuation`,
  `scan_rows_joins_a_real_wrapped_description_sub_line_continuation`,
  `scan_rows_accepts_a_real_band_wrap_across_a_page_break_with_a_header_reprint`,
  `scan_rows_accepts_a_real_band_wrap_split_by_a_filing_id_footer_and_header_reprint`,
  `scan_rows_accepts_a_real_row_level_location_sub_line_inside_the_transactions_region`,
  `scan_rows_accepts_a_real_scrambled_case_full_text_location_sub_line_in_a_row`,
  `parse_document_feeds_a_rows_own_location_sub_line_into_vehicle_location_raw` +
  `parse_document_still_falls_back_to_the_vehicle_bullet_location_without_a_row_l_sub_line`
  (regression guard for the same `vehicle_location_raw` wiring change),
  `scan_rows_accepts_the_real_2014_three_line_header_block_shape`, and an end-to-end
  `parse_document_succeeds_against_the_real_2014_three_line_header_and_type_token_evidence`.
  `cargo test -p us_house`: 75 passed + 1 ignored (was 65+1 at Task 4.11's close), every
  pre-existing test unchanged and green. `cargo fmt --check` and `cargo clippy --all-targets --
  -D warnings` both clean, workspace-wide; `cargo test --workspace`: 431 passed, 63 ignored, 0
  failed (was 420 at Task 4.11's close).
  Live re-verification against `govfolio_081_rehearsal`: `cargo run -p worker --bin backfill --
  adapter us_house --from <year> --to <year> --dry-run --limit 60` for 2014, 2018, 2020, 2022,
  2023, and 2026, before vs. after. Each sub-issue's OWN named failure text dropped to **zero** in
  every sampled year that exhibited it: `unknown transaction type token` (2018 2→0, 2020 2→0);
  `unattached asset text after the last row` (2022 1→0, 2023 1→0, 2026 2→0); `band "..." wrap not
  followed by a` `$…` `continuation` (2023 2→0); `L: sub-line inside the Transactions region` (2020
  1→0, 2022 1→0, 2023 3→0, 2026 8→0); `unrecognized table header block` (2014 30→0). Per-year adds
  moved 2022 24→25, 2023 24→26, 2026 33→37 (documents that used to hard-reject now flow through to
  a real Gold classification); some formerly-hard-rejecting rows (e.g. Filing IDs #20000077,
  #20020708) now correctly parse but route to the pre-existing, already-benign
  `needs_llm_extraction` seam instead (low mean confidence from the SAME scrambled/loose match that
  unblocked them — no `ANTHROPIC_API_KEY` configured in this dev environment — exactly invariant 6
  working as designed, not a miss).
  REAL FINDINGS surfaced, explicitly NOT fixed (separate from the five named sub-issues, discovered
  by this task's own re-verification once the (e)/(a) fixes let previously-blocked 2014/2018-era
  documents parse further than ever before):
  (1) **non-zero-padded TRANSACTION/NOTIFICATION dates** (distinct from Task 4.11's already-fixed
  SIGNATURE date — a different field, `find_anchor`'s strict `is_date10`, not
  `extract_signed_date`): e.g. real Filing ID #20000800 (2014) `"...P 11/1/2013 11/1/2013
  $100,001 - $250,000"`, #20001769 (2014) `"...P 09/8/2014 09/8/2014 $15,001 - $50,000"` — the
  anchor is never recognized at all, so the whole row becomes unattached text that then collides
  with the row's own sub-line block, surfacing as `sub-line "..." amid unattached asset text`. This
  is now the single most common 2014/2018 failure post-fix. A likely low-risk fix (reuse the
  existing `is_lenient_date` helper Task 4.11 already built, in `find_anchor` instead of
  `is_date10`) but NOT one of the five sub-issues this task was scoped to — left undone, flagged
  for a follow-up task.
  (2) **an asset name splitting across a page break such that the anchor (type/dates/amount)
  already completes on the FIRST page while the remaining name fragment lands on the FOLLOWING
  page, after a header-block reprint, unattached before the row's own sub-line block** — real
  evidence: Filing ID #20033762 (2026, `"SBA Communications Corporation -"` on the anchor line,
  then, after a page break + header-block reprint, `"Class A Common Stock (SBAC) [ST]"` orphaned
  before the next `F S: New`), #20033983 (2026, `"Eaton Corporation, PLC Ordinary"` /
  [page break] / `"Shares (ETN) [ST]"`). Produces the SAME `sub-line "..." amid unattached asset
  text` failure text as (b) but is a structurally different cause (asset text trailing AFTER its
  own row's anchor, not a sub-line value wrapping). Confidently disambiguating this from an
  ordinary new row's own short asset-name preamble needs a lookahead heuristic (peek past the
  header-block-reprint boundary to see whether the very next real content is another anchor — new
  row — or a sub-label — orphaned continuation of the PREVIOUS row) that is qualitatively riskier
  (more surface for silently mis-attributing real asset-name text to the wrong row) than any of the
  five sub-issues actually assigned here. Left correctly fail-closed, not attempted; flagged for
  whoever picks up the next narrowing pass, with the disambiguation strategy above if pursued.
  (3) **a genuinely new, unrecognized "Asset Class Details" section heading** in the 2014-era
  document anatomy (scrambled-case `"aSSet claSS DetailS"`), appearing after a `SUBHOlDINg OF:`
  sub-line and before Comments — not in `is_next_section_heading`'s vocabulary, so
  `transactions_region`'s end boundary scans past it, sweeping it (and whatever follows, up to the
  next recognized heading) into `pending`, surfacing as `unattached asset text after the last row`.
  Real evidence: Filing IDs #20001087/#20001259 (2014, `["aSSet claSS DetailS", "UBS Financial
  Services Inc. Traditional IRA"]`). A likely easy, additive fix (one more entry in
  `is_next_section_heading`'s `FULL_FORMS`) but, like (1) and (2), not one of the five sub-issues
  this task was scoped to — left undone, flagged for a follow-up task.
  These three are genuinely new discoveries, not a re-litigation of anything Tasks 4.5-4.11 already
  closed; the already-named, already-tracked gaps (non-zero-padded/garbled signature dates,
  `Digitally Signed:` line variants, the `gfedc`/`gfedcb` artifacts, the 2014 footnote-absence gap,
  `needs_llm_extraction` with no `ANTHROPIC_API_KEY` configured) recur as expected and are not
  re-litigated here either.
- [x] **Task 4.13 — non-zero-padded transaction/notification dates in `find_anchor` (blocks
  Task 5's full-range run; flagged by Task 4.12 as the dominant remaining 2014/2018 blocker).**
  Real finding, distinct from Task 4.11's already-fixed SIGNATURE date case: row-level
  transaction/notification dates (the two adjacent `MM/DD/YYYY` tokens `find_anchor` anchors a
  row on) can also be non-zero-padded (e.g. a single-digit month or day), and `find_anchor`'s
  own date-shape check is the same strict `is_date10` the signature-date fix already proved too
  strict for real historical data. `is_lenient_date` (added in Task 4.11, a confirmed strict
  superset of `is_date10`) already exists and is reusable here directly — this is expected to be
  a small, low-risk, additive swap, not a fresh investigation.
  Fix: use `is_lenient_date` (not `is_date10`) wherever `find_anchor` identifies the row-anchor
  date pair, additively — well-formed zero-padded rows must keep matching exactly as before. Do
  not touch Tasks 4.5-4.12's already-closed logic beyond this one swap.
  Acceptance: a test proving a row with a real non-zero-padded transaction/notification date
  (matching Task 4.12's own cited real examples if practical) now anchors and parses correctly,
  with all existing tests continuing to pass. Re-run a real dry-run sample against 2014/2018 and
  confirm this specific failure mode's occurrence count drops substantially.
  **Closed 2026-07-07**: exactly the swap this task scoped —
  `crates/adapters/us_house/src/parse.rs`'s `find_anchor` now checks both anchor date tokens with
  `is_lenient_date` instead of `is_date10` (Task 4.11's confirmed strict superset — every
  existing zero-padded row keeps matching, proven by `anchor_splits_type_dates_and_band` and the
  2-digit-year-rejection case in `non_anchor_lines_pass_through`, both still green unmodified).
  Real evidence, reusing Task 4.12's own cited example directly: Filing ID #20000800 (2014, pdf
  sha256 40babda90c0d13a76da969956206164657a5d7004c8e49809fdfecf8f024ac9c, independently
  re-fetched and re-verified this session — sha256 matches exactly), whose real row `"...P
  11/1/2013 11/1/2013 $100,001 - $250,000"` was previously invisible to `find_anchor` entirely
  (`is_date10` hard-rejects a 1-digit day), falling into `unattached asset text after the last
  row`.
  One consequence surfaced by the swap, not by any separate investigation: `is_date10` had
  exactly two call sites in the whole crate, both inside `find_anchor` — with both swapped to
  `is_lenient_date`, `is_date10` became fully dead code (the goal text's premise that it was
  "still used elsewhere for signature dates' base check" does not hold — `extract_signed_date`
  calls `is_lenient_date` directly and never called `is_date10`, confirmed by grep across the
  whole crate before making this change). Left in place, `cargo clippy --all-targets -- -D
  warnings` fails on `dead_code`. Removed the now-unused `is_date10` function per root
  CLAUDE.md's own orphan-cleanup rule ("Remove imports/variables/functions that YOUR changes
  made unused") — no other logic touched, `is_date10`'s doc comments elsewhere (Task 4.11's,
  describing `is_lenient_date` as its strict superset) are left as-is, unmodified per "touch only
  what you must".
  2 new tests, both real-evidence-cited:
  `find_anchor_accepts_real_non_zero_padded_transaction_and_notification_dates` (unit, real
  #20000800 line, plus a same-test zero-padded regression check) and an end-to-end
  `parse_document_succeeds_against_real_2014_non_zero_padded_row_date_evidence` (real header
  block + asset-name + anchor line from #20000800, spliced with clean synthetic doc
  id/filer-info/signature and a single clean `FILINg sTaTUs:` line — isolates this fix from a
  separate, out-of-scope gap this session found in the SAME real document: its DESCRIPTION
  sub-line value wraps across a genuinely BLANK line, which Task 4.12(b)'s mid-sub-line join
  deliberately does not bridge, so the full, unedited real document still fails closed today on
  `unattached asset text after the last row: ["requirement to report."]` — a different root
  cause, not fixed here). `cargo test -p us_house`: 78 passed + 1 ignored (baseline measured
  directly at this task's start, on the last-committed tree: 76 passed + 1 ignored — the goal
  file's own prior "75+1 at Task 4.12's close" note appears to be a pre-existing one-off
  undercount, not something this task re-litigates). `cargo fmt --check` and `cargo clippy
  --all-targets -- -D warnings` both clean, workspace-wide; `cargo test --workspace`: 433 passed,
  63 ignored, 0 failed (was 431 at Task 4.12's close — exactly +2, matching the 2 new tests).
  Live re-verification against `govfolio_081_rehearsal`: `cargo run -p worker --bin backfill --
  adapter us_house --from <year> --to <year> --dry-run --limit 60` for 2014 and 2018, before
  (stashed fix) vs. after. The direct symptom of this failure mode —
  `sub-line "..." amid unattached asset text — grammar break`, caused by the row's own anchor
  never being recognized so its later FILING STATUS/LOCATION sub-line collides with the
  unattached asset-name text — dropped substantially in both years: 2014 18→7, 2018 9→4 (both
  real Filing IDs #20000800 and #20001769, Task 4.12's own two named examples, confirmed directly:
  #20000800 now progresses past the row entirely to the separate DESCRIPTION-wrap gap noted
  above; #20001769 now parses successfully through to the benign `needs_llm_extraction` seam).
  Per-year `adds` stayed at 0/60 in this particular 60-sample slice for both years — expected and
  not a miss: most formerly-hard-rejecting rows now correctly parse but route to the
  pre-existing, benign `needs_llm_extraction` seam (no `ANTHROPIC_API_KEY` configured), exactly
  invariant 6 working as designed, matching Task 4.12's own close-out precedent.
  REAL FINDINGS surfaced, explicitly NOT fixed (separate, out of scope per this task): the
  remaining 2014 occurrences were independently re-fetched and confirmed to be Task 4.12's own
  already-named finding (3) — the unrecognized "Asset Class Details" section heading (real
  Filing ID #20000780, pdf sha256
  af3d1d3e89ee1d0475cfde8d93f0c823966c8eb7cccc5679841601febb656d9a, confirmed directly), not a
  date issue. The remaining 2018 occurrences surface a genuinely NEW, previously undocumented
  artifact: a standalone `"/."` token (distinct from the already-known `gfedc`/`gfedcb` glyph-leak
  family) trailing a mid-document `Filing ID #` page-footer, orphaned before the next row's own
  `FILINg sTaTus:` sub-line — real evidence, independently re-fetched: Filing ID #20010109 (2018,
  pdf sha256 7c5cbcb691b686fd0e01c3ffc11a96ac7b4581b861795771ad8c34b0b2a16d1d), text verbatim
  `"...gIlD) [sT] P 07/17/2018 08/15/2018 $1,001 - $15,000 gfedc"` / blank / `"Filing ID
  #20010109"` / blank / `"/."` / blank / `"FIlINg sTaTus: New"`. Left correctly fail-closed, not
  attempted; flagged here for whoever picks up the next narrowing pass.
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
