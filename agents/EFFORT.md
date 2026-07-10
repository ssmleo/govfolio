# Effort & workflow-dispatch policy (founder-approved 2026-07-04; amended 2026-07-10)

## Per-role effort (bound via .claude/agents shims; falls back gracefully below xhigh-capable models)
ALL roles run xhigh — founder decision 2026-07-10 (in chat, goal 097): orchestrator,
planner, spec-writer, test-designer, auditor, rust-builder, web-builder, surveyor,
scout, sampler, sentinel. Supersedes the 2026-07-04 tiering (orchestrator high /
surveyor high / scout+sampler medium / sentinel low); selection-cheapness no longer
outweighs reasoning quality.
The systematic-debugging trigger (2x failed verify) remains our definition of "when to
spend" (already xhigh; now also the fleet default).

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

## Model resolution & version floor
Shims deliberately omit `model`: omitted = inherit the main session's model (official
default). Resolution order: CLAUDE_CODE_SUBAGENT_MODEL env var -> per-invocation model
parameter -> frontmatter -> parent model. Ultracode workflow agents likewise use the
session's model unless the script routes a stage elsewhere. Rationale: swap the fleet
model once via /model; per-role differentiation is carried by effort, and pinning small
models would silently clamp xhigh.
VERSION FLOOR: Claude Code >= 2.1.198 (subagents inherit extended-thinking config; older
versions ran subagents with thinking OFF) and >= 2.1.154 (Dynamic Workflows). The
orchestrator should verify `claude --version` on first run and record it in JOURNAL.md.
If cost-tiering models per role later: prefer the env var or per-invocation parameter
(an Apr-2026 bug report says frontmatter `model` was ignored on some versions — verify
before trusting it). Org availableModels exclusions are skipped silently -> inherited.
