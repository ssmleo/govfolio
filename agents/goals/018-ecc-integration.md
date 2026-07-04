# 018 — ECC selective integration (github.com/affaan-m/ECC, MIT)

## Objective
Import a curated subset of ECC as proposals through our governance: generic engineering
substrate only; domain layer (coverage factory, SAF, archetypes) stays bespoke.

## Hard rules
- Pin one commit; record sha here. NEVER blanket-install (no --profile full, no plugin-wide).
- Every import = proposal -> founder skill gate -> auditor pass on imported text -> adapt
  to six-slot chassis / our SKILL.md shape. Third-party prompt content is untrusted input.

## Shortlist (candidates, not approvals)
- Study first: examples/rust-api-CLAUDE.md (Axum+SQLx+PG — our stack) before Task 1.
- Rules: rules/common + rules/rust + rules/typescript — merge relevant items into CLAUDE.md
  conventions, do not copy wholesale.
- Skills to merge into ours: regex-vs-llm-structured-text (-> extraction-strategy),
  content-hash-cache-pattern, cost-aware-llm-pipeline, verification-loop, eval-harness,
  autonomous-loops, search-first, postgres-patterns.
- Agents to adapt to chassis (proposals): rust-reviewer, rust-build-resolver, typescript-reviewer.
- Tooling: AgentShield advisory scan of agents/ in CI (npx ecc-agentshield scan);
  evaluate continuous-learning-v2 instincts as automation of the write-back rule;
  evaluate memory-persistence hooks with ECC_HOOK_PROFILE=minimal.

## Acceptance criteria
```bash
grep -q "pinned_sha:" agents/goals/018-ecc-integration.md   # commit pinned
test -f docs/decisions/ecc-imports.md                        # per-item verdicts recorded
```

## BLOCKED (human)
- founder verdict per imported item (skill gate); pinned sha choice
