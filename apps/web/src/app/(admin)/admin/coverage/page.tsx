import type { AdminBlockedJurisdiction, AdminCoverage, AdminRegimeCoverage } from "@/lib/api";
import {
  ApiError,
  adminBackfill,
  adminCoverage,
  adminPipeline,
  listRegimes,
} from "@/lib/api";
import { formatCount, formatUtcMinute } from "@/lib/format";
import { Unavailable } from "@/components/admin/Unavailable";
import { Badge } from "@/components/admin/ui/Badge";
import { Card } from "@/components/admin/ui/Card";
import { Progress } from "@/components/admin/ui/Progress";
import { Screen } from "@/components/admin/ui/Screen";
import { Stat } from "@/components/admin/ui/Stat";
import { Table, type TableColumn } from "@/components/admin/ui/Table";
import { RegimeYearHeatmap } from "./RegimeYearHeatmap";
import { buildDossiersById } from "./dossier-data";
import { CoverageRegimeExplorer } from "./CoverageRegimeExplorer";

export const dynamic = "force-dynamic";

// Coverage-phase ramp, legend-ordered per the design (dc.html:1424) — same
// order Overview's world wall uses.
const PHASE_ORDER = [
  "live",
  "built",
  "specced",
  "sampled",
  "surveyed",
  "scouted",
  "stub",
  "blocked",
] as const;

const PHASE_COLOR: Record<string, string> = {
  live: "var(--adm-phase-live)",
  built: "var(--adm-phase-built)",
  specced: "var(--adm-phase-specced)",
  sampled: "var(--adm-phase-sampled)",
  surveyed: "var(--adm-phase-surveyed)",
  scouted: "var(--adm-phase-scouted)",
  stub: "var(--adm-phase-stub)",
  blocked: "var(--adm-phase-blocked)",
};

function phaseRank(phase: string): number {
  const i = PHASE_ORDER.findIndex((p) => p === phase);
  return i === -1 ? PHASE_ORDER.length : i;
}

function formatPct(pct: number | null | undefined): string {
  return pct == null ? "—" : `${pct.toFixed(1)}%`;
}

const BLOCKED_COLUMNS: ReadonlyArray<TableColumn<AdminBlockedJurisdiction>> = [
  {
    key: "jurisdiction",
    header: "Jurisdiction",
    render: (b) => <span style={{ fontWeight: 600, color: "var(--adm-ink)" }}>{b.name}</span>,
  },
  {
    key: "status",
    header: "Status",
    render: () => <Badge variant="danger">blocked</Badge>,
  },
  {
    key: "reason",
    header: "Reason",
    render: (b) => (
      <span style={{ fontSize: "11.5px", color: "var(--adm-meta)", fontStyle: "italic" }}>
        {b.blocked_reason ?? "no reason recorded"}
      </span>
    ),
  },
];

