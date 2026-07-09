import type { BadgeVariant } from "./Badge";

const TONE_CLASS: Record<BadgeVariant, string> = {
  success: "text-[var(--adm-success-ink)]",
  warning: "text-[var(--adm-warning-ink)]",
  danger: "text-[var(--adm-danger-ink)]",
  info: "text-[var(--adm-info-ink)]",
  neutral: "text-[var(--adm-ink)]",
};

export interface StatProps {
  label: string;
  value: string | number;
  /** Small caption under the number — units, a caveat, a comparison. */
  caption?: string;
  /**
   * Color the number ONLY when it's a status claim worth noticing
   * (omit, or "neutral", for a plain count).
   */
  tone?: BadgeVariant;
}

// One number, one label: the base unit of every overview grid on this
// dashboard. The number is always tabular mono; grey unless `tone` says
// otherwise.
export function Stat({ label, value, caption, tone = "neutral" }: StatProps) {
  return (
    <div className="flex flex-col gap-1">
      <p className="adm-eyebrow">{label}</p>
      <p className={`adm-num text-2xl leading-none font-semibold ${TONE_CLASS[tone]}`}>{value}</p>
      {caption !== undefined && (
        <p className="text-xs text-[var(--adm-muted)]">{caption}</p>
      )}
    </div>
  );
}
