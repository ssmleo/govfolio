export interface YearBar {
  /** Native hover title on the bar. */
  year: string;
  value: number;
}

export interface YearBarsProps {
  /** Chronological; the LAST bar renders gold (the current year, dc.html:1437). */
  years: readonly YearBar[];
  firstLabel: string;
  lastLabel: string;
}

/**
 * Dossier year-coverage bars (dc.html:1285-1292 + mkYears 1430-1440): a 64px
 * flex row of neutral bars with a 4px floor, the last one gold, and faint
 * mono first/last labels underneath.
 */
export function YearBars({ years, firstLabel, lastLabel }: YearBarsProps) {
  const max = Math.max(0, ...years.map((y) => y.value));
  return (
    <div>
      <div style={{ display: "flex", alignItems: "flex-end", gap: "4px", height: "64px" }}>
        {years.map((y, i) => (
          <div
            key={y.year}
            title={y.year}
            style={{
              flex: 1,
              minWidth: "6px",
              borderRadius: "1px 1px 0 0",
              height: `${Math.max(4, max > 0 ? Math.round((56 * y.value) / max) : 0)}px`,
              background:
                i === years.length - 1
                  ? "var(--adm-series-gold)"
                  : "var(--adm-series-neutral)",
            }}
          />
        ))}
      </div>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          marginTop: "6px",
          fontFamily: "var(--adm-font-data)",
          fontSize: "10px",
          color: "var(--adm-faint)",
        }}
      >
        <span>{firstLabel}</span>
        <span>{lastLabel}</span>
      </div>
    </div>
  );
}
