# Parallel coverage factory

The hardened operational procedure is
[`docs/runbooks/autonomous-loop.md`](autonomous-loop.md). This page retains only
coverage-specific scaling and legibility rules.

## Preconditions

1. Claims are atomic and generation-fenced through `jurisdiction-lease`; never write
   registry lease columns directly.
2. Every producer has a dedicated worktree/branch and fenced lane identity.
3. Guardrail counters are shared host-wide.
4. Release 0 singleton/circuit/storm-fuse proof and Release 1 receipt/integrator proof
   are green. Factory providers remain stopped before both releases.
5. Producer count follows the metric gate in hardening design section 9. Do not start
   extra shell runners.

## One producer iteration

`agents/workflows/factory-lane.md` is authoritative:

1. claim one row and retain its generation;
2. execute and validate one current phase;
3. renew only by exact generation while working;
4. commit locally without JOURNAL;
5. submit `govfolio-loop submit-receipt <receipt.json>`;
6. wait on `govfolio-loop receipt-status <receipt-id>` and stop.

The singleton integrator owns push, PR, merge, canonical JOURNAL, exact-SHA CI proof,
and atomic phase/lease apply. Direct advance/live/block and producer merge paths are
retired.

## Legibility

- DOING: held leases with lane, generation, age, and pending receipt from
  `jurisdiction-lease status`.
- WAITING: nonterminal receipt states from the supervisor.
- DONE: `applied` receipts whose exact source SHA is an ancestor of green
  `origin/main`, with one canonical receipt JOURNAL line.
- LEFT: nonterminal, nonpending registry rows ordered by epoch/priority.

An aging lease with no pending receipt is stalled producer work. A pending receipt is
integrator work and must never be reclaimed manually. Preserve unresolved evidence and
let startup reconciliation decide the next action.
