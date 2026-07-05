import Link from "next/link";

import type { ReviewAuditEntry } from "@/lib/api";
import { formatDateTime } from "@/lib/format";

// Every resolve attempt, oldest first, exactly as the API records it —
// applied, conflicting and failed alike (all actions audit-logged, §7.2).
export function AuditLog({ entries }: { entries: ReviewAuditEntry[] }) {
  return (
    <section className="audit-log" aria-label="Audit log">
      <h2>Audit log</h2>
      {entries.length === 0 ? (
        <p className="empty">No resolve attempts yet.</p>
      ) : (
        <div className="table-scroll">
          <table>
            <caption className="visually-hidden">
              Resolve attempts for this task, oldest first
            </caption>
            <thead>
              <tr>
                <th scope="col">When</th>
                <th scope="col">Reviewer</th>
                <th scope="col">Verdict</th>
                <th scope="col">Outcome</th>
                <th scope="col">Note</th>
                <th scope="col">Affected records</th>
              </tr>
            </thead>
            <tbody>
              {entries.map((entry) => (
                <tr key={entry.id} className="audit-row">
                  <td className="cell-date">{formatDateTime(entry.created_at)}</td>
                  <td>{entry.reviewer}</td>
                  <td className="cell-action">{entry.verdict}</td>
                  <td data-outcome={entry.outcome}>{entry.outcome}</td>
                  <td>{entry.note ?? "—"}</td>
                  <td>
                    {entry.affected_record_ids.length === 0 ? (
                      <span className="muted">none</span>
                    ) : (
                      entry.affected_record_ids.map((recordId, index) => (
                        <span key={recordId}>
                          {index > 0 ? ", " : null}
                          <Link className="mono" href={`/r/${recordId}`}>
                            {recordId}
                          </Link>
                        </span>
                      ))
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
