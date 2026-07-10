import Link from "next/link";
import { notFound } from "next/navigation";

import type { ReviewAuditEntry, ReviewTaskDetail } from "@/lib/api";
import { ApiError, getReviewTask, reviewTaskAudit } from "@/lib/api";
import { formatDateTime } from "@/lib/format";
import { AccessNotice } from "@/components/reviewer/AccessNotice";
import { AuditLog } from "@/components/reviewer/AuditLog";
import { BronzeDocument } from "@/components/reviewer/BronzeDocument";
import { PreReviewNote } from "@/components/reviewer/PreReviewNote";
import { ResolvePanel } from "@/components/reviewer/ResolvePanel";
import { TaskFields } from "@/components/reviewer/TaskFields";
import { ProvenanceBlock } from "@/components/ProvenanceBlock";
import { SupersessionChain } from "@/components/SupersessionChain";

import { resolveTaskAction } from "./actions";

// One review task: extracted fields beside the Bronze document, the
// pre-review note, the resolve actions, and the full audit log (design §7.2).
// Always fresh — adjudication state must never be stale.
export const dynamic = "force-dynamic";

interface Params {
  params: Promise<{ id: string }>;
}

async function fetchTaskOr404(
  id: string,
): Promise<{ detail: ReviewTaskDetail; audit: ReviewAuditEntry[] }> {
  try {
    const [detail, audit] = await Promise.all([getReviewTask(id), reviewTaskAudit(id)]);
    return { detail, audit };
  } catch (error) {
    if (error instanceof ApiError && (error.status === 404 || error.status === 400)) {
      notFound();
    }
    throw error;
  }
}

export default async function ReviewTaskPage({ params }: Params) {
  const { id } = await params;
  let loaded: { detail: ReviewTaskDetail; audit: ReviewAuditEntry[] };
  try {
    loaded = await fetchTaskOr404(id);
  } catch (error) {
    // Admin gate (goal 050): surface the API's 401/403 envelope honestly
    // instead of a generic 500 — no fake task state.
    if (error instanceof ApiError && (error.status === 401 || error.status === 403)) {
      return <AccessNotice error={error} />;
    }
    throw error;
  }
  const { detail, audit } = loaded;
  const { task, record, extraction } = detail;

  return (
    <>
      <section className="record-head">
        <p className="kind">
          <Link href="/review">← Review queue</Link>
        </p>
        <h1>{task.reason}</h1>
        <p className="task-meta">
          <span className={`badge badge-task-${task.status}`} data-status={task.status}>
            {task.status}
          </span>{" "}
          <span className="muted">
            priority <span className="mono">{task.priority_score}</span> · opened{" "}
            {formatDateTime(task.created_at)}
            {task.resolved_at ? ` · resolved ${formatDateTime(task.resolved_at)}` : null}
            {task.assignee ? ` · assignee ${task.assignee}` : null}
          </span>
        </p>
        <p className="muted mono">task {task.id}</p>
      </section>

      {record ? (
        <div className="review-split">
          <div className="review-left">
            <TaskFields record={record.record} />
            {record.supersedes.length > 0 || record.superseded_by.length > 0 ? (
              <section aria-label="Correction history">
                <h2>Correction history</h2>
                <SupersessionChain
                  supersedes={record.supersedes}
                  supersededBy={record.superseded_by}
                />
              </section>
            ) : null}
          </div>
          <div className="review-right">
            <BronzeDocument provenance={record.provenance} />
            <ProvenanceBlock provenance={record.provenance} />
          </div>
        </div>
      ) : (
        <p className="muted">
          This task targets {task.target_kind}{" "}
          <span className="mono">{task.target_id}</span> — no disclosure record to
          display side by side.
        </p>
      )}

      <PreReviewNote extraction={extraction ?? null} />

      <ResolvePanel
        taskId={task.id}
        status={task.status}
        targetKind={task.target_kind}
        targetId={task.target_id}
        record={record?.record ?? null}
        action={resolveTaskAction}
      />

      <AuditLog entries={audit} />
    </>
  );
}
