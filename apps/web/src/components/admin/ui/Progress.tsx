import type { BadgeVariant } from "./Badge";

const FILL_CLASS: Record<BadgeVariant, string> = {
  success: "bg-[var(--adm-success-ink)]",
  warning: "bg-[var(--adm-warning-ink)]",
  danger: "bg-[var(--adm-danger-ink)]",
  info: "bg-[var(--adm-info-ink)]",
  neutral: "bg-[var(--adm-accent)]",
};

export interface ProgressProps {
  /** 0–1 fraction; clamped to [0, 1]. */
  value: number;
  tone?: BadgeVariant;
  label?: string;
}

// A thin completion bar — years covered vs. declared target, backlog
// cleared vs. opened, etc. Track is neutral; the fill is colored only when
// `tone` names a state worth noticing.
export function Progress({ value, tone = "neutral", label }: ProgressProps) {
  const pct = Math.round(Math.max(0, Math.min(1, value)) * 100);
  return (
    <div className="flex flex-col gap-1">
      {label !== undefined && (
        <div className="flex items-baseline justify-between text-xs text-[var(--adm-muted)]">
          <span>{label}</span>
          <span className="adm-num">{pct}%</span>
        </div>
      )}
      <div
        role="progressbar"
        aria-valuenow={pct}
        aria-valuemin={0}
        aria-valuemax={100}
        className="h-1.5 w-full overflow-hidden rounded-full bg-[var(--adm-rule)]"
      >
        <div className={`h-full ${FILL_CLASS[tone]}`} style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}
