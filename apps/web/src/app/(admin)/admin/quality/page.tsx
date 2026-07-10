import type { CSSProperties } from "react";
import Link from "next/link";

import type { AdminQuality, AdminRegimePrecision } from "@/lib/api";
import { ApiError, adminQuality } from "@/lib/api";
import { formatCount, formatUtcMinute } from "@/lib/format";
import { Badge, stateVariant } from "@/components/admin/ui/Badge";
import { Card } from "@/components/admin/ui/Card";
import { Progress } from "@/components/admin/ui/Progress";
import { Screen } from "@/components/admin/ui/Screen";
import { Stat } from "@/components/admin/ui/Stat";
import { Table, type TableColumn } from "@/components/admin/ui/Table";
import { BarRows } from "@/components/admin/charts/BarRows";
import { ColumnChart } from "@/components/admin/charts/ColumnChart";
import { Unavailable } from "@/components/admin/Unavailable";
import { SweepButton } from "./SweepButton";

// Section D (data quality & review ops): the review queue's shape, the
// invariant-3 unlinked-instrument backlog, this month's sampling-audit
// precision, and two integrity spot checks. The br CPF-collision sweep is a
// whole-dataset scan — opt-in only (?sweep=br), never run on page load.
export const dynamic = "force-dynamic";

interface Search {
  searchParams: Promise<{ sweep?: string }>;
}

function formatPercent(value: number | null | undefined): string {
  if (value === null || value === undefined) {
    return "—";
  }
  return `${(value * 100).toFixed(1)}%`;
}

function formatHours(value: number | null | undefined): string {
  if (value === null || value === undefined) {
    return "—";
  }
  return `${value.toFixed(1)}h`;
}

/**
 * Masks a CPF for display (dc.html:799 shows "***.481.219-**") — the raw
 * value never renders. Anything that isn't the standard 11 digits masks
 * entirely rather than leaking an unexpected shape.
 */
function maskCpf(cpf: string): string {
  const digits = cpf.replace(/\D/g, "");
  if (digits.length !== 11) {
    return "***";
  }
  return `***.${digits.slice(3, 6)}.${digits.slice(6, 9)}-**`;
}

// dc.html:676 — invariant caption under the target-kind bars (reused for the
// kept D4 idempotency note, restyled to the same design language).
const CAPTION_STYLE: CSSProperties = {
  margin: "16px 0 0",
  fontSize: 11,
  color: "var(--adm-meta)",
  borderTop: "1px solid var(--adm-rule)",
  paddingTop: 12,
};

// dc.html:762-763 — numeric precision cells (mono/tabular come from the
// Table's `numeric` td class; size + ink here).
const NUM_CELL_STYLE: CSSProperties = {
  fontSize: "12.5px",
  color: "var(--adm-text-secondary)",
};

const PRECISION_COLUMNS: ReadonlyArray<TableColumn<AdminRegimePrecision>> = [
  {
    key: "regime_id",
    header: "Regime",
    render: (row) => (
      <span style={{ fontFamily: "var(--adm-font-data)", fontSize: 12, color: "var(--adm-ink)" }}>
        {row.regime_id}
      </span>
    ),
  },
  {
    key: "body",
    header: "Body",
    render: (row) => <span style={{ fontSize: 12, color: "var(--adm-muted)" }}>{row.body}</span>,
  },
  {
    key: "sampled",
    header: "Sampled",
    numeric: true,
    render: (row) => <span style={NUM_CELL_STYLE}>{formatCount(row.sampled)}</span>,
  },
  {
    key: "audited",
    header: "Audited",
    numeric: true,
    render: (row) => <span style={NUM_CELL_STYLE}>{formatCount(row.audited)}</span>,
  },
  {
    key: "discrepancies",
    header: "Discrepancies",
    numeric: true,
    // Amber when any discrepancy exists (approved deviation #12 — the design
    // mock ambered only >2).
    render: (row) => (
      <span
        style={{
          fontSize: "12.5px",
          color: row.discrepancies > 0 ? "var(--adm-warning-ink)" : "var(--adm-text-secondary)",
        }}
      >
        {formatCount(row.discrepancies)}
      </span>
    ),
  },
  {
    key: "precision_estimate",
    header: "Precision",
    numeric: true,
    render: (row) => (
      <span style={{ fontSize: "12.5px", fontWeight: 600, color: "var(--adm-accent-deep)" }}>
        {formatPercent(row.precision_estimate)}
      </span>
    ),
  },
];

