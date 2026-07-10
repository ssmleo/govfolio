import type { AdminBlockedJurisdiction, AdminCoverage, AdminRegimeCoverage } from "@/lib/api";
import {
  ApiError,
  adminBackfill,
  adminCoverage,
  adminPipeline,
  listJurisdictions,
  listRegimes,
} from "@/lib/api";
import { CoverageHeatmap, type CoverageCell } from "@/components/admin/CoverageHeatmap";
import { Unavailable } from "@/components/admin/Unavailable";
import { Card } from "@/components/admin/ui/Card";
import { Progress } from "@/components/admin/ui/Progress";
import { Stat } from "@/components/admin/ui/Stat";
import { Table, type TableColumn } from "@/components/admin/ui/Table";
import { CountsBar } from "@/components/admin/charts/CountsBar";
import { RegimeYearHeatmap } from "./RegimeYearHeatmap";
import { buildDossiersById } from "./dossier-data";
import { CoverageRegimeExplorer } from "./CoverageRegimeExplorer";

export const dynamic = "force-dynamic";

const PHASE_ORDER = [
  "stub",
  "scouted",
  "surveyed",
  "sampled",
  "specced",
  "built",
  "live",
  "blocked",
] as const;

const PHASE_COLOR: Record<string, string> = {
  stub: "var(--adm-phase-stub)",
  scouted: "var(--adm-phase-scouted)",
  surveyed: "var(--adm-phase-surveyed)",
  sampled: "var(--adm-phase-sampled)",
  specced: "var(--adm-phase-specced)",
  built: "var(--adm-phase-built)",
  live: "var(--adm-phase-live)",
  blocked: "var(--adm-phase-blocked)",
};

function phaseRank(phase: string): number {
  const i = PHASE_ORDER.findIndex((p) => p === phase);
  return i === -1 ? PHASE_ORDER.length : i;
}

function formatTimestamp(iso: string): string {
  return `${iso.replace("T", " ").slice(0, 19)} UTC`;
}

function formatPct(pct: number | null | undefined): string {
  return pct == null ? "—" : `${pct.toFixed(1)}%`;
}

interface TierChartRow extends Record<string, unknown> {
  regime: string;
  bronze: number;
  silver: number;
  gold: number;
}

const TIER_CHART_LIMIT = 12;

function tierChartRows(regimes: readonly AdminRegimeCoverage[]): TierChartRow[] {
  return regimes
    .map((r) => {
      const bronze = r.bronze_documents ?? 0;
      const silver = r.silver_rows ?? 0;
      const gold = r.gold_records;
      return {
        regime: `${r.body} (${r.jurisdiction_id})`,
        bronze,
        silver,
        gold,
        total: bronze + silver + gold,
      };
    })
    .sort((a, b) => b.total - a.total)
    .slice(0, TIER_CHART_LIMIT);
}

const BLOCKED_COLUMNS: ReadonlyArray<TableColumn<AdminBlockedJurisdiction>> = [
  {
    key: "jurisdiction",
    header: "jurisdiction",
    render: (b) => (
      <div className="flex flex-col">
        <span className="font-medium">{b.name}</span>
        <span className="text-xs text-[var(--adm-muted)]">{b.jurisdiction_id}</span>
      </div>
    ),
  },
  {
    key: "reason",
    header: "blocked reason",
    render: (b) =>
      b.blocked_reason ?? <span className="text-[var(--adm-muted)] italic">no reason recorded</span>,
  },
];

