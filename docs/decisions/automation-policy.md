# Automation policy — full autonomy with mechanical guardrails
Founder decision 2026-07-04: automate EVERY decision; no human gates. Risk accepted
explicitly for irreversible infra. Enforcement shifts from human judgment to mechanical,
fail-closed guardrails so autonomy stays recoverable (unattended != unbounded).

## Lifted gates (now automated)
- Agent SKILL SELECTION: promoted from founder-gate to the codified allocator below.
  Rationale: 6 rounds of consistent manual approval (2 amendments, both consistency
  fixes) proved the pattern stable; a proven manual rule becomes a coded rule.
- FIXTURE expected outputs / verification-type judgments: auto-resolved. High-confidence
  extraction + second-model cross-check produces expected.*.json; records publish as
  `unverified` (existing two-stage machine) and flow to the sampling-audit queue instead
  of BLOCKING the loop. Ground truth becomes a sampled check, not a stop.
- MASS reprocess, EPOCH go/no-go, LAUNCH: automated against their acceptance commands.

## Guardrailed-autonomous infra (irreversible; risk accepted; fail CLOSED)
1. Prod migrations: auto-apply IF expand-only. CI `check-migration-safety` rejects
   DROP/destructive DDL -> converts to work item. Mandatory pre-apply snapshot.
2. terraform apply: auto IF destroy/replace count <= DESTROY_BUDGET (default 2).
   `check-tf-plan` parses `terraform plan -json`; over budget -> halt to work item.
   Remote state + locking + versioning (every apply recoverable).
3. Billing/money: auto within HARD CAP (monthly ceiling + per-action limit). Over -> halt.

All three: fail closed (halt to work item, never proceed on ambiguity), recoverable
(snapshot / versioned state / cap), and add NO human gate. A halt files a goal; the loop
continues other work.

## Skill allocator (deterministic; replaces the founder gate)
Rule (unchanged from what was approved 6x): a role gets exactly the skills for the
artifacts its output contract touches; ceiling 6 standing slots; a pack (<=3 same-source,
same-domain) = 1 slot; situational skills load on triggers, off-ceiling. New agents
self-allocate by this rule at creation; auditor spot-checks ceiling compliance.

Artifact -> skill map (authoritative):
  fetches web/source        -> polite-fetching, evidence-archiving
  researches a regime       -> regime-research
  writes/updates a SAF      -> saf-authoring
  captures fixtures         -> fixture-capture
  authors details/contracts -> schema-contracts
  decides parse strategy    -> extraction-strategy   (spec-writer exclusive)
  writes Rust to green      -> rust-tdd, conformance-diffing
  verifies others' work     -> adversarial-verification, conformance-diffing
  plans/decomposes          -> plan-decomposition
  watches live sources      -> drift-detection
  approaches any output human-facing pre-automation -> (historically human-gate-etiquette;
    now: emit to work queue, never block)
