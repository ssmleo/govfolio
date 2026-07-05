# 021 — LLM extraction fallback

## Objective
Implement the extractor interface stubbed in plan Task 8: schema-constrained LLM extraction for low-confidence/scanned PDFs, sha-cached, confidence-scored, second-model cross-check for high-impact rows.

## Context (read first)
- design §5.3, §4.3 · crates/pipeline/src/adapter.rs

## Acceptance criteria
```bash
cargo test -p pipeline extraction
cargo run -p pipeline --bin conformance -- us_house   # scanned fixture case goes green
```

## Checklist
- [ ] extractor iface impl  - [ ] cache by sha+version  - [ ] confidence  - [ ] cross-check  - [ ] scanned fixture
  - first leg DONE 2026-07-05 (test-designer): fixture captured at
    `crates/adapters/us_house/fixtures-llm/scanned_paper_ptr/` (DocID 9115811, sha
    `2f4b2b6e98e044e6368a072275804bc61dda52f6f1e15c09ddb9074ea1b8952c`, text layer proven
    absent) with independent visual-transcription expecteds; capture record + paper-form
    conventions + flagged uncertainties in `crates/adapters/us_house/fixtures-llm/MANIFEST.json`.
    Parked in `fixtures-llm/` because conformance `run_cases` + `e2e_local.rs` (asserts 4 dirs)
    + `factory.rs` (cases<->dirs bijection) auto-discover `fixtures/` — red-CI guard.

## BLOCKED — E1 lock supersede needed before second leg lands (2026-07-05, test-designer)
Context: `docs/regimes/us-house/reference/E1.lock.json` sha-pins `fixtures/MANIFEST.json`
and `docs/regimes/us-house.md`; supersede is founder-gated per
`docs/decisions/role-eval-thresholds.md` (and test-designer is a SCORED role — must not
amend its own reference corpus). The first-leg SAF write-backs are therefore RECORDED but
NOT applied to the pinned files. Whoever supersedes the lock (v2 bump + note) applies:
1. `docs/regimes/us-house.md`: resolve open question "Do paper PTRs have any text layer?"
   (answer: NO — pdftotext emits 1 byte, a lone form-feed, on E13/9115811); add evidence row
   E13 (9115811.pdf sha above, retrieval log `evidence/f312caf4….retrieval.json`); add §7 row 5
   (scanned paper PTR) + quirks entry for paper-form anatomy (no Filing ID, NAME lacks `Hon.`,
   no signature block — clerk received stamp only, no cap-gains column, no [XX] codes,
   checkbox vocabulary) — full text staged in `fixtures-llm/MANIFEST.json`.
2. `fixtures/MANIFEST.json`: move the scanned_paper_ptr entry from `fixtures-llm/MANIFEST.json`
   into `cases`, add `Diana Harshbarger|TN01 -> 0HSEMBR0000000000000000005` and
   `9115811 -> 0HSEFNG0000000000009115811` to conformance_ids.
Builder second leg (same PR as, or after, the lock supersede): implement the seam, prime the
sha-keyed cache from expected.silver.json (case is LLM-path: NO text parse), add the ULID
mapping to normalize.rs, move the case dir into `fixtures/`, bump e2e's expected case count
4 -> 5, tick the scanned-fixture box when `conformance -- us_house` prints 5/5 green.
Options considered: (a) supersede lock autonomously — rejected (founder-gated + scored role
amending own reference); (b) skip write-back recording — rejected (SAF discipline);
(c) record here + fixtures-llm manifest, defer pinned-file edits to the lock supersede — CHOSEN.
