import type {
  AdminDriftKindRow,
  AdminFailedRun,
  AdminFreezeBoardRow,
  AdminFunnelRow,
  AdminPipeline,
} from "@/lib/api";
import { ApiError, adminPipeline } from "@/lib/api";
import { formatAge, formatCount, formatDateTime, formatUtcMinute } from "@/lib/format";
import { AutoRefresh } from "@/components/admin/AutoRefresh";
import { Unavailable } from "@/components/admin/Unavailable";
import { FunnelRows, type FunnelRow } from "@/components/admin/charts/FunnelRows";
import { TrendChart } from "@/components/admin/charts/TrendChart";
import { Badge } from "@/components/admin/ui/Badge";
import { Card } from "@/components/admin/ui/Card";
import { CodeChip } from "@/components/admin/ui/CodeChip";
import { Screen } from "@/components/admin/ui/Screen";
import { Table, type TableColumn } from "@/components/admin/ui/Table";

export const dynamic = "force-dynamic";

const MONO = "var(--adm-font-data)";

// dc.html:360 — primary mono cell (regime codes).
const inkMonoCell: React.CSSProperties = {
  fontFamily: MONO,
  fontSize: "12.5px",
  color: "var(--adm-ink)",
};

// dc.html:362-364 — secondary mono cell (dates, kinds, ages).
const mutedMonoCell: React.CSSProperties = {
  fontFamily: MONO,
  fontSize: "11.5px",
  color: "var(--adm-muted)",
};

// dc.html:427/489 — plain card caption.
const captionStyle: React.CSSProperties = {
  margin: "12px 0 0",
  fontSize: 11,
  color: "var(--adm-meta)",
};

// dc.html:480 — caption with the hairline rule above it.
const ruledCaptionStyle: React.CSSProperties = {
  ...captionStyle,
  borderTop: "1px solid var(--adm-rule)",
  paddingTop: 12,
};

const FUNNEL_LEGEND = [
  { label: "gold inserted", color: "var(--adm-series-funnel-gold)" },
  { label: "review tasks", color: "var(--adm-series-funnel-review)" },
  { label: "suppressed", color: "var(--adm-series-funnel-suppressed)" },
] as const;

// dc.html:1531 — "2026-07-09 09:41" (minute precision, no zone suffix).
function frozenSinceLabel(iso: string): string {
  return formatUtcMinute(iso).slice(0, 16);
}

// dc.html:1574 — "09:41 UTC".
function hhmmUtc(iso: string): string {
  return formatUtcMinute(iso).slice(11);
}

// dc.html:478 — "Aug 2025" x-range labels from a "YYYY-MM" month key.
function monthLabel(month: string): string {
  return new Date(`${month}-01T00:00:00Z`).toLocaleString("en-US", {
    month: "short",
    year: "numeric",
    timeZone: "UTC",
  });
}

function renderConformanceNote(note: string) {
  const parts = note.split("`");
  return parts.map((part, i) =>
    i % 2 === 1 ? <CodeChip key={i}>{part}</CodeChip> : <span key={i}>{part}</span>,
  );
}

function freezeColumns(now: Date): TableColumn<AdminFreezeBoardRow>[] {
  return [
    {
      key: "regime",
      header: "Regime",
      render: (row) => <span style={inkMonoCell}>{row.regime_code}</span>,
    },
    {
      key: "state",
      header: "State",
      render: (row) =>
        row.frozen ? (
          <Badge variant="danger">frozen</Badge>
        ) : (
          <Badge variant="success">watching</Badge>
        ),
    },
    {
      key: "since",
      header: "Frozen since",
      nowrap: true,
      render: (row) => (
        <span style={mutedMonoCell}>
          {row.frozen_at != null ? frozenSinceLabel(row.frozen_at) : "—"}
        </span>
      ),
    },
    {
      key: "kind",
      header: "Kind",
      render: (row) => <span style={mutedMonoCell}>{row.frozen_kind ?? "—"}</span>,
    },
    {
      key: "checked",
      header: "Last checked",
      nowrap: true,
      render: (row) => (
        <span style={mutedMonoCell} title={formatDateTime(row.last_checked_at)}>
          {formatAge(row.last_checked_at, now)} ago
        </span>
      ),
    },
    {
      key: "drift",
      header: "Open drift",
      numeric: true,
      nowrap: true,
      render: (row) => (
        <span
          style={{
            fontSize: "12.5px",
            color:
              row.open_drift_count > 0 ? "var(--adm-warning-ink)" : "var(--adm-text-secondary)",
          }}
        >
          {formatCount(row.open_drift_count)}
        </span>
      ),
    },
    {
      key: "worst",
      header: "Worst open kind",
      nowrap: true,
      render: (row) => <span style={mutedMonoCell}>{row.worst_open_drift_kind ?? "—"}</span>,
    },
  ];
}

