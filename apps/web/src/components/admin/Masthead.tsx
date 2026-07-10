import Link from "next/link";

import { AdminClock } from "./AdminClock";

// The instrument-panel header, design-exact (dc.html:36-50): wordmark with
// a gold full stop, the "Administrative Console" tag, an environment badge
// (text stays NODE_ENV-derived — the design's static "Production" is just
// what that renders in prod), a "Founder" role badge, and a live UTC clock
// (AdminClock — the one client-rendered piece here). No founder-mode/role
// model exists anywhere in this repo (the whole admin surface is gated by
// one shared X-Admin-Token, not per-user roles — see lib/api.ts
// adminHeaders()), so "Founder" is the design's static label for the one
// person this console serves, not derived from any auth/session state.
export function Masthead() {
  const env = process.env.NODE_ENV === "production" ? "production" : "development";

  return (
    <header
      className="relative z-20 flex h-[var(--adm-masthead-h)] shrink-0 items-center gap-[18px] border-b border-[var(--adm-masthead-rule)] px-7"
      style={{ background: "var(--adm-masthead-bg)" }}
    >
      <div className="flex items-baseline gap-3">
        <Link
          href="/admin"
          className="font-[family-name:var(--adm-font-display)] text-[20px] font-bold text-[var(--adm-heading)] no-underline"
        >
          Govfolio<span className="text-[var(--adm-accent)]">.</span>
        </Link>
        <span
          style={{
            fontSize: "10px",
            fontWeight: 700,
            letterSpacing: ".24em",
            textTransform: "uppercase",
            color: "var(--adm-muted)",
          }}
        >
          Administrative Console
        </span>
      </div>
      <div className="flex-1" />
      <div className="flex items-center gap-2.5">
        <span
          className="inline-flex items-center gap-[7px] rounded-[2px] border border-[var(--adm-chip-border)]"
          style={{
            padding: "3px 10px",
            fontSize: "10px",
            fontWeight: 700,
            letterSpacing: ".14em",
            textTransform: "uppercase",
            color: "var(--adm-nav-inactive)",
          }}
        >
          <span className="inline-block h-1.5 w-1.5 rounded-full bg-[#4FB582]" />
          {env}
        </span>
        <span
          className="inline-flex items-center rounded-[2px] border border-[var(--adm-gold-45)]"
          style={{
            padding: "3px 10px",
            fontSize: "10px",
            fontWeight: 700,
            letterSpacing: ".14em",
            textTransform: "uppercase",
            color: "var(--adm-accent)",
          }}
        >
          Founder
        </span>
        <AdminClock />
      </div>
    </header>
  );
}
