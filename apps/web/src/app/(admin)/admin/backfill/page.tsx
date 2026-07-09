import type { Metadata } from "next";

import type {
  AdminBackfillRun,
  AdminFetchDensityBucket,
  AdminRegimeCompletion,
  AdminRegimeFreshness,
} from "@/lib/api";
import { ApiError, adminBackfill } from "@/lib/api";
import { AutoRefresh } from "@/components/admin/AutoRefresh";
import { Unavailable } from "@/components/admin/Unavailable";
import { Card } from "@/components/admin/ui/Card";
import { Badge, stateVariant } from "@/components/admin/ui/Badge";
import { Progress } from "@/components/admin/ui/Progress";
import { Stat } from "@/components/admin/ui/Stat";
import { Table, type TableColumn } from "@/components/admin/ui/Table";
import { CountsBar } from "@/components/admin/charts/CountsBar";

export const dynamic = "force-dynamic";

export const metadata: Metadata = { title: "Backfill" };

function formatTimestamp(iso: string | null | undefined): string {
  if (iso == null) {
    return "—";
  }
  return `${new Date(iso).toISOString().replace("T", " ").slice(0, 16)} UTC`;
}

function formatDays(seconds: number | null | undefined): string {
  if (seconds == null) {
    return "—";
  }
  return `${(seconds / 86400).toFixed(1)}d`;
}

function pad2(n: number): string {
  return n.toString().padStart(2, "0");
}

interface DensityRow {
  hour: string;
  [regimeCode: string]: string | number;
}

function buildDensityRows(
  buckets: readonly AdminFetchDensityBucket[],
  generatedAt: string,
): { regimes: string[]; rows: DensityRow[] } {
  const regimes = Array.from(new Set(buckets.map((b) => b.regime_code))).sort();
  const byKey = new Map(
    buckets.map((b) => [`${new Date(b.hour_start).toISOString()}|${b.regime_code}`, b.fetched]),
  );

  const end = new Date(generatedAt);
  end.setUTCMinutes(0, 0, 0);

  const rows: DensityRow[] = [];
  for (let i = 47; i >= 0; i -= 1) {
    const d = new Date(end.getTime() - i * 3_600_000);
    const iso = d.toISOString();
    const row: DensityRow = {
      hour: `${pad2(d.getUTCMonth() + 1)}/${pad2(d.getUTCDate())} ${pad2(d.getUTCHours())}h`,
    };
    for (const regime of regimes) {
      row[regime] = byKey.get(`${iso}|${regime}`) ?? 0;
    }
    rows.push(row);
  }
  return { regimes, rows };
}

function CompletionRow({ completion }: { completion: AdminRegimeCompletion }) {
  const total = completion.target_years.length;
  const succeededInTarget = completion.years_succeeded.filter((y) =>
    completion.target_years.includes(y),
  ).length;
  const hasTarget = total > 0;
  const fraction = hasTarget ? succeededInTarget / total : 0;
  const unlogged = completion.years_with_data.filter(
    (y) => completion.target_years.includes(y) && !completion.years_succeeded.includes(y),
  );

  return (
    <div className="flex flex-col gap-1.5 border-b border-[var(--adm-rule)] py-3 first:pt-0 last:border-b-0">
      <div className="flex items-baseline justify-between gap-3">
        <span className="font-[family-name:var(--adm-font-data)] text-sm font-semibold">
          {completion.regime_code}
        </span>
        {hasTarget ? (
          <span className="adm-num text-xs text-[var(--adm-muted)]">
            {succeededInTarget} / {total} years
          </span>
        ) : (
          <span className="adm-eyebrow">no declared target</span>
        )}
      </div>
      {hasTarget && <Progress value={fraction} tone={fraction >= 1 ? "success" : "warning"} />}
      <p className="text-xs text-[var(--adm-muted)]">
        {completion.missing_years.length > 0
          ? `missing: ${completion.missing_years.join(", ")}`
          : hasTarget
            ? "all declared years covered."
            : `${completion.years_with_data.length} year(s) on record.`}
        {unlogged.length > 0 &&
          ` · ${unlogged.length} year(s) with data but no logged run.`}
      </p>
    </div>
  );
}