const otherStageColumns: TableColumn<AdminFunnelRow>[] = [
  {
    key: "adapter",
    header: "Adapter",
    render: (row) => <span style={inkMonoCell}>{row.adapter}</span>,
  },
  {
    key: "stage",
    header: "Stage",
    render: (row) => <span style={mutedMonoCell}>{row.stage}</span>,
  },
  {
    key: "runs",
    header: "Runs",
    numeric: true,
    render: (row) => (
      <span style={{ fontSize: "12.5px", color: "var(--adm-text-secondary)" }}>
        {formatCount(row.runs)}
      </span>
    ),
  },
  {
    key: "succeeded",
    header: "Succeeded",
    numeric: true,
    render: (row) => (
      <span style={{ fontSize: "12.5px", color: "var(--adm-text-secondary)" }}>
        {formatCount(row.succeeded)}
      </span>
    ),
  },
  {
    key: "failed",
    header: "Failed",
    numeric: true,
    render: (row) => (
      <span
        style={{
          fontSize: "12.5px",
          color: row.failed > 0 ? "var(--adm-danger-ink)" : "var(--adm-text-secondary)",
        }}
      >
        {formatCount(row.failed)}
      </span>
    ),
  },
  {
    key: "running",
    header: "Running",
    numeric: true,
    render: (row) => (
      <span style={{ fontSize: "12.5px", color: "var(--adm-text-secondary)" }}>
        {formatCount(row.running)}
      </span>
    ),
  },
];

const driftColumns: TableColumn<AdminDriftKindRow>[] = [
  {
    key: "kind",
    header: "Kind",
    render: (row) => (
      <span
        style={{ fontFamily: MONO, fontSize: 12, color: "var(--adm-ink)" }}
        title={`${formatCount(row.superseded_count)} superseded`}
      >
        {row.drift_kind}
      </span>
    ),
  },
  {
    key: "open",
    header: "Open",
    numeric: true,
    render: (row) => (
      <span
        style={{
          fontSize: "12.5px",
          color: row.open_count > 0 ? "var(--adm-warning-ink)" : "var(--adm-text-secondary)",
        }}
      >
        {formatCount(row.open_count)}
      </span>
    ),
  },
  {
    key: "resolved",
    header: "Resolved",
    numeric: true,
    render: (row) => (
      <span style={{ fontSize: "12.5px", color: "var(--adm-text-secondary)" }}>
        {formatCount(row.resolved_count)}
      </span>
    ),
  },
  {
    key: "detections",
    header: "Detections",
    numeric: true,
    render: (row) => (
      <span style={{ fontSize: "12.5px", color: "var(--adm-muted)" }}>
        {formatCount(row.detections)}
      </span>
    ),
  },
];

