# 080 — US backfill + launch

## Objective
Run the same pipeline over US archives back to 2012; human-gated diff review; complete docs/runbooks/launch-checklist.md (SLOs live, legal pages, budget alerts, status page).

## Acceptance criteria
```bash
cargo run -p worker --bin backfill -- --adapter us_house --from 2012 --dry-run   # human reviews diff, then real run
```

## Checklist
- [x] dry-run diff — `backfill` bin + diff report (adds/changes/supersessions), bounded + polite,
  writes nothing; acceptance green (7,544 PTRs discovered 2012→2026). `crates/worker/src/backfill.rs`
  + `crates/worker/src/bin/backfill.rs`; tests `crates/worker/tests/backfill.rs`.
- [ ] real run — HALT (see below)
- [x] SLO dashboards — documented in `docs/runbooks/launch-checklist.md` §2 (signals exist;
  dashboard WIRING is infra-blocked on goal 020).
- [ ] legal pages — human-lane; REQUIRED artifacts listed in launch-checklist §5 (copy NOT written).
- [x] checklist done — `docs/runbooks/launch-checklist.md` (each item tagged
  buildable-done / infra-blocked / human-lane).

## Findings (dry-run over live archive, 2026-07-06)
- Historical `{YYYY}FD.zip` EXISTS + parses back to 2012. This ANSWERS the us-house SAF's
  historical-depth open question — but the SAF is a FROZEN E1 eval reference
  (`role_evals_reference_bundle_frozen` pins its sha); editing it drifts the freeze, so folding
  this finding into the SAF is a deliberate re-freeze, a goal-016 follow-up. Recorded here + in
  the adapter `discover_year` doc comment instead.
- PTR (FilingType `P`) e-filing begins ~2015: 2012–2014 hold **zero** P rows (valid empty, not a
  failure). Per-year P counts: 2015=728 … 2018≈830 (peak) … 2026=274 (partial). Total 7,544.
- Bounded dry-run fail-closed two real 2026 filings the 5-fixture adapter doesn't yet handle:
  (a) `L:` LOCATION sub-line inside the Transactions region (DocID 20034201); (b) paper/scanned
  filings → LLM seam. Both fail closed per-filing (invariant 6) without sinking the run — the dry
  run is the tool that enumerates them. Adapter-hardening follow-up in launch-checklist §1.
- Fixed en route: conditional-GET validators were shared across year URLs → `If-Modified-Since`
  false-304'd every later year (whole sweep read empty). Now keyed per year
  (`crates/adapters/us_house/src/adapter.rs`).
- Backfill audit trail: reuse `pipeline_run` (a backfill is the same pipeline, more years); no new
  table, no migration. Rationale in launch-checklist §1.

## HALT (human/infra) — the real (write-to-prod) backfill + launch gates
The dry-run half is buildable-done. The real run and the launch sign-offs are genuine
human/infra HALTs (fail closed; the loop continues other work):

1. **Real write-to-prod backfill** — needs, IN ORDER:
   a. Founder runs `gcloud auth application-default login` once (ADC — inherited from goal 020
      HALT; interactive, agents must never attempt it — `docs/runbooks/deploy.md`).
   b. Apply the cloud substrate: `terraform -chdir=infra plan` → `check-tf-plan.sh` → apply
      (within DESTROY_BUDGET; billing counts against the HARD CAP).
   c. Run `cargo run -p worker --bin backfill -- --adapter us_house --from 2012` **without**
      `--dry-run`, against the applied substrate. (The bin refuses to run without `--dry-run`
      until then, printing these preconditions.)
   d. **Founder reviews the emitted diff and gives go/no-go** before any mass supersession is
      promoted (design §5.6: reprocessing is human-gated for mass changes).
2. **Legal / methodology PUBLIC pages** — residual human lane (`/CLAUDE.md`): privacy, terms,
   methodology (per launch regime), corrections policy, takedown/redaction contact, pricing copy.
   Listed as REQUIRED artifacts in launch-checklist §5; the loop must not author public legal copy.
3. **Launch go/no-go** — human decision (launch-checklist §6).
