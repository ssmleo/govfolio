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
import { Screen } from "@/components/admin/ui/Screen";
import { Stat } from "@/components/admin/ui/Stat";
import { Table, type TableColumn } from "@/components/admin/ui/Table";
import { DensityColumns, type DensityHour } from "@/components/admin/charts/DensityColumns";
import { formatCount, formatDays, formatMonthDayTime, formatUtcMinute } from "@/lib/format";

export const dynamic = "force-dynamic";

export const metadata: Metadata = { title: "Backfill" };

function pad2(n: number): string {
  return n.toString().padStart(2, "0");
}

// B4 series ramp (dc.html:1657-1661): top-3 regimes by 48h volume, then
// "others". #8FB2E8 has no --adm-series-* token (only the state token
// --adm-info-ink carries that hex), so the raw design value stays raw here.
const DENSITY_SERIES_COLORS = [
  "var(--adm-series-gold)",
  "#8FB2E8",
  "var(--adm-series-funnel-gold)",
] as const;
const DENSITY_OTHERS_COLOR = "var(--adm-series-neutral)";

interface DensityLegendItem {
  label: string;
  color: string;
}

function buildDensity(
  buckets: readonly AdminFetchDensityBucket[],
  generatedAt: string,
): { legend: DensityLegendItem[]; hours: DensityHour[] } {
  const volume = new Map<string, number>();
  for (const b of buckets) {
    volume.set(b.regime_code, (volume.get(b.regime_code) ?? 0) + b.fetched);
  }
  const ranked = Array.from(volume.entries())
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([regime]) => regime);
  const top = ranked
    .slice(0, DENSITY_SERIES_COLORS.length)
    .map((regime, i) => ({ regime, color: DENSITY_SERIES_COLORS[i] ?? DENSITY_OTHERS_COLOR }));
  const others = ranked.slice(DENSITY_SERIES_COLORS.length);

  const byKey = new Map(
    buckets.map((b) => [`${new Date(b.hour_start).toISOString()}|${b.regime_code}`, b.fetched]),
  );

  const end = new Date(generatedAt);
  end.setUTCMinutes(0, 0, 0);

  const hours: DensityHour[] = [];
  for (let i = 47; i >= 0; i -= 1) {
    const d = new Date(end.getTime() - i * 3_600_000);
    const iso = d.toISOString();
    const at = (regime: string): number => byKey.get(`${iso}|${regime}`) ?? 0;
    // Bottom-up order (dc.html:621-626): primary series on the baseline.
    const segments = top.map(({ regime, color }) => ({ value: at(regime), color }));
    if (others.length > 0) {
      segments.push({
        value: others.reduce((sum, regime) => sum + at(regime), 0),
        color: DENSITY_OTHERS_COLOR,
      });
    }
    const total = segments.reduce((sum, s) => sum + s.value, 0);
    hours.push({ title: `${pad2(d.getUTCHours())}:00 UTC · ${total} docs`, segments });
  }

  const legend: DensityLegendItem[] = top.map(({ regime, color }) => ({ label: regime, color }));
  if (others.length > 0) {
    legend.push({ label: "others", color: DENSITY_OTHERS_COLOR });
  }
  return { legend, hours };
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

  // "All declared years covered" requires every target year to actually be
  // succeeded, not merely "not missing" — a target year can have raw data
  // but no successful logged run (the `unlogged` bucket below), and that's
  // not coverage.
  const complete = hasTarget && succeededInTarget === total;
  const note = [
    completion.missing_years.length > 0
      ? `missing: ${completion.missing_years.join(", ")}`
      : complete
        ? "all declared years covered."
        : !hasTarget
          ? `${completion.years_with_data.length} year(s) on record.`
          : null,
    ...(unlogged.length > 0 ? [`${unlogged.length} year(s) with data but no logged run.`] : []),
  ]
    .filter((part): part is string => part !== null)
    .join(" · ");

  return (
    <div style={{ borderTop: "1px solid var(--adm-rule)", padding: "11px 0" }}>
      <div
        style={{
          display: "flex",
          alignItems: "baseline",
          justifyContent: "space-between",
          gap: 12,
          marginBottom: 7,
        }}
      >
        <span
          style={{
            fontFamily: "var(--adm-font-data)",
            fontSize: "12.5px",
            color: "var(--adm-ink)",
          }}
        >
          {completion.regime_code}
        </span>
        <span className="adm-num" style={{ fontSize: 11, color: "var(--adm-meta)" }}>
          {hasTarget ? `${succeededInTarget} / ${total} years` : "no declared target"}
        </span>
      </div>
      {hasTarget && (
        <Progress
          value={fraction}
          color={
            fraction >= 1 ? "var(--adm-series-funnel-gold)" : "var(--adm-series-funnel-review)"
          }
        />
      )}
      <p style={{ margin: "6px 0 0", fontSize: 11, color: "var(--adm-meta)" }}>{note}</p>
    </div>
  );
}

