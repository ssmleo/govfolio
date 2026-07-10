export interface CardProps {
  /** Design-plan section ref, e.g. "A1" — renders as a gold "§ A1" in the eyebrow. */
  section?: string;
  /** Eyebrow label after the section ref, e.g. "World coverage". */
  label?: string;
  title?: string;
  /** Right-aligned metadata in the eyebrow row (mono 11px). */
  meta?: React.ReactNode;
  /** Right slot in the eyebrow row next to meta — e.g. a Badge or a toggle. */
  action?: React.ReactNode;
  /** Entry-animation delay in seconds (gfRise stagger); omit for no animation. */
  rise?: number;
  /** "danger" swaps the border to the danger rule (failures / DLQ / frozen sentinel). */
  tone?: "danger";
  /** Dashed placeholder card (dc.html:1101): 1px dashed rule, transparent, no shadow. */
  dashed?: boolean;
  /** Card h2 is 17px by default; the overview hero card (§ A1) uses 19. */
  titleSize?: 17 | 19;
  children: React.ReactNode;
  className?: string;
}

// The base panel of the instrument panel: a flat surface with a hairline
// border and subtle depth cue. Recipe dc.html:121 — border is
// --adm-card-border (#23272F), NOT the fainter --adm-rule (#1D222B).
export function Card({
  section,
  label,
  title,
  meta,
  action,
  rise,
  tone,
  dashed,
  titleSize,
  children,
  className,
}: CardProps) {
  const surface: React.CSSProperties = dashed
    ? {
        border: "1px dashed var(--adm-rule-strong)",
        borderRadius: 3,
        padding: "var(--adm-card-pad)",
      }
    : {
        background: "var(--adm-surface)",
        border: `1px solid ${tone === "danger" ? "var(--adm-danger-card-rule)" : "var(--adm-card-border)"}`,
        borderRadius: 3,
        padding: "var(--adm-card-pad)",
        boxShadow: "var(--adm-card-shadow)",
      };
  if (rise !== undefined) {
    surface.animation = `gfRise .5s ease ${rise}s both`;
  }

  const hasEyebrow =
    section !== undefined || label !== undefined || meta !== undefined || action !== undefined;

  return (
    <section className={className} style={surface}>
      {hasEyebrow && (
        <div
          style={{
            display: "flex",
            alignItems: "baseline",
            justifyContent: "space-between",
            gap: 12,
          }}
        >
          <p className="adm-card-eyebrow">
            {section !== undefined && (
              <span style={{ color: "var(--adm-accent)" }}>§ {section}</span>
            )}
            {section !== undefined && label !== undefined && " · "}
            {label}
          </p>
          {(meta !== undefined || action !== undefined) && (
            <span
              style={{ display: "inline-flex", alignItems: "baseline", gap: 12, flexShrink: 0 }}
            >
              {meta !== undefined && (
                <span
                  style={{
                    fontFamily: "var(--adm-font-data)",
                    fontSize: 11,
                    color: "var(--adm-meta)",
                    textAlign: "right",
                  }}
                >
                  {meta}
                </span>
              )}
              {action}
            </span>
          )}
        </div>
      )}
      {title !== undefined && (
        <h2 style={{ margin: "8px 0", ...(titleSize === 19 ? { fontSize: 19 } : undefined) }}>
          {title}
        </h2>
      )}
      {children}
    </section>
  );
}
