import type { CSSProperties } from "react";

import type { AdminGrowthDay, AdminPgTable, AdminStorage } from "@/lib/api";
import { ApiError, adminStorage } from "@/lib/api";
import { Unavailable } from "@/components/admin/Unavailable";
import { BarRows } from "@/components/admin/charts/BarRows";
import type { TrendPoint } from "@/components/admin/charts/TrendChart";
import { TrendChart } from "@/components/admin/charts/TrendChart";
import { Card } from "@/components/admin/ui/Card";
import { Progress } from "@/components/admin/ui/Progress";
import { Screen } from "@/components/admin/ui/Screen";
import type { TableColumn } from "@/components/admin/ui/Table";
import { Table } from "@/components/admin/ui/Table";
import { formatCount, formatUtcMinute } from "@/lib/format";

// Section E of the admin dashboard (goal 091): what's stored, how much, and
// where it lives. Always fresh — a storage snapshot must not be ISR-stale.
export const dynamic = "force-dynamic";

function formatBytes(bytes: number): string {
  if (bytes <= 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB", "TB"] as const;
  const exp = Math.min(Math.floor(Math.log2(bytes) / 10), units.length - 1);
  const value = bytes / 1024 ** exp;
  return `${value.toFixed(exp === 0 ? 0 : 1)} ${units[exp]}`;
}

// dc.html:824 — mime/scheme bar labels are 11.5px in full ink (replaces the
// BarRows default 11px secondary label type).
const BAR_LABEL_CLASS = "text-[11.5px] text-[var(--adm-ink)]";

// dc.html:829/849/922 — card footnote: 12px above, 11px, meta ink.
const CAPTION_STYLE: CSSProperties = {
  margin: "12px 0 0",
  fontSize: 11,
  color: "var(--adm-meta)",
};

// The API reports the URI scheme bare ("gs") and plain paths as "local";
// the design renders real schemes with their "://" (dc.html:1716-1717).
function schemeLabel(scheme: string): string {
  return scheme === "local" ? "local" : `${scheme}://`;
}

// dc.html:1732 — dead-tuple percentage ink: >=40 danger, >=20 warning, else meta.
function deadPctColor(pct: number): string {
  if (pct >= 40) {
    return "var(--adm-danger-ink)";
  }
  if (pct >= 20) {
    return "var(--adm-warning-ink)";
  }
  return "var(--adm-meta)";
}

interface GrowthSeries {
  gold: TrendPoint[];
  filings: TrendPoint[];
}

// E2 contract: growth_30d omits days with zero activity on both clocks —
// absent means zero. Densify to a continuous daily series so the trend line
// doesn't interpolate across quiet days. Window: the 30 UTC calendar days
// ending at the snapshot date, widened to include any earlier day the API
// reported (its `>= now() - 30 days` window can clip a 31st partial day).
function densifyGrowth(days: readonly AdminGrowthDay[], generatedAt: string): GrowthSeries {
  const DAY_MS = 86_400_000;
  const byDay = new Map(days.map((d) => [d.day, d]));
  const endMs = Date.parse(`${generatedAt.slice(0, 10)}T00:00:00Z`);
  let startMs = endMs - 29 * DAY_MS;
  const firstReported = days[0]?.day;
  if (firstReported !== undefined) {
    startMs = Math.min(startMs, Date.parse(`${firstReported}T00:00:00Z`));
  }
  const gold: TrendPoint[] = [];
  const filings: TrendPoint[] = [];
  for (let t = startMs; t <= endMs; t += DAY_MS) {
    const day = new Date(t).toISOString().slice(0, 10);
    const row = byDay.get(day);
    gold.push({ label: day, value: row?.gold_records ?? 0 });
    filings.push({ label: day, value: row?.filings ?? 0 });
  }
  return { gold, filings };
}

// dc.html:914-916 — Size/Live cells: mono 12px secondary ink (the td itself
// carries the mono/tabular class via `numeric`).
const CELL_SECONDARY: CSSProperties = { fontSize: 12, color: "var(--adm-text-secondary)" };

export default async function StoragePage() {
  let data: AdminStorage;
  try {
    data = await adminStorage();
  } catch (error) {
    if (error instanceof ApiError && [401, 403, 503].includes(error.status)) {
      return <Unavailable reason={error.message} />;
    }
    throw error;
  }

  const totalSchemeDocs = data.bronze_by_scheme.reduce((sum, s) => sum + s.documents, 0);
  const maxSchemeDocs = Math.max(0, ...data.bronze_by_scheme.map((s) => s.documents));
  const gsDocs = data.bronze_by_scheme.find((s) => s.scheme === "gs")?.documents ?? 0;
  const gsFraction = totalSchemeDocs > 0 ? gsDocs / totalSchemeDocs : 0;

  const totalGoldRecords = data.growth_30d.reduce((sum, d) => sum + d.gold_records, 0);
  const totalFilings = data.growth_30d.reduce((sum, d) => sum + d.filings, 0);
  const growth = densifyGrowth(data.growth_30d, data.generated_at);

  const pgTableColumns: TableColumn<AdminPgTable>[] = [
    {
      key: "table_name",
      header: "Table",
      render: (row) => (
        <span
          style={{ fontFamily: "var(--adm-font-data)", fontSize: 11.5, color: "var(--adm-ink)" }}
        >
          {row.table_name}
        </span>
      ),
    },
    {
      key: "total_bytes",
      header: "Size",
      numeric: true,
      nowrap: true,
      render: (row) => <span style={CELL_SECONDARY}>{formatBytes(row.total_bytes)}</span>,
    },
    {
      key: "live_tuples",
      header: "Live",
      numeric: true,
      render: (row) => <span style={CELL_SECONDARY}>{formatCount(row.live_tuples)}</span>,
    },
    {
      key: "dead_tuples",
      header: "Dead",
      numeric: true,
      render: (row) => (
        <span style={{ fontSize: 12, color: "var(--adm-muted)" }}>
          {formatCount(row.dead_tuples)}
        </span>
      ),
    },
    {
      key: "dead_ratio",
      header: "Dead %",
      numeric: true,
      render: (row) => {
        const total = row.live_tuples + row.dead_tuples;
        const pct = total > 0 ? Math.round((100 * row.dead_tuples) / total) : 0;
        return <span style={{ fontSize: 12, color: deadPctColor(pct) }}>{pct}%</span>;
      },
    },
  ];

  return (
    <Screen
      label="Storage"
      kicker="Section E"
      title="Storage & tiers"
      subtitle="Bronze documents, Silver/Gold row counts, and the Postgres footprint underneath."
      meta={<>snapshot {formatUtcMinute(data.generated_at)}</>}
    >
      <div
        style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, alignItems: "start" }}
      >
        <Card section="E1" label="Bronze documents" title="By mime type" rise={0.05}>
          {/* design h2 sits 12px above content (dc.html:821); Card gives 8. */}
          <div style={{ marginTop: 4 }}>
            <BarRows
              rows={data.bronze_by_mime.map((m) => ({ label: m.mime_type, value: m.documents }))}
              labelWidth={150}
              labelClassName={BAR_LABEL_CLASS}
              barHeight={6}
              fill="var(--adm-series-bronze)"
              valueWidth={66}
              ruled
            />
            <p style={CAPTION_STYLE}>
              Bronze is immutable and sha256-addressed;{" "}
              <span style={{ fontFamily: "var(--adm-font-data)" }}>asset_description_raw</span> is
              always retained.
            </p>
          </div>
        </Card>

        <Card section="E1" label="Cloud migration" title="By storage scheme" rise={0.1}>
          <div style={{ marginTop: 4 }}>
            {totalSchemeDocs > 0 && (
              <div style={{ marginBottom: 14 }}>
                <Progress
                  value={gsFraction}
                  color="var(--adm-series-funnel-gold)"
                  label="migrated to cloud storage (gs://)"
                />
              </div>
            )}
            {/* one BarRows per scheme: the design colors gs:// green and
                everything else (file, local) neutral, scaled against the
                largest scheme. */}
            {data.bronze_by_scheme.map((row) => (
              <BarRows
                key={row.scheme}
                rows={[{ label: schemeLabel(row.scheme), value: row.documents }]}
                max={maxSchemeDocs}
                labelWidth={150}
                labelClassName={BAR_LABEL_CLASS}
                barHeight={6}
                fill={
                  row.scheme === "gs"
                    ? "var(--adm-series-funnel-gold)"
                    : "var(--adm-series-neutral)"
                }
                valueWidth={66}
                ruled
              />
            ))}
            <p style={CAPTION_STYLE}>
              Scheme flip is the whole cloud migration for Bronze — env flip only, no rewrite.
            </p>
          </div>
        </Card>
      </div>

      <Card section="E2" label="Last 30 days" title="Gold + filing growth" rise={0.15} className="mt-4">
        {/* design h2 sits 16px above the chart grid (dc.html:855); Card gives 8. */}
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 26, marginTop: 8 }}>
          <div>
            <div
              style={{
                display: "flex",
                alignItems: "baseline",
                justifyContent: "space-between",
                marginBottom: 8,
              }}
            >
              <span className="adm-microlabel">Gold records</span>
              <span
                style={{
                  fontFamily: "var(--adm-font-data)",
                  fontSize: 15,
                  fontWeight: 600,
                  color: "var(--adm-accent-deep)",
                  fontVariantNumeric: "tabular-nums",
                }}
              >
                {formatCount(totalGoldRecords)}
              </span>
            </div>
            <TrendChart
              points={growth.gold}
              size="small"
              palette="gold"
              ariaLabel="Gold records created per day, last 30 days"
            />
          </div>
          <div>
            <div
              style={{
                display: "flex",
                alignItems: "baseline",
                justifyContent: "space-between",
                marginBottom: 8,
              }}
            >
              <span className="adm-microlabel">Filings discovered</span>
              <span
                style={{
                  fontFamily: "var(--adm-font-data)",
                  fontSize: 15,
                  fontWeight: 600,
                  color: "var(--adm-text-secondary)",
                  fontVariantNumeric: "tabular-nums",
                }}
              >
                {formatCount(totalFilings)}
              </span>
            </div>
            <TrendChart
              points={growth.filings}
              size="small"
              palette="silver"
              ariaLabel="Filings discovered per day, last 30 days"
            />
          </div>
        </div>
      </Card>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: ".85fr 1.15fr",
          gap: 16,
          marginTop: 16,
          alignItems: "start",
        }}
      >
        <Card section="Schema" title="Rows per table" rise={0.2}>
          <div style={{ marginTop: 4 }}>
            {data.table_rows.map((t) => (
              <div
                key={t.table_name}
                style={{
                  display: "flex",
                  alignItems: "baseline",
                  justifyContent: "space-between",
                  gap: 12,
                  borderTop: "1px solid var(--adm-rule)",
                  padding: "8px 0",
                }}
              >
                <span
                  style={{
                    fontFamily: "var(--adm-font-data)",
                    fontSize: 11.5,
                    color: "var(--adm-text-secondary)",
                  }}
                >
                  {t.table_name}
                </span>
                <span
                  style={{
                    fontFamily: "var(--adm-font-data)",
                    fontSize: 12,
                    color: "var(--adm-ink)",
                    fontVariantNumeric: "tabular-nums",
                  }}
                >
                  {formatCount(t.row_count)}
                </span>
              </div>
            ))}
          </div>
        </Card>

        <Card
          section="E3"
          label="Postgres"
          title="Top tables"
          meta={
            <>
              database size{" "}
              <span style={{ color: "var(--adm-accent-deep)", fontWeight: 600 }}>
                {formatBytes(data.pg.database_size_bytes)}
              </span>
            </>
          }
          rise={0.25}
        >
          <div style={{ marginTop: 4 }}>
            <Table
              columns={pgTableColumns}
              rows={data.pg.top_tables}
              getRowKey={(row) => row.table_name}
              emptyMessage="No tables reported."
            />
            <p style={CAPTION_STYLE}>
              outbox dead-tuple ratio reflects dispatch churn — autovacuum keeps pace; watch past
              40%.
            </p>
          </div>
        </Card>
      </div>
    </Screen>
  );
}
