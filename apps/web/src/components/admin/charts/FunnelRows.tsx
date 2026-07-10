export interface FunnelSegment {
  value: number;
  color: string;
}

export interface FunnelRow {
  adapter: string;
  /** Stacked left-to-right inside the track (gold / review / suppressed in the design). */
  segments: readonly FunnelSegment[];
  /** Right column text — a formatted candidate count, or "frozen". */
  totalLabel: string;
  /** `danger` renders the total in danger ink (frozen adapters, dc.html:1552-1553). */
  totalTone?: "danger";
}

export interface FunnelRowsProps {
  rows: readonly FunnelRow[];
  /** Shared scale denominator across all rows (the design's `maxCand`, dc.html:1544). */
  max: number;
}

/**
 * Per-adapter promotion funnel (dc.html:387-399 + 1540-1558): right-aligned
 * mono adapter label, a 16px track with color segments sharing one absolute
 * scale, and a tabular total column. Segment widths are the design's exact
 * unrounded `100 * n / max`% (dc.html:1548-1549); a frozen adapter passes
 * zero-value segments + `totalTone: "danger"`.
 */
export function FunnelRows({ rows, max }: FunnelRowsProps) {
  const denom = max > 0 ? max : 1;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "11px" }}>
      {rows.map((row) => (
        <div
          key={row.adapter}
          style={{ display: "flex", alignItems: "center", gap: "12px" }}
        >
          <span
            style={{
              width: "132px",
              flexShrink: 0,
              fontFamily: "var(--adm-font-data)",
              fontSize: "11.5px",
              color: "var(--adm-text-secondary)",
              textAlign: "right",
            }}
          >
            {row.adapter}
          </span>
          <div
            style={{
              flex: 1,
              height: "16px",
              background: "var(--adm-progress-track)",
              borderRadius: "1px",
              overflow: "hidden",
              display: "flex",
            }}
          >
            {row.segments.map((seg, i) => (
              <div
                key={i}
                style={{
                  width: `${(100 * seg.value) / denom}%`,
                  background: seg.color,
                  height: "100%",
                }}
              />
            ))}
          </div>
          <span
            style={{
              width: "70px",
              flexShrink: 0,
              fontFamily: "var(--adm-font-data)",
              fontSize: "11.5px",
              color:
                row.totalTone === "danger"
                  ? "var(--adm-danger-ink)"
                  : "var(--adm-text-secondary)",
              textAlign: "right",
              fontVariantNumeric: "tabular-nums",
            }}
          >
            {row.totalLabel}
          </span>
        </div>
      ))}
    </div>
  );
}
