"use client";

import { useState } from "react";

import type { AdminRegimeCoverage } from "@/lib/api";
import { formatCount } from "@/lib/format";
import { Badge, stateVariant } from "@/components/admin/ui/Badge";
import { Table, type TableColumn } from "@/components/admin/ui/Table";
import type { RegimeDossierData } from "./dossier-data";
import { RegimeDossier } from "./RegimeDossier";

export interface CoverageRegimeExplorerProps {
  regimes: readonly AdminRegimeCoverage[];
  collapsedStubCount: number;
  dossiers: Readonly<Record<string, RegimeDossierData>>;
}

// Three 4px stacked bars normalized to the largest silver_rows value across
// the visible regimes (dc.html:256-262, normalization dc.html:1450) — the
// same "how big relative to the biggest pipe" reading as the design, widths
// clamped at 100% since our data (unlike the design's fabricated rows)
// doesn't guarantee gold <= silver <= some shared ceiling.
function TierMiniBars({
  bronze,
  silver,
  gold,
  maxSilver,
}: {
  bronze: number | null;
  silver: number | null;
  gold: number;
  maxSilver: number;
}) {
  const bars: ReadonlyArray<{ value: number | null; color: string }> = [
    { value: bronze, color: "var(--adm-series-bronze)" },
    { value: silver, color: "var(--adm-series-silver)" },
    { value: gold, color: "var(--adm-series-gold)" },
  ];
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 2, width: 110 }}>
      {bars.map((bar, i) => {
        const pct =
          bar.value === null ? 0 : Math.min(100, Math.round((100 * bar.value) / maxSilver));
        return (
          <div
            key={i}
            style={{
              height: 4,
              borderRadius: 1,
              background: "var(--adm-progress-track)",
              overflow: "hidden",
            }}
          >
            <div
              style={{ height: "100%", width: `${pct}%`, background: bar.color, borderRadius: 1 }}
            />
          </div>
        );
      })}
    </div>
  );
}

// Owns the dossier's open/close state (goal 094, Task 6): the regime table
// (design dc.html:236-270 columns, plus a compact regime-code caption so
// each row stays text-identifiable — see e2e/admin.spec.ts's row-click and
// body-text assertions) and the slide-over it opens live together so
// clicking a row can flip local state without `page.tsx` needing to become
// a client component.
export function CoverageRegimeExplorer({
  regimes,
  collapsedStubCount,
  dossiers,
}: CoverageRegimeExplorerProps) {
  const [openRegimeId, setOpenRegimeId] = useState<string | null>(null);
  const openDossier = openRegimeId !== null ? (dossiers[openRegimeId] ?? null) : null;
  const maxSilver = Math.max(1, ...regimes.map((r) => r.silver_rows ?? 0));

  const columns: ReadonlyArray<TableColumn<AdminRegimeCoverage>> = [
    {
      key: "regime",
      header: "Regime",
      render: (r) => (
        <div className="flex flex-col">
          <span style={{ fontWeight: 600, color: "var(--adm-ink)" }}>{r.body}</span>
          <span style={{ fontSize: "11px", color: "var(--adm-meta)" }}>{r.jurisdiction_name}</span>
          {r.regime_codes.length > 0 && (
            <span
              className="adm-num"
              style={{ fontSize: "10.5px", color: "var(--adm-faint)" }}
            >
              {r.regime_codes.join(", ")}
            </span>
          )}
        </div>
      ),
    },
    {
      key: "phase",
      header: "Phase",
      render: (r) => (
        <div className="flex flex-wrap items-center gap-1">
          <Badge variant={stateVariant(r.coverage_phase)}>{r.coverage_phase}</Badge>
          {r.built_not_backfilled && <Badge variant="warning">gap</Badge>}
        </div>
      ),
    },
    {
      key: "tiers",
      header: "Tiers",
      render: (r) => (
        <TierMiniBars
          bronze={r.bronze_documents ?? null}
          silver={r.silver_rows ?? null}
          gold={r.gold_records}
          maxSilver={maxSilver}
        />
      ),
    },
    {
      key: "politicians",
      header: "Politicians",
      numeric: true,
      render: (r) => (
        <span style={{ color: "var(--adm-text-secondary)" }}>{formatCount(r.politicians)}</span>
      ),
    },
    {
      key: "filings",
      header: "Filings",
      numeric: true,
      render: (r) => (
        <span style={{ color: "var(--adm-text-secondary)" }}>{formatCount(r.filings)}</span>
      ),
    },
    {
      key: "gold",
      header: "Gold",
      numeric: true,
      render: (r) => (
        <span style={{ fontWeight: 600, color: "var(--adm-accent-deep)" }}>
          {formatCount(r.gold_records)}
        </span>
      ),
    },
    {
      key: "bronze",
      header: "Bronze",
      numeric: true,
      render: (r) =>
        r.bronze_documents == null ? (
          <span title="Regime not bridged to any adapter" style={{ color: "var(--adm-meta)" }}>
            —
          </span>
        ) : (
          <span style={{ color: "var(--adm-text-secondary)" }}>
            {formatCount(r.bronze_documents)}
          </span>
        ),
    },
    {
      key: "silver",
      header: "Silver",
      numeric: true,
      render: (r) =>
        r.silver_rows == null ? (
          <span title="No staging table for this adapter yet" style={{ color: "var(--adm-meta)" }}>
            —
          </span>
        ) : (
          <span style={{ color: "var(--adm-text-secondary)" }}>{formatCount(r.silver_rows)}</span>
        ),
    },
    {
      key: "first_filed",
      header: "First filed",
      nowrap: true,
      render: (r) => (
        <span style={{ fontSize: "11.5px", color: "var(--adm-muted)" }}>
          {r.first_filed_date ?? "—"}
        </span>
      ),
    },
    {
      key: "last_filed",
      header: "Last filed",
      nowrap: true,
      render: (r) => (
        <span style={{ fontSize: "11.5px", color: "var(--adm-muted)" }}>
          {r.last_filed_date ?? "—"}
        </span>
      ),
    },
  ];

  return (
    <>
      <p className="mb-3" style={{ fontSize: "12px", color: "var(--adm-meta)" }}>
        Select a regime to open its dossier.
      </p>
      <Table
        columns={columns}
        rows={regimes}
        getRowKey={(r) => r.regime_id}
        emptyMessage="No regimes registered."
        onRowClick={(r) => setOpenRegimeId(r.regime_id)}
      />
      {collapsedStubCount > 0 && (
        <p className="mt-3" style={{ fontSize: "11.5px", color: "var(--adm-meta)" }}>
          +{formatCount(collapsedStubCount)} stub regime{collapsedStubCount === 1 ? "" : "s"} hold
          no activity yet — see the world wall on{" "}
          <a href="/admin">Overview</a> for the full picture.
        </p>
      )}
      <RegimeDossier data={openDossier} onClose={() => setOpenRegimeId(null)} />
    </>
  );
}
