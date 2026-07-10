import Link from "next/link";

import { AdminClock } from "./AdminClock";

// The instrument-panel header (goal 094): wordmark + a static
// "Administrative Console" tag, an environment badge, an operator-role
// tag, and a live UTC clock (AdminClock — the one client-rendered piece
// here). No founder-mode/role model exists anywhere in this repo (the
// whole admin surface is gated by one shared X-Admin-Token, not per-user
// roles — see lib/api.ts adminHeaders()), so both badges are static
// labels, not derived from any auth/session state; adding a real role
// model is out of this task's scope, not a gap it silently papers over.
export function Masthead() {
  const env = process.env.NODE_ENV === "production" ? "production" : "development";

  return (
    <header className="flex h-[var(--adm-masthead-h)] shrink-0 items-center justify-between border-b border-[var(--adm-rule-strong)] bg-[var(--adm-surface)] px-4">
      <div className="flex items-baseline gap-3">
        <Link
          href="/admin"
          className="font-[family-name:var(--adm-font-display)] text-lg font-bold text-[var(--adm-heading)] no-underline"
        >
          govfolio
        </Link>
        <span className="adm-eyebrow">Administrative Console</span>
      </div>
      <div className="flex items-center gap-3">
        <span className="adm-num rounded-[2px] border border-[var(--adm-chip-border)] px-1.5 py-0.5 text-[0.625rem] font-semibold uppercase text-[var(--adm-muted)]">
          {env}
        </span>
        <span className="adm-num rounded-[2px] border border-[var(--adm-chip-border)] px-1.5 py-0.5 text-[0.625rem] font-semibold uppercase text-[var(--adm-muted)]">
          operator
        </span>
        <AdminClock />
      </div>
    </header>
  );
}
