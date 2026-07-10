import { Fragment, type CSSProperties, type ReactNode } from "react";
import type { AdminInfra, AdminScheduler } from "@/lib/api";
import { ApiError, adminInfra } from "@/lib/api";
import { Badge } from "@/components/admin/ui/Badge";
import { Card } from "@/components/admin/ui/Card";
import { CodeChip } from "@/components/admin/ui/CodeChip";
import { Screen } from "@/components/admin/ui/Screen";
import { Table, type TableColumn } from "@/components/admin/ui/Table";
import { Unavailable } from "@/components/admin/Unavailable";
import { formatUtcMinute } from "@/lib/format";

// Section G (money & infra) is intentionally the lowest-fidelity page in
// this dashboard: everything below is either a static terraform mirror or
// an explicit "not observable" statement — never a live GCP read, never a
// fabricated number.
export const dynamic = "force-dynamic";

// dc.html:1098/1135/1146 — card source footers: mono 10.5px faint ink.
const SOURCE_FOOTER: CSSProperties = {
  fontFamily: "var(--adm-font-data)",
  fontSize: "10.5px",
  color: "var(--adm-faint)",
};

// dc.html:1104 — inline code inside the terraform note: mono 11.5px.
const NOTE_CODE: CSSProperties = {
  fontFamily: "var(--adm-font-data)",
  fontSize: "11.5px",
};

/** Renders an API note verbatim, with any `backtick`-fenced segments in mono. */
function noteWithCodeSegments(note: string): ReactNode {
  const parts = note.split("`");
  if (parts.length < 3) {
    return note;
  }
  return parts.map((part, index) =>
    index % 2 === 1 ? (
      <span key={index} style={NOTE_CODE}>
        {part}
      </span>
    ) : (
      <Fragment key={index}>{part}</Fragment>
    ),
  );
}

const SCHEDULER_COLUMNS: ReadonlyArray<TableColumn<AdminScheduler>> = [
  {
    key: "name",
    header: "Job",
    nowrap: true,
    render: (row) => (
      <span
        style={{ fontFamily: "var(--adm-font-data)", fontSize: 12, color: "var(--adm-ink)" }}
      >
        {row.name}
      </span>
    ),
  },
  {
    key: "schedule",
    header: "Schedule",
    nowrap: true,
    render: (row) => (
      <span
        style={{
          fontFamily: "var(--adm-font-data)",
          fontSize: "11.5px",
          color: "var(--adm-accent-deep)",
        }}
      >
        {row.schedule}
      </span>
    ),
  },
  {
    key: "time_zone",
    header: "TZ",
    render: (row) => (
      <span
        style={{
          fontFamily: "var(--adm-font-data)",
          fontSize: "11.5px",
          color: "var(--adm-muted)",
        }}
      >
        {row.time_zone}
      </span>
    ),
  },
  {
    key: "paused",
    header: "State",
    // Approved deviation #3: the REAL declared paused flag drives the badge —
    // paused:true → warning "paused", otherwise info "running".
    render: (row) =>
      row.paused ? (
        <Badge variant="warning">paused</Badge>
      ) : (
        <Badge variant="info">running</Badge>
      ),
  },
  {
    key: "description",
    header: "Description",
    render: (row) => (
      <span style={{ fontSize: 12, color: "var(--adm-muted)" }}>{row.description}</span>
    ),
  },
];

export default async function InfraPage() {
  let infra: AdminInfra;
  try {
    infra = await adminInfra();
  } catch (error) {
    if (
      error instanceof ApiError &&
      (error.status === 401 || error.status === 403 || error.status === 503)
    ) {
      return <Unavailable reason={error.message} />;
    }
    throw error;
  }

  return (
    <Screen
      label="Infra"
      kicker="Section G · static v1"
      title="Infra"
      subtitle="Money ceiling and terraform mirror — figures are declared in terraform or explicitly unavailable, never a live GCP read."
      meta={<>as of {formatUtcMinute(infra.generated_at)}</>}
    >
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1fr 1fr",
          gap: 16,
          alignItems: "stretch",
        }}
      >
        <Card section="G1" label="Money" title="Budget" rise={0.05}>
          <p className="adm-microlabel" style={{ margin: "16px 0 6px" }}>
            Hard cap
          </p>
          <p
            style={{
              margin: 0,
              fontFamily: "var(--adm-font-data)",
              fontSize: 30,
              fontWeight: 600,
              lineHeight: 1,
              color: "var(--adm-accent-deep)",
              fontVariantNumeric: "tabular-nums",
            }}
          >
            ${infra.budget.hard_cap_usd}{" "}
            <span style={{ fontSize: 14, color: "var(--adm-muted)", fontWeight: 400 }}>
              / month
            </span>
          </p>
          <p style={{ margin: "10px 0 0", fontSize: "11.5px", color: "var(--adm-muted)" }}>
            {infra.budget.live_spend_unavailable_reason}
          </p>
          <p
            style={{
              ...SOURCE_FOOTER,
              margin: "14px 0 0",
              borderTop: "1px solid var(--adm-rule)",
              paddingTop: 12,
            }}
          >
            source: {infra.budget.source}
          </p>
        </Card>

        <Card
          section="G3"
          label="Terraform"
          dashed
          rise={0.1}
          className="flex flex-col justify-center"
        >
          <h2 style={{ margin: "8px 0 10px", color: "var(--adm-muted)" }}>
            Not observable from here
          </h2>
          <p style={{ margin: 0, fontSize: "12.5px", color: "var(--adm-meta)" }}>
            {noteWithCodeSegments(infra.terraform_note)}
          </p>
        </Card>
      </div>

      <Card
        section="G2"
        label="Terraform mirror"
        title="Scheduler jobs"
        rise={0.15}
        className="mt-[16px]"
      >
        <div style={{ marginTop: 12 }}>
          <Table
            columns={SCHEDULER_COLUMNS}
            rows={infra.schedulers}
            getRowKey={(row) => row.name}
          />
        </div>
        <p style={{ ...SOURCE_FOOTER, margin: "12px 0 0" }}>source: {infra.schedulers_source}</p>
      </Card>

      <Card
        section="G2"
        label="Terraform mirror"
        title="Task queues"
        rise={0.2}
        className="mt-[16px]"
      >
        <div style={{ display: "flex", flexWrap: "wrap", gap: 8, marginTop: 14 }}>
          {infra.queues.map((queue) => (
            <CodeChip key={queue} color="neutral" size="lg">
              {queue}
            </CodeChip>
          ))}
        </div>
        <p style={{ ...SOURCE_FOOTER, margin: "14px 0 0" }}>
          source: {infra.queues_source} · live depth unavailable in this environment
        </p>
      </Card>
    </Screen>
  );
}