export default async function CoveragePage() {
  let coverage: AdminCoverage;
  let heatmapCells: CoverageCell[];
  let dossiersById: ReturnType<typeof buildDossiersById>;
  try {
    const [coverageData, jurisdictions, backfill, pipeline, regimes] = await Promise.all([
      adminCoverage(),
      listJurisdictions(),
      adminBackfill(),
      adminPipeline(),
      listRegimes(),
    ]);
    coverage = coverageData;
    const blockedReasonById = new Map(
      coverageData.blocked.map((b) => [b.jurisdiction_id, b.blocked_reason ?? null] as const),
    );
    heatmapCells = jurisdictions.map((j) => ({
      id: j.id,
      name: j.name,
      phase: j.coverage_phase,
      blockedReason: blockedReasonById.get(j.id) ?? null,
    }));
    dossiersById = buildDossiersById(coverageData, backfill, pipeline, regimes);
  } catch (error) {
    if (error instanceof ApiError && (error.status === 401 || error.status === 403 || error.status === 503)) {
      return <Unavailable reason={error.message} />;
    }
    throw error;
  }

  const orderedPhases = [...coverage.phases].sort((a, b) => phaseRank(a.phase) - phaseRank(b.phase));
  const regimeLabelById = new Map(
    coverage.regimes.map((r) => [r.regime_id, `${r.jurisdiction_name} — ${r.body}`] as const),
  );
  const chartRows = tierChartRows(coverage.regimes);

  // A stub regime with zero politicians/filings/gold carries no information the
  // world-coverage strip and phase-counts card above don't already show — listing
  // all ~189 of them as near-empty rows here is noise, not density. Anything with
  // real progress (past "stub") or any real data stays, most-advanced first.
  const activeRegimes = coverage.regimes
    .filter((r) => r.coverage_phase !== "stub" || r.politicians > 0 || r.filings > 0 || r.gold_records > 0)
    .sort((a, b) => phaseRank(b.coverage_phase) - phaseRank(a.coverage_phase) || b.gold_records - a.gold_records);
  const collapsedStubCount = coverage.regimes.length - activeRegimes.length;

  return (
    <div className="mx-auto flex max-w-[1400px] flex-col gap-6 px-4 py-6">
      <section className="flex flex-wrap items-baseline justify-between gap-3">
        <div>
          <p className="adm-eyebrow mb-1">Coverage</p>
          <h1>World coverage</h1>
          <p className="mt-1 text-sm text-[var(--adm-muted)]">
            Every seeded jurisdiction and regime — healthy, covered, and what&rsquo;s left.
          </p>
        </div>
        <p className="adm-num text-xs text-[var(--adm-muted)]">
          generated {formatTimestamp(coverage.generated_at)}
        </p>
      </section>

      <Card eyebrow={`A1 · ${heatmapCells.length} jurisdictions`} title="World coverage strip">
        <CoverageHeatmap jurisdictions={heatmapCells} size="full" hrefFor={(id) => `/jurisdictions/${id}`} />
      </Card>

      <Card eyebrow="A1" title="Phase counts">
        <ul className="m-0 flex list-none flex-wrap gap-x-8 gap-y-4 p-0">
          {orderedPhases.map((p) => (
            <li key={p.phase} className="flex items-center gap-2">
              <span
                aria-hidden="true"
                style={{
                  display: "inline-block",
                  width: "0.65rem",
                  height: "0.65rem",
                  borderRadius: "1px",
                  background: PHASE_COLOR[p.phase] ?? "var(--adm-rule-strong)",
                }}
              />
              <span className="adm-num text-lg font-semibold">{p.jurisdictions.toLocaleString()}</span>
              <span className="text-xs text-[var(--adm-muted)]">{p.phase}</span>
            </li>
          ))}
        </ul>
      </Card>

      <Card eyebrow="A1 / A5" title="Blocked jurisdictions">
        <Table
          columns={BLOCKED_COLUMNS}
          rows={coverage.blocked}
          getRowKey={(b) => b.jurisdiction_id}
          emptyMessage="No jurisdictions blocked."
        />
      </Card>

      <Card eyebrow="A2 / A3 / A5" title="Regime coverage">
        <p className="mb-4 text-xs text-[var(--adm-muted)]">
          <span
            className={`adm-num font-semibold ${coverage.bronze_unbridged > 0 ? "text-[var(--adm-warning-ink)]" : ""}`}
          >
            {coverage.bronze_unbridged.toLocaleString()}
          </span>{" "}
          Bronze document{coverage.bronze_unbridged === 1 ? "" : "s"} not attributable to any bridged
          regime.
        </p>

        {chartRows.length > 0 && (
          <>
            <CountsBar
              data={chartRows}
              categoryKey="regime"
              series={[
                { key: "bronze", label: "bronze" },
                { key: "silver", label: "silver", color: "var(--adm-info-ink)" },
                { key: "gold", label: "gold", color: "var(--adm-accent-deep)" },
              ]}
            />
            <p className="mt-2 text-xs text-[var(--adm-muted)]">
              Top {Math.min(TIER_CHART_LIMIT, coverage.regimes.length)} of {coverage.regimes.length}{" "}
              regimes by combined tier volume. Bronze/silver render as 0 where unbridged or lacking a
              staging table — see the table below for exact availability.
            </p>
            <div className="my-4 border-t border-[var(--adm-rule)]" />
          </>
        )}

        <CoverageRegimeExplorer
          regimes={activeRegimes}
          collapsedStubCount={collapsedStubCount}
          dossiers={dossiersById}
        />
      </Card>

      <Card eyebrow="A4" title="Records by regime × year">
        <RegimeYearHeatmap
          cells={coverage.heatmap}
          regimeLabel={(id) => regimeLabelById.get(id) ?? id}
        />
        {coverage.heatmap_missing_event_date > 0 && (
          <p className="mt-3 text-xs text-[var(--adm-muted)]">
            <span className="adm-num font-semibold">
              {coverage.heatmap_missing_event_date.toLocaleString()}
            </span>{" "}
            Gold record{coverage.heatmap_missing_event_date === 1 ? "" : "s"} carry no{" "}
            <code>event_date</code> and are excluded from this grid.
          </p>
        )}
      </Card>

      <Card eyebrow="A6" title="Entity inventory">
        <div className="grid grid-cols-2 gap-6 sm:grid-cols-4">
          <Stat label="politicians" value={coverage.entities.politicians.toLocaleString()} />
          <Stat
            label="w/ wikidata QID"
            value={coverage.entities.politicians_with_wikidata.toLocaleString()}
            caption={formatPct(coverage.entities.politician_wikidata_pct)}
          />
          <Stat label="instruments" value={coverage.entities.instruments.toLocaleString()} />
          <Stat
            label="w/ ticker"
            value={coverage.entities.instruments_with_ticker.toLocaleString()}
            caption={formatPct(coverage.entities.instrument_ticker_pct)}
          />
          <Stat
            label="w/ ISIN"
            value={coverage.entities.instruments_with_isin.toLocaleString()}
            caption={formatPct(coverage.entities.instrument_isin_pct)}
          />
          <Stat label="gold records" value={coverage.entities.records_total.toLocaleString()} />
          <Stat
            label="no matched instrument"
            value={coverage.entities.records_null_instrument.toLocaleString()}
            caption="invariant-3 backlog"
          />
        </div>
        <div className="mt-5 flex flex-col gap-3 sm:max-w-sm">
          {coverage.entities.politician_wikidata_pct != null && (
            <Progress
              value={coverage.entities.politician_wikidata_pct / 100}
              label="politicians with wikidata QID"
            />
          )}
          {coverage.entities.instrument_ticker_pct != null && (
            <Progress
              value={coverage.entities.instrument_ticker_pct / 100}
              label="instruments with ticker"
            />
          )}
          {coverage.entities.instrument_isin_pct != null && (
            <Progress value={coverage.entities.instrument_isin_pct / 100} label="instruments with ISIN" />
          )}
        </div>
      </Card>
    </div>
  );
}
