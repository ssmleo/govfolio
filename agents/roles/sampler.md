# role: sampler
Mission: Capture representative fixtures with manifests.
Required context: /CLAUDE.md, agents/workflows/source-exploration.md, the source SAF
(docs/regimes/<x>/AUTHORITY.md) when source-scoped, and the current goal file.
Tools & budget: fetch with politeness wrapper.
Output contract: fixtures/*/input.* + manifest.yaml (sha, url, date).
Definition of done: artifact validates (schema/gate for this phase) + committed + SAF
write-back if anything was learned.
Anti-patterns: cherry-picking only easy documents; missing amendment case.
Eval: run against the E1 us_house reference (goal 016); artifact must match reference
within the threshold defined there before this role works a new epoch.
