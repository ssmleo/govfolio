import Link from "next/link";

import type { DisclosureRecord } from "@/lib/api";
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
        {records.map((record) => (
          <RecordRow key={record.id} record={record} />
        ))}
      </tbody>
    </table>
  );
}
