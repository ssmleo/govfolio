import type { AdminOverview, AdminQueueDepths, Jurisdiction } from "@/lib/api";
import { ApiError, adminOverview, listJurisdictions } from "@/lib/api";
import { formatCount, formatUtcMinute } from "@/lib/format";
import { Badge } from "@/components/admin/ui/Badge";
import { Card } from "@/components/admin/ui/Card";
import { Screen } from "@/components/admin/ui/Screen";
import { Stat } from "@/components/admin/ui/Stat";
import {
  WorldWall,
  type WorldWallCell,
  type WorldWallLegendItem,
} from "@/components/admin/WorldWall";
import { Unavailable } from "@/components/admin/Unavailable";

// The landing page (goal 091 phase 2, redesign sweep): "healthy? covered?
// what's left?" answerable in five seconds. SentinelTicker (mounted once in
// the shared layout) already covers the ambient poll — this page is the
// deeper, static-per-request read of the same snapshot plus the world
// coverage wall (dc.html:106-196).
export const dynamic = "force-dynamic";

// Coverage-phase ramp, legend-ordered per the design (dc.html:1424) —
// progression toward brand gold, plus blocked.
const PHASE_ORDER = [
  "live",
  "built",
  "specced",
  "sampled",
  "surveyed",
  "scouted",
  "stub",
  "blocked",
] as const;

const PHASE_COLOR: Record<string, string> = {
  live: "var(--adm-phase-live)",
  built: "var(--adm-phase-built)",
  specced: "var(--adm-phase-specced)",
  sampled: "var(--adm-phase-sampled)",
  surveyed: "var(--adm-phase-surveyed)",
  scouted: "var(--adm-phase-scouted)",
  stub: "var(--adm-phase-stub)",
  blocked: "var(--adm-phase-blocked)",
};

// Unknown phases render as stub, matching the design's PH() fallback
// (dc.html:1414) — the readout still shows the real phase string.
function phaseColor(phase: string): string {
  return PHASE_COLOR[phase] ?? "var(--adm-phase-stub)";
}

function queueTone(count: number, tone?: "warning" | "danger"): "warning" | "danger" | undefined {
  return tone !== undefined && count > 0 ? tone : undefined;
}

// § B5 queue-depth stats (dc.html:1492-1499): drift amber when > 0,
// delivery dead red when > 0, everything else plain ink.
const QUEUE_TILES: ReadonlyArray<{
  key: keyof AdminQueueDepths;
  label: string;
  toneWhenPositive?: "warning" | "danger";
}> = [
  { key: "outbox_undispatched", label: "Outbox undispatched" },
  { key: "review_open", label: "Review open" },
  { key: "drift_open", label: "Drift open", toneWhenPositive: "warning" },
  { key: "sample_pending", label: "Sample pending" },
  { key: "delivery_dlq", label: "Delivery dead", toneWhenPositive: "danger" },
  { key: "usage_unbilled", label: "Usage unbilled" },
];

