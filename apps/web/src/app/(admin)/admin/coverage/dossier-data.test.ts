import { describe, expect, it } from "vitest";

import type {
  AdminFreezeBoardRow,
  AdminHeatmapCell,
  AdminRegimeCoverage,
  AdminRegimeFreshness,
  Regime,
} from "@/lib/api";

import {
  ADAPTER_CRATE_BY_REGIME_CODE,
  adapterCratesFor,
  assembleRegimeDossier,
  buildDossiersById,
  factsFor,
  freshnessNoteFor,
  goldByYearFor,
  integrityNoteFor,
  regimeNoteFor,
  tiersFor,
} from "./dossier-data";

function regime(overrides: Partial<AdminRegimeCoverage> = {}): AdminRegimeCoverage {
  return {
    body: "US House",
    bronze_documents: 100,
    built_not_backfilled: false,
    coverage_phase: "live",
    filings: 40,
    first_filed_date: "2020-01-01",
    gold_records: 30,
    jurisdiction_id: "us",
    jurisdiction_name: "United States",
    last_filed_date: "2026-06-01",
    politicians: 10,
    record_types: ["transaction"],
    regime_codes: ["us_house"],
    regime_id: "reg-us-house",
    silver_rows: 50,
    ...overrides,
  };
}

describe("ADAPTER_CRATE_BY_REGIME_CODE", () => {
  it("maps every real adapter regime code, verified against each adapter's own RegimeRef", () => {
    expect(ADAPTER_CRATE_BY_REGIME_CODE).toEqual({
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
    });
  });

  it("never contains the illustrative-mockup 'br_camara' string", () => {
    expect(Object.keys(ADAPTER_CRATE_BY_REGIME_CODE)).not.toContain("br_camara");
    expect(Object.values(ADAPTER_CRATE_BY_REGIME_CODE)).not.toContain("br_camara");
  });
});

describe("adapterCratesFor", () => {
  it("resolves the eu_fr_de_annual compound crate for all three of its regime codes", () => {
    expect(adapterCratesFor(["eu_parliament_dpi", "fr_hatvp_dia", "de_bundestag"])).toEqual([
      { regimeCode: "eu_parliament_dpi", crate: "eu_fr_de_annual" },
      { regimeCode: "fr_hatvp_dia", crate: "eu_fr_de_annual" },
      { regimeCode: "de_bundestag", crate: "eu_fr_de_annual" },
    ]);
  });

  it("never guesses a crate for an unmapped code — surfaces null instead", () => {
    expect(adapterCratesFor(["not_a_real_code"])).toEqual([
      { regimeCode: "not_a_real_code", crate: null },
    ]);
  });

  it("returns empty for an unbridged regime", () => {
    expect(adapterCratesFor([])).toEqual([]);
  });
});

describe("factsFor", () => {
  it("renders unbridged and missing dates honestly", () => {
    const facts = factsFor(regime({ regime_codes: [], first_filed_date: null, last_filed_date: null }));
    expect(facts).toContainEqual({ label: "bridge code(s)", value: "unbridged" });
    expect(facts).toContainEqual({ label: "first filed", value: "—" });
    expect(facts).toContainEqual({ label: "last filed", value: "—" });
  });

  it("joins bridge codes and record types when present", () => {
    const facts = factsFor(regime({ regime_codes: ["us_house"], record_types: ["transaction", "annual"] }));
    expect(facts).toContainEqual({ label: "bridge code(s)", value: "us_house" });
    expect(facts).toContainEqual({ label: "record types", value: "transaction, annual" });
  });
});

describe("tiersFor", () => {
  it("preserves null for unbridged/no-staging-table tiers instead of coercing to 0", () => {
    const tiers = tiersFor(regime({ bronze_documents: null, silver_rows: null, gold_records: 0 }));
    expect(tiers).toEqual({ bronze: null, silver: null, gold: 0, maxTier: 0 });
  });

  it("computes maxTier across the three tiers", () => {
    const tiers = tiersFor(regime({ bronze_documents: 100, silver_rows: 50, gold_records: 30 }));
    expect(tiers.maxTier).toBe(100);
  });
});

describe("goldByYearFor", () => {
  const cells: AdminHeatmapCell[] = [
    { record_type: "transaction", records: 5, regime_id: "reg-us-house", year: 2020 },
    { record_type: "annual", records: 3, regime_id: "reg-us-house", year: 2020 },
    { record_type: "transaction", records: 7, regime_id: "reg-us-house", year: 2021 },
    { record_type: "transaction", records: 99, regime_id: "reg-other", year: 2020 },
  ];

  it("sums records per year for one regime only, sorted ascending, ignoring other regimes", () => {
    expect(goldByYearFor("reg-us-house", cells)).toEqual([
      { year: "2020", count: 8 },
      { year: "2021", count: 7 },
    ]);
  });

  it("returns empty for a regime with no dated records", () => {
    expect(goldByYearFor("reg-nothing", cells)).toEqual([]);
  });
});

