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
