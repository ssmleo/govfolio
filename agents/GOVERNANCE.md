# Agent governance

## Agent-creation protocol (FOUNDER GATE)
1. Any new agent role (or change to an existing role's skill set) starts as a proposal
   using agents/roles/_PROPOSAL_TEMPLATE.md.
2. The proposal MUST list proposed skills with a one-line rationale each.
3. HARD STOP: the founder reviews and opines on the skill selection. No role file gains
   or changes a `Skills:` section without recorded founder approval (PR comment or chat,
   referenced in the commit message).
4. On approval: finalize the role file (Skills: section), update agents/SKILLS-MATRIX.md,
   commit with reference to the approval.
Applies retroactively: roles created before this protocol get their Skills sections only
after founder review of the initial matrix.

## Chassis rule
Every role file follows the six-slot chassis (agents/archetypes/_CHASSIS.md) and names its
archetype. New archetypes and archetype changes are founder-gated like skill changes.

## Skill rules
- One procedure per skill (agents/skills/<name>/SKILL.md): purpose, when to load, core
  checklist, anti-patterns. Deepened via write-back, never forked per role.
- Roles load ONLY the skills listed in their Skills: section, plus /CLAUDE.md and the
  relevant SAF. Skill sprawl is a bug: if a role needs >6 skills, split the role.

## Orchestrator constraints
The orchestrator (agents/roles/orchestrator.md) selects, dispatches, verifies, records.
It never writes production code, never self-certifies, never approves proposals, never
unblocks human lanes. Its full workflow: agents/workflows/orchestration.md.

## A1 — Standing vs situational skills; packs (approved 2026-07-04)
Standing skills load every iteration; ceiling 6 slots. Situational skills are
founder-gated allocations loaded ONLY when their role-file trigger fires; they never
co-load by default and do not count against the ceiling. A pack (<=3 same-source,
same-domain skills) occupies one slot and is gated as a unit.

## A2 — Doer split along the language boundary (approved 2026-07-04)
The doer archetype instantiates per language boundary: rust-builder (data plane) and
web-builder (presentation edge). extraction-strategy is held exclusively by spec-writer;
builders read the decided strategy from the SAF.

## A3 — Imports (approved 2026-07-04)
Bespoke skills outrank imported ones on any conflict. Imports enter ONLY by vendor-copy
at a pinned sha into agents/skills/imported/<lib>@<shortsha>/ with license and per-file
verdicts in docs/decisions/skill-imports.md. Live plugin/marketplace installs are
forbidden: an auto-update is an unreviewed prompt change. Some imports merge into
bespoke skills or chassis text after audit instead of occupying slots
(verification-before-completion -> chassis DoD; test-driven-development -> rust-tdd).

## Effort policy
agents/EFFORT.md and .claude/agents shim frontmatter are prompt-changing artifacts:
edits are founder-gated like role edits. Shims must remain thin (no behavior beyond
loading the governed role file).
