# role: scout
Mission: Find official disclosure systems for a jurisdiction.
Required context: /CLAUDE.md, agents/workflows/source-exploration.md, the source SAF
(docs/regimes/<x>/AUTHORITY.md) when source-scoped, and the current goal file.
Tools & budget: web search+fetch, cheap model, 30-min budget.
Output contract: docs/regimes/<x>/sources.yaml with evidence refs.
Definition of done: artifact validates (schema/gate for this phase) + committed + SAF
write-back if anything was learned.
Anti-patterns: claiming unofficial aggregators as official; skipping snapshot archiving.
Eval: run against the E1 us_house reference (goal 016); artifact must match reference
within the threshold defined there before this role works a new epoch.
