"use client";

import { useState } from "react";

import type { AdminRegimeCoverage } from "@/lib/api";
import { Badge, stateVariant } from "@/components/admin/ui/Badge";
import { Table, type TableColumn } from "@/components/admin/ui/Table";
import type { RegimeDossierData } from "./dossier-data";
import { RegimeDossier } from "./RegimeDossier";

export interface CoverageRegimeExplorerProps {
  regimes: readonly AdminRegimeCoverage[];
  collapsedStubCount: number;
  dossiers: Readonly<Record<string, RegimeDossierData>>;
}

const REGIME_COLUMNS: ReadonlyArray<TableColumn<AdminRegimeCoverage>> = [
  {
    key: "regime",
    header: "regime",
    render: (r) => (
      <div className="flex flex-col">
        <span className="font-medium">{r.body}</span>
        <span className="text-xs text-[var(--adm-muted)]">{r.jurisdiction_name}</span>
      </div>
    ),
  },
  {
    key: "phase",
    header: "phase",
    render: (r) => (
      <div className="flex flex-wrap items-center gap-1">
        <Badge variant={stateVariant(r.coverage_phase)}>{r.coverage_phase}</Badge>
        {r.built_not_backfilled && <Badge variant="warning">gap</Badge>}
      </div>
    ),
  },
  {
    key: "bridge",
    header: "bridge",
    render: (r) =>
      r.regime_codes.length === 0 ? (
        <Badge variant="neutral">unbridged</Badge>
      ) : (
        <span className="text-xs text-[var(--adm-muted)]">{r.regime_codes.join(", ")}</span>
      ),
  },
  {
    key: "politicians",
    header: "politicians",
    numeric: true,
    render: (r) => r.politicians.toLocaleString(),
  },
  {
    key: "filings",
    header: "filings",
    numeric: true,
    render: (r) => r.filings.toLocaleString(),
  },
  {
    key: "gold",
    header: "gold",
    numeric: true,
    render: (r) => r.gold_records.toLocaleString(),
  },
  {
    key: "bronze",
    header: "bronze",
    numeric: true,
    render: (r) =>
      r.bronze_documents == null ? (
        <span title="Regime not bridged to any adapter" className="text-[var(--adm-muted)]">
          —
        </span>
      ) : (
        r.bronze_documents.toLocaleString()
      ),
  },
  {
    key: "silver",
    header: "silver",
    numeric: true,
    render: (r) =>
      r.silver_rows == null ? (
        <span title="No staging table for this adapter yet" className="text-[var(--adm-muted)]">
          —
        </span>
      ) : (
        r.silver_rows.toLocaleString()
      ),
  },
  {
    key: "first_filed",
    header: "first filed",
    render: (r) => r.first_filed_date ?? "—",
  },
  {
    key: "last_filed",
    header: "last filed",
    render: (r) => r.last_filed_date ?? "—",
  },
];

// Owns the dossier's open/close state (goal 094, Task 6): the regime table
// (moved here from `page.tsx` verbatim, plus `onRowClick`) and the slide-over
// it opens live together so clicking a row can flip local state without
// `page.tsx` needing to become a client component.
export function CoverageRegimeExplorer({
  regimes,
  collapsedStubCount,
  dossiers,
}: CoverageRegimeExplorerProps) {
  const [openRegimeId, setOpenRegimeId] = useState<string | null>(null);
  const openDossier = openRegimeId !== null ? (dossiers[openRegimeId] ?? null) : null;

  return (
    <>
      <Table
        columns={REGIME_COLUMNS}
        rows={regimes}
        getRowKey={(r) => r.regime_id}
        emptyMessage="No regimes registered."
        onRowClick={(r) => setOpenRegimeId(r.regime_id)}
      />
      {collapsedStubCount > 0 && (
        <p className="mt-3 text-xs text-[var(--adm-muted)]">
          +{collapsedStubCount.toLocaleString()} stub regime{collapsedStubCount === 1 ? "" : "s"} with no
          activity yet — see the world coverage strip above for the full 196-jurisdiction picture.
        </p>
      )}
      <RegimeDossier data={openDossier} onClose={() => setOpenRegimeId(null)} />
    </>
  );
}
