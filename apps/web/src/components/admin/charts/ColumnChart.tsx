import { formatCount } from "@/lib/format";

export interface ColumnDatum {
  bucket: string;
  count: number;
}

export interface ColumnChartProps {
  columns: readonly ColumnDatum[];
  /**
   * `linear` = review-age buckets (dc.html:1678); `sqrt` = delivery-attempt
   * buckets where one bucket dwarfs the rest and the tail still needs to be
   * visible (dc.html:1768, with a 3px floor).
   */
  scale: "linear" | "sqrt";
  height?: number;
  maxBarPx?: number;
  gap?: number;
}

/**
 * Small labeled column chart (dc.html:682-694 / 1037-1049): tabular count
 * above each bar, neutral bars flat on top corners, and a ruled bucket-label
 * row underneath. Bars scale to `maxBarPx` at the max count.
 */
export function ColumnChart({
  columns,
  scale,
  height = 110,
  maxBarPx = 80,
  gap = 16,
}: ColumnChartProps) {
  const max = Math.max(0, ...columns.map((c) => c.count));
  const barPx = (count: number): number => {
    const ratio = max > 0 ? count / max : 0;
    return scale === "sqrt"
      ? Math.max(3, Math.round(maxBarPx * Math.sqrt(ratio)))
      : Math.round(maxBarPx * ratio);
  };

  return (
    <div>
      <div
        style={{
          display: "flex",
          alignItems: "flex-end",
          gap: `${gap}px`,
          height: `${height}px`,
        }}
      >
        {columns.map((c) => (
          <div
            key={c.bucket}
            style={{
              flex: 1,
              display: "flex",
              flexDirection: "column",
              justifyContent: "flex-end",
              height: "100%",
              gap: "6px",
            }}
          >
            <span
              style={{
                textAlign: "center",
                fontFamily: "var(--adm-font-data)",
                fontSize: "11.5px",
                color: "var(--adm-ink)",
                fontVariantNumeric: "tabular-nums",
              }}
            >
              {formatCount(c.count)}
            </span>
            <div
              style={{
                width: "100%",
                height: `${barPx(c.count)}px`,
                background: "var(--adm-series-neutral)",
                borderRadius: "1px 1px 0 0",
              }}
            />
          </div>
        ))}
      </div>
      <div
        style={{
          display: "flex",
          gap: `${gap}px`,
          marginTop: "8px",
          borderTop: "1px solid var(--adm-rule-strong)",
          paddingTop: "6px",
        }}
      >
        {columns.map((c) => (
          <span
            key={c.bucket}
            style={{
              flex: 1,
              textAlign: "center",
              fontFamily: "var(--adm-font-data)",
              fontSize: "10.5px",
              color: "var(--adm-meta)",
            }}
          >
            {c.bucket}
          </span>
        ))}
      </div>
    </div>
  );
}