// dc.html:436-447 — headerless failure rows.
function FailureRows({ failures }: { failures: readonly AdminFailedRun[] }) {
  return (
    <div className="overflow-x-auto">
      <table style={{ width: "100%", borderCollapse: "collapse" }}>
        <tbody>
          {failures.map((run) => {
            const ts = run.finished_at ?? run.started_at;
            return (
              <tr key={run.id}>
                <td
                  style={{
                    padding: "10px 14px 10px 0",
                    borderBottom: "1px solid var(--adm-rule)",
                    fontFamily: MONO,
                    fontSize: 12,
                    color: "var(--adm-ink)",
                    whiteSpace: "nowrap",
                    verticalAlign: "top",
                  }}
                >
                  {run.adapter}
                </td>
                <td
                  style={{
                    padding: "10px 14px 10px 0",
                    borderBottom: "1px solid var(--adm-rule)",
                    verticalAlign: "top",
                    whiteSpace: "nowrap",
                  }}
                >
                  <Badge variant="danger">{run.stage}</Badge>
                </td>
                <td
                  style={{
                    padding: "10px 14px 10px 0",
                    borderBottom: "1px solid var(--adm-rule)",
                    fontSize: 12,
                    color: "var(--adm-text-secondary)",
                  }}
                >
                  {run.error ?? "—"}
                </td>
                <td
                  style={{
                    padding: "10px 0",
                    borderBottom: "1px solid var(--adm-rule)",
                    textAlign: "right",
                    fontFamily: MONO,
                    fontSize: "11.5px",
                    color: "var(--adm-muted)",
                    whiteSpace: "nowrap",
                    verticalAlign: "top",
                  }}
                  title={
                    run.finished_at != null
                      ? formatDateTime(ts)
                      : `started ${formatDateTime(run.started_at)}, unfinished`
                  }
                >
                  {hhmmUtc(ts)}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

export default async function PipelinePage() {
  let pipeline: AdminPipeline;
  try {
    pipeline = await adminPipeline();
  } catch (error) {
    if (error instanceof ApiError && (error.status === 401 || error.status === 403 || error.status === 503)) {
      return <Unavailable reason={error.message} />;
    }
    throw error;
  }

  const now = new Date();
  const frozenCodes = pipeline.freeze_board
    .filter((row) => row.frozen)
    .map((row) => row.regime_code);
  const frozenSet = new Set(frozenCodes);

  const publishRows = pipeline.funnel.filter((row) => row.stage === "publish");
  const funnelRows: FunnelRow[] = publishRows.map((row) => {
    const frozen = frozenSet.has(row.adapter);
    const seg = (n: number | null | undefined) => (frozen ? 0 : (n ?? 0));
    return {
      adapter: row.adapter,
      segments: [
        { value: seg(row.gold_inserted), color: "var(--adm-series-funnel-gold)" },
        { value: seg(row.review_tasks), color: "var(--adm-series-funnel-review)" },
        { value: seg(row.suppressed), color: "var(--adm-series-funnel-suppressed)" },
      ],
      totalLabel: frozen ? "frozen" : formatCount(row.candidates ?? 0),
      totalTone: frozen ? ("danger" as const) : undefined,
    };
  });
  const funnelMax = Math.max(
    0,
    ...publishRows.map((row) => (frozenSet.has(row.adapter) ? 0 : (row.candidates ?? 0))),
  );
  const funnelNote =
    frozenCodes.length > 0
      ? `${frozenCodes.join(", ")} publish suspended while frozen.`
      : `All ${publishRows.length} adapters publishing.`;

  const otherStageRuns = pipeline.funnel.filter((row) => row.stage !== "publish");

  const supersedeMonths = [...pipeline.supersede_activity].reverse();
  const supersedeTrend = supersedeMonths.map((row) => ({
    label: row.month,
    value: row.superseding_records,
  }));
  const firstSupersedeMonth = supersedeMonths[0];
  const lastSupersedeMonth = supersedeMonths[supersedeMonths.length - 1];
  const totalSupersedingRecords = pipeline.supersede_activity.reduce(
    (sum, row) => sum + row.superseding_records,
    0,
  );
  const totalAmendedFilings = pipeline.supersede_activity.reduce(
    (sum, row) => sum + row.amended_filings,
    0,
  );

  const hasFailures = pipeline.recent_failures.length > 0;

  return (
    <Screen
      label="Pipeline"
      kicker="Section C"
      title="Pipeline"
      subtitle="Adapter freeze state, drift, stage funnel, and supersede activity."
      meta={<>generated {formatUtcMinute(pipeline.generated_at)}</>}
    >
      <AutoRefresh seconds={30} />

      <Card
        section="C1"
        label="Sentinel"
        title="Freeze & drift board"
        rise={0.05}
        meta={
          <>
            frozen{" "}
            <span
              style={{
                color: frozenCodes.length > 0 ? "var(--adm-danger-ink)" : "var(--adm-text-secondary)",
                fontWeight: 600,
              }}
            >
              {frozenCodes.length}
            </span>{" "}
            / {pipeline.freeze_board.length} regimes
          </>
        }
      >
        <Table
          columns={freezeColumns(now)}
          rows={pipeline.freeze_board}
          getRowKey={(row) => row.regime_code}
          emptyMessage="No regimes tracked yet."
        />
      </Card>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1.15fr .85fr",
          gap: 16,
          marginTop: 16,
          alignItems: "start",
        }}
      >
        <Card section="C2" label="Trailing 7d" title="Publish-stage funnel" rise={0.12}>
          {funnelRows.length > 0 ? (
            <>
              <div style={{ display: "flex", gap: 14, marginBottom: 14 }}>
                {FUNNEL_LEGEND.map((item) => (
                  <span
                    key={item.label}
                    style={{ display: "inline-flex", alignItems: "center", gap: 6 }}
                  >
                    <span
                      style={{
                        width: 9,
                        height: 9,
                        borderRadius: 1,
                        background: item.color,
                        display: "inline-block",
                      }}
                    />
                    <span style={{ fontSize: "10.5px", color: "var(--adm-meta)" }}>
                      {item.label}
                    </span>
                  </span>
                ))}
              </div>
              <FunnelRows rows={funnelRows} max={funnelMax} />
              <p style={{ ...ruledCaptionStyle, margin: "14px 0 0" }}>
                Candidates per adapter; colored share = gold inserted, review-routed, suppressed.{" "}
                {funnelNote}
              </p>
            </>
          ) : (
            <p className="adm-muted" style={{ fontSize: "12.5px" }}>
              No publish-stage runs recorded yet.
            </p>
          )}
          <p className="adm-microlabel" style={{ margin: "14px 0 6px" }}>
            Other stages
          </p>
          <Table
            columns={otherStageColumns}
            rows={otherStageRuns}
            getRowKey={(row) => `${row.adapter}-${row.stage}`}
            emptyMessage="No non-publish runs recorded yet."
          />
        </Card>

        <Card section="C3" label="Drift" title="Drift by kind" rise={0.19}>
          <Table
            columns={driftColumns}
            rows={pipeline.drift_by_kind}
            getRowKey={(row) => row.drift_kind}
            emptyMessage="No drift reports recorded yet."
          />
          <p style={captionStyle}>
            Fail closed: zero-row parses and schema drift freeze the adapter and open a review
            task.
          </p>
        </Card>
      </div>

      <Card
        section="C4"
        label="Failures"
        title="Recent failures"
        rise={0.26}
        tone={hasFailures ? "danger" : undefined}
        className="mt-[16px]"
      >
        {hasFailures ? (
          <FailureRows failures={pipeline.recent_failures} />
        ) : (
          <p
            style={{
              margin: 0,
              color: "var(--adm-muted)",
              fontSize: "12.5px",
              borderTop: "1px solid var(--adm-rule)",
              paddingTop: 12,
            }}
          >
            No failed runs recorded in the trailing 24 hours.
          </p>
        )}
      </Card>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1.6fr .9fr",
          gap: 16,
          marginTop: 16,
          alignItems: "start",
        }}
      >
        <Card section="C6" label="Corrections" title="Supersede activity" rise={0.33}>
          <div style={{ display: "flex", gap: 34, marginBottom: 14 }}>
            <div>
              <p className="adm-microlabel" style={{ margin: "0 0 5px" }}>
                Superseding records
              </p>
              <p
                style={{
                  margin: 0,
                  fontFamily: MONO,
                  fontSize: 22,
                  fontWeight: 600,
                  lineHeight: 1,
                  color: "var(--adm-heading)",
                  fontVariantNumeric: "tabular-nums",
                }}
              >
                {formatCount(totalSupersedingRecords)}
              </p>
            </div>
            <div>
              <p className="adm-microlabel" style={{ margin: "0 0 5px" }}>
                Amended filings
              </p>
              <p
                style={{
                  margin: 0,
                  fontFamily: MONO,
                  fontSize: 22,
                  fontWeight: 600,
                  lineHeight: 1,
                  color: "var(--adm-heading)",
                  fontVariantNumeric: "tabular-nums",
                }}
              >
                {formatCount(totalAmendedFilings)}
              </p>
            </div>
          </div>
          {firstSupersedeMonth !== undefined && lastSupersedeMonth !== undefined ? (
            <TrendChart
              points={supersedeTrend}
              size="wide"
              endpointDot
              xLeftLabel={monthLabel(firstSupersedeMonth.month)}
              xRightLabel={monthLabel(lastSupersedeMonth.month)}
              ariaLabel="Superseding records per month"
            />
          ) : (
            <p className="adm-muted" style={{ fontSize: "12.5px" }}>
              No supersede activity recorded yet.
            </p>
          )}
          <p style={ruledCaptionStyle}>
            Gold rows inserted as supersessions per month — invariant 1: corrections supersede
            facts, they never overwrite them.
          </p>
        </Card>

        <Card section="C5" label="Conformance" title="Adapter conformance" rise={0.4}>
          <p style={{ margin: "0 0 10px", fontSize: "12.5px", color: "var(--adm-text-secondary)" }}>
            {renderConformanceNote(pipeline.conformance_note)}
          </p>
          <CodeChip block>cargo run -p pipeline --bin conformance -- us_house</CodeChip>
          <p style={captionStyle}>
            Fixtures are snapshot-committed; every (regime, record type) validates against its
            schema at promotion.
          </p>
        </Card>
      </div>
    </Screen>
  );
}