function BrCollisionSweep({
  sweep,
  requested,
}: {
  sweep: AdminQuality["collision_sweep"];
  requested: boolean;
}) {
  if (!requested || sweep === null || sweep === undefined) {
    // Idle copy + pending swap live inside SweepButton — the pending state is
    // the real router-transition state, so it has to be client-side.
    return <SweepButton />;
  }

  if (sweep.pass) {
    return (
      <div style={{ display: "flex", alignItems: "center", gap: 10, marginTop: 12 }}>
        <Badge variant="success">pass</Badge>
        <span style={{ fontSize: "12.5px", color: "var(--adm-muted)" }}>
          No CPF collisions found across the br dataset.
        </span>
        <Link href="/admin/quality" style={{ fontSize: "11.5px" }}>
          hide results
        </Link>
      </div>
    );
  }

  return (
    <>
      <div style={{ display: "flex", alignItems: "center", gap: 10, margin: "12px 0 12px" }}>
        <Badge variant="danger">{sweep.collisions.length} collision(s)</Badge>
        <Link href="/admin/quality" style={{ fontSize: "11.5px" }}>
          hide results
        </Link>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        {sweep.collisions.map((row) => (
          <div
            key={row.politician_id}
            style={{
              display: "flex",
              flexWrap: "wrap",
              gap: "8px 26px",
              borderTop: "1px solid var(--adm-rule)",
              paddingTop: 12,
            }}
          >
            <span
              style={{ fontFamily: "var(--adm-font-data)", fontSize: 12, color: "var(--adm-ink)" }}
            >
              {row.politician_id}
            </span>
            <span style={{ fontSize: 12, color: "var(--adm-text-secondary)" }}>
              {row.canonical_name}
            </span>
            <span
              style={{
                fontFamily: "var(--adm-font-data)",
                fontSize: 12,
                color: "var(--adm-warning-ink)",
              }}
            >
              {row.distinct_cpfs} distinct CPFs
            </span>
            <span
              style={{ fontFamily: "var(--adm-font-data)", fontSize: 12, color: "var(--adm-meta)" }}
            >
              {row.cpfs.map(maskCpf).join(" · ")}
            </span>
          </div>
        ))}
      </div>
    </>
  );
}