describe("integrityNoteFor", () => {
  const freezeBoard: AdminFreezeBoardRow[] = [
    {
      frozen: false,
      last_checked_at: "2026-07-01T00:00:00Z",
      open_drift_count: 2,
      regime_code: "us_house",
    },
    {
      frozen: true,
      frozen_at: "2026-06-01T00:00:00Z",
      frozen_kind: "layout_shift",
      last_checked_at: "2026-07-01T00:00:00Z",
      open_drift_count: 1,
      regime_code: "br",
    },
  ];

  it("reports frozen with its kind when any bridged code is frozen", () => {
    expect(integrityNoteFor(["br"], freezeBoard)).toBe(
      "Frozen — publication halted (layout_shift). See Pipeline for detail.",
    );
  });

  it("reports open drift count when not frozen", () => {
    expect(integrityNoteFor(["us_house"], freezeBoard)).toBe("Not frozen; 2 open drift reports.");
  });

  it("is honest about no freeze-board row existing (unbridged or never run)", () => {
    expect(integrityNoteFor(["nonexistent"], freezeBoard)).toBe(
      "No freeze-board row for this regime's adapter(s) — not yet run, or unbridged.",
    );
  });
});

describe("freshnessNoteFor", () => {
  const freshness: AdminRegimeFreshness[] = [
    { lag_p50_seconds: 3600, lag_p90_seconds: 90000, regime_code: "us_house" },
    { lag_p50_seconds: null, lag_p90_seconds: null, regime_code: "br" },
  ];

  it("formats p50/p90 lag in human units", () => {
    expect(freshnessNoteFor(["us_house"], freshness)).toBe("Discovery lag p50 1.0h, p90 1.0d.");
  });

  it("is honest when no filing carries a published_at", () => {
    expect(freshnessNoteFor(["br"], freshness)).toBe(
      "No filing under this regime carries a published_at yet — discovery lag not computable.",
    );
  });

  it("is honest when there is no freshness row at all", () => {
    expect(freshnessNoteFor(["nonexistent"], freshness)).toBe(
      "No freshness data for this regime's adapter(s) — not yet run, or unbridged.",
    );
  });
});

describe("regimeNoteFor", () => {
  function regimeDetail(overrides: Partial<Regime> = {}): Regime {
    return {
      body: "US House",
      effective_from: "2012-01-01",
      id: "reg-us-house",
      jurisdiction_id: "us",
      regime_type: "transaction_report",
      value_precision: "banded",
      ...overrides,
    };
  }

  it("returns null (no fabricated note) when no Regime row joins", () => {
    expect(regimeNoteFor(undefined)).toBeNull();
  });

  it("synthesizes a sentence from only the fields the regime actually carries", () => {
    expect(
      regimeNoteFor(regimeDetail({ cadence: "within 45 days of trade", disclosure_lag_days: 45 })),
    ).toBe("transaction_report regime, banded values, within 45 days of trade, filed within 45 days.");
  });

  it("omits cadence/lag clauses when the regime doesn't carry them", () => {
    expect(regimeNoteFor(regimeDetail({ cadence: null, disclosure_lag_days: null }))).toBe(
      "transaction_report regime, banded values.",
    );
  });
});

describe("assembleRegimeDossier / buildDossiersById", () => {
  it("assembles a full dossier for a bridged, non-frozen regime with dated Gold records", () => {
    const row = regime();
    const cells: AdminHeatmapCell[] = [
      { record_type: "transaction", records: 30, regime_id: "reg-us-house", year: 2025 },
    ];
    const freezeBoard: AdminFreezeBoardRow[] = [
      { frozen: false, last_checked_at: "2026-07-01T00:00:00Z", open_drift_count: 0, regime_code: "us_house" },
    ];
    const freshness: AdminRegimeFreshness[] = [
      { lag_p50_seconds: 3600, lag_p90_seconds: 7200, regime_code: "us_house" },
    ];
    const detail: Regime = {
      body: "US House",
      effective_from: "2012-01-01",
      id: "reg-us-house",
      jurisdiction_id: "us",
      regime_type: "transaction_report",
      value_precision: "banded",
    };

    const dossier = assembleRegimeDossier(row, cells, freezeBoard, freshness, detail);

    expect(dossier.regimeId).toBe("reg-us-house");
    expect(dossier.title).toBe("US House — United States");
    expect(dossier.adapterCrates).toEqual([{ regimeCode: "us_house", crate: "us_house" }]);
    expect(dossier.goldByYear).toEqual([{ year: "2025", count: 30 }]);
    expect(dossier.integrityNote).toBe("Not frozen; no open drift reports.");
    expect(dossier.regimeNote).toBe("transaction_report regime, banded values.");
    expect(dossier.politenessNote).toMatch(/not observable from here/);
  });

  it("keys one dossier per coverage row by regime_id", () => {
    const coverage = { heatmap: [] as AdminHeatmapCell[], regimes: [regime(), regime({ regime_id: "reg-2", body: "US Senate", regime_codes: ["us_senate"] })] };
    const backfill = { freshness: [] as AdminRegimeFreshness[] };
    const pipeline = { freeze_board: [] as AdminFreezeBoardRow[] };

    const dossiers = buildDossiersById(coverage, backfill, pipeline, []);

    expect(Object.keys(dossiers).sort()).toEqual(["reg-2", "reg-us-house"]);
    expect(dossiers["reg-2"]?.title).toBe("US Senate — United States");
    expect(dossiers["reg-us-house"]?.regimeNote).toBeNull();
  });
});
