import type { CSSProperties } from "react";

import type { AdminAttemptsBucket, AdminServing, AdminUsageDay } from "@/lib/api";
import { ApiError, adminServing } from "@/lib/api";
import { Badge, stateVariant } from "@/components/admin/ui/Badge";
import { Card } from "@/components/admin/ui/Card";
import { Screen } from "@/components/admin/ui/Screen";
import { Stat } from "@/components/admin/ui/Stat";
import { BarRows } from "@/components/admin/charts/BarRows";
import { ColumnChart } from "@/components/admin/charts/ColumnChart";
import { TrendChart } from "@/components/admin/charts/TrendChart";
import { Unavailable } from "@/components/admin/Unavailable";
import { formatCount, formatMonthDayTime, formatUtcMinute } from "@/lib/format";

export const dynamic = "force-dynamic";

const DAY_MS = 86_400_000;

// Latency percentiles render as "3.1s" / "41.0s" (dc.html sv.dispatch/sv.send);
// "—" when the distribution is empty.
function formatSecondsTenth(value: number | null | undefined): string {
  if (value === null || value === undefined) {
    return "—";
  }
  return `${value.toFixed(1)}s`;
}

// Trend x-axis day labels, "Jun 26" (dc.html:956).
function formatDay(day: string): string {
  return new Date(`${day}T00:00:00Z`).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    timeZone: "UTC",
  });
}

// usage_by_day omits days with zero metered requests; rebuild the full 14-day
// window ending on the snapshot's UTC date so the trend renders one point per
// day — real zeros, never interpolation.
function densifyUsage(usage: readonly AdminUsageDay[], generatedAt: string): AdminUsageDay[] {
  const byDay = new Map(usage.map((row) => [row.day, row.requests]));
  const end = new Date(generatedAt);
  const endUtc = Date.UTC(end.getUTCFullYear(), end.getUTCMonth(), end.getUTCDate());
  return Array.from({ length: 14 }, (_, i) => {
    const day = new Date(endUtc - (13 - i) * DAY_MS).toISOString().slice(0, 10);
    return { day, requests: byDay.get(day) ?? 0 };
  });
}

// Real tier names (`free` | `pro` | `data`) → the design's account stat labels
// (dc.html:1748); unknown tiers fall back to "<tier> users" rather than guessing.
const TIER_LABEL: Record<string, string> = {
  free: "Free users",
  pro: "Pro users",
  data: "API users",
};
const TIER_ORDER = ["free", "pro", "data"];

function tierRank(tier: string): number {
  const rank = TIER_ORDER.indexOf(tier);
  return rank === -1 ? TIER_ORDER.length : rank;
}

// Design buckets 1 / 2 / 3 / ≥4 (dc.html:1766) from the exact-count histogram.
function attemptsBuckets(
  histogram: readonly AdminAttemptsBucket[],
): { bucket: string; count: number }[] {
  const counts = [0, 0, 0, 0];
  for (const row of histogram) {
    if (row.attempts >= 4) {
      counts[3] = (counts[3] ?? 0) + row.count;
    } else if (row.attempts >= 1) {
      counts[row.attempts - 1] = (counts[row.attempts - 1] ?? 0) + row.count;
    }
  }
  return [
    { bucket: "1", count: counts[0] ?? 0 },
    { bucket: "2", count: counts[1] ?? 0 },
    { bucket: "3", count: counts[2] ?? 0 },
    { bucket: "≥4", count: counts[3] ?? 0 },
  ];
}

const MONO = "var(--adm-font-data)";

// "Dispatch — outbox → delivery" / "Send — created → sent" sub-labels
// (dc.html:987/996): 10px/700/.14em sits between adm-card-eyebrow (.16em) and
// adm-microlabel (9.5px), so it stays inline.
const SUB_LABEL: CSSProperties = {
  fontSize: "10px",
  fontWeight: 700,
  letterSpacing: ".14em",
  textTransform: "uppercase",
  color: "var(--adm-meta)",
};

// Status × channel table cells (dc.html:1017-1027).
const STATUS_TH: CSSProperties = {
  textAlign: "left",
  padding: "8px 12px 8px 0",
  borderBottom: "1px solid var(--adm-rule-strong)",
};
const STATUS_TD: CSSProperties = {
  padding: "9px 12px 9px 0",
  borderBottom: "1px solid var(--adm-rule)",
};

// Dead-letter table cells (dc.html:1062-1066) — headerless by design.
const DLQ_TD: CSSProperties = {
  padding: "10px 14px 10px 0",
  borderBottom: "1px solid var(--adm-rule)",
};

