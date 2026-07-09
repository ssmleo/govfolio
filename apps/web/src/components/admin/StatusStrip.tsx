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

// Polls the overview snapshot every 15s and renders the handful of numbers
// an operator scans first: is anything frozen, is anything failing, how
// deep are the queues. Loading/error states stay small and quiet — this
// strip is ambient, not an alarm panel.
export function StatusStrip() {
  const { data, isLoading, isError } = useQuery({
    queryKey: ["admin", "overview"],
    queryFn: fetchOverview,
    refetchInterval: 15000,
  });

  if (isLoading) {
    return (
      <div className="border-b border-[var(--adm-rule)] bg-[var(--adm-surface-sunken)] px-4 py-2 text-xs text-[var(--adm-muted)]">
        loading status…
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div className="border-b border-[var(--adm-rule)] bg-[var(--adm-surface-sunken)] px-4 py-2 text-xs text-[var(--adm-muted)]">
        status strip unavailable
      </div>
    );
  }

  const frozen = data.frozen_regimes.length;
  const q = data.queue_depths;

  return (
    <div className="flex flex-wrap items-baseline gap-x-6 gap-y-1 border-b border-[var(--adm-rule)] bg-[var(--adm-surface-sunken)] px-4 py-2">
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
    </div>
  );
}
