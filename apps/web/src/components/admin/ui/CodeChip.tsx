const CHIP_COLOR = {
  meta: "var(--adm-meta)",
  neutral: "var(--adm-neutral-ink)",
  gold: "var(--adm-accent)",
} as const;

export interface CodeChipProps {
  children: React.ReactNode;
  /** Ink: meta grey (default; dossier codes), steel #9AA0AA (infra queue chips), or brand gold. */
  color?: keyof typeof CHIP_COLOR;
  /** Inline padding: "sm" 2px 8px (dossier chips, dc.html:1258) / "lg" 4px 10px (dc.html:1143). */
  size?: "sm" | "lg";
  /** Full-width command block (dc.html:488): gold text, 8px 10px pad, x-scrolls, never wraps. */
  block?: boolean;
  className?: string;
}

// Mono chip for regime codes, queue names, and shell commands — sunken
// surface, hairline border, 2px radius.
export function CodeChip({ children, color, size = "sm", block, className }: CodeChipProps) {
  const style: React.CSSProperties = {
    fontFamily: "var(--adm-font-data)",
    fontSize: 11,
    color: CHIP_COLOR[color ?? (block ? "gold" : "meta")],
    background: "var(--adm-surface-sunken)",
    border: "1px solid var(--adm-card-border)",
    borderRadius: 2,
    padding: size === "lg" ? "4px 10px" : "2px 8px",
  };
  if (block) {
    style.display = "block";
    style.padding = "8px 10px";
    style.overflowX = "auto";
    style.whiteSpace = "nowrap";
  }
  return (
    <span className={className} style={style}>
      {children}
    </span>
  );
}
