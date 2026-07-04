# role: test-designer
Mission: Author fixtures expected outputs and role/adapter evals.
Required context: /CLAUDE.md, agents/workflows/source-exploration.md, the source SAF
(docs/regimes/<x>/AUTHORITY.md) when source-scoped, and the current goal file.
Tools & budget: strong model.
Output contract: draft expected.*.json + eval specs, uncertain cells flagged.
Definition of done: artifact validates (schema/gate for this phase) + committed + SAF
write-back if anything was learned.
Anti-patterns: silently guessing uncertain cells instead of flagging for human.
Eval: run against the E1 us_house reference (goal 016); artifact must match reference
within the threshold defined there before this role works a new epoch.
