# skill: conformance-diffing
Purpose: read and extend adapter conformance
Load when: verifying adapters, bouncing builds
Core checklist:
- run conformance bin -> read unified diff -> classify: parser bug vs fixture wrong vs regime change -> route accordingly
Anti-patterns: editing expected.json to make red green without human
Learnings (dated):
- 2026-07-04: the commanded runner (`cargo run -p pipeline --bin conformance -- <x>`)
  cannot link adapter crates — they depend on pipeline for the trait and cargo rejects
  normal-dep cycles. Pattern: every adapter ships a 3-line `conformance_entry` bin
  calling `pipeline::conformance::adapter_entry`; the pipeline bin re-execs it via
  `$CARGO`. Dev-dep cycles ARE legal, so pipeline's integration tests link
  fixture_fake directly. Name adapter entry bins uniquely (`conformance_entry`, never
  `conformance`): same-named bins across workspace packages clobber one
  target/debug artifact, and Windows cannot overwrite the outer exe while it runs.
- 2026-07-04: deliberately-broken fixtures live OUTSIDE the default dir
  (`fixtures-broken/` beside `fixtures/`, with a README saying "do not fix"): the bin
  only reads `fixtures/`; the harness test points at `fixtures-broken/` explicitly and
  asserts the FAIL + unified diff — a harness that cannot fail proves nothing.
- 2026-07-04: deep-compare parsed `serde_json::Value` (key-order-insensitive), then
  diff pretty-printed strings via `similar`; assert on `-`/`+` lines and `@@`, not
  whole-diff string equality. Missing (regime, record_type) details schema is a case
  FAILURE (fail closed), not a skip.
- 2026-07-04: expected.*.json floats — serde_json's DEFAULT float parsing is
  best-effort and can land 1 ulp off (`0.9800000190734863` parsed as `…864`), so a
  fixture that pins the exact f64 image of an f32 confidence deep-compares UNEQUAL
  against a bit-identical actual. Harness-side fix, not a fixture edit: enable
  serde_json's `float_roundtrip` feature in pipeline (serialize side was already
  exact). Diff smell: expected/actual differ only in a float's last digit.
- 2026-07-05: with 3+ adapters the shared `conformance_entry` bin name emits cargo's
  output-filename-collision warning (`target/debug/conformance_entry.exe`) — benign under
  the dispatcher (each `cargo run -p <adapter> --bin conformance_entry` relinks its own
  package's bin) and pre-existing since fixture_fake+us_house; do not "fix" by renaming,
  the dispatcher contract requires the constant bin name.
- 2026-07-05: when a page never prints its own external id (us_senate view pages), thread
  it into `parse` via a conformance-mode `sha256 → id` constant table mirroring the
  MANIFEST pins (pool-backed runs resolve from `raw_document.source_url`); unknown sha
  fails closed — same never-guess posture as the us_house politician table.
Write-back: deepen this file when the procedure teaches you something; same PR.