export default async function ServingPage() {
  let data: AdminServing;
  try {
    data = await adminServing();
  } catch (error) {
    if (error instanceof ApiError && (error.status === 401 || error.status === 403 || error.status === 503)) {
      return <Unavailable reason={error.message} />;
    }
    throw error;
  }

  const usageDays = densifyUsage(data.usage_by_day, data.generated_at);
  const firstDay = usageDays[0];
  const lastDay = usageDays[usageDays.length - 1];
  const usagePoints = usageDays.map((row) => ({ label: formatDay(row.day), value: row.requests }));

  const endpointRows = data.top_endpoints_7d.map((row) => ({
    label: row.endpoint,
    value: row.requests,
  }));

  const tiers = [...data.accounts.users_by_tier].sort(
    (a, b) => tierRank(a.tier) - tierRank(b.tier),
  );
  const attempts = attemptsBuckets(data.deliveries.attempts_histogram);
  const dead = data.deliveries.recent_dead;

  return (
    <Screen
      label="Serving"
      kicker="Section F"
      title="Serving & product"
      subtitle="API usage, accounts, alert-pipeline latency, and delivery health."
      meta={`generated ${formatUtcMinute(data.generated_at)}`}
    >
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1.15fr .85fr",
          gap: 16,
          alignItems: "start",
        }}
      >
        <Card section="F1" label="Usage" title="Requests per day" meta="last 14 days" rise={0.05}>
          <div style={{ marginTop: 14 }}>
            <TrendChart
              points={usagePoints}
              size="wide"
              endpointDot
              xLeftLabel={firstDay === undefined ? undefined : formatDay(firstDay.day)}
              xRightLabel={
                lastDay === undefined
                  ? undefined
                  : `${formatDay(lastDay.day)} · ${formatCount(lastDay.requests)} requests`
              }
              ariaLabel="Requests per day, last 14 days"
            />
          </div>
          <p
            className="adm-card-eyebrow"
            style={{ margin: "16px 0 10px", borderTop: "1px solid var(--adm-rule)", paddingTop: 14 }}
          >
            Top endpoints · last 7 days
          </p>
          {endpointRows.length > 0 ? (
            <BarRows
              rows={endpointRows}
              labelWidth={236}
              barHeight={12}
              fill="var(--adm-series-gold)"
              valueWidth={56}
              gap={9}
            />
          ) : (
            <p className="adm-muted" style={{ margin: 0, fontSize: "12.5px" }}>
              No metered requests in the last 7 days.
            </p>
          )}
        </Card>

        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          <Card section="F1" label="Accounts" title="Accounts" rise={0.1}>
            <div
              style={{
                marginTop: 16,
                display: "grid",
                gridTemplateColumns: "repeat(3, 1fr)",
                gap: "16px 14px",
              }}
            >
              {tiers.map((row) => (
                <Stat
                  key={row.tier}
                  label={TIER_LABEL[row.tier] ?? `${row.tier} users`}
                  value={formatCount(row.users)}
                  size={20}
                />
              ))}
              <Stat label="Active keys" value={formatCount(data.accounts.active_keys)} size={20} />
              <Stat
                label="Revoked keys"
                value={formatCount(data.accounts.revoked_keys)}
                size={20}
              />
              <Stat
                label="Subscriptions"
                value={formatCount(data.accounts.active_subscriptions)}
                size={20}
              />
            </div>
          </Card>

          <Card section="F2" label="Alert latency" title="Alert pipeline" rise={0.15}>
            <p style={{ ...SUB_LABEL, margin: "16px 0 10px" }}>Dispatch — outbox → delivery</p>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 14 }}>
              <Stat
                label="p50"
                value={formatSecondsTenth(data.alert_latency.dispatch_p50_seconds)}
                size={18}
              />
              <Stat
                label="p90"
                value={formatSecondsTenth(data.alert_latency.dispatch_p90_seconds)}
                size={18}
              />
              <Stat
                label="p99"
                value={formatSecondsTenth(data.alert_latency.dispatch_p99_seconds)}
                size={18}
              />
              <Stat
                label="Dispatched"
                value={formatCount(data.alert_latency.dispatched_count)}
                size={18}
              />
              <Stat
                label="Pre-dispatched"
                value={formatCount(data.alert_latency.pre_dispatched_count)}
                size={18}
              />
            </div>
            <p
              style={{
                ...SUB_LABEL,
                margin: "16px 0 10px",
                borderTop: "1px solid var(--adm-rule)",
                paddingTop: 14,
              }}
            >
              Send — created → sent
            </p>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 14 }}>
              <Stat
                label="p50"
                value={formatSecondsTenth(data.alert_latency.send_p50_seconds)}
                size={18}
              />
              <Stat
                label="p90"
                value={formatSecondsTenth(data.alert_latency.send_p90_seconds)}
                size={18}
              />
              <Stat label="Sent" value={formatCount(data.alert_latency.sent_count)} size={18} />
            </div>
            <p style={{ margin: "14px 0 0", fontSize: "11px", color: "var(--adm-meta)" }}>
              Postgres-side timestamps; egress after{" "}
              <span style={{ fontFamily: MONO }}>sent</span> is not observable from here.
            </p>
          </Card>
        </div>
      </div>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1fr 1fr",
          gap: 16,
          marginTop: 16,
          alignItems: "start",
        }}
      >
        <Card section="F3" label="Deliveries" title="Status × channel" rise={0.2}>
          {data.deliveries.by_status_channel.length > 0 ? (
            <table style={{ width: "100%", borderCollapse: "collapse", marginTop: 4 }}>
              <thead>
                <tr>
                  <th className="adm-microlabel" style={STATUS_TH}>
                    Status
                  </th>
                  <th className="adm-microlabel" style={STATUS_TH}>
                    Channel
                  </th>
                  <th
                    className="adm-microlabel"
                    style={{ ...STATUS_TH, textAlign: "right", padding: "8px 0" }}
                  >
                    Deliveries
                  </th>
                </tr>
              </thead>
              <tbody>
                {data.deliveries.by_status_channel.map((row) => (
                  <tr key={`${row.status}-${row.channel}`}>
                    <td style={STATUS_TD}>
                      <Badge variant={stateVariant(row.status)}>{row.status}</Badge>
                    </td>
                    <td
                      style={{
                        ...STATUS_TD,
                        fontFamily: MONO,
                        fontSize: "12px",
                        color: "var(--adm-text-secondary)",
                      }}
                    >
                      {row.channel}
                    </td>
                    <td
                      style={{
                        ...STATUS_TD,
                        padding: "9px 0",
                        textAlign: "right",
                        fontFamily: MONO,
                        fontSize: "12.5px",
                        color: "var(--adm-ink)",
                        fontVariantNumeric: "tabular-nums",
                      }}
                    >
                      {formatCount(row.count)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p className="adm-muted" style={{ margin: "12px 0 0", fontSize: "12.5px" }}>
              No deliveries recorded.
            </p>
          )}
        </Card>

        <Card section="F3" label="Deliveries" title="Attempts before success" rise={0.25}>
          <div style={{ marginTop: 14 }}>
            <ColumnChart columns={attempts} scale="sqrt" />
          </div>
          <p style={{ margin: "14px 0 0", fontSize: "11px", color: "var(--adm-meta)" }}>
            Retries back off exponentially; five failed attempts dead-letters the delivery.
          </p>
        </Card>
      </div>

      <Card
        section="F3"
        label="Dead letters"
        title="Recent dead-letters"
        tone={dead.length > 0 ? "danger" : undefined}
        rise={0.3}
        className="mt-[16px]"
      >
        {dead.length > 0 ? (
          <table style={{ width: "100%", borderCollapse: "collapse", marginTop: 4 }}>
            <tbody>
              {dead.map((row) => (
                <tr key={row.id}>
                  <td
                    style={{
                      ...DLQ_TD,
                      fontFamily: MONO,
                      fontSize: "11.5px",
                      color: "var(--adm-ink)",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {row.id}
                  </td>
                  <td
                    style={{
                      ...DLQ_TD,
                      fontFamily: MONO,
                      fontSize: "11.5px",
                      color: "var(--adm-muted)",
                    }}
                  >
                    {row.channel}
                  </td>
                  <td
                    style={{
                      ...DLQ_TD,
                      textAlign: "right",
                      fontFamily: MONO,
                      fontSize: "12px",
                      color: "var(--adm-danger-ink)",
                      fontVariantNumeric: "tabular-nums",
                    }}
                  >
                    {row.attempts}
                  </td>
                  <td style={{ ...DLQ_TD, fontSize: "12px", color: "var(--adm-text-secondary)" }}>
                    {row.last_error ?? "—"}
                  </td>
                  <td
                    style={{
                      ...DLQ_TD,
                      padding: "10px 0",
                      textAlign: "right",
                      fontFamily: MONO,
                      fontSize: "11.5px",
                      color: "var(--adm-muted)",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {formatMonthDayTime(row.updated_at)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <p
            style={{
              margin: "12px 0 0",
              fontSize: "12.5px",
              color: "var(--adm-muted)",
              borderTop: "1px solid var(--adm-rule)",
              paddingTop: 12,
            }}
          >
            No dead-lettered deliveries.
          </p>
        )}
      </Card>
    </Screen>
  );
}
