import Link from "next/link";

export interface CoverageCell {
  id: string;
  name: string;
  /** `stub` | `scouted` | `surveyed` | `sampled` | `specced` | `built` | `live` | `blocked` (design §5.8). */
  phase: string;
  /** Rendered in the hover title when `phase === "blocked"`. */
  blockedReason?: string | null;
}

export interface CoverageHeatmapProps {
  jurisdictions: readonly CoverageCell[];
  /** phase → CSS color; defaults to the §5.8 ramp defined in admin.css (`--adm-phase-*`). */
  phaseColor?: Readonly<Record<string, string>>;
  /** `full` (large, legend, linked cells) on /admin/coverage; `compact` (small, no legend) on the overview. */
  size?: "compact" | "full";
  /** Links each cell through to its jurisdiction page. Only used at `size="full"`. */
  hrefFor?: (jurisdictionId: string) => string;
}

const DEFAULT_PHASE_COLOR: Record<string, string> = {
  blocked: "var(--adm-phase-blocked)",
  stub: "var(--adm-phase-stub)",
  scouted: "var(--adm-phase-scouted)",
  surveyed: "var(--adm-phase-surveyed)",
  sampled: "var(--adm-phase-sampled)",
  specced: "var(--adm-phase-specced)",
  built: "var(--adm-phase-built)",
  live: "var(--adm-phase-live)",
};

// `blocked` sorts first — it's the one phase that needs intervention, not
// just patience — then the §5.8 progression toward `live`.
const PHASE_ORDER: readonly string[] = [
  "blocked",
  "stub",
  "scouted",
  "surveyed",
  "sampled",
  "specced",
  "built",
  "live",
];

function phaseRank(phase: string): number {
  const i = PHASE_ORDER.indexOf(phase);
  return i === -1 ? PHASE_ORDER.length : i;
}

/**
 * THE SIGNATURE ELEMENT (goal 091): every seeded jurisdiction as one cell in
 * a dense CSS-grid wall, colored by `coverage_phase`. Cells sort worst-first
 * (blocked, then stub → live) so gaps cluster together — one glance answers
 * "what's left". Pure CSS grid (Recharts has no heatmap primitive); fully
 * server-renderable — hover detail rides the native `title` attribute, no
 * client JS required. A screen reader gets one summary label for the whole
 * grid (`role="img"`) rather than 196 individually-announced cells.
 */
export function CoverageHeatmap({
  jurisdictions,
  phaseColor = DEFAULT_PHASE_COLOR,
  size = "full",
  hrefFor,
}: CoverageHeatmapProps) {
  const sorted = [...jurisdictions].sort((a, b) => {
    const byPhase = phaseRank(a.phase) - phaseRank(b.phase);
    return byPhase !== 0 ? byPhase : a.name.localeCompare(b.name);
  });

  const cellSize = size === "full" ? "1.5rem" : "0.5rem";
  const gap = size === "full" ? "3px" : "2px";

  return (
    <div>
      <div
        role="img"
        aria-label={`Coverage phase for ${jurisdictions.length} jurisdictions`}
        style={{
          display: "grid",
          gridTemplateColumns: `repeat(auto-fill, minmax(${cellSize}, 1fr))`,
          gap,
        }}
      >
        {sorted.map((j) => {
          const color = phaseColor[j.phase] ?? "var(--adm-rule-strong)";
          const title =
            j.phase === "blocked" && j.blockedReason
              ? `${j.name} — blocked: ${j.blockedReason}`
              : `${j.name} — ${j.phase}`;
          const cell = (
            <div
              title={title}
              aria-hidden="true"
              style={{ aspectRatio: "1 / 1", background: color, borderRadius: "1px" }}
            />
          );
          return size === "full" && hrefFor ? (
            <Link key={j.id} href={hrefFor(j.id)} aria-label={title} className="block">
              {cell}
            </Link>
          ) : (
            <div key={j.id}>{cell}</div>
          );
        })}
      </div>
      {size === "full" && (
        <ul className="mt-3 flex list-none flex-wrap gap-x-4 gap-y-1 p-0 text-xs text-[var(--adm-muted)]">
          {PHASE_ORDER.map((phase) => (
            <li key={phase} className="flex items-center gap-1.5">
              <span
                aria-hidden="true"
                style={{
                  display: "inline-block",
                  width: "0.65rem",
                  height: "0.65rem",
                  borderRadius: "1px",
                  background: phaseColor[phase] ?? "var(--adm-rule-strong)",
                }}
              />
              {phase}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
