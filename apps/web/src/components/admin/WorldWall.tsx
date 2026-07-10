"use client";

import { useState } from "react";

export interface WorldWallCell {
  name: string;
  phase: string;
  /** Resolved CSS color (the caller maps phase → `--adm-phase-*`). */
  color: string;
}

export interface WorldWallLegendItem {
  phase: string;
  color: string;
  count: number;
}

export interface WorldWallProps {
  cells: readonly WorldWallCell[];
  legend: readonly WorldWallLegendItem[];
  /** Jurisdiction total for the idle readout line. */
  total: number;
}

/**
 * THE SIGNATURE ELEMENT, redesign form (dc.html:127-141 + 1416-1427 +
 * 1944-1946): the world-coverage cell wall with a mono readout line that
 * echoes the hovered jurisdiction, plus the phase legend. The only client
 * chart component — hover readout is state; everything else stays inline
 * styles exactly as the design has them.
 */
export function WorldWall({ cells, legend, total }: WorldWallProps) {
  const [hovered, setHovered] = useState<number | null>(null);
  const hoverCell = hovered === null ? undefined : cells[hovered];

  return (
    <div>
      <div
        role="img"
        aria-label={`Coverage phase for ${total} jurisdictions`}
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fill,minmax(14px,1fr))",
          gap: "3px",
        }}
      >
        {cells.map((c, i) => (
          <span
            key={`${c.name}-${i}`}
            title={`${c.name} — ${c.phase}`}
            onMouseEnter={() => setHovered(i)}
            onMouseLeave={() => setHovered((current) => (current === i ? null : current))}
            style={{
              display: "block",
              aspectRatio: "1",
              borderRadius: "2px",
              background: c.color,
              cursor: "default",
              ...(hovered === i
                ? { outline: "1px solid var(--adm-accent-deep)", outlineOffset: "1px" }
                : undefined),
            }}
          />
        ))}
      </div>
      <p
        style={{
          margin: "14px 0 0",
          fontFamily: "var(--adm-font-data)",
          fontSize: "11.5px",
          color: hoverCell === undefined ? "var(--adm-meta)" : "var(--adm-accent-deep)",
          borderTop: "1px solid var(--adm-rule)",
          paddingTop: "12px",
        }}
      >
        {hoverCell === undefined
          ? `${total} jurisdictions · hover to inspect`
          : `${hoverCell.name} — ${hoverCell.phase}`}
      </p>
      <div style={{ display: "flex", flexWrap: "wrap", gap: "14px", marginTop: "12px" }}>
        {legend.map((l) => (
          <span
            key={l.phase}
            style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}
          >
            <span
              style={{
                width: "9px",
                height: "9px",
                borderRadius: "1px",
                background: l.color,
                display: "inline-block",
              }}
            />
            <span
              style={{
                fontFamily: "var(--adm-font-data)",
                fontSize: "11px",
                color: "var(--adm-text-secondary)",
                fontVariantNumeric: "tabular-nums",
              }}
            >
              {l.count}
            </span>
            <span style={{ fontSize: "10.5px", color: "var(--adm-meta)" }}>{l.phase}</span>
          </span>
        ))}
      </div>
    </div>
  );
}
