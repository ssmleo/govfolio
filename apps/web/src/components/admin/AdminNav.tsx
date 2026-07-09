"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

const LINKS: ReadonlyArray<{ href: string; label: string }> = [
  { href: "/admin", label: "overview" },
  { href: "/admin/coverage", label: "coverage" },
  { href: "/admin/backfill", label: "backfill" },
  { href: "/admin/pipeline", label: "pipeline" },
  { href: "/admin/quality", label: "quality" },
  { href: "/admin/storage", label: "storage" },
  { href: "/admin/serving", label: "serving" },
  { href: "/admin/infra", label: "infra" },
  { href: "/admin/loop", label: "loop" },
];

// The instrument-panel top bar: dense, flat, one row. `/admin` itself only
// matches exactly (every other route starts with `/admin/...` and would
// otherwise always read as "current").
export function AdminNav() {
  const pathname = usePathname();

  return (
    <nav
      aria-label="Admin sections"
      className="flex flex-wrap items-baseline gap-x-5 gap-y-1 border-b border-[var(--adm-rule-strong)] bg-[var(--adm-surface)] px-4 py-2.5"
    >
      <span className="adm-eyebrow mr-1 font-[family-name:var(--adm-font-display)] text-sm font-semibold normal-case tracking-normal text-[var(--adm-ink)]">
        govfolio admin
      </span>
      <ul className="flex flex-wrap items-baseline gap-x-4 gap-y-1 list-none m-0 p-0">
        {LINKS.map((link) => {
          const current = pathname === link.href;
          return (
            <li key={link.href}>
              <Link
                href={link.href}
                aria-current={current ? "page" : undefined}
                className={
                  current
                    ? "border-b-2 border-[var(--adm-accent)] pb-0.5 text-sm font-semibold text-[var(--adm-accent-deep)] no-underline"
                    : "border-b-2 border-transparent pb-0.5 text-sm text-[var(--adm-muted)] no-underline hover:text-[var(--adm-ink)]"
                }
              >
                {link.label}
              </Link>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
