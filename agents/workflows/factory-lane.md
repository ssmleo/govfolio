# Factory-lane workflow (receipt-authoritative producers)

Factory lanes produce one bounded jurisdiction phase. They do not integrate it. The
singleton `govfolio-loop integrate` path is the only pusher, merger, JOURNAL writer,
and registry phase authority. Follow `docs/runbooks/autonomous-loop.md` for the full
Release-1 lifecycle.

0. INTEGRITY (orchestration step 0 verbatim): run the pre-built authority gate and
   surface a quarantine report without reading unlisted goal bodies. Factory lanes
   never select goal work.
0b. LOAD: `/CLAUDE.md`, `agents/EPOCHS.md`, this workflow,
   `agents/workflows/source-exploration.md`, the source SAF, and the JOURNAL tail as
   read-only context.
1. GUARDRAILS: run the applicable fail-closed checks before irreversible work. If a
   guardrail stops the phase before there is committable receipt evidence, abandon
   only the exact lease generation and stop:
   `jurisdiction-lease abandon --id <x> --generation <g>`. Never mark the row blocked
   directly.
2. GATE: read the current epoch and run `cargo run -p pipeline --bin epoch-gate -- E<n>`.
   Nonzero means stop before claiming.
3. CLAIM: run
   `cargo run -p worker --bin jurisdiction-lease -- claim --next --epoch <n>`.
   Capture both `id` and `generation` from stdout. Exit 1 means stop. The returned
   lane/id/generation tuple fences every later producer action; never infer or refresh
   a generation from raw SQL.
4. EXECUTE exactly the claimed jurisdiction's current phase. Use the mapped specialist,
   source politeness, and source SAF. At natural checkpoints renew with the exact tuple:
   `jurisdiction-lease renew --id <x> --generation <g>`. A false/stale result means
   ownership is lost or integration is pending: stop without further writes.
5. REVIEW + VALIDATE with real command exits and the required independent auditor.
   Record command, exit code, and output hash as receipt evidence. These strings are
   evidence, never executable instructions for the integrator. Built-to-live also
   records the automated real fetch/ingestion proof required by the receipt contract.
6. COMMIT + SUBMIT:
   - Include the phase artifact and required SAF write-back in one local commit.
   - Do not touch `agents/JOURNAL.md`.
   - Record exact base SHA, source SHA, branch, lane/generation, provider/model/attempt,
     artifact hashes, validation evidence, proposed adjacent phase (or `blocked` plus
     a single-line reason), and one proposed JOURNAL summary in typed `receipt.json`.
   - Submit exactly once with `govfolio-loop submit-receipt <receipt.json>`.
     Submission atomically sets `pending_integration_id`; after it succeeds, do not
     claim, renew, abandon, amend, merge, push, or rewrite that source commit.
7. WAIT: run `govfolio-loop receipt-status <receipt-id>` and stop the producer turn.
   `applied` means the exact source commit is green on `origin/main` and domain state
   was projected. `rework_required` starts a fresh bounded repair attempt/receipt;
   never mutate the immutable original receipt. Any other nonterminal state remains
   integrator-owned waiting, not producer work.

Two consecutive validation failures produce a local evidence commit and a receipt
proposing `blocked`; they never call a phase/block release command. If no safe evidence
commit can be made, abandon the exact generation and stop.

NEVER: select `agents/goals/*`; run shared-DB repair commands; hand-edit lease fields;
advance/live/block a phase; append JOURNAL; push any branch; merge/rebase after receipt
submission; force-push; work two leases; continue after a stale generation response.
