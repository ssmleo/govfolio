-- 0008_sentinel_watch: continuous drift-defense state (design §5.6 fail-closed,
-- §5.8 sentinel WATCH; goal 017). Expand-only: two additive tables, no rewrite
-- of existing rows.
--
-- The sentinel probes each live source's discovery page weekly (Cloud
-- Scheduler, infra/scheduler.tf) and compares four signals against the last
-- known baseline: HTTP status, a structural layout-hash of the listing markup,
-- the discoverable filing count, and the presence of regime markers. A shift in
-- any signal is classified, ranked by severity, deduped, and filed as a
-- drift_report -- and, for the fail-closed kinds (§5.6: a layout shift, or a
-- discoverable count that falls to zero), it freezes that regime's publication.
--
-- Keyed by REGIME CODE (the stable cross-system identifier the adapter,
-- fixtures and conformance already share -- RegimeRef.code), not the regime
-- ULID: the code has no column on disclosure_regime to reference, and this is
-- operational watch state, not a canonical Gold fact, so no foreign key binds
-- it to the regime row. A resolve path clears `frozen` when a human or the
-- orchestrator confirms the source recovered (out of scope for this leg).

-- Per-source baseline: the last observation the next pass diffs against.
create table sentinel_watch (
  regime_code      text primary key,
  last_status      int,
  last_layout_hash text,
  last_count       bigint,
  last_etag        text,
  last_modified    text,
  frozen           boolean not null default false,
  frozen_kind      text,
  frozen_at        timestamptz,
  last_checked_at  timestamptz not null default now()
);

-- One ranked, deduped anomaly. `dedup_key` = regime_code:kind:signature; the
-- partial unique index below makes re-detection of the SAME open anomaly a
-- no-op update (bumps detections + last_detected_at) instead of a duplicate
-- row. priority_score ranks severity so the orchestrator (design §5.8: CI red
-- -> drift -> queue -> factory) picks the worst first. `freezes_publication`
-- records whether this anomaly froze publication (`freeze` is a reserved word);
-- `review_task_id` links the auto-filed work item (the human/orchestrator
-- artifact).
create table drift_report (
  id                  text primary key,
  regime_code         text not null,
  drift_kind          text not null check (drift_kind in
                        ('layout_shift','count_zero','regime_change',
                         'status_error','probe_error','count_delta')),
  priority_score      real not null,
  freezes_publication boolean not null default false,
  dedup_key         text not null,
  detail            jsonb not null default '{}',
  status            text not null default 'open'
                      check (status in ('open','resolved','superseded')),
  review_task_id    text references review_task(id),
  detections        int not null default 1,
  first_detected_at timestamptz not null default now(),
  last_detected_at  timestamptz not null default now()
);

-- Dedup: at most one OPEN report per (regime, kind, signature).
create unique index drift_report_open_dedup
  on drift_report (dedup_key) where status = 'open';

-- Orchestrator work selection: worst open drift first.
create index drift_report_open_rank
  on drift_report (priority_score desc, id) where status = 'open';
