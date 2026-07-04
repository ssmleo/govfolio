# Effort & workflow-dispatch policy (founder-approved 2026-07-04)

## Per-role effort (bound via .claude/agents shims; falls back gracefully below xhigh-capable models)
orchestrator high (runs every iteration — selection must stay cheap) · planner xhigh ·
spec-writer xhigh · test-designer xhigh · auditor xhigh (adversarial re-derivation is
reasoning-heavy) · rust-builder xhigh · web-builder xhigh (platform guidance: xhigh is
the starting point for coding/agentic work) · surveyor high · scout medium ·
sampler medium · sentinel low.
The systematic-debugging trigger (2x failed verify) is our definition of "when to spend":
that dispatch always runs xhigh.

## Ultracode dispatch (per-task keyword ONLY — never session-wide /effort ultracode)
Eligible task classes: goal-080 backfill sweep; goal-016 calibration sweeps; repo-wide
migrations/refactors; security audits of agents/; SURVEY-phase deep research
(/deep-research wrapped in evidence-archiving: its citations become archived snapshots);
multi-source drift root-cause. Ineligible: routine single-goal iterations.
Rules: (1) first dispatch of a new class runs on a reduced scope to gauge token cost;
(2) read the generated workflow script before approving anything that touches write
paths; (3) workflow results do NOT self-certify — they still pass our validators,
conformance suites, and auditor gates (model-verifies-model never replaces
world-verifies-model); (4) log each workflow dispatch in JOURNAL.md with a cost note.
Changes to this file are founder-gated like role edits.
