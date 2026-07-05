// Typed test fixtures built from the GENERATED contract types.
import type { DisclosureRecord, Provenance } from "@/lib/api";

export function makeRecord(overrides: Partial<DisclosureRecord> = {}): DisclosureRecord {
  return {
    id: "01KWQVPG6B08S4VX92NZED3C16",
    filing_id: "01KWQVPG5YKE014G57NX4PSRNG",
    politician_id: "01KWQVPG4GWCBFCRAY26F9Z15Y",
    regime_id: "0HSEREG0000000000000000001",
    instrument_id: null,
    asset_description_raw: "Boeing Company (BA) [ST]",
    record_type: "transaction",
    asset_class: "equity",
    side: "sell",
    transaction_date: "2025-12-09",
    as_of_date: null,
    notified_date: "2025-12-09",
    event_date: "2025-12-09",
    value: { low: "1001.00", high: "15000.00", currency: "USD" },
    owner: "self",
    verification_state: "unverified",
    extraction_confidence: 0.98,
    extracted_by: "us_house_ptr/text@1",
    fingerprint: "5baf2a7463dbcd53acd42068477586ca9a46d090a48ed9b4d39dad219a420900",
    supersedes_record_id: null,
    details: {},
    created_at: "2026-07-05T00:43:48.798177Z",
    ...overrides,
  };
}

export function makeProvenance(overrides: Partial<Provenance> = {}): Provenance {
  return {
    filing: {
      id: "01KWQVPG5YKE014G57NX4PSRNG",
      external_id: "20033759",
      filed_date: "2026-01-07",
      published_at: "2026-01-08T15:00:00Z",
    },
    raw_document: {
      id: "01KWQVPG5Y0000000000000000",
      sha256: "94781947c3975677a2fa8f7839f6c0f074b3d3a2ff6019b3cfd8ee4942f6262e",
      fetched_at: "2026-07-05T00:43:48Z",
      source_url:
        "https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2026/20033759.pdf",
    },
    regime: {
      id: "0HSEREG0000000000000000001",
      jurisdiction_id: "us",
      body: "US House",
      regime_type: "transaction_report",
      value_precision: "banded",
      cadence: "rolling; statutory <=30d from notification, <=45d from transaction",
      disclosure_lag_days: 45,
      source_url: "https://disclosures-clerk.house.gov/FinancialDisclosure",
      effective_from: "2012-04-04",
      effective_to: null,
    },
    ...overrides,
  };
}
