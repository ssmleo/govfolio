import type { AdminLoop } from "@/lib/api";
import { ApiError, adminLoop } from "@/lib/api";
import { Badge, stateVariant } from "@/components/admin/ui/Badge";
import { Card } from "@/components/admin/ui/Card";
import { Screen } from "@/components/admin/ui/Screen";
import { Stat } from "@/components/admin/ui/Stat";
import { Unavailable } from "@/components/admin/Unavailable";
import { formatCount, formatMonthDayTime, formatUtcMinute } from "@/lib/format";

// Section H (autonomous-loop meta) only answers where the API is mounted
// against a repo checkout — the loop reading its own progress. `GET
// /v1/admin/loop` throws 503 in the cloud posture (no `GOVFOLIO_REPO_ROOT`);
// that's an expected state here, not an error.
export const dynamic = "force-dynamic";

const GOAL_TH_STYLE: React.CSSProperties = {
  textAlign: "left",
  padding: "8px 14px 8px 0",
  borderBottom: "1px solid var(--adm-rule-strong)",
};

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
    <Screen
      label="Loop"
      kicker="Section H · autonomous loop"
      title="Loop"
      subtitle="Goal queue, git activity, and budget-guardrail trips — read straight from the checkout the API is mounted against."
      meta={
        <>
          {data.repo_root}
          <br />
          as of {formatUtcMinute(data.generated_at)}
        </>
      }
    >
      <Card
        section="H1"
        label="Goals"
        title="Goal queue"
        meta="agents/goals/000-INDEX.md"
        rise={0.05}
      >
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(4,1fr)",
            gap: 14,
            margin: "16px 0",
          }}
        >
          <Stat label="Done" value={formatCount(doneCount)} size={24} tone="success" />
          <Stat label="In progress" value={formatCount(inProgressCount)} size={24} tone="info" />
          <Stat label="Open" value={formatCount(openCount)} size={24} />
          <Stat label="Halted" value={formatCount(haltedCount)} size={24} tone="danger" />
        </div>
        {data.goals.length === 0 ? (
          <p className="adm-muted" style={{ fontSize: "12.5px" }}>
            No goals found.
          </p>
        ) : (
          // Hand-rolled instead of ui/Table: the design's goal table
          // (dc.html:1189) uses 9px row padding where Table hardcodes 10px.
          <table style={{ width: "100%", borderCollapse: "collapse" }}>
            <thead>
              <tr>
                <th className="adm-microlabel" style={GOAL_TH_STYLE}>
                  #
                </th>
                <th className="adm-microlabel" style={GOAL_TH_STYLE}>
                  Goal
                </th>
                <th className="adm-microlabel" style={GOAL_TH_STYLE}>
                  State
                </th>
                <th className="adm-microlabel" style={{ ...GOAL_TH_STYLE, padding: "8px 0" }}>
                  Halt
                </th>
              </tr>
            </thead>
            <tbody>
              {data.goals.map((goal) => (
                <tr
                  key={goal.number}
                  className="hover:bg-[var(--adm-row-hover)]"
                  style={{ transition: "background .12s ease" }}
                >
                  <td
                    className="adm-num"
                    style={{
                      padding: "9px 14px 9px 0",
                      borderBottom: "1px solid var(--adm-rule)",
                      fontSize: 12,
                      color: goal.halted ? "var(--adm-danger-ink)" : "var(--adm-text-secondary)",
                    }}
                  >
                    {goal.number}
                  </td>
                  <td
                    style={{
                      padding: "9px 14px 9px 0",
                      borderBottom: "1px solid var(--adm-rule)",
                      fontSize: "12.5px",
                      color: "var(--adm-ink)",
                    }}
                  >
                    {goal.title}
                  </td>
                  <td
                    style={{ padding: "9px 14px 9px 0", borderBottom: "1px solid var(--adm-rule)" }}
                  >
                    <Badge variant={stateVariant(goal.state)}>{goal.state}</Badge>
                  </td>
                  <td style={{ padding: "9px 0", borderBottom: "1px solid var(--adm-rule)" }}>
                    {goal.halted ? <Badge variant="danger">halt</Badge> : null}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <p style={{ margin: "12px 0 0", fontSize: 11, color: "var(--adm-meta)" }}>
          A halt is ambiguity filed as a goal — the loop continues other work. Public pricing,
          legal, and methodology copy stay human-gated by policy.
        </p>
      </Card>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1.25fr .75fr",
          gap: 16,
          marginTop: 16,
          alignItems: "start",
        }}
      >
        <Card
          section="H2"
          label="Git"
          title="Last commits"
          meta={
            git === null ? undefined : (
              <>
                branch{" "}
                <span style={{ color: "var(--adm-accent-deep)" }}>
                  {git.branch ?? "detached HEAD"}
                </span>{" "}
                · dirty{" "}
                <span style={{ color: "var(--adm-text-secondary)" }}>
                  {formatCount(git.dirty_files)}
                </span>
              </>
            )
          }
          rise={0.12}
        >
          {git === null ? (
            <p className="adm-muted" style={{ marginTop: 12, fontSize: "12.5px" }}>
              Git activity unavailable — a git subprocess failed in this checkout.
            </p>
          ) : (
            <div style={{ marginTop: 12 }}>
              {git.commits.map((commit) => (
                <div
                  key={commit.sha}
                  style={{
                    display: "flex",
                    alignItems: "baseline",
                    gap: 14,
                    borderTop: "1px solid var(--adm-rule)",
                    padding: "8px 0",
                  }}
                >
                  <span
                    className="adm-num"
                    style={{ flexShrink: 0, fontSize: "11.5px", color: "var(--adm-accent-deep)" }}
                  >
                    {commit.sha.slice(0, 7)}
                  </span>
                  <span
                    className="adm-num"
                    style={{
                      flexShrink: 0,
                      fontSize: 11,
                      color: "var(--adm-faint)",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {formatMonthDayTime(commit.committed_at)}
                  </span>
                  <span
                    style={{
                      flex: 1,
                      minWidth: 0,
                      fontSize: "12.5px",
                      color: "var(--adm-text-secondary)",
                      whiteSpace: "nowrap",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                    }}
                  >
                    {commit.subject}
                  </span>
                </div>
              ))}
            </div>
          )}
        </Card>

        <Card section="H3" label="Guardrails" title="Budget skips" rise={0.19}>
          <p style={{ margin: "12px 0 8px", fontSize: 11, color: "var(--adm-meta)" }}>
            agents/JOURNAL.md · BACKFILL_BUDGET skip lines
          </p>
          {skips === null ? (
            <p className="adm-muted" style={{ fontSize: "12.5px" }}>
              agents/JOURNAL.md unavailable in this checkout.
            </p>
          ) : skips.length === 0 ? (
            <p className="adm-muted" style={{ fontSize: "12.5px" }}>
              No BACKFILL_BUDGET skips recorded.
            </p>
          ) : (
            skips.map((skip, index) => (
              <div
                key={`${skip.date}-${index}`}
                style={{ borderTop: "1px solid var(--adm-rule)", padding: "10px 0" }}
              >
                <p className="adm-num" style={{ fontSize: "10.5px", color: "var(--adm-faint)" }}>
                  {skip.date}
                </p>
                <p style={{ marginTop: 4, fontSize: 12, color: "var(--adm-text-secondary)" }}>
                  {skip.line}
                </p>
              </div>
            ))
          )}
          <p
            style={{
              margin: "12px 0 0",
              fontSize: 11,
              color: "var(--adm-meta)",
              borderTop: "1px solid var(--adm-rule)",
              paddingTop: 12,
            }}
          >
            Over-cap actions halt and file a goal — money never moves past the ceiling
            autonomously.
          </p>
        </Card>
      </div>
    </Screen>
  );
}
