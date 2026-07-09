# 092 — br historical backfill: remaining pre-2018 + post-2022 general-election years

## Objective
Extend `br`'s real historical backfill (Câmara + Senado, both bodies, one invocation per
year via the existing `seed-br-candidates`/`backfill-real-br` bins) past the two years
already proven (2018, 2022) to the remaining general-election years, using the SAME code
path unchanged — no new adapter/parsing code.

## Scope
General-election cadence: `year % 4 == 2` (2002, 2006, 2010, 2014, 2018✓, 2022✓, 2026).

In:
- **2014** — real write. Same modern `bem_candidato` CSV schema as 2018/2022 (confirmed,
  `docs/regimes/br/AUTHORITY.md` Quirks log), so this is a genuine new real-write year:
  seed → dry-run gate vs `BACKFILL_BUDGET` → real write → idempotency re-check (2nd
  invocation) → `check-br-identity-collisions` → journal → commit.
- **2010, 2006** — attempt for real via the actual bins (not re-derive from the prior
  sampler-only doc note); AUTHORITY.md already documents `bem_candidato`'s CSV column
  layout differs from 2014+ and the adapter fails closed (invariant 6) rather than
  misparsing. Expect the SAME fail-closed outcome from `seed-br-candidates`/
  `backfill-real-br` directly. This is a documented, already-known gap — verify, journal,
  do NOT write new parsing code to support the old schema (out of scope, needs a real
  design increment).
- **2002** (and, if reached, 1998/1994) — `bem_candidato` 404s (no itemized asset data
  exists this far back per AUTHORITY.md). Attempt, expect clean per-year fail-closed
  (same mechanism already proven for 2019/2021 non-election years), journal. Do not chase
  every year back to 1933 — stop once the "no bem_candidato data" class of gap is
  reconfirmed once or twice; going further back is the same already-understood gap.
- **2026** (post-2022) — single cheap availability check only (one conditional
  GET/HEAD against the `candidatos-2026`/`bem_candidato_2026` TSE resources, concurrency
  1). The election itself is Oct 2026 and today is 2026-07-09 — expect NOT YET PUBLISHED.
  Journal the finding; no write attempt if unavailable.

Out:
- New CSV-schema-variant parsing for 2006/2010 (real code change, needs its own
  scout/spec pass — file as a follow-up, do not build here).
- Any code change to `resolve_politician`/cross-cargo or cross-year identity resolution
  (documented residual risk, not this goal's job — see AUTHORITY.md Quirks log and
  `agents/JOURNAL.md` 2026-07-07 entries). A NEW cross-body/cross-time collision distinct
  from the already-fixed `JULIO CESAR DOS SANTOS` case is a FINDING to flag in
  `agents/JOURNAL.md`, not a fix-in-place.
- Widening past `DEPUTADO FEDERAL`/`SENADOR`/suplentes (already done) to any other cargo.

## Context (read first)
- `docs/regimes/br/AUTHORITY.md` — Quirks log (2006/2010 schema mismatch, 2002/1998/1994
  404, TSE election-year cadence, CPF/voter-title unmasked-PII handling, cross-year
  identity-match caveats) and open_questions.
- `crates/worker/src/bin/seed-br-candidates.rs` / `backfill-real-br.rs` — current bins,
  already handle BOTH bodies (Câmara+Senado) in one invocation; per-year fail-closed
  isolation is by design (module doc comments).
- `crates/worker/src/bin/check-br-identity-collisions.rs` — standing collision sweep,
  report-only, run after each new year's real write.
- `docs/runbooks/dev-host-windows.md` — local pg (5433), `pg-local.ps1`, no-psql
  workarounds, DB Toolbox MCP for interactive SQL.
- `[[subagent-orchestration-lessons]]` (memory) — isolate `CARGO_TARGET_DIR` from other
  concurrent agents in this shared repo; never assume a background loop's turn ending
  means the underlying process stopped.
- Invariant 10 (politeness): concurrency 1 per source regardless of how many years this
  goal touches in one session.

## Acceptance criteria (per year attempted)
```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
# per year Y (real-write years only, i.e. 2014 initially):
cargo run -p worker --bin seed-br-candidates -- --from Y --to Y
BACKFILL_BUDGET=50000 cargo run -p worker --bin backfill-real-br -- --from Y --to Y
# dry-run record_delta must have been checked against BACKFILL_BUDGET before the line above
# idempotency: re-run the same backfill-real-br invocation a second time — 0 new Gold rows
BACKFILL_BUDGET=50000 cargo run -p worker --bin backfill-real-br -- --from Y --to Y
cargo run -p worker --bin check-br-identity-collisions -- --pass PASS
```
Fail-closed years (2010, 2006, 2002, …) only need the seed/backfill bins run once each,
with output confirming the SAME already-documented fail-closed reason (not silently
different) — no idempotency re-check needed since nothing new was written.

## Checklist
- [ ] 2014: seed + dry-run gate + real write + idempotency re-check + collision check + journal + commit
- [ ] 2010: verify (expect fail-closed, schema mismatch) + journal (no code change)
- [ ] 2006: verify (expect fail-closed, schema mismatch) + journal (no code change)
- [ ] 2002: verify (expect fail-closed, no asset data) + journal (no code change)
- [ ] 2026: availability check only + journal

## BLOCKED (human)
(empty — schema-variant parsing for 2006/2010 is a flagged follow-up, not a human blocker)
