export interface ProgressProps {
  /** 0–1 fraction; clamped to [0, 1]. */
  value: number;
  /** Explicit fill color (dc.html fill() takes one); defaults to brand gold. */
  color?: string;
  /** Track height in px (dc.html:258 tier bars 4, :301 coverage bars 5). */
  height?: 4 | 5 | 6;
  /** Optional label row above the track (dc.html:296-301). */
  label?: string;
}

// A thin completion bar — years covered vs. declared target, backlog
// cleared vs. opened, etc. Not rounded-full: design radius is 1px.
export function Progress({ value, color = "var(--adm-accent)", height = 5, label }: ProgressProps) {
  const pct = Math.max(0, Math.min(1, value)) * 100;
  const pctLabel = Math.round(pct);
  return (
    <div>
      {label !== undefined && (
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "baseline",
            marginBottom: 5,
          }}
        >
          <span style={{ fontSize: 11, color: "var(--adm-muted)" }}>{label}</span>
          <span
            style={{
              fontFamily: "var(--adm-font-data)",
              fontSize: 11,
              color: "var(--adm-text-secondary)",
            }}
          >
            {pctLabel}%
          </span>
        </div>
      )}
      <div
        role="progressbar"
        aria-valuenow={pctLabel}
        aria-valuemin={0}
        aria-valuemax={100}
        style={{
          height,
          borderRadius: 1,
          background: "var(--adm-progress-track)",
          overflow: "hidden",
        }}
      >
        <div
          style={{ height: "100%", width: `${pct}%`, background: color, borderRadius: 1 }}
        />
      </div>
    </div>
  );
}
