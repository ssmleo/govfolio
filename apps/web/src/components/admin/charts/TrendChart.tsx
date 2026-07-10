export interface TrendPoint {
  label: string;
  value: number;
}

export interface TrendChartProps {
  points: readonly TrendPoint[];
  /** `wide` = 560×150 section trend (dc.html:469-479); `small` = 420×110 stat trend (dc.html:862-877). */
  size: "wide" | "small";
  palette?: "gold" | "silver";
  /** Marks the last point with the 3px gold dot (dc.html:475). */
  endpointDot?: boolean;
  xLeftLabel?: string;
  xRightLabel?: string;
  ariaLabel: string;
}

interface Geometry {
  width: number;
  height: number;
  top: number;
  bottom: number;
  /** Interior hairline y positions (wide only; dc.html:470-471). */
  hairlines: readonly number[];
}

const GEOMETRY: Record<TrendChartProps["size"], Geometry> = {
  wide: { width: 560, height: 150, top: 12, bottom: 130, hairlines: [40, 85] },
  small: { width: 420, height: 110, top: 10, bottom: 95, hairlines: [] },
};

const PALETTE: Record<"gold" | "silver", { area: string; stroke: string }> = {
  gold: { area: "var(--adm-series-gold-area)", stroke: "var(--adm-series-gold)" },
  // dc.html:875 — the silver series area alpha has no admin.css token.
  silver: { area: "rgba(155,163,173,.10)", stroke: "var(--adm-series-silver)" },
};

/**
 * mkTrend port (dc.html:1587-1592): integer-rounded polyline over the full
 * viewBox width, plus the closed area path down to the baseline. Degenerate
 * series (a single point, or max ≤ 0 where `v / max` is NaN) collapse to a
 * flat baseline path instead of emitting NaN coordinates.
 */
function trendGeometry(
  values: readonly number[],
  width: number,
  bottom: number,
  top: number,
): { path: string; area: string; lastX: number; lastY: number } {
  const max = values.length > 0 ? Math.max(...values) : 0;
  const xy: readonly (readonly [number, number])[] =
    values.length < 2 || max <= 0
      ? [
          [0, bottom],
          [width, bottom],
        ]
      : values.map(
          (v, i) =>
            [
              Math.round(i * (width / (values.length - 1))),
              Math.round(bottom - (v / max) * (bottom - top)),
            ] as const,
        );
  const path = "M" + xy.map((p) => p[0] + "," + p[1]).join(" L");
  const area = path + ` L${width},${bottom} L0,${bottom} Z`;
  const last = xy[xy.length - 1] ?? ([width, bottom] as const);
  return { path, area, lastX: last[0], lastY: last[1] };
}

/**
 * Hand-rolled area trend (dc.html:469-479 wide / 862-877 small): stretched
 * SVG (`preserveAspectRatio="none"`), hairline grid, filled area + 1.5px
 * stroke, optional endpoint dot, mono x-range labels below. Pure server
 * component — hover detail is not part of the design here.
 */
export function TrendChart({
  points,
  size,
  palette = "gold",
  endpointDot = false,
  xLeftLabel,
  xRightLabel,
  ariaLabel,
}: TrendChartProps) {
  const geo = GEOMETRY[size];
  const colors = PALETTE[palette];
  const { path, area, lastX, lastY } = trendGeometry(
    points.map((p) => p.value),
    geo.width,
    geo.bottom,
    geo.top,
  );

  return (
    <div>
      <svg
        viewBox={`0 0 ${geo.width} ${geo.height}`}
        preserveAspectRatio="none"
        role="img"
        aria-label={ariaLabel}
        style={{ display: "block", width: "100%", height: `${geo.height}px` }}
      >
        <title>{ariaLabel}</title>
        {geo.hairlines.map((y) => (
          <line
            key={y}
            x1={0}
            y1={y}
            x2={geo.width}
            y2={y}
            strokeWidth={1}
            style={{ stroke: "var(--adm-rule)" }}
          />
        ))}
        <line
          x1={0}
          y1={geo.bottom}
          x2={geo.width}
          y2={geo.bottom}
          strokeWidth={1}
          style={{ stroke: "var(--adm-rule-strong)" }}
        />
        <path d={area} style={{ fill: colors.area }} />
        <path d={path} strokeWidth={1.5} style={{ fill: "none", stroke: colors.stroke }} />
        {endpointDot && (
          <circle cx={lastX} cy={lastY} r={3} style={{ fill: "var(--adm-series-gold-dot)" }} />
        )}
      </svg>
      {(xLeftLabel !== undefined || xRightLabel !== undefined) && (
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            marginTop: "8px",
            fontFamily: "var(--adm-font-data)",
            fontSize: "10.5px",
            color: "var(--adm-meta)",
          }}
        >
          <span>{xLeftLabel}</span>
          <span>{xRightLabel}</span>
        </div>
      )}
    </div>
  );
}
