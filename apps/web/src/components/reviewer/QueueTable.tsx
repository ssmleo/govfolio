import Link from "next/link";

import type { ReviewQueueItem } from "@/lib/api";
import { formatAge, formatConfidence, formatValueInterval } from "@/lib/format";
import { VerificationBadge } from "@/components/VerificationBadge";

// The review queue (design §7.2), in the API's ranking order VERBATIM —
// priority_score desc, then created_at asc, then id. This component never
// re-sorts: the ranking is the API's contract, not the UI's opinion.
export function QueueTable({ items, now }: { items: ReviewQueueItem[]; now: Date }) {
  if (items.length === 0) {
    return <p className="empty">No review tasks in this view.</p>;
  }
  return (
    <div className="table-scroll">
      <table className="queue">
        <caption className="visually-hidden">
          Review tasks in ranking order (priority desc, then age)
        </caption>
        <thead>
          <tr>
            <th scope="col">Priority</th>
            <th scope="col">Reason</th>
            <th scope="col">Target record</th>
            <th scope="col">Confidence</th>
            <th scope="col">Extracted by</th>
            <th scope="col">Age</th>
          </tr>
        </thead>
        <tbody>
          {items.map(({ task, record }) => (
            <tr key={task.id} className="queue-row">
              <td className="cell-priority mono">{task.priority_score}</td>
              <td className="cell-reason">
                <Link href={`/review/${task.id}`}>{task.reason}</Link>
              </td>
              <td className="cell-target">
                {record ? (
                  <>
                    <span className="target-politician">{record.politician_name}</span>
                    {" — "}
                    {record.asset_description_raw}
                    {record.value ? (
                      <span className="muted"> · {formatValueInterval(record.value)}</span>
                    ) : null}{" "}
                    <VerificationBadge state={record.verification_state} />
                  </>
                ) : (
                  <span className="muted">
                    {task.target_kind} {task.target_id} (not a disclosure record)
                  </span>
                )}
              </td>
              <td className="cell-confidence">
                {record?.extraction_confidence != null
                  ? formatConfidence(record.extraction_confidence)
                  : "—"}
              </td>
              <td className="cell-extractor mono">{record?.extracted_by ?? "—"}</td>
              <td className="cell-age">
                <time dateTime={task.created_at}>{formatAge(task.created_at, now)}</time>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
