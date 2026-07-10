export interface ScreenProps {
  /** data-screen-label value — stable hook for tests/tooling. */
  label: string;
  /** Gold uppercase kicker above the title, e.g. "Mission control". */
  kicker?: string;
  title: string;
  subtitle?: React.ReactNode;
  /** Right-aligned mono meta ("as of …"); use <br /> between lines. */
  meta?: React.ReactNode;
  children: React.ReactNode;
}

// Screen frame (server component): fade-in section + the standard header
// block every admin screen opens with — dc.html:107-117.
export function Screen({ label, kicker, title, subtitle, meta, children }: ScreenProps) {
  return (
    <section data-screen-label={label} style={{ animation: "gfFade .35s ease both" }}>
      <div
        style={{
          display: "flex",
          alignItems: "flex-end",
          justifyContent: "space-between",
          gap: 16,
          marginBottom: 24,
        }}
      >
        <div>
          {kicker !== undefined && (
            <p className="adm-kicker" style={{ margin: "0 0 7px" }}>
              {kicker}
            </p>
          )}
          <h1>{title}</h1>
          {subtitle !== undefined && (
            <p style={{ margin: "9px 0 0", color: "var(--adm-muted)", maxWidth: 580 }}>
              {subtitle}
            </p>
          )}
        </div>
        {meta !== undefined && (
          <div
            style={{
              textAlign: "right",
              fontFamily: "var(--adm-font-data)",
              fontSize: 11,
              color: "var(--adm-meta)",
              lineHeight: 1.8,
              flexShrink: 0,
            }}
          >
            {meta}
          </div>
        )}
      </div>
      {children}
    </section>
  );
}
