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
- [x] extractor iface impl  - [x] cache by sha+version  - [x] confidence  - [x] cross-check  - [x] scanned fixture
  - second leg DONE 2026-07-05 (rust-builder): `pipeline::extraction`
    (`AnthropicExtractor` machinery: reqwest+rustls Messages client, no SDK, forced
    tool use with the silver-row schema as `input_schema` + local jsonschema
    re-validation, retries w/ exponential backoff, key never logged —
    `HttpTransport` Debug redacts; models env-overridable, defaults
    `claude-haiku-4-5-20251001` primary / `claude-sonnet-5` cross-check);
    cache key `(document_sha256, extractor_tag, model_id)` in two tiers (committed
    in-case `extraction.cache.json` + pg `extraction_cache`, migration 0004
    expand-only, migrate pin 4→5); conformance cache PRIMED MECHANICALLY from
    expected.silver.json via `prime_from_expected_silver` (provenance records
    ground truth 77740d8, equality enforced by
    `cargo test -p pipeline extraction_cache_entry…`); confidence 0.9 f32 stamped,
    <0.9 or schema-invalid cache entries fail closed; cross-check per SAF §6.3
    (bands ≥ $500,001 — NOTE: dispatch prompt said $1M, SAF is authoritative —
    + watchlist stub `WATCHLIST_POLITICIANS: &[] `), field-level compare, mismatch =
    `CrossCheckMismatch` + `llm_crosscheck_mismatch` review_task + freeze; live
    test `#[ignore = "needs ANTHROPIC_API_KEY"]` (skips loudly without key);
    scanned fixture moved into `fixtures/`, staged MANIFEST entry + conformance
    ids applied, `conformance -- us_house` 5/5 GREEN OFFLINE, e2e 4→5 filings
    (13 gold/outbox rows; paper filer resolves via new prefix-less canonical
    alias seeding in `seed_roster`); clerk-stamp date parser (`2026 MAY -6` →
    2026-05-06) for signed/filed dates. Live-mode note: on a cache miss the DocID
    is threaded from `raw_document.source_url` (§2.3 URL shape) since the paper
    form prints no Filing ID; a live extraction without pool/indexed URL fails
    closed.
  - E1 lock SUPERSEDED to v2 (2026-07-05, per this goal's founder-approved
    BLOCKED staging): `E1.lock.json` now `version: 2`,
    `supersedes: b4238f01962cde59bdf459ec7d2d84949cc02d428fc5160667af3d178c4c1c1d`
    (sha256 of the v1 lock file's LF bytes, verified against `git show HEAD:`),
    reason + date recorded in the lock; v2 re-pins the two amended files
    (fixtures/MANIFEST.json, docs/regimes/us-house.md — staged write-backs
    applied: open question resolved, §7 row 5, E13, paper-anatomy quirks) and
    adds pins for the scanned case (input/silver/gold + ground-truth-derived
    extraction.cache.json); all other v1 pins byte-identical.
    `pipeline::evals::reference` verifies the supersession trail (v2+ without
    supersedes/reason/date fails closed); rust-builder scorer marker 4/4→5/5.
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
