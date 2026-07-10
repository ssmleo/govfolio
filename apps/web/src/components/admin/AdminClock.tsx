"use client";

import { useEffect, useState } from "react";

function formatUtcClock(date: Date): string {
  return `${date.toISOString().slice(11, 19)} UTC`;
}

// Masthead's one live pixel: a ticking UTC clock. Same setInterval-in-
// useEffect shape as AutoRefresh.tsx. Starts `null` (renders a placeholder)
// so the server-rendered markup and the client's first hydration pass
// match exactly — the real time is only ever read after mount, avoiding a
// hydration-mismatch warning. Split into its own file (rather than nested
// inline in Masthead.tsx) because "use client" is a whole-module boundary:
// Masthead stays server-rendered, and this is the one piece of it that
// isn't.
export function AdminClock() {
  const [now, setNow] = useState<Date | null>(null);

  useEffect(() => {
    setNow(new Date());
    const id = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(id);
  }, []);

  return (
    <span className="adm-num text-xs text-[var(--adm-muted)]">
      {now !== null ? formatUtcClock(now) : "--:--:-- UTC"}
    </span>
  );
}
