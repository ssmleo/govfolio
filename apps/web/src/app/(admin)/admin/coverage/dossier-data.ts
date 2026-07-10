// Pure data assembly for the Regime Dossier slide-over (goal 094, Task 6). No
// JSX, no rendering — everything here takes the admin DTOs already fetched by
// `page.tsx` (`AdminCoverage`, `AdminBackfill`, `AdminPipeline`, `Regime[]`)
// and returns plain, JSON-serializable data for `RegimeDossier.tsx` to render.
// Fully unit-testable in isolation from React.

import type {
  AdminBackfill,
  AdminCoverage,
  AdminFreezeBoardRow,
  AdminHeatmapCell,
  AdminPipeline,
  AdminRegimeCoverage,
  AdminRegimeFreshness,
  Regime,
} from "@/lib/api";

/**
 * Adapter regime code -> crate directory name (`crates/adapters/<crate>`).
 * Verified from each adapter's own `RegimeRef` (grepped, not assumed):
 *   - `crates/adapters/br/src/adapter.rs` -> `RegimeRef { code: "br" }`
 *   - `crates/adapters/us_house/src/adapter.rs` -> `code: "us_house"`
 *   - `crates/adapters/us_senate/src/adapter.rs` -> `code: "us_senate"`
 *   - `crates/adapters/uk_commons_register/src/adapter.rs` -> `code: "uk_commons_register"`
 *   - `crates/adapters/canada_ciec/src/adapter.rs` -> `code: "canada_ciec"`
 *   - `crates/adapters/australia_register/src/adapter.rs` -> `code: "australia_register"`
 *   - `crates/adapters/fixture_fake/src/lib.rs` -> `code: "fixture_fake"`
 *   - `crates/adapters/eu_fr_de_annual/src/lib.rs` is ONE crate dispatching to
 *     three sub-adapters by detected source; its three regime codes come from
 *     `eu.rs`/`fr.rs`/`de.rs`'s own `const REGIME`: `eu_parliament_dpi`,
 *     `fr_hatvp_dia`, `de_bundestag`. There is no `br_camara` regime code
 *     anywhere in the adapter source — that string never appeared outside an
 *     illustrative mockup and is deliberately absent from this map.
 */
export const ADAPTER_CRATE_BY_REGIME_CODE: Readonly<Record<string, string>> = {
  fixture_fake: "fixture_fake",
  us_house: "us_house",
  us_senate: "us_senate",
  uk_commons_register: "uk_commons_register",
  canada_ciec: "canada_ciec",
  australia_register: "australia_register",
  eu_parliament_dpi: "eu_fr_de_annual",
  fr_hatvp_dia: "eu_fr_de_annual",
  de_bundestag: "eu_fr_de_annual",
  br: "br",
};

/** House copy for a fact the dashboard genuinely cannot compute from stored data. */
export const POLITENESS_NOTE =
  "Per-source politeness (crawl-delay, contact UA, concurrency) lives in the adapter crate's own `politeness()`, not in any stored fact — not observable from here.";

export interface DossierFact {
  label: string;
  value: string;
}

export interface TierComposition {
  bronze: number | null;
  silver: number | null;
  gold: number;
  /** Largest of the three (nulls treated as 0) — the bar-scaling denominator. */
  maxTier: number;
}

export interface GoldYearPoint extends Record<string, unknown> {
  /** Calendar year as a label (`Histogram`/`CountsBar` both key on a string category). */
  year: string;
  count: number;
}

/** One bridged adapter regime code, and the crate it maps to (`null` = unmapped — never guessed). */
export interface AdapterCrateRef {
  regimeCode: string;
  crate: string | null;
}

export interface RegimeDossierData {
  regimeId: string;
  /** `"{body} — {jurisdiction_name}"`, for the slide-over header. */
  title: string;
  regimeCodes: readonly string[];
  facts: DossierFact[];
  tiers: TierComposition;
  goldByYear: GoldYearPoint[];
  adapterCrates: AdapterCrateRef[];
  integrityNote: string;
  freshnessNote: string;
  /** `null` when no `Regime` row joins this coverage row (no fabricated note). */
  regimeNote: string | null;
  politenessNote: string;
}

export function factsFor(regime: AdminRegimeCoverage): DossierFact[] {
  return [
    { label: "jurisdiction", value: `${regime.jurisdiction_name} (${regime.jurisdiction_id})` },
    { label: "phase", value: regime.coverage_phase },
    {
      label: "bridge code(s)",
      value: regime.regime_codes.length > 0 ? regime.regime_codes.join(", ") : "unbridged",
    },
    { label: "politicians", value: regime.politicians.toLocaleString() },
    { label: "filings", value: regime.filings.toLocaleString() },
    { label: "first filed", value: regime.first_filed_date ?? "—" },
    { label: "last filed", value: regime.last_filed_date ?? "—" },
    {
      label: "record types",
      value: regime.record_types.length > 0 ? regime.record_types.join(", ") : "—",
    },
  ];
}

export function tiersFor(regime: AdminRegimeCoverage): TierComposition {
  const bronze = regime.bronze_documents ?? null;
  const silver = regime.silver_rows ?? null;
  const gold = regime.gold_records;
  return { bronze, silver, gold, maxTier: Math.max(bronze ?? 0, silver ?? 0, gold) };
}

