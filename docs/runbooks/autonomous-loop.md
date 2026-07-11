# Autonomous loop runbook

This is the operational authority for the Claude/Codex loop after hardening Releases
0-1. The Rust supervisor owns singleton/fencing, provider processes, circuits, evidence,
receipts, and integration. Legacy shell files are launch shims only.

## Safety boundary

- One host-wide supervisor and one fenced owner per lane.
- Release 1 runs receipt-producing jurisdiction work only when claim preflight returns an
  exact `lane_id` and `lease_generation`.
- Producers commit locally. They never push, merge, open a PR, append JOURNAL, or mutate
  `coverage_phase`, `blocked_reason`, or terminal lease state.
- `govfolio-loop integrate` is the singleton pusher/PR owner/JOURNAL writer/domain-state
  finalizer. There is no manual lane-to-main merge path.
- A row with `pending_integration_id` is unavailable to claim, resume, stale reclaim,
  targeted claim, renew, and abandon. The integrator must reconcile it.
- Validation commands embedded in receipts are evidence strings. The integrator never
  executes them; it runs the repository-owned validation matrix.

## Start and observe

Before first start or after any provider fingerprint change, complete the native and
live canary procedure in `docs/runbooks/loop-provider-compatibility.md`. Both lane-0
providers and every configured factory provider must have a current exact proof.

The initial dual-provider layout is one Codex-owned orchestrator lane plus one Claude
factory lane in a different worktree:

```text
GOVFOLIO_LOOP_PROVIDER=codex
GOVFOLIO_LOOP_LANE_ID=orchestrator-0
GOVFOLIO_LOOP_WORKTREE=<dedicated orchestrator worktree>
GOVFOLIO_LOOP_BRANCH=loop/orchestrator-0
GOVFOLIO_LOOP_FALLBACK_PROVIDER=claude
GOVFOLIO_CODEX_MODEL=<approved model>
GOVFOLIO_CLAUDE_MODEL=<approved model>
GOVFOLIO_EPOCH=E3
GOVFOLIO_BRONZE_ROOT=<dedicated shared Bronze directory outside build target>
GOVFOLIO_FACTORY_LANES=1
GOVFOLIO_FACTORY_1_WORKTREE=<separate Claude factory worktree>
GOVFOLIO_FACTORY_1_BRANCH=loop/factory-1
GOVFOLIO_FACTORY_1_MODEL=<approved Claude model>
```

Do not point two lanes at one worktree. Further factory lanes remain disabled until the
measured utilization, queue-wait, and conflict gates in goal 111 permit one-at-a-time
expansion. The orchestrator is not claim-gated: it may complete trusted queue work such
as opening E3. Factory lanes remain zero-spend while the configured epoch gate is red or
no jurisdiction is claimable, then begin automatically once both conditions are green.
Do not use the Cargo `target/` directory as the live Bronze root; provider write access
to sacred raw assets must not also grant write access to pre-built supervisor binaries.

Run the pre-built binary through the repository shim:

```text
./agents/run-loop.sh
```

Inspect bounded supervisor/lease/receipt state through:

```text
./agents/monitor.sh
cargo run -p worker --bin jurisdiction-lease -- status
govfolio-loop receipt-status <receipt-id>
```

Do not start a second shell loop to increase throughput. Release 3 changes producer count
only after its metric gate; duplicate singleton acquisition must fail.

## Producer lifecycle

1. Preflight authority, Git/worktree, compiler/linker, provider, DB, Bronze, epoch,
   claimability, and disk. Any non-pass result creates no provider attempt.
2. Claim once:
   `jurisdiction-lease claim --next --epoch <n>`. Persist the returned jurisdiction,
   lane, and generation as one fence tuple.
3. Run exactly one current phase. Renew natural long-running checkpoints with
   `jurisdiction-lease renew --id <x> --generation <g>`.
4. Validate and independently review. Collect command exits/output hashes and artifact
   hashes. Built-to-live includes the real fetch/ingestion proof.
5. Commit the artifact and SAF write-back locally. Confirm JOURNAL is untouched.
6. Create the typed immutable receipt using exact base/source SHAs and the claimed
   lane/generation. The proposed phase is adjacent, or `blocked` with a single-line
   reason. Submit:
   `govfolio-loop submit-receipt <receipt.json>`.
7. After submission, stop all producer mutation and wait:
   `govfolio-loop receipt-status <receipt-id>`.

If work stops before there is a safe receipt commit, abandon only the exact generation:

```text
cargo run -p worker --bin jurisdiction-lease -- abandon --id <x> --generation <g>
```

A stale/pending response means stop; never rediscover or overwrite the generation. Direct
`jurisdiction-lease advance` and phase-changing `release` are retired and must error.

## Integrator lifecycle

The singleton integrator continuously reconciles nonterminal receipts:

```text
govfolio-loop integrate
```

For each receipt it builds from a fresh exact `origin/main`, proves base/source ancestry,
rejects producer JOURNAL edits, merges the exact source SHA without committing early,
appends exactly one canonical receipt JOURNAL line, and runs repository-owned targeted and
full checks. If main moved, it discards the unpublished candidate and rebuilds.

It pushes a new integration branch without force, opens a PR, enables merge-commit
auto-merge, and requires `rust`, `db`, `web`, and `guardrails`. It applies registry state
only after CI is green on the actual merge SHA, `origin/main` equals that SHA, source
ancestry is proven, and exactly one receipt JOURNAL line exists.

Receipt states are:

```text
submitted -> preparing -> awaiting_ci -> merged_unapplied -> applied
```

Conflict/check failures become `rework_required`. Producers receive at most two immutable
repair receipts; then that receipt is deferred while unrelated receipts continue.

## Crash recovery

- Restart the same pre-built supervisor. Do not launch providers by hand.
- `merged_unapplied` is expected recovery state: the integrator re-verifies the exact main
  merge and atomically applies it; it never re-merges the producer branch.
- `submitted`, `preparing`, and `awaiting_ci` resume from durable receipt state.
- An ambiguous provider stream or dirty failed worktree fences the lane for reconciliation;
  do not delete its external evidence or reset the worktree.
- A pending jurisdiction is never manually cleared. Receipt reconciliation/application is
  the only authority.

After the supervisor is stopped and the worktree has been repaired from authoritative
state without discarding evidence, clear only the verified lane fence:

```text
govfolio-loop recover-lane <lane-id>
```

The command refuses active/non-recovery lanes and requires the stored worktree and branch
to be clean plus the authority and skill contracts to pass. It performs no reset; the
next start acquires a strictly newer lane fence.

## Evidence and retention

Runtime evidence lives outside the repository under the configured govfolio-loop state
root. Suppressed ticks create no attempt directory or blob. Normal resolved attempts retain
14 days under the 5 GiB cap; unresolved, ambiguous, conflict, and unapplied evidence is
preserved. Launches pause below the greater of 5 GiB or 10% free volume space.

Never commit runtime logs, provider transcripts, receipt scratch files, or secret-bearing
human output. Human summaries redact tokens, authorization headers, connection strings,
and secret-shaped environment values.
