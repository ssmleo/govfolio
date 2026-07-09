import type { AdminLoop, AdminLoopGoal } from "@/lib/api";
import { ApiError, adminLoop } from "@/lib/api";
import { Card } from "@/components/admin/ui/Card";
import { Stat } from "@/components/admin/ui/Stat";
import { Badge, stateVariant } from "@/components/admin/ui/Badge";
import { Table, type TableColumn } from "@/components/admin/ui/Table";
import { Unavailable } from "@/components/admin/Unavailable";

// Section H (autonomous-loop meta) only answers where the API is mounted
// against a repo checkout — the loop reading its own progress. `GET
// /v1/admin/loop` throws 503 in the cloud posture (no `GOVFOLIO_REPO_ROOT`);
// that's an expected state here, not an error.
export const dynamic = "force-dynamic";

function formatGeneratedAt(iso: string): string {
  return new Date(iso).toUTCString();
}

function formatCommitTime(iso: string): string {
  return new Date(iso).toISOString().slice(0, 16).replace("T", " ");
}

const GOAL_COLUMNS: ReadonlyArray<TableColumn<AdminLoopGoal>> = [
  {
    key: "number",
    header: "#",
    render: (goal) => (
      <span className={goal.halted ? "adm-num font-semibold text-[var(--adm-danger-ink)]" : "adm-num"}>
        {goal.number}
      </span>
    ),
  },
  {
    key: "title",
    header: "goal",
    render: (goal) => goal.title,
  },
  {
    key: "state",
    header: "state",
    render: (goal) => <Badge variant={stateVariant(goal.state)}>{goal.state}</Badge>,
  },
  {
    key: "halted",
    header: "halt",
    render: (goal) => (goal.halted ? <Badge variant="danger">HALT</Badge> : null),
  },
];

export default async function LoopPage() {
  let data: AdminLoop;
  try {
    data = await adminLoop();
  } catch (error) {
    if (error instanceof ApiError && (error.status === 401 || error.status === 403 || error.status === 503)) {
      return <Unavailable reason={error.message} />;
    }
    throw error;
  }

  const git = data.git ?? null;
  const skips = data.budget_skips ?? null;
  const doneCount = data.goals.filter((goal) => goal.state === "done").length;
  const inProgressCount = data.goals.filter((goal) => goal.state === "in_progress").length;
  const openCount = data.goals.filter((goal) => goal.state === "open").length;
  const haltedCount = data.goals.filter((goal) => goal.halted).length;

  return (
    <div className="mx-auto flex max-w-5xl flex-col gap-4 px-4 py-6">
      <section className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="adm-eyebrow mb-1">Section H · autonomous loop</p>
          <h1>Loop</h1>
          <p className="mt-1 max-w-2xl text-sm adm-muted">
            Goal queue, git activity, and budget-guardrail trips, read straight from the checkout
            the API is mounted against.
          </p>
        </div>
        <div className="flex flex-col items-end gap-1">
          <p className="adm-num text-xs adm-muted">{data.repo_root}</p>
          <p className="adm-num text-xs adm-muted">as of {formatGeneratedAt(data.generated_at)}</p>
        </div>
      </section>

      <Card eyebrow="H1" title="Goal queue">
        <p className="mb-3 text-xs adm-muted">agents/goals/000-INDEX.md</p>
        <div className="mb-4 grid grid-cols-2 gap-4 sm:grid-cols-4">
          <Stat label="done" value={doneCount} tone={doneCount > 0 ? "success" : "neutral"} />
          <Stat
            label="in progress"
            value={inProgressCount}
            tone={inProgressCount > 0 ? "info" : "neutral"}
          />
          <Stat label="open" value={openCount} />
          <Stat label="halted" value={haltedCount} tone={haltedCount > 0 ? "danger" : "neutral"} />
        </div>
        <Table
          columns={GOAL_COLUMNS}
          rows={data.goals}
          getRowKey={(goal) => goal.number}
          emptyMessage="No goals found."
        />
      </Card>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Card eyebrow="H2" title="Git activity">
          {git === null ? (
            <p className="text-sm adm-muted">
              Git activity unavailable — a git subprocess failed in this checkout.
            </p>
          ) : (
            <>
              <div className="mb-4 flex flex-wrap gap-6">
                <Stat label="branch" value={git.branch ?? "detached HEAD"} />
                <Stat
                  label="dirty files"
                  value={git.dirty_files}
                  tone={git.dirty_files > 0 ? "warning" : "neutral"}
                />
              </div>
              <p className="adm-eyebrow mb-2">last {git.commits.length} commits</p>
              <ul className="flex max-h-72 flex-col gap-1.5 overflow-y-auto pr-1">
                {git.commits.map((commit) => (
                  <li
                    key={commit.sha}
                    className="flex items-baseline gap-3 border-b border-[var(--adm-rule)] pb-1.5 text-sm last:border-0 last:pb-0"
                  >
                    <span className="adm-num shrink-0 text-xs adm-muted">
                      {commit.sha.slice(0, 7)}
                    </span>
                    <span className="adm-num shrink-0 text-xs adm-muted">
                      {formatCommitTime(commit.committed_at)}
                    </span>
                    <span className="min-w-0 flex-1 truncate">{commit.subject}</span>
                  </li>
                ))}
              </ul>
            </>
          )}
        </Card>

        <Card eyebrow="H3" title="Budget skips">
          <p className="mb-3 text-xs adm-muted">agents/JOURNAL.md · BACKFILL_BUDGET skip: lines</p>
          {skips === null ? (
            <p className="text-sm adm-muted">agents/JOURNAL.md unavailable in this checkout.</p>
          ) : skips.length === 0 ? (
            <p className="text-sm adm-muted">No BACKFILL_BUDGET skips recorded.</p>
          ) : (
            <ul className="flex max-h-72 flex-col gap-2 overflow-y-auto pr-1 text-sm">
              {skips.map((skip, index) => (
                <li
                  key={`${skip.date}-${index}`}
                  className="border-b border-[var(--adm-rule)] pb-2 last:border-0 last:pb-0"
                >
                  <p className="adm-num text-xs adm-muted">{skip.date}</p>
                  <p className="mt-0.5">{skip.line}</p>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </div>
    </div>
  );
}
