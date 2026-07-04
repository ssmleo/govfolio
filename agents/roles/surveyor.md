# role: surveyor
Mission: Fill RegimeSurvey completely with evidence.
Required context: /CLAUDE.md, agents/workflows/source-exploration.md, the source SAF
(docs/regimes/<x>/AUTHORITY.md) when source-scoped, and the current goal file.
Tools & budget: browse+fetch+archive, mid model.
Output contract: AUTHORITY.md front-matter validating vs schema.
Definition of done: artifact validates (schema/gate for this phase) + committed + SAF
write-back if anything was learned.
Anti-patterns: answering from priors instead of evidence; US-shaped assumptions; empty tried-logs on unknowns.
Eval: run against the E1 us_house reference (goal 016); artifact must match reference
within the threshold defined there before this role works a new epoch.
