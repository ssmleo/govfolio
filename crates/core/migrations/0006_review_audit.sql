-- 0006_review_audit: reviewer action log (design §7.2 "all actions
-- audit-logged", goal 041a). Expand-only.
--
-- One row per resolve ATTEMPT, whatever came of it — `outcome` says which:
--   'applied'  — the verdict landed. Written by pipeline promote in the SAME
--                transaction as its writes, so "verdict applied" and "audit
--                row exists" are atomic — neither can be observed alone.
--   'conflict' — the task was already resolved; nothing was written
--                (the API's 409 path).
--   'failed'   — the resolution errored and rolled back whole (e.g. a
--                correction failing the details contract, invariant 5).
-- Non-applied attempts have no surviving transaction to join, so promote
-- records them after the fact on the pool — still exactly one row per
-- attempt, all owned by the single write path (pipeline::promote).
--
-- `outcome` extends the goal-041 column list (id, review_task_id, reviewer,
-- verdict, note, affected_record_ids, created_at): without it, conflict and
-- failure attempts would be indistinguishable from applied ones, and the log
-- could not honestly record "attempted but did not land".
--
-- `reviewer` is caller-supplied free text until accounts land (goal 050).

create table review_audit (
  id                  text primary key,
  review_task_id      text not null references review_task(id),
  reviewer            text not null,
  verdict             text not null check (verdict in ('confirm','edit','reject')),
  outcome             text not null check (outcome in ('applied','conflict','failed')),
  note                text,
  affected_record_ids jsonb not null default '[]'::jsonb,
  created_at          timestamptz not null default now()
);

-- The audit endpoint reads one task's attempts in id (= insertion-time) order.
create index review_audit_task on review_audit (review_task_id, id);
