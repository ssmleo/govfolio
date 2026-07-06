# 070 — trust hardening

## Objective
Monthly sampling-audit job + precision report, public corrections log page, per-regime redaction pass (pre-publication), adapter drift detection with fail-closed freeze.

## Context (read first)
- design §7.4–7.5, §5.6

## Acceptance criteria
```bash
cargo test -p pipeline redaction drift && pnpm --filter web test -- corrections
```

## Checklist
- [x] sampler  - [x] corrections page  - [x] redaction rules/regime  - [x] drift freeze

## Leg A (pipeline) — done 2026-07-06
Rust data-plane half: sampler + redaction + drift-freeze. Corrections PAGE is leg B
(web-builder), untouched here.

- **Sampler** — `worker::sampler` (`select_sample` deterministic seeded stratified draw +
  `run_sampling_audit` + `precision_report`) + bin `worker --bin sample-audit` (monthly job).
  Queue table `sample_audit` (migration **0009**, expand-only, keyed by `regime_id`); the
  precision report is a computed aggregate (per regime: sampled/audited/discrepancies +
  estimate) — no separate report table. This is the queue automation-policy's expected.*.json
  auto-resolution feeds. Deterministic (seedable) → reproducible tests.
- **Redaction** — `pipeline::redaction` (rules-as-data per regime code), applied in
  `publish::publish_filing` AND `promote::supersede` BEFORE the details-contract check
  (so a stored Gold row is exactly what passed the contract; a misconfigured strip of a
  REQUIRED field fails closed at that gate). Covers: (1) FR HATVP **patrimony** (`dsp*`)
  whole-record **suppression** — belt-and-suspenders for the Art. LO 135-2 €45k
  republication ban (adapter already excludes `dsp*` at discovery); (2) per-regime PII
  **field strip** (us_senate paper counsel name/signature; fr declarant private markers).
  Bronze + staged Silver keep the raw verbatim (invariant 2) — redaction only ever touches
  the id-bound Gold candidate clone.
- **Drift-freeze** — `publish::publish_filing` checks `sentinel_watch.frozen` (goal-017
  freeze flag, kept in sync with the open freezing `drift_report`) and REFUSES to publish a
  frozen regime (fail closed, §5.6): opens `publish_blocked_frozen` review_task, writes zero
  Gold, fails the publish stage (retryable → republishes on recovery). Added the goal-017
  follow-up **unfreeze/recover** path: `WatchStore::recover` (+ `PgWatchStore` impl, wired
  into `watch_pass` on a clean pass) clears `frozen` and resolves the freezing drift report +
  linked review_task when the source recovers.
- **T11 promote.rs invariants intact** — extended (redaction added), not forked; two-stage
  publication (unverified→verified) + supersede-never-update untouched.

### Corrections endpoint (leg B handoff)
Left to leg B — **no `/v1/corrections` endpoint added here** (keeps leg A pipeline-only, no
openapi/TS regen). The corrections data is already API-reachable: `disclosure_record` carries
`verification_state='corrected'` + `supersedes_record_id` (the supersession chain), and the
`/v1/records` filter grammar already supports `verification_state` — so leg B can build the
corrections log from `GET /v1/records?verification_state=corrected` (each row links its
superseded original) without a new endpoint. `disclosure_record.corrected` outbox events also
exist. If leg B prefers a dedicated read endpoint, it should add + regen the contract itself.
