// One visual language for "nothing to show, and here's exactly why" — used
// for the loop page's 503 (no repo checkout mounted) and for 401/403 auth
// failures on any admin page. Calm, not alarming: this is an expected,
// documented state, not an error the operator needs to act on urgently.
export function Unavailable({ reason }: { reason: string }) {
  return (
    <section
      aria-label="Unavailable"
      className="rounded-sm border border-[var(--adm-rule-strong)] bg-[var(--adm-surface)] px-5 py-6"
    >
      <p className="adm-eyebrow mb-1.5">Unavailable in this environment</p>
      <p className="text-[0.9375rem] text-[var(--adm-muted)]">{reason}</p>
    </section>
  );
}
