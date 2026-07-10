"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useEffect } from "react";

interface NavLink {
  href: string;
  label: string;
  /**
   * Section letter chip — the same A–H codes every page's `Card eyebrow`
   * already renders (e.g. `admin/coverage/page.tsx` uses "A1", "A2 / A3 /
   * A5", ...). Overview carries none: it's Command's single aggregate
   * screen, not one lettered section — no new taxonomy is invented here.
   */
  chip?: string;
}

interface NavGroup {
  label: string;
  links: readonly NavLink[];
}

// 5 groups / 9 links / letter chips A–H (goal 094): the old flat AdminNav's
// LINKS, regrouped by pipeline phase — Command (the one aggregate screen),
// Acquisition (getting data in), Refinery (turning it into Gold + checking
// it), Platform (storage + serving it out), Autonomy (infra + the agent
// loop running underneath). Order matches the old LINKS array exactly, so
// the digit shortcuts below (assigned by flattened position) don't shift
// meaning for anyone used to the old nav.
const GROUPS: readonly NavGroup[] = [
  { label: "Command", links: [{ href: "/admin", label: "Overview" }] },
  {
    label: "Acquisition",
    links: [
      { href: "/admin/coverage", label: "Coverage", chip: "A" },
      { href: "/admin/backfill", label: "Backfill", chip: "B" },
    ],
  },
  {
    label: "Refinery",
    links: [
      { href: "/admin/pipeline", label: "Pipeline", chip: "C" },
      { href: "/admin/quality", label: "Quality", chip: "D" },
    ],
  },
  {
    label: "Platform",
    links: [
      { href: "/admin/storage", label: "Storage", chip: "E" },
      { href: "/admin/serving", label: "Serving", chip: "F" },
    ],
  },
  {
    label: "Autonomy",
    links: [
      { href: "/admin/infra", label: "Infra", chip: "G" },
      { href: "/admin/loop", label: "Loop", chip: "H" },
    ],
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

// The grouped instrument-panel sidebar (goal 094): replaces the flat
// AdminNav top bar. Two bits of client-only interactivity, kept inline (one
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
      className="flex w-[var(--adm-sidebar-w)] shrink-0 flex-col gap-5 border-r border-[var(--adm-rule)] bg-[var(--adm-surface)] px-3 py-4"
    >
      {GROUPS.map((group) => (
        <div key={group.label} className="flex flex-col gap-1">
          <p className="adm-eyebrow mb-1 px-2">{group.label}</p>
          <ul className="m-0 flex list-none flex-col gap-0.5 p-0">
            {group.links.map((link) => {
              const current = pathname === link.href;
              const shortcut = FLAT_LINKS.indexOf(link) + 1;
              return (
                <li key={link.href}>
                  <Link
                    href={link.href}
                    aria-current={current ? "page" : undefined}
                    className={
                      current
                        ? "flex items-center gap-2 rounded-sm bg-[var(--adm-surface-sunken)] px-2 py-1.5 text-sm font-semibold text-[var(--adm-accent-deep)] no-underline"
                        : "flex items-center gap-2 rounded-sm px-2 py-1.5 text-sm text-[var(--adm-nav-inactive)] no-underline hover:bg-[var(--adm-surface-sunken)] hover:text-[var(--adm-nav-hover)]"
                    }
                  >
                    <span className="adm-num w-3 shrink-0 text-[0.6875rem] text-[var(--adm-faint)]">
                      {shortcut}
                    </span>
                    <span className="flex-1">{link.label}</span>
                    {link.chip !== undefined && (
                      <span
                        className="adm-num rounded-[2px] border px-1 py-0.5 text-[0.625rem] font-semibold"
                        style={
                          current
                            ? { color: "var(--adm-accent-deep)", borderColor: "var(--adm-accent)" }
                            : {
                                color: "var(--adm-nav-chip-inactive)",
                                borderColor: "var(--adm-chip-border)",
                              }
                        }
                      >
                        {link.chip}
                      </span>
                    )}
                  </Link>
                </li>
              );
            })}
          </ul>
        </div>
      ))}
    </nav>
  );
}
