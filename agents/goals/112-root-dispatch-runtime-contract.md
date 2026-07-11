# 112 — root dispatch + runtime tool contract

## Objective

Close the live-rollout gap where the supervisor passed raw root prompts to Codex/Claude,
so coordinator turns could read workflow/queue context before a governed skill receipt.
Relaunch only after both lane coordinators receive deterministic envelopes and the
supervisor mechanically rejects a missing or mismatched root receipt.

## Scope

- Render the root `GOVFOLIO_DISPATCH_V1` envelope with the trusted resolver before any
  provider spawn. Lane 0 uses role `orchestrator`; the factory coordinator uses the same
  coordinating role plus explicit `trigger:parallel-work`, while its factory prompt
  continues to narrow selection to lease-backed jurisdiction work.
- Prefix a root pre-receipt boundary and exact expected `SKILLS_LOADED` receipt, and
  reject completed provider output unless that exact standalone receipt is present in
  the structured stream.
- Inject explicit prebuilt runtime paths (`GOVFOLIO_AUTHORITY_BIN`,
  `GOVFOLIO_LOOP_BIN`, `GOVFOLIO_EPOCH_GATE_BIN`, `GOVFOLIO_LEASE_BIN`) plus epoch into
  the sanitized provider environment. Never restore ambient credentials or raw secrets.
- Isolate compiler/link canary caches per lane so simultaneous startup cannot create a
  false block.
- Preserve supervisor-only provider ownership; no raw provider runner or WSL fallback.

## Acceptance

```bash
cargo test -p loop-supervisor root_dispatch
cargo test -p loop-supervisor provider_environment
cargo test -p loop-supervisor orchestrator_preflight
cargo clippy -p loop-supervisor --all-targets -- -D warnings
node scripts/agents/codex-contract.test.mjs
```

Bounded live acceptance after protected-main merge: Codex lane 0 emits the exact root
receipt before queue selection; Claude factory remains zero-spend while E3 is red. Both
worktrees remain clean until governed work begins.

## Checklist

- [ ] Trusted root envelope composed before provider spawn
- [ ] Exact structured root receipt required on completion
- [ ] Prebuilt runtime tool paths explicitly sanitized into provider environment
- [ ] Compiler canary cache isolated per lane
- [ ] Tests, memory write-back, protected-main CI, and bounded relaunch green

## BLOCKED (human)

(empty)
