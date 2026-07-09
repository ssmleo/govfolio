import type { AdminInfra, AdminScheduler } from "@/lib/api";
import { ApiError, adminInfra } from "@/lib/api";
import { Card } from "@/components/admin/ui/Card";
import { Stat } from "@/components/admin/ui/Stat";
import { Badge, stateVariant } from "@/components/admin/ui/Badge";
import { Table, type TableColumn } from "@/components/admin/ui/Table";
import { Unavailable } from "@/components/admin/Unavailable";

// Section G (money & infra) is intentionally the lowest-fidelity page in
// this dashboard: everything below is either a static terraform mirror or
// an explicit "not observable" statement — never a live GCP read, never a
// fabricated number.
export const dynamic = "force-dynamic";

function formatGeneratedAt(iso: string): string {
  return new Date(iso).toUTCString();
}

function formatHardCap(usd: string): string {
  return `$${usd} / month`;
}

const SCHEDULER_COLUMNS: ReadonlyArray<TableColumn<AdminScheduler>> = [
  {
    key: "name",
    header: "job",
    render: (row) => row.name,
  },
  {
    key: "schedule",
    header: "schedule",
    render: (row) => <code className="adm-num text-xs">{row.schedule}</code>,
  },
  {
    key: "time_zone",
    header: "tz",
    render: (row) => <span className="adm-muted">{row.time_zone}</span>,
  },
  {
    key: "paused",
    header: "state",
    render: (row) => {
      const label = row.paused ? "paused" : "running";
      return <Badge variant={stateVariant(label)}>{label}</Badge>;
    },
  },
  {
    key: "description",
    header: "description",
    render: (row) => <span className="adm-muted">{row.description}</span>,
  },
];

export default async function InfraPage() {
  let infra: AdminInfra;
  try {
    infra = await adminInfra();
  } catch (error) {
    if (error instanceof ApiError && (error.status === 401 || error.status === 403 || error.status === 503)) {
      return <Unavailable reason={error.message} />;
    }
    throw error;
  }

  return (
    <div className="mx-auto flex max-w-5xl flex-col gap-4 px-4 py-6">
      <section className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="adm-eyebrow mb-1">Section G · static v1</p>
          <h1>Infra</h1>
          <p className="mt-1 max-w-2xl text-sm adm-muted">
            Money ceiling and terraform mirror. Nothing on this page is a live GCP read —
            figures are either declared in terraform or explicitly unavailable.
          </p>
        </div>
        <div className="flex flex-col items-end gap-1">
          <Badge variant="neutral">{infra.environment}</Badge>
          <p className="text-xs adm-muted">
            as of <span className="adm-num">{formatGeneratedAt(infra.generated_at)}</span>
          </p>
        </div>
      </section>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Card eyebrow="G1" title="Budget">
          <Stat
            label="hard cap"
            value={formatHardCap(infra.budget.hard_cap_usd)}
            caption={infra.budget.live_spend_unavailable_reason}
          />
          <p className="mt-4 text-xs adm-muted">source: {infra.budget.source}</p>
        </Card>

        <div>
          <p className="adm-eyebrow mb-2">G3 · Terraform</p>
          <Unavailable reason={infra.terraform_note} />
        </div>
      </div>

      <Card eyebrow="G2" title="Scheduler jobs (terraform mirror)">
        <Table columns={SCHEDULER_COLUMNS} rows={infra.schedulers} getRowKey={(row) => row.name} />
        <p className="mt-3 text-xs adm-muted">source: {infra.schedulers_source}</p>
      </Card>

      <Card eyebrow="G2" title="Task queues (terraform mirror)">
        <div className="flex flex-wrap gap-1.5">
          {infra.queues.map((queue) => (
            <Badge key={queue} variant="neutral">
              {queue}
            </Badge>
          ))}
        </div>
        <p className="mt-3 text-xs adm-muted">source: {infra.queues_source}</p>
      </Card>
    </div>
  );
}
