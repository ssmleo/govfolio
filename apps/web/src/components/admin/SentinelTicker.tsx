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

// One ticker stat (dc.html:60-66): microlabel + a mono 13px/600 tabular
// value whose color is a status claim — red for anything broken, amber for
// open drift, info-blue for running, reading ink otherwise.
function Item({
  label,
  value,
  tone,
}: {
  label: string;
  value: number;
  tone?: "danger" | "warning" | "info";
}) {
  const color =
    tone === "danger"
      ? "var(--adm-danger-ink)"
      : tone === "warning"
        ? "var(--adm-warning-ink)"
        : tone === "info"
          ? "var(--adm-info-ink)"
          : "var(--adm-ink)";
  return (
    <span className="inline-flex items-baseline gap-[7px] whitespace-nowrap">
      <span className="adm-microlabel">{label}</span>
      <span className="adm-num text-[13px] font-semibold" style={{ color }}>
        {value}
      </span>
    </span>
  );
}

interface SentinelState {
  word: string;
  wordColor: string;
  dotColor: string;
}

// Real frozen/failed/drift counts only — the design's `scenario`
// nominal/incident toggle stays fake in the prototype; here the real data
// already reflects real state. Anything frozen names its count ("{n}
// frozen"); a failed run in the last 24h with nothing frozen is "failing";
// open drift with nothing broken yet is "watch"; otherwise "all clear".
// Dot colors come from dc.html:1933 (green #4FB582 / red #E28074), plus
// amber #E4B45E for the watch state the prototype's binary toggle never
// rendered.
function deriveState(data: AdminOverview): SentinelState {
  const frozen = data.frozen_regimes.length;
  if (frozen > 0) {
    return { word: `${frozen} frozen`, wordColor: "var(--adm-danger-ink)", dotColor: "#E28074" };
  }
  if (data.runs_24h.failed > 0) {
    return { word: "failing", wordColor: "var(--adm-danger-ink)", dotColor: "#E28074" };
  }
  if (data.queue_depths.drift_open > 0) {
    return { word: "watch", wordColor: "var(--adm-warning-ink)", dotColor: "#E4B45E" };
  }
  return { word: "all clear", wordColor: "var(--adm-success-ink)", dotColor: "#4FB582" };
}

// The sentinel strip (dc.html:53-73): a pulsing status dot + derived state
// word, the six numbers an operator scans first (is anything frozen, is
// anything failing, how deep are the queues), a right-pinned Gold planner
// estimate, and a 15s gold sweep line pacing the 15s poll. Loading/error
// states stay small and quiet — this ticker is ambient, not an alarm panel.
export function SentinelTicker() {
  const { data, isLoading, isError } = useQuery({
    queryKey: ["admin", "overview"],
    queryFn: fetchOverview,
    refetchInterval: 15000,
  });

  if (isLoading) {
    return (
      <div className="flex h-[var(--adm-ticker-h)] items-center border-b border-[var(--adm-rule)] bg-[var(--adm-ticker-bg)] px-7 text-xs text-[var(--adm-muted)]">
        loading sentinel…
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div className="flex h-[var(--adm-ticker-h)] items-center border-b border-[var(--adm-rule)] bg-[var(--adm-ticker-bg)] px-7 text-xs text-[var(--adm-muted)]">
        sentinel ticker unavailable
      </div>
    );
  }

  const frozen = data.frozen_regimes.length;
  const q = data.queue_depths;
  const state = deriveState(data);

  return (
    <div className="relative z-[19] flex h-[var(--adm-ticker-h)] items-center gap-[26px] overflow-hidden border-b border-[var(--adm-rule)] bg-[var(--adm-ticker-bg)] px-7">
      <span className="inline-flex items-center gap-2">
        <span
          className="inline-block h-[7px] w-[7px] rounded-full"
          style={{ background: state.dotColor, animation: "gfPulse 2.4s ease-in-out infinite" }}
        />
        <span
          style={{
            fontSize: "10px",
            fontWeight: 700,
            letterSpacing: ".2em",
            textTransform: "uppercase",
            color: "var(--adm-muted)",
          }}
        >
          Sentinel
        </span>
        <span
          style={{
            fontSize: "10px",
            fontWeight: 700,
            letterSpacing: ".12em",
            textTransform: "uppercase",
            color: state.wordColor,
          }}
        >
          {state.word}
        </span>
      </span>
      <div className="h-4 w-px shrink-0 bg-[#262B34]" />
      <div
        className="flex min-w-0 flex-1 items-baseline gap-[22px] overflow-hidden"
        style={{
          maskImage: "linear-gradient(90deg,#000 88%,transparent)",
          WebkitMaskImage: "linear-gradient(90deg,#000 88%,transparent)",
        }}
      >
        {/* DOM text stays lowercase (as before this redesign — the e2e shell
            spec asserts it case-sensitively); .adm-microlabel's uppercase
            transform renders it identically to the design's "Frozen" /
            "Failed 24h" / "DLQ" casing. */}
        <Item label="frozen" value={frozen} tone={frozen > 0 ? "danger" : undefined} />
        <Item label="running" value={data.runs_24h.running} tone="info" />
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
      {/* Gold records: the planner's reltuples estimate, not a count —
          null means postgres has never analyzed the table, rendered as an
          honest dash, never a fabricated number. */}
      <span className="inline-flex items-baseline gap-[7px] whitespace-nowrap">
        <span className="adm-microlabel">gold records</span>
        {data.gold_records_estimate != null ? (
          <span className="adm-num text-[13px] font-semibold text-[var(--adm-accent)]">
            {/* Locale pinned: the console is English-only and tests must not
                depend on the host's ICU default. */}
            {data.gold_records_estimate.toLocaleString("en-US")}
          </span>
        ) : (
          <span className="adm-num text-[13px] font-semibold text-[var(--adm-faint)]">—</span>
        )}
      </span>
      <div
        className="absolute left-0 h-[2px] w-full origin-left"
        style={{
          bottom: "-1px",
          background: "linear-gradient(90deg,rgba(194,161,94,0),var(--adm-gold-55))",
          animation: "gfSweep 15s linear infinite",
        }}
      />
    </div>
  );
}