export default async function BackfillPage() {
  let data: Awaited<ReturnType<typeof adminBackfill>>;
  try {
    data = await adminBackfill();
  } catch (error) {
    if (error instanceof ApiError && (error.status === 401 || error.status === 403 || error.status === 503)) {
      return <Unavailable reason={error.message} />;
    }
    throw error;
  }

  const runColumns: TableColumn<AdminBackfillRun>[] = [
    { key: "year", header: "year", numeric: true, render: (r) => r.year },
    {
      key: "regime",
      header: "regime",
      render: (r) => (
        <span className="font-[family-name:var(--adm-font-data)]">{r.regime_code}</span>
      ),
    },
    { key: "kind", header: "kind", render: (r) => r.kind },
    {
      key: "status",
      header: "status",
      render: (r) => <Badge variant={stateVariant(r.status)}>{r.status}</Badge>,
    },
    { key: "filings", header: "filings", numeric: true, render: (r) => r.filings },
    { key: "published", header: "published", numeric: true, render: (r) => r.published },
    { key: "gold_inserted", header: "gold inserted", numeric: true, render: (r) => r.gold_inserted },
    {
      key: "failed_count",
      header: "failed",
      numeric: true,
      render: (r) => (
        <span className={r.failed_count > 0 ? "text-[var(--adm-danger-ink)]" : undefined}>
          {r.failed_count}
        </span>
      ),
    },
  ];

  const freshnessColumns: TableColumn<AdminRegimeFreshness>[] = [
    {
      key: "regime",
      header: "regime",
      render: (f) => (
        <span className="font-[family-name:var(--adm-font-data)]">{f.regime_code}</span>
      ),
    },
    {
      key: "sentinel",
      header: "sentinel checked",
      render: (f) => formatTimestamp(f.sentinel_last_checked_at),
    },
    { key: "fetched", header: "last fetched", render: (f) => formatTimestamp(f.last_fetched_at) },
    {
      key: "discovered",
      header: "last discovered",
      render: (f) => formatTimestamp(f.last_discovered_at),
    },
    { key: "lag_p50", header: "lag p50", numeric: true, render: (f) => formatDays(f.lag_p50_seconds) },
    { key: "lag_p90", header: "lag p90", numeric: true, render: (f) => formatDays(f.lag_p90_seconds) },
  ];

  const { regimes: densityRegimes, rows: densityRows } = buildDensityRows(
    data.fetch_density,
    data.generated_at,
  );
  const q = data.queue_depths;

  return (
    <div className="flex flex-col gap-6 px-4 py-6">
      <AutoRefresh seconds={30} />

      <section className="flex flex-wrap items-baseline justify-between gap-3">
        <div>
          <h1>Backfill & ingestion</h1>
          <p className="mt-1 text-sm text-[var(--adm-muted)]">
            Run history, coverage vs declared targets, source freshness, and fetch politeness.
          </p>
        </div>
        <p className="adm-num text-xs text-[var(--adm-muted)]">
          as of {formatTimestamp(data.generated_at)}
        </p>
      </section>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-[2fr_1fr]">
        <Card eyebrow="B2 · completion" title="Coverage vs declared targets">
          <p className="mb-3 text-xs text-[var(--adm-muted)]">{data.targets_note}</p>
          {data.completion.length === 0 ? (
            <p className="text-sm text-[var(--adm-muted)]">No regimes tracked yet.</p>
          ) : (
            data.completion.map((c) => <CompletionRow key={c.regime_code} completion={c} />)
          )}
        </Card>

        <Card eyebrow="B5 · queues" title="Queue depths">
          <div className="grid grid-cols-2 gap-4">
            <Stat
              label="pipeline running"
              value={q.pipeline_running}
              tone={q.pipeline_running > 0 ? "info" : "neutral"}
            />
            <Stat
              label="pipeline failed"
              value={q.pipeline_failed}
              tone={q.pipeline_failed > 0 ? "danger" : "neutral"}
            />
            <Stat label="outbox undispatched" value={q.outbox_undispatched} />
            <Stat label="review open" value={q.review_open} />
            <Stat
              label="drift open"
              value={q.drift_open}
              tone={q.drift_open > 0 ? "warning" : "neutral"}
            />
            <Stat label="sample pending" value={q.sample_pending} />
            <Stat
              label="delivery dlq"
              value={q.delivery_dlq}
              tone={q.delivery_dlq > 0 ? "danger" : "neutral"}
            />
          </div>
          <p className="mt-4 text-xs text-[var(--adm-muted)]">{data.cloud_tasks_note}</p>
        </Card>
      </div>

      <Card eyebrow="B1 · runs" title="Recent backfill runs">
        <p className="mb-3 text-xs text-[var(--adm-muted)]">{data.history_note}</p>
        <Table
          columns={runColumns}
          rows={data.runs}
          getRowKey={(r) => r.id}
          emptyMessage="No backfill_run rows recorded yet."
        />
      </Card>

      <Card eyebrow="B3 · freshness" title="Source freshness & filing lag">
        <Table
          columns={freshnessColumns}
          rows={data.freshness}
          getRowKey={(f) => f.regime_code}
          emptyMessage="No freshness data yet."
        />
      </Card>

      <Card eyebrow="B4 · politeness proxy" title="Fetch density, last 48h">
        <p className="mb-3 text-xs text-[var(--adm-muted)]">{data.fetch_density_note}</p>
        {data.fetch_density.length === 0 ? (
          <p className="text-sm text-[var(--adm-muted)]">
            No fetch activity recorded in the last 48 hours.
          </p>
        ) : (
          <CountsBar
            data={densityRows}
            categoryKey="hour"
            series={densityRegimes.map((r) => ({ key: r, label: r }))}
            stacked
            height={220}
          />
        )}
      </Card>
    </div>
  );
}
