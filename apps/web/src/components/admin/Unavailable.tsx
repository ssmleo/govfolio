// One visual language for "nothing to show, and here's exactly why" — used
// for the loop page's 503 (no repo checkout mounted) and for 401/403 auth
// failures on any admin page. Calm, not alarming: this is an expected,
// documented state, not an error the operator needs to act on urgently.
// Styled as the design's dashed "not observable from here" card
// (dc.html:1101-1105): dashed rule, transparent ground, and a muted h2 —
// deliberately dimmer than a real card's heading ink.
export function Unavailable({ reason }: { reason: string }) {
  return (
    <section
      aria-label="Unavailable"
      className="rounded-[3px] border border-dashed border-[var(--adm-rule-strong)]"
      style={{ padding: "var(--adm-card-pad)" }}
    >
      <h2 style={{ margin: "0 0 10px", color: "var(--adm-muted)" }}>
        Unavailable in this environment
      </h2>
      <p style={{ margin: 0, fontSize: "12.5px", color: "var(--adm-meta)" }}>{reason}</p>
    </section>
  );
}
