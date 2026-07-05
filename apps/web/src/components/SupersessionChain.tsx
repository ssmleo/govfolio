import Link from "next/link";

import type { DisclosureRecord } from "@/lib/api";
import { formatDateTime } from "@/lib/format";
import { VerificationBadge } from "@/components/VerificationBadge";

// Supersession history in both directions (invariant 1: supersede, never
// update — corrections are new rows, so history is a chain of records).
export function SupersessionChain({
  supersedes,
  supersededBy,
}: {
  supersedes: DisclosureRecord[];
  supersededBy: DisclosureRecord[];
}) {
  if (supersedes.length === 0 && supersededBy.length === 0) {
    return (
      <p className="muted" data-testid="no-supersession">
        No corrections on file: this is the only version of this record.
      </p>
    );
  }
  return (
    <div className="supersession">
      {supersededBy.length > 0 ? (
        <div className="supersession-arm" data-testid="superseded-by">
          <h3>Superseded by</h3>
          <ol>
            {supersededBy.map((record) => (
              <li key={record.id}>
                <Link href={`/r/${record.id}`}>{record.asset_description_raw}</Link>{" "}
                <VerificationBadge state={record.verification_state} />{" "}
                <span className="muted">inserted {formatDateTime(record.created_at)}</span>
              </li>
            ))}
          </ol>
        </div>
      ) : null}
      {supersedes.length > 0 ? (
        <div className="supersession-arm" data-testid="supersedes">
          <h3>Supersedes</h3>
          <ol>
            {supersedes.map((record) => (
              <li key={record.id}>
                <Link href={`/r/${record.id}`}>{record.asset_description_raw}</Link>{" "}
                <VerificationBadge state={record.verification_state} />{" "}
                <span className="muted">inserted {formatDateTime(record.created_at)}</span>
              </li>
            ))}
          </ol>
        </div>
      ) : null}
    </div>
  );
}
