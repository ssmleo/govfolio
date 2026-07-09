/**
 * The four state colors (design direction, non-negotiable): saturated color
 * on this surface is ALWAYS a status claim, never decoration.
 *   success = live / succeeded / done / resolved / pass / sent
 *   warning = halted / paused / skipped_budget / pending / open
 *   danger  = frozen / failed / dead / blocked / discrepant
 *   info    = running / in_progress
 * `neutral` is the non-claim default for labels that aren't a status at all.
 */
export type BadgeVariant = "success" | "warning" | "danger" | "info" | "neutral";

const VARIANT_CLASS: Record<BadgeVariant, string> = {
  success:
    "text-[var(--adm-success-ink)] bg-[var(--adm-success-bg)] border-[var(--adm-success-rule)]",
  warning:
    "text-[var(--adm-warning-ink)] bg-[var(--adm-warning-bg)] border-[var(--adm-warning-rule)]",
  danger: "text-[var(--adm-danger-ink)] bg-[var(--adm-danger-bg)] border-[var(--adm-danger-rule)]",
  info: "text-[var(--adm-info-ink)] bg-[var(--adm-info-bg)] border-[var(--adm-info-rule)]",
  neutral: "text-[var(--adm-muted)] bg-[var(--adm-surface-sunken)] border-[var(--adm-rule-strong)]",
};

/**
 * Maps the raw status/verdict strings the admin API actually returns to one
 * of the four state colors. Unrecognized strings (e.g. free-text reasons)
 * fall back to `neutral` rather than guessing a color for them.
 */
const KNOWN_STATE_VARIANT: Record<string, BadgeVariant> = {
  live: "success",
  succeeded: "success",
  done: "success",
  resolved: "success",
  pass: "success",
  sent: "success",
  confirm: "success",

  halted: "warning",
  paused: "warning",
  skipped_budget: "warning",
  pending: "warning",
  pending_digest: "warning",
  open: "warning",
  edit: "warning",
  stub: "warning",
  scouted: "warning",
  surveyed: "warning",
  sampled: "warning",
  specced: "warning",
  built: "warning",

  frozen: "danger",
  failed: "danger",
  dead: "danger",
  blocked: "danger",
  discrepant: "danger",
  reject: "danger",
  superseded: "danger",

  running: "info",
  in_progress: "info",
};

/** Classifies a raw status/phase string into one of the four state colors. */
export function stateVariant(state: string): BadgeVariant {
  return KNOWN_STATE_VARIANT[state] ?? "neutral";
}

export interface BadgeProps {
  variant: BadgeVariant;
  children: React.ReactNode;
  className?: string;
}

export function Badge({ variant, children, className }: BadgeProps) {
  return (
    <span
      className={`inline-block rounded-[2px] border px-1.5 py-0.5 text-xs font-semibold tracking-[0.02em] whitespace-nowrap ${VARIANT_CLASS[variant]} ${className ?? ""}`}
    >
      {children}
    </span>
  );
}