export default async function QualityPage({ searchParams }: Search) {
  const { sweep } = await searchParams;
  const requestedSweep = sweep === "br";

  let data: AdminQuality;
  try {
    data = await adminQuality(requestedSweep ? { sweep: "br" } : {});
  } catch (error) {
    if (error instanceof ApiError && (error.status === 401 || error.status === 403 || error.status === 503)) {
      return <Unavailable reason={error.message} />;
    }
    throw error;
  }

  const ageBuckets = [
    { bucket: "<1d", count: data.open_age_buckets.under_1d },
    { bucket: "1–7d", count: data.open_age_buckets.d1_to_7 },
    { bucket: "7–30d", count: data.open_age_buckets.d7_to_30 },
    { bucket: ">30d", count: data.open_age_buckets.over_30d },
  ];

  const { unlinked_instruments: unlinked, raw_retention: retention } = data;
  const listedShare = unlinked.total > 0 ? unlinked.listed / unlinked.total : 0;
  const linkedShare =
    retention.raw_documents > 0 ? retention.linked_to_filing / retention.raw_documents : 0;

  // dc.html:1680-1687 — Total reads amber (the backlog headline), the
  // breakdown counts in heading ink (#F0EBDF).
  const unlinkedStats = [
    { label: "Total", value: unlinked.total, tone: "var(--adm-accent-amber)" },
    { label: "Listed", value: unlinked.listed, tone: "var(--adm-heading)" },
    { label: "Equity", value: unlinked.equity, tone: "var(--adm-heading)" },
    { label: "Bond", value: unlinked.bond, tone: "var(--adm-heading)" },
    { label: "Fund", value: unlinked.fund, tone: "var(--adm-heading)" },
    { label: "Option", value: unlinked.option, tone: "var(--adm-heading)" },
  ];

  return (
    <Screen
      label="Quality"
      kicker="Section D"
      title="Quality"
      subtitle="Review-queue health, the never-guess backlog, this month’s sampling precision, and two integrity spot checks."
      meta={`as of ${formatUtcMinute(data.generated_at)}`}
    >
      <div
        style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, alignItems: "start" }}
      >
        <Card section="D1" label="Review queue" title="Open by reason" rise={0.05}>
          <div style={{ marginTop: 14 }}>
            <BarRows
              rows={data.open_by_reason.map((r) => ({ label: r.reason, value: r.tasks }))}
              labelWidth={186}
              labelAlign="right"
              barHeight={12}
              fill="var(--adm-series-funnel-review)"
              valueWidth={28}
            />
          </div>
        </Card>

        <Card section="D1" label="Review queue" title="Open by target kind" rise={0.1}>
          <div style={{ marginTop: 14 }}>
            <BarRows
              rows={data.open_by_target_kind.map((r) => ({ label: r.target_kind, value: r.tasks }))}
              labelWidth={186}
              labelAlign="right"
              barHeight={12}
              fill="var(--adm-series-funnel-review)"
              valueWidth={28}
            />
          </div>
          <p style={CAPTION_STYLE}>
            Invariant 3: below-threshold instrument matches stay NULL and open a task — the queue
            is the system refusing to guess.
          </p>
        </Card>

        <Card section="D1" label="Review queue" title="Open-queue age" rise={0.15}>
          <div style={{ marginTop: 14 }}>
            <ColumnChart columns={ageBuckets} scale="linear" />
          </div>
        </Card>

        <Card section="D1" label="Review queue" title="30-day resolution" rise={0.2}>
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "1fr 1fr",
              gap: 14,
              marginTop: 14,
              marginBottom: 14,
            }}
          >
            <Stat
              label="Resolved tasks"
              value={formatCount(data.resolution_30d.resolved_tasks)}
              size={22}
            />
            <Stat
              label="Median to resolve"
              value={formatHours(data.resolution_30d.median_hours_to_resolve)}
              size={22}
            />
          </div>
          {data.resolution_30d.verdicts.length === 0 ? (
            <p className="adm-muted" style={{ fontSize: "12.5px" }}>
              No applied verdicts in the last 30 days.
            </p>
          ) : (
            data.resolution_30d.verdicts.map((verdict) => (
              <div
                key={verdict.verdict}
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  gap: 12,
                  borderTop: "1px solid var(--adm-rule)",
                  padding: "8px 0",
                }}
              >
                <Badge variant={stateVariant(verdict.verdict)}>{verdict.verdict}</Badge>
                <span
                  style={{
                    fontFamily: "var(--adm-font-data)",
                    fontSize: "12.5px",
                    color: "var(--adm-text-secondary)",
                    fontVariantNumeric: "tabular-nums",
                  }}
                >
                  {formatCount(verdict.attempts)}
                </span>
              </div>
            ))
          )}
        </Card>
      </div>

      <Card
        className="mt-[16px]"
        section="D2"
        label="Never guess"
        title="NULL-instrument backlog"
        rise={0.25}
      >
        <div
          style={{ display: "grid", gridTemplateColumns: "repeat(6,1fr)", gap: 14, marginTop: 16 }}
        >
          {unlinkedStats.map((stat) => (
            <Stat
              key={stat.label}
              label={stat.label}
              value={formatCount(stat.value)}
              size={22}
              tone={stat.tone}
            />
          ))}
        </div>
        <div style={{ marginTop: 18, maxWidth: 420 }}>
          <Progress
            value={listedShare}
            color="var(--adm-series-funnel-gold)"
            label="listed share of NULL-instrument rows"
          />
        </div>
      </Card>

      <Card
        className="mt-[16px]"
        section="D6"
        label="Sampling audit"
        title="Precision, current month"
        meta={`sample month ${data.precision_current_month.sample_month}`}
        rise={0.3}
      >
        <div style={{ marginTop: 12 }}>
          <Table
            columns={PRECISION_COLUMNS}
            rows={data.precision_current_month.regimes}
            getRowKey={(row) => row.regime_id}
            emptyMessage="No sample batch drawn this month."
          />
        </div>
      </Card>

      <Card
        className="mt-[16px]"
        section="D3"
        label="Identity integrity"
        title="br CPF collision sweep"
        rise={0.35}
      >
        <BrCollisionSweep sweep={data.collision_sweep} requested={requestedSweep} />
      </Card>

      {/* D4/D5 have no dc.html card — kept from the v1 page, restyled to the
          design language (approved deviation #6). */}
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1fr 1fr",
          gap: 16,
          alignItems: "start",
          marginTop: 16,
        }}
      >
        <Card section="D4" label="Pipeline integrity" title="Idempotency" rise={0.4}>
          <div
            style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 14, marginTop: 16 }}
          >
            <Stat label="Backfill runs" value={formatCount(data.idempotency.runs)} size={22} />
            <Stat
              label="Replayed"
              value={formatCount(data.idempotency.replayed_total)}
              size={22}
            />
          </div>
          <p style={CAPTION_STYLE}>{data.idempotency.note}</p>
        </Card>

        <Card section="D5" label="Pipeline integrity" title="Raw retention" rise={0.45}>
          <div
            style={{ display: "grid", gridTemplateColumns: "repeat(3,1fr)", gap: 14, marginTop: 16 }}
          >
            <Stat label="Raw documents" value={formatCount(retention.raw_documents)} size={22} />
            <Stat
              label="Linked to filing"
              value={formatCount(retention.linked_to_filing)}
              size={22}
            />
            <Stat label="Orphaned" value={formatCount(retention.orphaned)} size={22} />
          </div>
          <div style={{ marginTop: 18, maxWidth: 420 }}>
            <Progress
              value={linkedShare}
              color="var(--adm-series-funnel-gold)"
              label="linked share of raw documents"
            />
          </div>
        </Card>
      </div>
    </Screen>
  );
}
