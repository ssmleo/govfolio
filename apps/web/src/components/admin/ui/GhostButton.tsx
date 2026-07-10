"use client";

export type GhostButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement>;

// Gold ghost action button — dc.html:778 ("Run collision sweep").
// Background lives in classes (not the style object) so the hover
// variant can win; 10.5px is design-critical half-px type, so the
// font-size stays in an explicit style object.
export function GhostButton({ className, style, children, type, ...rest }: GhostButtonProps) {
  return (
    <button
      type={type ?? "button"}
      {...rest}
      className={`rounded-[2px] border border-[var(--adm-gold-45)] bg-[var(--adm-gold-08)] hover:bg-[var(--adm-gold-16)] font-bold uppercase tracking-[.14em] cursor-pointer ${className ?? ""}`}
      style={{
        padding: "8px 16px",
        fontFamily: "var(--adm-font-body)",
        fontSize: "10.5px",
        color: "var(--adm-accent-deep)",
        transition: "background .15s ease",
        ...style,
      }}
    >
      {children}
    </button>
  );
}
