"use client";

import { useQuery } from "@tanstack/react-query";

import type { AdminOverview } from "@/lib/api";

async function fetchOverview(): Promise<AdminOverview> {
  // Same-origin proxy (see admin/ops-proxy/route.ts) — never calls the Rust
  // API directly, so the browser never sees `X-Admin-Token`.
  const res = await fetch("/admin/ops-proxy", { cache: "no-store" });
  if (!res.ok) {
    throw new Error(`ops-proxy responded ${res.status}`);
  }
  return (await res.json()) as AdminOverview;
}

function Item({
  label,
  value,
  tone,
}: {
  label: string;
  value: number;
  tone?: "danger" | "warning";
}) {
  const toneClass =
    tone === "danger"
      ? "text-[var(--adm-danger-ink)]"
      : tone === "warning"
        ? "text-[var(--adm-warning-ink)]"
        : "text-[var(--adm-ink)]";
  return (
    <span className="flex items-baseline gap-1.5 whitespace-nowrap">
      <span className="adm-eyebrow">{label}</span>
      <span className={`adm-num text-sm font-semibold ${toneClass}`}>{value}</span>
    </span>
  );
}

type SentinelState = "NOMINAL" | "WATCH" | "INCIDENT";

const STATE_TONE: Record<SentinelState, string> = {
  NOMINAL: "text-[var(--adm-success-ink)]",
  WATCH: "text-[var(--adm-warning-ink)]",
  INCIDENT: "text-[var(--adm-danger-ink)]",
};

// Real frozen/failed/drift counts only — replaces the mockup's fake
// `scenario` nominal/incident toggle, which this dashboard never needed
// since the real data already reflects real state. Anything frozen or a
// failed run in the last 24h means something is actually broken
// (INCIDENT); open drift with nothing broken yet is worth a look (WATCH);
// otherwise NOMINAL.
function deriveState(data: AdminOverview): SentinelState {
  if (data.frozen_regimes.length > 0 || data.runs_24h.failed > 0) return "INCIDENT";
  if (data.queue_depths.drift_open > 0) return "WATCH";
  return "NOMINAL";
}

// Polls the overview snapshot every 15s and renders the handful of numbers
// an operator scans first: is anything frozen, is anything failing, how
// deep are the queues — plus a one-word derived state and the live Gold
// planner estimate. Loading/error states stay small and quiet — this
// ticker is ambient, not an alarm panel.
export function SentinelTicker() {
  const { data, isLoading, isError } = useQuery({
    queryKey: ["admin", "overview"],
    queryFn: fetchOverview,
    refetchInterval: 15000,
  });

  if (isLoading) {
    return (
      <div className="flex h-[var(--adm-ticker-h)] items-center border-b border-[var(--adm-rule)] bg-[var(--adm-surface-sunken)] px-4 text-xs text-[var(--adm-muted)]">
        loading sentinel…
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div className="flex h-[var(--adm-ticker-h)] items-center border-b border-[var(--adm-rule)] bg-[var(--adm-surface-sunken)] px-4 text-xs text-[var(--adm-muted)]">
        sentinel ticker unavailable
      </div>
    );
  }

  const frozen = data.frozen_regimes.length;
  const q = data.queue_depths;
  const state = deriveState(data);

  return (
    <div className="flex h-[var(--adm-ticker-h)] flex-wrap items-center gap-x-6 gap-y-1 border-b border-[var(--adm-rule)] bg-[var(--adm-surface-sunken)] px-4">
      <span className={`adm-num text-xs font-bold tracking-[0.08em] ${STATE_TONE[state]}`}>
        {state}
      </span>
      <Item label="frozen" value={frozen} tone={frozen > 0 ? "danger" : undefined} />
      <Item label="running" value={data.runs_24h.running} />
      <Item
        label="failed 24h"
        value={data.runs_24h.failed}
        tone={data.runs_24h.failed > 0 ? "danger" : undefined}
      />
      <Item label="review open" value={q.review_open} />
      <Item
        label="drift open"
        value={q.drift_open}
        tone={q.drift_open > 0 ? "warning" : undefined}
      />
      <Item label="dlq" value={q.delivery_dlq} tone={q.delivery_dlq > 0 ? "danger" : undefined} />
      {/* Gold records: the planner's reltuples estimate, not a count —
          null means postgres has never analyzed the table, rendered as an
          honest dash, never a fabricated number. */}
      <span className="flex items-baseline gap-1.5 whitespace-nowrap">
        <span className="adm-eyebrow">gold records</span>
        {data.gold_records_estimate != null ? (
          <span className="adm-num text-sm font-semibold text-[var(--adm-ink)]">
            {data.gold_records_estimate.toLocaleString()}
          </span>
        ) : (
          <span className="adm-num text-sm font-semibold text-[var(--adm-faint)]">—</span>
        )}
      </span>
    </div>
  );
}