/**
 * Gold records by year for one regime — same grouping approach as
 * `RegimeYearHeatmap.tsx` (filter by regime, sum `records` per `year`), just
 * collapsed to a single series instead of a regime x year grid.
 */
export function goldByYearFor(
  regimeId: string,
  heatmap: readonly AdminHeatmapCell[],
): GoldYearPoint[] {
  const totals = new Map<number, number>();
  for (const cell of heatmap) {
    if (cell.regime_id !== regimeId) continue;
    totals.set(cell.year, (totals.get(cell.year) ?? 0) + cell.records);
  }
  return [...totals.entries()]
    .sort(([a], [b]) => a - b)
    .map(([year, count]) => ({ year: String(year), count }));
}

export function adapterCratesFor(regimeCodes: readonly string[]): AdapterCrateRef[] {
  return regimeCodes.map((regimeCode) => ({
    regimeCode,
    crate: ADAPTER_CRATE_BY_REGIME_CODE[regimeCode] ?? null,
  }));
}

export function integrityNoteFor(
  regimeCodes: readonly string[],
  freezeBoard: readonly AdminFreezeBoardRow[],
): string {
  const rows = freezeBoard.filter((row) => regimeCodes.includes(row.regime_code));
  if (rows.length === 0) {
    return "No freeze-board row for this regime's adapter(s) — not yet run, or unbridged.";
  }
  const frozen = rows.filter((row) => row.frozen);
  if (frozen.length > 0) {
    const kinds = [...new Set(frozen.map((row) => row.frozen_kind ?? "unknown"))].join(", ");
    return `Frozen — publication halted (${kinds}). See Pipeline for detail.`;
  }
  const openDrift = rows.reduce((sum, row) => sum + row.open_drift_count, 0);
  if (openDrift > 0) {
    return `Not frozen; ${openDrift} open drift report${openDrift === 1 ? "" : "s"}.`;
  }
  return "Not frozen; no open drift reports.";
}

function formatLagSeconds(seconds: number): string {
  const days = seconds / 86_400;
  if (days >= 1) return `${days.toFixed(1)}d`;
  const hours = seconds / 3600;
  if (hours >= 1) return `${hours.toFixed(1)}h`;
  return `${Math.round(seconds)}s`;
}

export function freshnessNoteFor(
  regimeCodes: readonly string[],
  freshness: readonly AdminRegimeFreshness[],
): string {
  const rows = freshness.filter((row) => regimeCodes.includes(row.regime_code));
  if (rows.length === 0) {
    return "No freshness data for this regime's adapter(s) — not yet run, or unbridged.";
  }
  const p50s = rows.map((row) => row.lag_p50_seconds).filter((v): v is number => v != null);
  if (p50s.length === 0) {
    return "No filing under this regime carries a published_at yet — discovery lag not computable.";
  }
  const p90s = rows.map((row) => row.lag_p90_seconds).filter((v): v is number => v != null);
  const p50Text = formatLagSeconds(Math.max(...p50s));
  const p90Text = p90s.length > 0 ? `, p90 ${formatLagSeconds(Math.max(...p90s))}` : "";
  return `Discovery lag p50 ${p50Text}${p90Text}.`;
}

export function regimeNoteFor(regime: Regime | undefined): string | null {
  if (regime === undefined) return null;
  const parts: string[] = [`${regime.regime_type} regime`, `${regime.value_precision} values`];
  if (regime.cadence) parts.push(regime.cadence);
  if (regime.disclosure_lag_days != null) {
    parts.push(`filed within ${regime.disclosure_lag_days} day${regime.disclosure_lag_days === 1 ? "" : "s"}`);
  }
  return `${parts.join(", ")}.`;
}

export function assembleRegimeDossier(
  regime: AdminRegimeCoverage,
  heatmap: readonly AdminHeatmapCell[],
  freezeBoard: readonly AdminFreezeBoardRow[],
  freshness: readonly AdminRegimeFreshness[],
  regimeDetail: Regime | undefined,
): RegimeDossierData {
  return {
    regimeId: regime.regime_id,
    title: `${regime.body} — ${regime.jurisdiction_name}`,
    regimeCodes: regime.regime_codes,
    facts: factsFor(regime),
    tiers: tiersFor(regime),
    goldByYear: goldByYearFor(regime.regime_id, heatmap),
    adapterCrates: adapterCratesFor(regime.regime_codes),
    integrityNote: integrityNoteFor(regime.regime_codes, freezeBoard),
    freshnessNote: freshnessNoteFor(regime.regime_codes, freshness),
    regimeNote: regimeNoteFor(regimeDetail),
    politenessNote: POLITENESS_NOTE,
  };
}

/** Builds one dossier per coverage row, keyed by `regime_id`, for O(1) lookup on row click. */
export function buildDossiersById(
  coverage: Pick<AdminCoverage, "regimes" | "heatmap">,
  backfill: Pick<AdminBackfill, "freshness">,
  pipeline: Pick<AdminPipeline, "freeze_board">,
  regimes: readonly Regime[],
): Record<string, RegimeDossierData> {
  const regimesById = new Map(regimes.map((regime) => [regime.id, regime] as const));
  const result: Record<string, RegimeDossierData> = {};
  for (const row of coverage.regimes) {
    result[row.regime_id] = assembleRegimeDossier(
      row,
      coverage.heatmap,
      pipeline.freeze_board,
      backfill.freshness,
      regimesById.get(row.regime_id),
    );
  }
  return result;
}
