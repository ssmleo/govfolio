import type { BadgeVariant } from "./Badge";

const TONE_COLOR: Record<BadgeVariant, string> = {
  success: "var(--adm-success-ink)",
  warning: "var(--adm-warning-ink)",
  danger: "var(--adm-danger-ink)",
  info: "var(--adm-info-ink)",
  neutral: "var(--adm-ink)",
};

export interface StatProps {
  label: string;
  value: string | number;
  /** Value size in px — 24 default (run-stats/queues); 22/20/18 for denser grids. */
  size?: 24 | 22 | 20 | 18;
  /**
   * A state name (colored = status claim) or an explicit CSS color
   * (e.g. "var(--adm-accent-deep)" for gold counts). Omit for plain ink.
   */
  tone?: BadgeVariant | (string & {});
  /**
   * Hairline left rule + 14px indent (dc.html:189, queue-depth grids).
   * Overview run-stats omit it (dc.html:171-177) — pass rule={false}.
   */
  rule?: boolean;
  /** Small caption under the number — units, a caveat, a comparison. */
  caption?: string;
}

// One number, one label: the base unit of every overview grid on this
// dashboard. The number is always tabular mono, weight 600, line-height 1.
export function Stat({ label, value, size = 24, tone, rule = true, caption }: StatProps) {
  const color =
    tone === undefined
      ? "var(--adm-ink)"
      : tone in TONE_COLOR
        ? TONE_COLOR[tone as BadgeVariant]
        : tone;
  return (
    <div style={rule ? { borderLeft: "1px solid var(--adm-rule)", paddingLeft: 14 } : undefined}>
      <p className="adm-microlabel" style={{ margin: "0 0 6px" }}>
        {label}
      </p>
      <p
        className="adm-num"
        style={{ margin: 0, fontSize: size, fontWeight: 600, lineHeight: 1, color }}
      >
        {value}
      </p>
      {caption !== undefined && (
        <p style={{ margin: "4px 0 0", fontSize: "10.5px", color: "var(--adm-meta)" }}>
          {caption}
        </p>
      )}
    </div>
  );
}
