import Link from "next/link";

import type {
  AdminBrCpfCollision,
  AdminQuality,
  AdminRegimePrecision,
  AdminVerdictCount,
} from "@/lib/api";
import { ApiError, adminQuality } from "@/lib/api";
import { Card } from "@/components/admin/ui/Card";
import { Badge, stateVariant } from "@/components/admin/ui/Badge";
import { Table, type TableColumn } from "@/components/admin/ui/Table";
import { Stat } from "@/components/admin/ui/Stat";
import { Progress } from "@/components/admin/ui/Progress";
import { CountsBar } from "@/components/admin/charts/CountsBar";
import { Histogram } from "@/components/admin/charts/Histogram";
import { Unavailable } from "@/components/admin/Unavailable";

// Section D (data quality & review ops): the review queue's shape, the
// invariant-3 unlinked-instrument backlog, this month's sampling-audit
// precision, and two integrity spot checks. The br CPF-collision sweep is a
// whole-dataset scan — opt-in only (?sweep=br), never run on page load.
export const dynamic = "force-dynamic";

interface Search {
  searchParams: Promise<{ sweep?: string }>;
}

function formatGeneratedAt(iso: string): string {
  return new Date(iso).toUTCString();
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

const VERDICT_COLUMNS: ReadonlyArray<TableColumn<AdminVerdictCount>> = [
  {
    key: "verdict",
    header: "verdict",
    render: (row) => <Badge variant={stateVariant(row.verdict)}>{row.verdict}</Badge>,
  },
  {
    key: "attempts",
    header: "attempts",
    numeric: true,
    render: (row) => row.attempts,
  },
];

const PRECISION_COLUMNS: ReadonlyArray<TableColumn<AdminRegimePrecision>> = [
  { key: "regime_id", header: "regime", render: (row) => row.regime_id },
  { key: "body", header: "body", render: (row) => <span className="adm-muted">{row.body}</span> },
  { key: "sampled", header: "sampled", numeric: true, render: (row) => row.sampled },
  { key: "audited", header: "audited", numeric: true, render: (row) => row.audited },
  { key: "discrepancies", header: "discrepancies", numeric: true, render: (row) => row.discrepancies },
  {
    key: "precision_estimate",
    header: "precision",
    numeric: true,
    render: (row) => formatPercent(row.precision_estimate),
  },
];

const COLLISION_COLUMNS: ReadonlyArray<TableColumn<AdminBrCpfCollision>> = [
  { key: "politician_id", header: "politician", render: (row) => row.politician_id },
  { key: "canonical_name", header: "canonical name", render: (row) => row.canonical_name },
  { key: "distinct_cpfs", header: "distinct cpfs", numeric: true, render: (row) => row.distinct_cpfs },
  {
    key: "cpfs",
    header: "cpfs",
    render: (row) => <span className="adm-num text-xs">{row.cpfs.join(", ")}</span>,
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
    return (
      <div className="flex flex-col gap-3">
        <p className="max-w-2xl text-sm adm-muted">
          Scans every br filing&apos;s staged rows to compare CPFs per politician — a
          whole-dataset scan, not a cheap query. Zero rows is a pass; any row needs
          investigation.
        </p>
        <Link
          href="/admin/quality?sweep=br"
          className="inline-flex w-fit items-center gap-1.5 rounded-sm border border-[var(--adm-rule-strong)] bg-[var(--adm-surface-sunken)] px-3 py-1.5 text-sm font-semibold text-[var(--adm-ink)] no-underline hover:bg-[var(--adm-rule)]"
        >
          Run br CPF collision sweep
        </Link>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <Badge variant={sweep.pass ? "success" : "danger"}>
          {sweep.pass ? "pass" : `${sweep.collisions.length} collision(s)`}
        </Badge>
        <Link href="/admin/quality" className="text-xs">
          hide results
        </Link>
      </div>
      {sweep.pass ? (
        <p className="text-sm adm-muted">No CPF collisions found across the br dataset.</p>
      ) : (
        <Table columns={COLLISION_COLUMNS} rows={sweep.collisions} getRowKey={(row) => row.politician_id} />
      )}
    </div>
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
  const linkedShare = retention.raw_documents > 0 ? retention.linked_to_filing / retention.raw_documents : 0;

  return (
    <div className="mx-auto flex max-w-5xl flex-col gap-8 px-4 py-6">
      <section className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="adm-eyebrow mb-1">Section D · data quality &amp; review ops</p>
          <h1>Quality</h1>
          <p className="mt-1 max-w-2xl text-sm adm-muted">
            Review-queue health, the never-guess backlog, this month&apos;s sampling precision,
            and two integrity spot checks.
          </p>
        </div>
        <p className="adm-num text-xs adm-muted">as of {formatGeneratedAt(data.generated_at)}</p>
      </section>

      <section className="flex flex-col gap-4">
        <h2>Review queue</h2>
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
          <Card eyebrow="D1" title="Open by reason">
            <CountsBar data={data.open_by_reason} categoryKey="reason" series={[{ key: "tasks", label: "open tasks" }]} />
          </Card>
          <Card eyebrow="D1" title="Open by target kind">
            <CountsBar
              data={data.open_by_target_kind}
              categoryKey="target_kind"
              series={[{ key: "tasks", label: "open tasks" }]}
            />
          </Card>
          <Card eyebrow="D1" title="Open-queue age">
            <Histogram data={ageBuckets} />
          </Card>
          <Card eyebrow="D1" title="30-day resolution">
            <div className="flex flex-col gap-4">
              <div className="grid grid-cols-2 gap-4">
                <Stat label="resolved tasks" value={data.resolution_30d.resolved_tasks} />
                <Stat
                  label="median time to resolve"
                  value={formatHours(data.resolution_30d.median_hours_to_resolve)}
                />
              </div>
              <Table
                columns={VERDICT_COLUMNS}
                rows={data.resolution_30d.verdicts}
                getRowKey={(row) => row.verdict}
                emptyMessage="No applied verdicts in the last 30 days."
              />
            </div>
          </Card>
        </div>
      </section>

      <section className="flex flex-col gap-4">
        <h2>Unlinked instruments</h2>
        <Card eyebrow="D2" title="NULL-instrument backlog">
          <div className="flex flex-col gap-4">
            <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-6">
              <Stat label="total" value={unlinked.total} />
              <Stat label="listed" value={unlinked.listed} />
              <Stat label="equity" value={unlinked.equity} />
              <Stat label="bond" value={unlinked.bond} />
              <Stat label="fund" value={unlinked.fund} />
              <Stat label="option" value={unlinked.option} />
            </div>
            <Progress value={listedShare} label="listed share of NULL-instrument rows" />
          </div>
        </Card>
      </section>

      <section className="flex flex-col gap-4">
        <h2>Precision — {data.precision_current_month.sample_month}</h2>
        <Card eyebrow="D6" title="Sampling audit, current month">
          <Table
            columns={PRECISION_COLUMNS}
            rows={data.precision_current_month.regimes}
            getRowKey={(row) => row.regime_id}
            emptyMessage="No sample batch drawn this month."
          />
        </Card>
      </section>

      <section className="flex flex-col gap-4">
        <h2>Pipeline integrity</h2>
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
          <Card eyebrow="D4" title="Idempotency">
            <div className="grid grid-cols-2 gap-4">
              <Stat label="backfill runs" value={data.idempotency.runs} />
              <Stat label="replayed" value={data.idempotency.replayed_total} />
            </div>
            <p className="mt-4 text-xs adm-muted">{data.idempotency.note}</p>
          </Card>
          <Card eyebrow="D5" title="Raw retention">
            <div className="flex flex-col gap-4">
              <div className="grid grid-cols-3 gap-4">
                <Stat label="raw documents" value={retention.raw_documents} />
                <Stat label="linked to filing" value={retention.linked_to_filing} />
                <Stat label="orphaned" value={retention.orphaned} />
              </div>
              <Progress value={linkedShare} label="linked share of raw documents" />
            </div>
          </Card>
        </div>
      </section>

      <section className="flex flex-col gap-4">
        <h2>br CPF collision sweep</h2>
        <Card eyebrow="D3" title="Identity collisions (opt-in)">
          <BrCollisionSweep sweep={data.collision_sweep} requested={requestedSweep} />
        </Card>
      </section>
    </div>
  );
}
