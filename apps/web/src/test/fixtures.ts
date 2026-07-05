// Typed test fixtures built from the GENERATED contract types.
import type {
  DisclosureRecord,
  ExtractionContext,
  Provenance,
  ReviewAuditEntry,
  ReviewQueueItem,
  ReviewTask,
  ReviewTargetSummary,
  RecordDetail,
} from "@/lib/api";

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

export function makeRecordDetail(overrides: Partial<RecordDetail> = {}): RecordDetail {
  return {
    record: makeRecord(),
    provenance: makeProvenance(),
    supersedes: [],
    superseded_by: [],
    ...overrides,
  };
}

export function makeTask(overrides: Partial<ReviewTask> = {}): ReviewTask {
  return {
    id: "01KWRTASK0000000000000001A",
    target_kind: "disclosure_record",
    target_id: "01KWQVPG6B08S4VX92NZED3C16",
    reason: "ptr_amendment_unlinked",
    priority_score: 4.5,
    status: "open",
    assignee: null,
    resolution: null,
    created_at: "2026-07-04T22:00:00Z",
    resolved_at: null,
    ...overrides,
  };
}

export function makeTargetSummary(
  overrides: Partial<ReviewTargetSummary> = {},
): ReviewTargetSummary {
  return {
    record_id: "01KWQVPG6B08S4VX92NZED3C16",
    asset_description_raw: "Boeing Company (BA) [ST]",
    politician_name: "David Rouzer",
    record_type: "transaction",
    value: { low: "1001.00", high: "15000.00", currency: "USD" },
    verification_state: "unverified",
    extraction_confidence: 0.98,
    extracted_by: "us_house_ptr/text@1",
    ...overrides,
  };
}

export function makeQueueItem(overrides: Partial<ReviewQueueItem> = {}): ReviewQueueItem {
  return {
    task: makeTask(),
    record: makeTargetSummary(),
    ...overrides,
  };
}

export function makeExtraction(
  overrides: Partial<ExtractionContext> = {},
): ExtractionContext {
  return {
    extracted_by: "us_house_ptr/llm@1",
    extraction_confidence: 0.83,
    cache: {
      model_id: "model-a-2026",
      cached_at: "2026-07-04T21:00:00Z",
      // The provenance payload is contract-opaque (serde_json::Value on the
      // wire); tests feed a realistic cross-check shape through the same
      // narrow door the API uses.
      provenance: JSON.parse(
        '{"source":"live","cross_checked":"agree","models":["model-a-2026","model-b-2026"]}',
      ) as NonNullable<ExtractionContext["cache"]>["provenance"],
    },
    ...overrides,
  };
}

export function makeAuditEntry(overrides: Partial<ReviewAuditEntry> = {}): ReviewAuditEntry {
  return {
    id: "01KWRAUDIT000000000000001A",
    review_task_id: "01KWRTASK0000000000000001A",
    reviewer: "reviewer-jane",
    verdict: "confirm",
    outcome: "applied",
    note: "matches the source document",
    affected_record_ids: ["01KWQVPG6B08S4VX92NZED3C16"],
    created_at: "2026-07-04T23:00:00Z",
    ...overrides,
  };
}
