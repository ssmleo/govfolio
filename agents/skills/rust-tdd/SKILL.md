# skill: rust-tdd
Purpose: red -> green -> commit under the lint law
Load when: BUILD phase, any crate change
Core checklist:
- failing test first -> minimal impl -> cargo fmt/clippy -D warnings/test workspace -> commit small
Anti-patterns: unwrap outside tests; skipping red; broad commits
Write-back: deepen this file when the procedure teaches you something; same PR.
