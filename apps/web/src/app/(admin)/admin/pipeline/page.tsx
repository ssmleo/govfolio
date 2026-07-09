import type {
  AdminDriftKindRow,
  AdminFailedRun,
  AdminFreezeBoardRow,
  AdminFunnelRow,
  AdminPipeline,
} from "@/lib/api";
import { ApiError, adminPipeline } from "@/lib/api";
import { formatAge, formatDateTime } from "@/lib/format";
import { AutoRefresh } from "@/components/admin/AutoRefresh";
import { Unavailable } from "@/components/admin/Unavailable";
import { CountsBar } from "@/components/admin/charts/CountsBar";
import { TrendArea } from "@/components/admin/charts/TrendArea";
import { Badge } from "@/components/admin/ui/Badge";
import { Card } from "@/components/admin/ui/Card";
import { Table, type TableColumn } from "@/components/admin/ui/Table";

export const dynamic = "force-dynamic";

const DASH = <span className="adm-muted">—</span>;

const FUNNEL_SERIES = [
  { key: "candidates", label: "candidates", color: "var(--adm-muted)" },
  { key: "gold_inserted", label: "gold inserted", color: "var(--adm-accent)" },
  { key: "outbox_written", label: "outbox written", color: "var(--adm-info-ink)" },
  { key: "review_tasks", label: "review tasks", color: "var(--adm-warning-ink)" },
  { key: "suppressed", label: "suppressed", color: "var(--adm-danger-ink)" },
] as const;

interface PublishFunnelDatum extends Record<string, unknown> {
  adapter: string;
  candidates: number;
  gold_inserted: number;
  outbox_written: number;
  review_tasks: number;
  suppressed: number;
}

function renderConformanceNote(note: string) {
  const parts = note.split("`");
  return parts.map((part, i) =>
    i % 2 === 1 ? (
      <code
        key={i}
        className="adm-num rounded-[2px] bg-[var(--adm-surface-sunken)] px-1 py-0.5 text-[0.8125rem]"
      >
        {part}
      </code>
    ) : (
      <span key={i}>{part}</span>
    ),
  );
}

function freezeColumns(now: Date): TableColumn<AdminFreezeBoardRow>[] {
  return [
    {
      key: "regime",
      header: "regime",
      render: (row) => <span className="adm-num">{row.regime_code}</span>,
    },
    {
      key: "state",
      header: "state",
      render: (row) =>
        row.frozen ? <Badge variant="danger">frozen</Badge> : <Badge variant="success">watching</Badge>,
    },
    {
      key: "since",
      header: "frozen since",
      render: (row) => (row.frozen_at != null ? formatDateTime(row.frozen_at) : DASH),
    },
    {
      key: "kind",
      header: "frozen kind",
      render: (row) => row.frozen_kind ?? DASH,
    },
    {
      key: "checked",
      header: "last checked",
      render: (row) => (
        <span title={formatDateTime(row.last_checked_at)}>{formatAge(row.last_checked_at, now)} ago</span>
      ),
    },
    {
      key: "drift",
      header: "open drift",
      numeric: true,
      render: (row) => (
        <span className={row.open_drift_count > 0 ? "text-[var(--adm-warning-ink)]" : undefined}>
          {row.open_drift_count}
        </span>
      ),
    },
    {
      key: "worst",
      header: "worst open kind",
      render: (row) => row.worst_open_drift_kind ?? DASH,
    },
  ];
}

const otherStageColumns: TableColumn<AdminFunnelRow>[] = [
  {
    key: "adapter",
    header: "adapter",
    render: (row) => <span className="adm-num">{row.adapter}</span>,
  },
  { key: "stage", header: "stage", render: (row) => row.stage },
  { key: "runs", header: "runs", numeric: true, render: (row) => row.runs },
  { key: "succeeded", header: "succeeded", numeric: true, render: (row) => row.succeeded },
  {
    key: "failed",
    header: "failed",
    numeric: true,
    render: (row) => (
      <span className={row.failed > 0 ? "text-[var(--adm-danger-ink)]" : undefined}>{row.failed}</span>
    ),
  },
  { key: "running", header: "running", numeric: true, render: (row) => row.running },
];

const driftColumns: TableColumn<AdminDriftKindRow>[] = [
  {
    key: "kind",
    header: "kind",
    render: (row) => <span className="adm-num">{row.drift_kind}</span>,
  },
  {
    key: "open",
    header: "open",
    numeric: true,
    render: (row) => (
      <span className={row.open_count > 0 ? "text-[var(--adm-warning-ink)]" : undefined}>
        {row.open_count}
      </span>
    ),
  },
  { key: "resolved", header: "resolved", numeric: true, render: (row) => row.resolved_count },
  { key: "superseded", header: "superseded", numeric: true, render: (row) => row.superseded_count },
  { key: "detections", header: "detections", numeric: true, render: (row) => row.detections },
];

