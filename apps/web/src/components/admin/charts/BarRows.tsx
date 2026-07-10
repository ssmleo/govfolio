import type { CSSProperties } from "react";

import { formatCount } from "@/lib/format";

export interface BarRow {
  label: string;
  value: number;
  /** Pre-formatted value column text; defaults to `formatCount(value)`. */
  display?: string;
  /** Native hover title on the row. */
  title?: string;
}

export interface BarRowsProps {
  rows: readonly BarRow[];
  /** Scale denominator; defaults to the max row value. */
  max?: number;
  labelWidth: number;
  labelAlign?: "left" | "right";
  /**
   * Replaces the default label type (mono 11px secondary ink) — the mime
   * variant (dc.html:824) passes its 11.5px/ink class here. The mono family
   * and column geometry stay either way.
   */
  labelClassName?: string;
  /** 12 = quality/endpoint rows (dc.html:657/963); 6 = mime rows (dc.html:825). */
  barHeight: 12 | 6;
  /** Bar fill color (raw or token). */
  fill: string;
  valueWidth: number;
  /** Ruled variant (dc.html:823): border-top + 9px vertical padding per row instead of a column gap. */
  ruled?: boolean;
  /** Column gap in px when not `ruled` (dc.html:653 quality rows use 10). */
  gap?: number;
}

/**
 * Horizontal labeled bar rows (dc.html:654-660 quality reasons, 960-967 API
 * endpoints, 822-828 storage mime). One row = fixed-width mono label, a
 * track with a single rounded fill, and a fixed-width tabular value column.
 * Fill width is `Math.round(100 * value / max)`% per the design's `fill()`
 * helper (dc.html:1388-1390).
 */
export function BarRows({
  rows,
  max,
  labelWidth,
  labelAlign = "left",
  labelClassName,
  barHeight,
  fill,
  valueWidth,
  ruled = false,
  gap = 10,
}: BarRowsProps) {
  const denom = max ?? Math.max(0, ...rows.map((r) => r.value));

  const labelStyle: CSSProperties = {
    width: `${labelWidth}px`,
    flexShrink: 0,
    fontFamily: "var(--adm-font-data)",
    textAlign: labelAlign,
    ...(labelClassName === undefined
      ? { fontSize: "11px", color: "var(--adm-text-secondary)" }
      : undefined),
  };

  return (
    <div style={ruled ? undefined : { display: "flex", flexDirection: "column", gap: `${gap}px` }}>
      {rows.map((row, i) => {
        const pct = denom > 0 ? Math.round((100 * row.value) / denom) : 0;
        return (
          <div
            key={`${row.label}-${i}`}
            title={row.title}
            style={{
              display: "flex",
              alignItems: "center",
              gap: "12px",
              ...(ruled
                ? { borderTop: "1px solid var(--adm-rule)", padding: "9px 0" }
                : undefined),
            }}
          >
            <span className={labelClassName} style={labelStyle}>
              {row.label}
            </span>
            <div
              style={{
                flex: 1,
                height: `${barHeight}px`,
                background: "var(--adm-progress-track)",
                borderRadius: "1px",
                overflow: "hidden",
              }}
            >
              <div
                style={{
                  width: `${pct}%`,
                  height: "100%",
                  background: fill,
                  borderRadius: "1px",
                }}
              />
            </div>
            <span
              style={{
                width: `${valueWidth}px`,
                flexShrink: 0,
                textAlign: "right",
                fontFamily: "var(--adm-font-data)",
                fontSize: "11.5px",
                // 6px (mime) rows read their value in secondary ink (dc.html:826);
                // 12px rows in full ink (dc.html:658/964).
                color: barHeight === 6 ? "var(--adm-text-secondary)" : "var(--adm-ink)",
                fontVariantNumeric: "tabular-nums",
              }}
            >
              {row.display ?? formatCount(row.value)}
            </span>
          </div>
        );
      })}
    </div>
  );
}
