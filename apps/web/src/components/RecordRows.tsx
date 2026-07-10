import { Fragment } from "react";

import Link from "next/link";

import type { DisclosureRecord } from "@/lib/api";
import { apiBaseUrl } from "@/lib/api";
import { formatDate, formatValueInterval } from "@/lib/format";
import { VerificationBadge } from "@/components/VerificationBadge";

// One record as a ledger row: date · action · asset (as filed) · value · state.
// Neutral as-filed language throughout (design §7.5).
function actionLabel(record: DisclosureRecord): string {
  if (record.record_type === "transaction") {
    return record.side ?? "transaction";
  }
  return record.record_type.replaceAll("_", " ");
}

// Brazil's archived document is a per-candidate JOIN of two nationwide TSE
// bulk files (crates/adapters/br/src/adapter.rs) — real, but a
// reconstruction, not verbatim government-issued single-candidate bytes.
// These are BR's two regime ids (crates/adapters/br/src/seed.rs REGIME_ID /
// REGIME_ID_SENADO). This is a deliberate, narrow exception to "no
// per-jurisdiction branching" — the underlying data genuinely differs in
// kind here, not just in presentation.
const BULK_RECONSTRUCTED_REGIME_IDS = new Set([
  "0BRAREG0000000000000000001",
  "0BRAREG0000000000000000002",
]);

function filingDocumentUrl(filingId: string): string {
  return `${apiBaseUrl()}/v1/filings/${encodeURIComponent(filingId)}/document`;
}

function FilingGroupHeader({ record }: { record: DisclosureRecord }) {
  return (
    <tr className="filing-group-header">
      <td colSpan={5}>
        <a
          href={filingDocumentUrl(record.filing_id)}
          target="_blank"
          rel="noopener noreferrer"
        >
          View filing
        </a>
        {BULK_RECONSTRUCTED_REGIME_IDS.has(record.regime_id) ? (
          <span className="muted">
            {" "}
            (reconstructed per-candidate from TSE&apos;s bulk disclosure files)
          </span>
        ) : null}
      </td>
    </tr>
  );
}

export function RecordRow({ record }: { record: DisclosureRecord }) {
  return (
    <tr className="record-row">
      <td className="cell-date">
        {record.event_date ? formatDate(record.event_date) : "—"}
      </td>
      <td className="cell-action">{actionLabel(record)}</td>
      <td className="cell-asset">
        <Link href={`/r/${record.id}`}>{record.asset_description_raw}</Link>
      </td>
      <td className="cell-value">
        {record.value ? formatValueInterval(record.value) : "—"}
      </td>
      <td className="cell-state">
        <VerificationBadge state={record.verification_state} />
      </td>
    </tr>
  );
}

export function RecordTable({
  records,
  caption,
}: {
  records: DisclosureRecord[];
  caption: string;
}) {
  if (records.length === 0) {
    return <p className="empty">No disclosure records yet for this view.</p>;
  }
  let previousFilingId: string | null = null;
  return (
    <table className="records">
      <caption className="visually-hidden">{caption}</caption>
      <thead>
        <tr>
          <th scope="col">Date</th>
          <th scope="col">Action</th>
          <th scope="col">Asset as filed</th>
          <th scope="col">Declared value</th>
          <th scope="col">State</th>
        </tr>
      </thead>
      <tbody>
        {records.map((record) => {
          const isNewFilingGroup = record.filing_id !== previousFilingId;
          previousFilingId = record.filing_id;
          return (
            <Fragment key={record.id}>
              {isNewFilingGroup ? (
                <FilingGroupHeader key={`${record.filing_id}-header`} record={record} />
              ) : null}
              <RecordRow key={record.id} record={record} />
            </Fragment>
          );
        })}
      </tbody>
    </table>
  );
}
