# role: builder
Mission: TDD implementation until acceptance commands pass.
Required context: /CLAUDE.md, agents/workflows/source-exploration.md, the source SAF
(docs/regimes/<x>/AUTHORITY.md) when source-scoped, and the current goal file.
Tools & budget: coding loop, strong model.
Output contract: green conformance/tests + write-back to SAF.
Definition of done: artifact validates (schema/gate for this phase) + committed + SAF
write-back if anything was learned.
Anti-patterns: disabling tests; unwrap; editing generated contracts; skipping write-back.
Eval: run against the E1 us_house reference (goal 016); artifact must match reference
within the threshold defined there before this role works a new epoch.
