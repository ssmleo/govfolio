import type {
  AdminMimeCount,
  AdminPgTable,
  AdminSchemeCount,
  AdminStorage,
  AdminTableRowCount,
} from "@/lib/api";
import { ApiError, adminStorage } from "@/lib/api";
import { Unavailable } from "@/components/admin/Unavailable";
import { TrendArea } from "@/components/admin/charts/TrendArea";
import { Card } from "@/components/admin/ui/Card";
import { Progress } from "@/components/admin/ui/Progress";
import { Stat } from "@/components/admin/ui/Stat";
import type { TableColumn } from "@/components/admin/ui/Table";
import { Table } from "@/components/admin/ui/Table";

// Section E of the admin dashboard (goal 091): what's stored, how much, and
// where it lives. Always fresh — a storage snapshot must not be ISR-stale.
export const dynamic = "force-dynamic";

const NUMBER_FORMAT = new Intl.NumberFormat("en-US");

function formatNum(n: number): string {
  return NUMBER_FORMAT.format(n);
}

function formatBytes(bytes: number): string {
  if (bytes <= 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB", "TB"] as const;
  const exp = Math.min(Math.floor(Math.log2(bytes) / 10), units.length - 1);
  const value = bytes / 1024 ** exp;
  return `${value.toFixed(exp === 0 ? 0 : 1)} ${units[exp]}`;
}

function formatDay(day: string): string {
  return day.slice(5);
}

function formatTimestamp(iso: string): string {
  return `${iso.slice(0, 16).replace("T", " ")} UTC`;
}

type DeadTupleTone = "danger" | "warning" | "neutral";

function deadTupleTone(liveTuples: number, deadTuples: number): DeadTupleTone {
  const total = liveTuples + deadTuples;
  if (total === 0) {
    return "neutral";
  }
  const ratio = deadTuples / total;
  if (ratio >= 0.4) {
    return "danger";
  }
  if (ratio >= 0.2) {
    return "warning";
  }
  return "neutral";
}

const DEAD_TUPLE_TONE_CLASS: Record<DeadTupleTone, string> = {
  danger: "text-[var(--adm-danger-ink)]",
  warning: "text-[var(--adm-warning-ink)]",
  neutral: "text-[var(--adm-muted)]",
};

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

  const totalMimeDocs = data.bronze_by_mime.reduce((sum, m) => sum + m.documents, 0);
  const totalSchemeDocs = data.bronze_by_scheme.reduce((sum, s) => sum + s.documents, 0);
  const gsDocs = data.bronze_by_scheme.find((s) => s.scheme === "gs")?.documents ?? 0;
  const gsFraction = totalSchemeDocs > 0 ? gsDocs / totalSchemeDocs : 0;

  const totalGoldRecords = data.growth_30d.reduce((sum, d) => sum + d.gold_records, 0);
  const totalFilings = data.growth_30d.reduce((sum, d) => sum + d.filings, 0);
  const goldSeries = data.growth_30d.map((d) => ({ x: formatDay(d.day), y: d.gold_records }));
  const filingSeries = data.growth_30d.map((d) => ({ x: formatDay(d.day), y: d.filings }));

  const mimeColumns: TableColumn<AdminMimeCount>[] = [
    { key: "mime_type", header: "mime type", render: (row) => row.mime_type },
    {
      key: "documents",
      header: "documents",
      numeric: true,
      render: (row) => formatNum(row.documents),
    },
    {
      key: "share",
      header: "share",
      render: (row) => (
        <div style={{ width: "6rem" }}>
          <Progress value={totalMimeDocs > 0 ? row.documents / totalMimeDocs : 0} />
        </div>
      ),
    },
  ];

  const schemeColumns: TableColumn<AdminSchemeCount>[] = [
    { key: "scheme", header: "scheme", render: (row) => row.scheme },
    {
      key: "documents",
      header: "documents",
      numeric: true,
      render: (row) => formatNum(row.documents),
    },
    {
      key: "share",
      header: "share",
      render: (row) => (
        <div style={{ width: "6rem" }}>
          <Progress value={totalSchemeDocs > 0 ? row.documents / totalSchemeDocs : 0} />
        </div>
      ),
    },
  ];

  const tableRowsColumns: TableColumn<AdminTableRowCount>[] = [
    { key: "table_name", header: "table", render: (row) => row.table_name },
    {
      key: "row_count",
      header: "rows",
      numeric: true,
      render: (row) => formatNum(row.row_count),
    },
  ];

  const pgTableColumns: TableColumn<AdminPgTable>[] = [
    { key: "table_name", header: "table", render: (row) => row.table_name },
    {
      key: "total_bytes",
      header: "size",
      numeric: true,
      render: (row) => formatBytes(row.total_bytes),
    },
    {
      key: "live_tuples",
      header: "live",
      numeric: true,
      render: (row) => formatNum(row.live_tuples),
    },
    {
      key: "dead_tuples",
      header: "dead",
      numeric: true,
      render: (row) => formatNum(row.dead_tuples),
    },
    {
      key: "dead_ratio",
      header: "dead %",
      numeric: true,
      render: (row) => {
        const total = row.live_tuples + row.dead_tuples;
        const pct = total > 0 ? Math.round((row.dead_tuples / total) * 100) : 0;
        const tone = deadTupleTone(row.live_tuples, row.dead_tuples);
        return <span className={DEAD_TUPLE_TONE_CLASS[tone]}>{pct}%</span>;
      },
    },
  ];

  return (
    <div className="flex flex-col gap-6 px-4 py-6">
      <header className="flex flex-col gap-1.5">
        <p className="adm-eyebrow">Section E</p>
        <h1>Storage & tiers</h1>
        <p className="text-sm text-[var(--adm-muted)]">
          Bronze documents, Silver/Gold row counts, and the Postgres footprint underneath.
          Snapshot {formatTimestamp(data.generated_at)}.
        </p>
      </header>

      <div className="flex flex-col gap-3">
        <div className="grid gap-4 md:grid-cols-2">
          <Card eyebrow="E1 · bronze documents" title="By mime type">
            <Table
              columns={mimeColumns}
              rows={data.bronze_by_mime}
              getRowKey={(row) => row.mime_type}
            />
          </Card>
          <Card eyebrow="E1 · cloud migration" title="By storage scheme">
            {totalSchemeDocs > 0 && (
              <div className="mb-3">
                <Progress value={gsFraction} label="migrated to cloud storage (gs://)" />
              </div>
            )}
            <Table
              columns={schemeColumns}
              rows={data.bronze_by_scheme}
              getRowKey={(row) => row.scheme}
            />
          </Card>
        </div>
        <p className="text-xs text-[var(--adm-muted)]">{data.bronze_note}</p>
      </div>

      <Card eyebrow="Schema" title="Rows per table">
        <Table
          columns={tableRowsColumns}
          rows={data.table_rows}
          getRowKey={(row) => row.table_name}
        />
      </Card>

      <Card eyebrow="E2 · last 30 days" title="Gold + filing growth">
        {data.growth_30d.length === 0 ? (
          <p className="text-sm text-[var(--adm-muted)]">No activity in the last 30 days.</p>
        ) : (
          <div className="grid gap-4 md:grid-cols-2">
            <div>
              <Stat
                label="Gold records"
                value={formatNum(totalGoldRecords)}
                caption="disclosure_record rows created"
              />
              <div className="mt-2">
                <TrendArea data={goldSeries} valueLabel="records" height={140} />
              </div>
            </div>
            <div>
              <Stat
                label="Filings discovered"
                value={formatNum(totalFilings)}
                caption="filing.discovered_at"
              />
              <div className="mt-2">
                <TrendArea data={filingSeries} valueLabel="filings" height={140} />
              </div>
            </div>
          </div>
        )}
      </Card>

      <Card eyebrow="E3 · postgres" title="Database size & top tables">
        <div className="mb-4">
          <Stat
            label="Database size"
            value={formatBytes(data.pg.database_size_bytes)}
            caption="pg_database_size(current_database())"
          />
        </div>
        <Table
          columns={pgTableColumns}
          rows={data.pg.top_tables}
          getRowKey={(row) => row.table_name}
          emptyMessage="No tables reported."
        />
      </Card>
    </div>
  );
}