export default async function AdminOverviewPage() {
  let data: AdminOverview;
  let jurisdictions: Jurisdiction[];
  try {
    [data, jurisdictions] = await Promise.all([adminOverview(), listJurisdictions()]);
  } catch (error) {
    if (error instanceof ApiError && (error.status === 401 || error.status === 403 || error.status === 503)) {
      return <Unavailable reason={error.message} />;
    }
    throw error;
  }

  const cells: WorldWallCell[] = jurisdictions.map((j) => ({
    name: j.name,
    phase: j.coverage_phase,
    color: phaseColor(j.coverage_phase),
  }));
  const legend: WorldWallLegendItem[] = PHASE_ORDER.map((phase) => ({
    phase,
    color: phaseColor(phase),
    count: cells.filter((c) => c.phase === phase).length,
  }));
  const liveCount = cells.filter((c) => c.phase === "live").length;
  const frozenCount = data.frozen_regimes.length;

  return (
    <Screen
      label="Overview"
      kicker="Mission control"
      title="Overview"
      subtitle="Coverage, queues, and runs across every disclosure regime — is anything broken, and what’s left to build."
      meta={
        <>
          as of {formatUtcMinute(data.generated_at)}
          <br />
          sentinel last checked{" "}
          {data.last_sentinel_check !== null && data.last_sentinel_check !== undefined
            ? formatUtcMinute(data.last_sentinel_check)
            : "never"}
        </>
      }
    >
      <div
        style={{ display: "grid", gridTemplateColumns: "1.9fr 1fr", gap: 16, alignItems: "start" }}
      >
        <Card
          section="A1"
          label="World coverage"
          meta={`${cells.length} jurisdictions`}
          title={`${liveCount} / ${cells.length} jurisdictions live`}
          titleSize={19}
          rise={0.05}
        >
          {/* Card's h2 carries an 8px bottom margin; the design wants 16px
              under the hero line (dc.html:126) — pad the remainder. */}
          <div style={{ paddingTop: 8 }}>
            <WorldWall cells={cells} legend={legend} total={cells.length} />
          </div>
        </Card>

        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          <Card
            section="C1"
            label="Sentinel"
            title="Frozen regimes"
            tone={frozenCount > 0 ? "danger" : undefined}
            rise={0.12}
            action={
              <Badge variant={frozenCount > 0 ? "danger" : "success"}>
                {frozenCount > 0 ? `${frozenCount} frozen` : "none frozen"}
              </Badge>
            }
          >
            {/* dc.html:151 — 12px under the h2 (8 from Card + 4 here). */}
            <div style={{ paddingTop: 4 }}>
              {frozenCount > 0 ? (
                data.frozen_regimes.map((f) => (
                  <div
                    key={f.regime_code}
                    style={{
                      display: "flex",
                      flexDirection: "column",
                      gap: 4,
                      borderTop: "1px solid var(--adm-rule)",
                      padding: "10px 0 2px",
                    }}
                  >
                    <div
                      style={{
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "space-between",
                        gap: 10,
                      }}
                    >
                      <span
                        style={{
                          fontFamily: "var(--adm-font-data)",
                          fontSize: "12.5px",
                          color: "var(--adm-heading)",
                        }}
                      >
                        {f.regime_code}
                      </span>
                      {f.frozen_kind !== null && f.frozen_kind !== undefined ? (
                        <Badge variant="danger">{f.frozen_kind}</Badge>
                      ) : (
                        <span style={{ color: "var(--adm-muted)" }}>—</span>
                      )}
                    </div>
                    <span
                      style={{
                        fontFamily: "var(--adm-font-data)",
                        fontSize: "10.5px",
                        color: "var(--adm-meta)",
                      }}
                    >
                      since{" "}
                      {f.frozen_at !== null && f.frozen_at !== undefined
                        ? formatUtcMinute(f.frozen_at)
                        : "—"}
                    </span>
                  </div>
                ))
              ) : (
                <p
                  style={{
                    margin: 0,
                    color: "var(--adm-muted)",
                    fontSize: "12.5px",
                    borderTop: "1px solid var(--adm-rule)",
                    paddingTop: 12,
                  }}
                >
                  No frozen regimes — every adapter publishing normally.
                </p>
              )}
            </div>
          </Card>

          <Card label="Trailing 24h" title="Pipeline runs" rise={0.19}>
            {/* dc.html:170 — 14px under the h2 (8 from Card + 6 here);
                run stats carry no left rule (dc.html:171-177). */}
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "repeat(3,1fr)",
                gap: 14,
                paddingTop: 6,
              }}
            >
              <Stat
                label="Running"
                value={formatCount(data.runs_24h.running)}
                tone={data.runs_24h.running > 0 ? "info" : undefined}
                rule={false}
              />
              <Stat
                label="Succeeded"
                value={formatCount(data.runs_24h.succeeded)}
                tone="success"
                rule={false}
              />
              <Stat
                label="Failed"
                value={formatCount(data.runs_24h.failed)}
                tone={data.runs_24h.failed > 0 ? "danger" : undefined}
                rule={false}
              />
            </div>
          </Card>
        </div>
      </div>

      <Card section="B5" label="Queues" title="Queue depths" rise={0.26} className="mt-[16px]">
        {/* dc.html:186 — 16px under the h2 (8 from Card + 8 here);
            queue stats keep the hairline left rule (dc.html:189). */}
        <div
          style={{ display: "grid", gridTemplateColumns: "repeat(6,1fr)", gap: 14, paddingTop: 8 }}
        >
          {QUEUE_TILES.map((tile) => (
            <Stat
              key={tile.key}
              label={tile.label}
              value={formatCount(data.queue_depths[tile.key])}
              tone={queueTone(data.queue_depths[tile.key], tile.toneWhenPositive)}
            />
          ))}
        </div>
      </Card>
    </Screen>
  );
}