const RUN_NUM_STYLE: React.CSSProperties = {
  fontSize: "12.5px",
  color: "var(--adm-text-secondary)",
};

const FRESHNESS_TS_STYLE: React.CSSProperties = {
  fontFamily: "var(--adm-font-data)",
  fontSize: "11.5px",
  color: "var(--adm-muted)",
};

function formatFreshnessTs(iso: string | null | undefined): string {
  return iso == null ? "—" : formatMonthDayTime(iso);
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
    {
      key: "year",
      header: "Year",
      numeric: true,
      render: (r) => (
        <span style={{ fontSize: "12.5px", color: "var(--adm-ink)" }}>{r.year}</span>
      ),
    },
    {
      key: "regime",
      header: "Regime",
      render: (r) => (
        <span
          style={{
            fontFamily: "var(--adm-font-data)",
            fontSize: 12,
            color: "var(--adm-text-secondary)",
          }}
        >
          {r.regime_code}
        </span>
      ),
    },
    {
      key: "kind",
      header: "Kind",
      render: (r) => <span style={{ fontSize: 12, color: "var(--adm-muted)" }}>{r.kind}</span>,
    },
    {
      key: "status",
      header: "Status",
      render: (r) => <Badge variant={stateVariant(r.status)}>{r.status}</Badge>,
    },
    {
      key: "filings",
      header: "Filings",
      numeric: true,
      render: (r) => <span style={RUN_NUM_STYLE}>{formatCount(r.filings)}</span>,
    },
    {
      key: "published",
      header: "Published",
      numeric: true,
      render: (r) => <span style={RUN_NUM_STYLE}>{formatCount(r.published)}</span>,
    },
    {
      key: "gold_inserted",
      header: "Gold inserted",
      numeric: true,
      nowrap: true,
      render: (r) => (
        <span style={{ fontSize: "12.5px", fontWeight: 600, color: "var(--adm-accent-deep)" }}>
          {formatCount(r.gold_inserted)}
        </span>
      ),
    },
    {
      key: "failed_count",
      header: "Failed",
      numeric: true,
      render: (r) => (
        <span
          style={{
            fontSize: "12.5px",
            color: r.failed_count > 0 ? "var(--adm-danger-ink)" : "var(--adm-text-secondary)",
          }}
        >
          {formatCount(r.failed_count)}
        </span>
      ),
    },
  ];

  const freshnessColumns: TableColumn<AdminRegimeFreshness>[] = [
    {
      key: "regime",
      header: "Regime",
      render: (f) => (
        <span
          style={{ fontFamily: "var(--adm-font-data)", fontSize: 12, color: "var(--adm-ink)" }}
        >
          {f.regime_code}
        </span>
      ),
    },
    {
      key: "sentinel",
      header: "Sentinel checked",
      nowrap: true,
      render: (f) => <span style={FRESHNESS_TS_STYLE}>{formatFreshnessTs(f.sentinel_last_checked_at)}</span>,
    },
    {
      key: "fetched",
      header: "Last fetched",
      nowrap: true,
      render: (f) => <span style={FRESHNESS_TS_STYLE}>{formatFreshnessTs(f.last_fetched_at)}</span>,
    },
    {
      key: "discovered",
      header: "Last discovered",
      nowrap: true,
      render: (f) => <span style={FRESHNESS_TS_STYLE}>{formatFreshnessTs(f.last_discovered_at)}</span>,
    },
    {
      key: "lag_p50",
      header: "Lag p50",
      numeric: true,
      nowrap: true,
      render: (f) => <span style={RUN_NUM_STYLE}>{formatDays(f.lag_p50_seconds)}</span>,
    },
    {
      key: "lag_p90",
      header: "Lag p90",
      numeric: true,
      nowrap: true,
      render: (f) => <span style={RUN_NUM_STYLE}>{formatDays(f.lag_p90_seconds)}</span>,
    },
  ];

  const density = buildDensity(data.fetch_density, data.generated_at);
  const q = data.queue_depths;

  return (
    <>
      <AutoRefresh seconds={30} />
      <Screen
        label="Backfill"
        kicker="Section B"
        title="Backfill & ingestion"
        subtitle="Run history, coverage vs declared targets, source freshness, and fetch politeness."
        meta={<>as of {formatUtcMinute(data.generated_at)}</>}
      >
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "1.15fr .85fr",
            gap: 16,
            alignItems: "start",
          }}
        >
          <Card section="B2" label="Completion" title="Coverage vs declared targets" rise={0.05}>
            {/* -2px top collapses with the card h2's 8px bottom to the design's 6px gap. */}
            <p style={{ margin: "-2px 0 8px", fontSize: 11, color: "var(--adm-meta)" }}>
              {data.targets_note}
            </p>
            {data.completion.length === 0 ? (
              <p className="adm-muted" style={{ fontSize: "12.5px" }}>
                No regimes tracked yet.
              </p>
            ) : (
              data.completion.map((c) => <CompletionRow key={c.regime_code} completion={c} />)
            )}
          </Card>

          <Card section="B5" label="Queues" title="Queue depths" rise={0.12}>
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "repeat(2, 1fr)",
                gap: "16px 14px",
                marginTop: 16,
              }}
            >
              <Stat
                label="Pipeline running"
                value={formatCount(q.pipeline_running)}
                size={22}
                tone={q.pipeline_running > 0 ? "info" : undefined}
              />
              <Stat
                label="Pipeline failed"
                value={formatCount(q.pipeline_failed)}
                size={22}
                tone={q.pipeline_failed > 0 ? "danger" : undefined}
              />
              <Stat label="Outbox undispatched" value={formatCount(q.outbox_undispatched)} size={22} />
              <Stat label="Review open" value={formatCount(q.review_open)} size={22} />
              <Stat
                label="Drift open"
                value={formatCount(q.drift_open)}
                size={22}
                tone={q.drift_open > 0 ? "warning" : undefined}
              />
              <Stat label="Sample pending" value={formatCount(q.sample_pending)} size={22} />
              <Stat
                label="Delivery DLQ"
                value={formatCount(q.delivery_dlq)}
                size={22}
                tone={q.delivery_dlq > 0 ? "danger" : undefined}
              />
            </div>
            <p
              style={{
                margin: "16px 0 0",
                fontSize: 11,
                color: "var(--adm-meta)",
                borderTop: "1px solid var(--adm-rule)",
                paddingTop: 12,
              }}
            >
              {data.cloud_tasks_note}
            </p>
          </Card>
        </div>

        <Card
          section="B1"
          label="Runs"
          title="Recent backfill runs"
          meta={`newest ${data.runs.length} logged runs`}
          rise={0.19}
          className="mt-[16px]"
        >
          <div style={{ marginTop: 12 }}>
            <Table
              columns={runColumns}
              rows={data.runs}
              getRowKey={(r) => r.id}
              emptyMessage="No backfill_run rows recorded yet."
            />
          </div>
        </Card>

        <Card
          section="B3"
          label="Freshness"
          title="Source freshness & filing lag"
          rise={0.26}
          className="mt-[16px]"
        >
          <div style={{ marginTop: 12 }}>
            <Table
              columns={freshnessColumns}
              rows={data.freshness}
              getRowKey={(f) => f.regime_code}
              emptyMessage="No freshness data yet."
            />
          </div>
        </Card>

        <Card
          section="B4"
          label="Politeness proxy"
          title="Fetch density, last 48h"
          rise={0.33}
          className="mt-[16px]"
        >
          {data.fetch_density.length === 0 ? (
            <p className="adm-muted" style={{ margin: "12px 0 0", fontSize: "12.5px" }}>
              No fetch activity recorded in the last 48 hours.
            </p>
          ) : (
            <>
              <div style={{ display: "flex", gap: 14, marginTop: 12, marginBottom: 14 }}>
                {density.legend.map((l) => (
                  <span
                    key={l.label}
                    style={{ display: "inline-flex", alignItems: "center", gap: 6 }}
                  >
                    <span
                      style={{
                        width: 9,
                        height: 9,
                        borderRadius: 1,
                        background: l.color,
                        display: "inline-block",
                      }}
                    />
                    <span style={{ fontSize: "10.5px", color: "var(--adm-meta)" }}>{l.label}</span>
                  </span>
                ))}
              </div>
              <DensityColumns hours={density.hours} />
            </>
          )}
          <p
            style={{
              margin: "12px 0 0",
              fontSize: 11,
              color: "var(--adm-meta)",
              borderTop: "1px solid var(--adm-rule)",
              paddingTop: 12,
            }}
          >
            Documents fetched per hour per regime — spikes sit inside the nightly cron windows.
            Concurrency 1, per-source min-interval, conditional GETs, identified UA.
          </p>
        </Card>
      </Screen>
    </>
  );
}