export default async function CoveragePage() {
  let coverage: AdminCoverage;
  let dossiersById: ReturnType<typeof buildDossiersById>;
  try {
    const [coverageData, backfill, pipeline, regimes] = await Promise.all([
      adminCoverage(),
      adminBackfill(),
      adminPipeline(),
      listRegimes(),
    ]);
    coverage = coverageData;
    dossiersById = buildDossiersById(coverageData, backfill, pipeline, regimes);
  } catch (error) {
    if (
      error instanceof ApiError &&
      (error.status === 401 || error.status === 403 || error.status === 503)
    ) {
      return <Unavailable reason={error.message} />;
    }
    throw error;
  }

  const orderedPhases = [...coverage.phases].sort(
    (a, b) => phaseRank(a.phase) - phaseRank(b.phase),
  );
  const regimeLabelById = new Map(
    coverage.regimes.map((r) => [r.regime_id, `${r.jurisdiction_name} — ${r.body}`] as const),
  );

  // A stub regime with zero politicians/filings/gold carries no information
  // the world wall (Overview) and phase-counts strip above don't already
  // show — listing all of them here is noise, not density. Anything with
  // real progress (past "stub") or any real data stays, most-advanced first.
  const activeRegimes: AdminRegimeCoverage[] = coverage.regimes
    .filter(
      (r) => r.coverage_phase !== "stub" || r.politicians > 0 || r.filings > 0 || r.gold_records > 0,
    )
    .sort(
      (a, b) => phaseRank(b.coverage_phase) - phaseRank(a.coverage_phase) || b.gold_records - a.gold_records,
    );
  const collapsedStubCount = coverage.regimes.length - activeRegimes.length;

  return (
    <Screen
      label="Coverage"
      kicker="Section A"
      title="World coverage"
      subtitle="Every seeded jurisdiction and regime — healthy, covered, and what’s left."
      meta={`generated ${formatUtcMinute(coverage.generated_at)}`}
    >
      <Card rise={0.05}>
        <div style={{ display: "flex", flexWrap: "wrap", gap: "14px 34px", alignItems: "baseline" }}>
          {orderedPhases.map((p) => (
            <span key={p.phase} style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
              <span
                style={{
                  width: 10,
                  height: 10,
                  borderRadius: 1,
                  background: PHASE_COLOR[p.phase] ?? "var(--adm-rule-strong)",
                  display: "inline-block",
                }}
              />
              <span
                className="adm-num"
                style={{ fontSize: "17px", fontWeight: 600, color: "var(--adm-heading)" }}
              >
                {formatCount(p.jurisdictions)}
              </span>
              <span style={{ fontSize: "11px", color: "var(--adm-meta)" }}>{p.phase}</span>
            </span>
          ))}
        </div>
      </Card>

      <Card
        section="A2 / A3"
        label="Regime coverage"
        title="Active regimes"
        meta={`${formatCount(coverage.bronze_unbridged)} bronze docs unbridged`}
        rise={0.12}
        className="mt-[16px]"
      >
        <CoverageRegimeExplorer
          regimes={activeRegimes}
          collapsedStubCount={collapsedStubCount}
          dossiers={dossiersById}
        />
      </Card>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1.25fr .75fr",
          gap: 16,
          marginTop: 16,
          alignItems: "start",
        }}
      >
        <Card section="A6" label="Entities" title="Entity inventory" rise={0.19}>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(4,1fr)", gap: "16px 14px" }}>
            <Stat label="Politicians" value={formatCount(coverage.entities.politicians)} size={20} />
            <Stat
              label="w/ Wikidata QID"
              value={formatCount(coverage.entities.politicians_with_wikidata)}
              size={20}
              caption={formatPct(coverage.entities.politician_wikidata_pct)}
            />
            <Stat label="Instruments" value={formatCount(coverage.entities.instruments)} size={20} />
            <Stat
              label="w/ ticker"
              value={formatCount(coverage.entities.instruments_with_ticker)}
              size={20}
              caption={formatPct(coverage.entities.instrument_ticker_pct)}
            />
            <Stat
              label="w/ ISIN"
              value={formatCount(coverage.entities.instruments_with_isin)}
              size={20}
              caption={formatPct(coverage.entities.instrument_isin_pct)}
            />
            <Stat
              label="Gold records"
              value={formatCount(coverage.entities.records_total)}
              size={20}
              tone="var(--adm-accent-deep)"
            />
            <Stat
              label="No matched instrument"
              value={formatCount(coverage.entities.records_null_instrument)}
              size={20}
              tone="warning"
              caption="invariant-3 backlog"
            />
          </div>
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              gap: 12,
              marginTop: 20,
              borderTop: "1px solid var(--adm-rule)",
              paddingTop: 16,
            }}
          >
            {coverage.entities.politician_wikidata_pct != null && (
              <Progress
                value={coverage.entities.politician_wikidata_pct / 100}
                color="var(--adm-series-funnel-gold)"
                label="politicians with Wikidata QID"
              />
            )}
            {coverage.entities.instrument_ticker_pct != null && (
              <Progress
                value={coverage.entities.instrument_ticker_pct / 100}
                color="var(--adm-series-funnel-gold)"
                label="instruments with ticker"
              />
            )}
            {coverage.entities.instrument_isin_pct != null && (
              <Progress
                value={coverage.entities.instrument_isin_pct / 100}
                color="var(--adm-series-funnel-gold)"
                label="instruments with ISIN"
              />
            )}
          </div>
        </Card>

        <Card section="A5" label="Blocked" title="Blocked jurisdictions" rise={0.26}>
          <Table
            columns={BLOCKED_COLUMNS}
            rows={coverage.blocked}
            getRowKey={(b) => b.jurisdiction_id}
            emptyMessage="No jurisdictions blocked."
          />
        </Card>
      </div>

      {/* Beyond the redesign's own screens: a regime x year density view
          (goal 094's A4) kept and restyled to the card language rather than
          dropped — the design has no equivalent, but the data is real and
          genuinely useful for spotting backfill gaps. */}
      <Card
        section="A4"
        label="Density"
        title="Records by regime × year"
        rise={0.33}
        className="mt-[16px]"
      >
        <RegimeYearHeatmap cells={coverage.heatmap} regimeLabel={(id) => regimeLabelById.get(id) ?? id} />
        {coverage.heatmap_missing_event_date > 0 && (
          <p style={{ marginTop: 12, fontSize: "11.5px", color: "var(--adm-meta)" }}>
            <span className="adm-num" style={{ fontWeight: 600 }}>
              {formatCount(coverage.heatmap_missing_event_date)}
            </span>{" "}
            Gold record{coverage.heatmap_missing_event_date === 1 ? "" : "s"} carry no{" "}
            <code className="adm-num">event_date</code> and are excluded from this grid.
          </p>
        )}
      </Card>
    </Screen>
  );
}
