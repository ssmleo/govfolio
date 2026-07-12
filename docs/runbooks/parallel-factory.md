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
6. `GOVFOLIO_BRONZE_ROOT` is one absolute, durable directory shared by every lane and
   outside every Cargo target. The launcher defaults it to the repository sibling
   `../govfolio-bronze`; an explicit absolute value may override that default.

## Shared caches and sacred Bronze

Every producer keeps its own worktree and Cargo target. Do not point concurrent agents
at one `CARGO_TARGET_DIR`: Cargo's build lock serializes work and Windows linkers or
antivirus scanners can collide on mutable artifacts. Agents may share the Cargo registry
and the optional user-local `sccache` described in
[`dev-host-windows.md`](dev-host-windows.md#5-optional-sccache-for-private-worktree-targets).

Bronze is not a compiler cache. New loop runs place it in the shared directory named by
`GOVFOLIO_BRONZE_ROOT`, outside disposable targets. Existing `target/bronze-*` stores
must be copied, never moved, and verified by relative path, file count, byte length, and
SHA-256 before the new root is used. Follow the Windows runbook's copy-and-verify
procedure. Do not clean the old target or delete old Bronze during migration; any
mismatch fails closed and preserves both copies for investigation.

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

`./agents/monitor.sh` combines the supervisor's fenced process/receipt status with the
read-only `loop-board` view: DONE/DOING/LEFT registry state, live provider processes,
structured journal/commit digests, semantic log tails, and aggressive stall tripwires.
Both binaries are pre-built; the monitor never compiles or mutates repository state.