const failureColumns: TableColumn<AdminFailedRun>[] = [
  {
    key: "adapter",
    header: "adapter",
    render: (row) => <span className="adm-num">{row.adapter}</span>,
  },
  { key: "stage", header: "stage", render: (row) => row.stage },
  {
    key: "error",
    header: "error",
    render: (row) =>
      row.error != null ? (
        <span className="block max-w-[40rem] truncate" title={row.error}>
          {row.error}
        </span>
      ) : (
        DASH
      ),
  },
  {
    key: "when",
    header: "finished",
    render: (row) =>
      row.finished_at != null ? (
        formatDateTime(row.finished_at)
      ) : (
        <span title={formatDateTime(row.started_at)}>started, unfinished</span>
      ),
  },
];

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
  const frozenCount = pipeline.freeze_board.filter((row) => row.frozen).length;

  const publishFunnel: PublishFunnelDatum[] = pipeline.funnel
    .filter((row) => row.stage === "publish")
    .map((row) => ({
      adapter: row.adapter,
      candidates: row.candidates ?? 0,
      gold_inserted: row.gold_inserted ?? 0,
      outbox_written: row.outbox_written ?? 0,
      review_tasks: row.review_tasks ?? 0,
      suppressed: row.suppressed ?? 0,
    }));

  const otherStageRuns = pipeline.funnel.filter((row) => row.stage !== "publish");

  const supersedeTrend = [...pipeline.supersede_activity]
    .reverse()
    .map((row) => ({ x: row.month, y: row.superseding_records }));
  const totalSupersedingRecords = pipeline.supersede_activity.reduce(
    (sum, row) => sum + row.superseding_records,
    0,
  );
  const totalAmendedFilings = pipeline.supersede_activity.reduce(
    (sum, row) => sum + row.amended_filings,
    0,
  );

  return (
    <div className="flex flex-col gap-4 px-4 pt-4">
      <AutoRefresh seconds={30} />

      <section className="flex flex-wrap items-baseline justify-between gap-3">
        <div>
          <h1>Pipeline</h1>
          <p className="adm-muted mt-1 text-sm">
            Adapter freeze state, drift, stage funnel, and supersede activity.
          </p>
        </div>
        <p className="adm-num adm-muted text-xs">generated {formatDateTime(pipeline.generated_at)}</p>
      </section>

      <Card
        eyebrow="C1"
        title="Freeze & drift board"
        action={
          <div className="flex items-baseline gap-2 text-xs">
            <span className="adm-eyebrow">frozen</span>
            <span
              className={`adm-num text-sm font-semibold ${
                frozenCount > 0 ? "text-[var(--adm-danger-ink)]" : "text-[var(--adm-ink)]"
              }`}
            >
              {frozenCount}
            </span>
            <span className="adm-muted">/ {pipeline.freeze_board.length} regimes</span>
          </div>
        }
      >
        <Table
          columns={freezeColumns(now)}
          rows={pipeline.freeze_board}
          getRowKey={(row) => row.regime_code}
          emptyMessage="No regimes tracked yet."
        />
      </Card>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Card eyebrow="C2" title="Publish-stage funnel">
          {publishFunnel.length > 0 ? (
            <CountsBar data={publishFunnel} categoryKey="adapter" series={FUNNEL_SERIES} height={220} />
          ) : (
            <p className="text-sm text-[var(--adm-muted)]">No publish-stage runs recorded yet.</p>
          )}
          <p className="adm-eyebrow mt-4 mb-1.5">Other stages</p>
          <Table
            columns={otherStageColumns}
            rows={otherStageRuns}
            getRowKey={(row) => `${row.adapter}-${row.stage}`}
            emptyMessage="No non-publish runs recorded yet."
          />
        </Card>

        <Card eyebrow="C3" title="Drift by kind">
          <Table
            columns={driftColumns}
            rows={pipeline.drift_by_kind}
            getRowKey={(row) => row.drift_kind}
            emptyMessage="No drift reports recorded yet."
          />
        </Card>
      </div>

      <Card eyebrow="C4" title="Recent failures">
        <Table
          columns={failureColumns}
          rows={pipeline.recent_failures}
          getRowKey={(row) => row.id}
          emptyMessage="No failed runs recorded."
        />
      </Card>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-[2fr_1fr]">
        <Card eyebrow="C6" title="Supersede activity">
          <div className="mb-3 flex gap-6">
            <div>
              <p className="adm-eyebrow">superseding records</p>
              <p className="adm-num text-xl font-semibold">{totalSupersedingRecords}</p>
            </div>
            <div>
              <p className="adm-eyebrow">amended filings</p>
              <p className="adm-num text-xl font-semibold">{totalAmendedFilings}</p>
            </div>
          </div>
          {supersedeTrend.length > 0 ? (
            <TrendArea data={supersedeTrend} valueLabel="superseding records" height={180} />
          ) : (
            <p className="text-sm text-[var(--adm-muted)]">No supersede activity recorded yet.</p>
          )}
          <p className="adm-muted mt-3 text-xs">
            Each point counts Gold <span className="adm-num">disclosure_record</span> rows inserted as
            supersessions that month — invariant 1: corrections supersede Gold facts, they never overwrite
            them.
          </p>
        </Card>

        <Card eyebrow="C5" title="Conformance">
          <p className="text-sm text-[var(--adm-muted)]">{renderConformanceNote(pipeline.conformance_note)}</p>
        </Card>
      </div>
    </div>
  );
}
