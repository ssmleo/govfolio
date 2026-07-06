import Link from "next/link";

import type { DisclosureRecord } from "@/lib/api";
import { formatDate, formatDateTime, formatValueInterval } from "@/lib/format";
import { VerificationBadge } from "@/components/VerificationBadge";

// A published correction (verification_state = 'corrected') alongside the
// earlier record it supersedes (invariant 1: corrections insert a new row, so
// the original is never overwritten — it stays on file and is linked here).
export interface CorrectionItem {
  correction: DisclosureRecord;
  /** The immediate predecessor this correction supersedes; `null` if not visible. */
  superseded: DisclosureRecord | null;
}

interface FieldChange {
  label: string;
  before: string;
  after: string;
}

// The display fields a reader can compare at a glance. Only fields that
// actually differ are shown — the log states what changed, nothing more.
function changedFields(before: DisclosureRecord, after: DisclosureRecord): FieldChange[] {
  const changes: FieldChange[] = [];

  if (before.asset_description_raw !== after.asset_description_raw) {
    changes.push({
      label: "Asset as filed",
      before: before.asset_description_raw,
      after: after.asset_description_raw,
    });
  }

  const beforeValue = before.value ? formatValueInterval(before.value) : "Not declared";
  const afterValue = after.value ? formatValueInterval(after.value) : "Not declared";
  if (beforeValue !== afterValue) {
    changes.push({ label: "Declared value", before: beforeValue, after: afterValue });
  }

  const beforeDate = before.event_date ? formatDate(before.event_date) : "Not declared";
  const afterDate = after.event_date ? formatDate(after.event_date) : "Not declared";
  if (beforeDate !== afterDate) {
    changes.push({ label: "Event date", before: beforeDate, after: afterDate });
  }

  return changes;
}

function CorrectionDiff({ item }: { item: CorrectionItem }) {
  if (!item.superseded) {
    return null;
  }
  const changes = changedFields(item.superseded, item.correction);
  if (changes.length === 0) {
    return (
      <p className="muted">
        Regime detail was corrected; open the record for the full comparison.
      </p>
    );
  }
  return (
    <table className="correction-diff">
      <caption className="visually-hidden">What this correction changed</caption>
      <thead>
        <tr>
          <th scope="col">Field</th>
          <th scope="col">Before</th>
          <th scope="col">After</th>
        </tr>
      </thead>
      <tbody>
        {changes.map((change) => (
          <tr key={change.label}>
            <th scope="row">{change.label}</th>
            <td className="diff-before">{change.before}</td>
            <td className="diff-after">{change.after}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export function CorrectionsLog({ items }: { items: CorrectionItem[] }) {
  if (items.length === 0) {
    return (
      <p className="empty" data-testid="corrections-empty">
        No corrections on record. When a published record is corrected, the new version
        and the original it supersedes both appear here.
      </p>
    );
  }
  return (
    <ol className="corrections-log">
      {items.map(({ correction, superseded }) => (
        <li key={correction.id} className="correction" data-testid="correction-entry">
          <div className="correction-head">
            <Link href={`/r/${correction.id}`} className="correction-asset">
              {correction.asset_description_raw}
            </Link>
            <VerificationBadge state={correction.verification_state} />
          </div>
          <p className="correction-when muted">
            Correction recorded {formatDateTime(correction.created_at)}
          </p>
          <CorrectionDiff item={{ correction, superseded }} />
          <p className="correction-links">
            {superseded ? (
              <Link href={`/r/${superseded.id}`} data-testid="superseded-link">
                See the earlier record it supersedes
              </Link>
            ) : (
              <Link href={`/r/${correction.id}`}>See the full correction history</Link>
            )}
          </p>
        </li>
      ))}
    </ol>
  );
}
