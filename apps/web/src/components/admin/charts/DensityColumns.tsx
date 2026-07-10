export interface DensitySegment {
  value: number;
  color: string;
}

export interface DensityHour {
  /** Native hover title, e.g. `03:00 UTC · 128 docs` (dc.html:1607). */
  title: string;
  /** Bottom-up stacking order: the first segment sits on the baseline. */
  segments: readonly DensitySegment[];
}

export interface DensityColumnsProps {
  /** Chronological hour columns (the design renders 48). */
  hours: readonly DensityHour[];
  /** Scale denominator; defaults to the max per-hour segment total. */
  maxTotal?: number;
  xLeftLabel?: string;
  xMidLabel?: string;
  xRightLabel?: string;
}

/**
 * Stacked hourly fetch-density columns (dc.html:619-631 + 1596-1609): 2px-gapped
 * flex columns against a strong baseline rule, each segment
 * `Math.round(114 * value / maxTotal)`px tall. The design's column is
 * `flex-direction: column; justify-content: flex-end` with the primary series
 * as the LAST child (dc.html:621-626) so it sits on the baseline — segments
 * arrive bottom-up, so the DOM renders them reversed to keep that stacking.
 */
export function DensityColumns({
  hours,
  maxTotal,
  xLeftLabel = "48h ago",
  xMidLabel = "24h ago",
  xRightLabel = "now",
}: DensityColumnsProps) {
  const totals = hours.map((h) => h.segments.reduce((sum, s) => sum + s.value, 0));
  const denom = maxTotal ?? Math.max(0, ...totals);
  const px = (value: number): number => (denom > 0 ? Math.round((114 * value) / denom) : 0);

  return (
    <div>
      <div
        style={{
          display: "flex",
          alignItems: "flex-end",
          gap: "2px",
          height: "120px",
          borderBottom: "1px solid var(--adm-rule-strong)",
        }}
      >
        {hours.map((h, i) => (
          <div
            key={i}
            title={h.title}
            style={{
              flex: 1,
              display: "flex",
              flexDirection: "column",
              justifyContent: "flex-end",
              height: "100%",
            }}
          >
            {[...h.segments].reverse().map((s, j) => (
              <div
                key={j}
                style={{ width: "100%", height: `${px(s.value)}px`, background: s.color }}
              />
            ))}
          </div>
        ))}
      </div>
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
        <span>{xMidLabel}</span>
        <span>{xRightLabel}</span>
      </div>
    </div>
  );
}
