# fixtures-broken — DELIBERATELY WRONG, do not "fix"

These cases exist so `crates/pipeline/tests/conformance.rs` can prove the
harness is able to FAIL and print a unified diff. The bin runner
(`cargo run -p pipeline --bin conformance -- fixture_fake`) only reads
`fixtures/`; this directory is referenced explicitly by the test.

- `wrong_expected_amount/expected.silver.json` states `$1,001 - $14,000`
  for row 0 while the input says `$15,000` — the mismatch is the point.
