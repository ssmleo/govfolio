"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useEffect } from "react";

interface NavLink {
  href: string;
  label: string;
  /**
   * Letter chip glyph (dc.html:1852-1858) — the same A–H codes every page's
   * card eyebrow already renders (e.g. `admin/coverage/page.tsx` uses "A1",
   * "A2 / A3 / A5", ...). Overview carries "◆" (U+25C6): it's Command's
   * single aggregate screen, not one lettered section — no new taxonomy is
   * invented here.
   */
  letter: string;
}

interface NavGroup {
  label: string;
  links: readonly NavLink[];
}

// 5 groups / 9 links / letter chips ◆ + A–H, grouped exactly as the design
// (dc.html:1852-1858): Command (the one aggregate screen), Acquisition
// (getting data in), Refinery (turning it into Gold + checking it),
// Platform (storage + serving + the infra it runs on), Autonomy (the agent
// loop running underneath). Flattened order is unchanged from the old
// LINKS array, so the digit shortcuts below (assigned by flattened
// position) don't shift meaning for anyone used to the old nav.
const GROUPS: readonly NavGroup[] = [
  { label: "Command", links: [{ href: "/admin", label: "Overview", letter: "◆" }] },
  {
    label: "Acquisition",
    links: [
      { href: "/admin/coverage", label: "Coverage", letter: "A" },
      { href: "/admin/backfill", label: "Backfill", letter: "B" },
    ],
  },
  {
    label: "Refinery",
    links: [
      { href: "/admin/pipeline", label: "Pipeline", letter: "C" },
      { href: "/admin/quality", label: "Quality", letter: "D" },
    ],
  },
  {
    label: "Platform",
    links: [
      { href: "/admin/storage", label: "Storage", letter: "E" },
      { href: "/admin/serving", label: "Serving", letter: "F" },
      { href: "/admin/infra", label: "Infra", letter: "G" },
    ],
  },
  {
    label: "Autonomy",
    links: [{ href: "/admin/loop", label: "Loop", letter: "H" }],
  },
];

// Flattened once, in display order, so a link's 1-based position doubles as
// its keyboard-shortcut digit (1-9) — the same order as the old LINKS array.
const FLAT_LINKS: readonly NavLink[] = GROUPS.flatMap((group) => group.links);

const LAST_SCREEN_KEY = "govfolio-admin-last-screen";

function isFormField(el: Element | null): boolean {
  if (el === null) return false;
  if (el.hasAttribute("contenteditable")) return true;
  return el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT";
}

// The grouped instrument-panel sidebar, design-exact (dc.html:79-99 +
// item style objects at 1868-1875): letter chip on the left, no visible
// digits (the design has none — the 1-9 shortcuts still work, they're just
// not advertised in the DOM), a flex spacer, then the Access panel pinned
// at the bottom. Two bits of client-only interactivity, kept inline (one
// call site, no extracted hook):
//   - digits 1-9 jump straight to the matching screen. Ignored whenever a
//     form field has focus (no text inputs exist in /admin today, but this
//     guard is what keeps the shortcut safe if one is ever added) and
//     whenever a modifier key is held (so it doesn't fight the browser's
//     own Ctrl/Cmd+digit tab-switching shortcuts).
//   - the current path is written to localStorage on every navigation, so a
//     later feature COULD offer "resume where you left off" — this
//     component never reads it back to redirect. Auto-redirecting /admin
//     based on it would silently break the e2e assertion that bare /admin
//     renders Overview, and is a real product decision, not this task's.
export function AdminSidebar() {
  const pathname = usePathname();
  const router = useRouter();

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      if (isFormField(document.activeElement)) return;
      const index = Number(event.key) - 1;
      if (!Number.isInteger(index) || index < 0 || index >= FLAT_LINKS.length) return;
      const target = FLAT_LINKS[index];
      if (target === undefined) return;
      router.push(target.href);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [router]);

  useEffect(() => {
    window.localStorage.setItem(LAST_SCREEN_KEY, pathname);
  }, [pathname]);

  return (
    <nav
      aria-label="Admin sections"
      className="flex w-[var(--adm-sidebar-w)] shrink-0 flex-col gap-5 border-r border-[var(--adm-rule)] bg-[var(--adm-sidebar-bg)]"
      style={{ padding: "20px 14px 24px" }}
    >
      {GROUPS.map((group) => (
        <div key={group.label}>
          <p
            style={{
              margin: "0 0 8px 10px",
              fontSize: "9.5px",
              fontWeight: 700,
              letterSpacing: ".22em",
              textTransform: "uppercase",
              color: "var(--adm-faint)",
            }}
          >
            {group.label}
          </p>
          <ul className="m-0 flex list-none flex-col gap-[2px] p-0">
            {group.links.map((link) => {
              const current = pathname === link.href;
              return (
                <li key={link.href}>
                  <Link
                    href={link.href}
                    title={link.href}
                    aria-current={current ? "page" : undefined}
                    className={
                      current
                        ? "flex items-center gap-2.5 rounded-[3px] bg-[var(--adm-gold-08)] text-[13px] font-semibold text-[var(--adm-heading)] no-underline shadow-[inset_2px_0_0_#C2A15E]"
                        : "flex items-center gap-2.5 rounded-[3px] text-[13px] text-[var(--adm-nav-inactive)] no-underline hover:bg-[var(--adm-nav-hover-bg)] hover:text-[var(--adm-nav-hover)]"
                    }
                    style={{
                      padding: "7px 10px",
                      transition: "background .15s ease, color .15s ease",
                    }}
                  >
                    <span
                      className="adm-num grid h-5 w-5 shrink-0 place-items-center rounded-[2px] border border-[var(--adm-chip-border)]"
                      style={{
                        fontSize: "10.5px",
                        color: current
                          ? "var(--adm-accent-deep)"
                          : "var(--adm-nav-chip-inactive)",
                      }}
                    >
                      {link.letter}
                    </span>
                    <span className="flex-1">{link.label}</span>
                  </Link>
                </li>
              );
            })}
          </ul>
        </div>
      ))}
      <div className="flex-1" />
      <div
        className="rounded-[3px] border border-[var(--adm-card-border)] bg-[var(--adm-access-bg)]"
        style={{ padding: "12px 14px" }}
      >
        <p
          style={{
            margin: "0 0 5px",
            fontSize: "9.5px",
            fontWeight: 700,
            letterSpacing: ".2em",
            textTransform: "uppercase",
            color: "var(--adm-accent)",
          }}
        >
          Access
        </p>
        <p style={{ margin: 0, fontSize: "11.5px", color: "var(--adm-nav-inactive)" }}>
          Founder token · full scope
        </p>
        <p className="adm-num" style={{ margin: "4px 0 0", fontSize: "10px", color: "var(--adm-faint)" }}>
          all reads are logged
        </p>
      </div>
    </nav>
  );
}
