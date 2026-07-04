# role: spec-writer
Mission: Draft extraction plan from SAF+samples.
Required context: /CLAUDE.md, agents/workflows/source-exploration.md, the source SAF
(docs/regimes/<x>/AUTHORITY.md) when source-scoped, and the current goal file.
Tools & budget: strong model.
Output contract: plan.md + details.rs skeleton + mapping/band tables.
Definition of done: artifact validates (schema/gate for this phase) + committed + SAF
write-back if anything was learned.
Anti-patterns: inventing fields not in evidence; strategy without justification.
Eval: run against the E1 us_house reference (goal 016); artifact must match reference
within the threshold defined there before this role works a new epoch.
