import type { AdminHeatmapCell } from "@/lib/api";

export interface RegimeYearHeatmapProps {
  cells: readonly AdminHeatmapCell[];
  regimeLabel: (regimeId: string) => string;
}

interface YearTotal {
  total: number;
  byType: Record<string, number>;
}

// A2/A4's density companion to the phase heatmap above: same "small colored
// cells, hover for detail" language, but one continuous dimension (record
// volume) instead of eight discrete phases. Rows are the regimes that
// actually carry a dated record — a regime with none simply has no row,
// which is itself the honest signal (see heatmap_missing_event_date for the
// undated remainder, surfaced by the caller).
export function RegimeYearHeatmap({ cells, regimeLabel }: RegimeYearHeatmapProps) {
  const [firstCell] = cells;
  if (firstCell === undefined) {
    return <p className="text-sm text-[var(--adm-muted)]">No dated Gold records yet.</p>;
  }

  const byRegime = new Map<string, Map<number, YearTotal>>();
  let minYear = firstCell.year;
  let maxYear = firstCell.year;
  let maxCellTotal = 0;

  for (const cell of cells) {
    minYear = Math.min(minYear, cell.year);
    maxYear = Math.max(maxYear, cell.year);
    let years = byRegime.get(cell.regime_id);
    if (years === undefined) {
      years = new Map<number, YearTotal>();
      byRegime.set(cell.regime_id, years);
    }
    let bucket = years.get(cell.year);
    if (bucket === undefined) {
      bucket = { total: 0, byType: {} };
      years.set(cell.year, bucket);
    }
    bucket.total += cell.records;
    bucket.byType[cell.record_type] = (bucket.byType[cell.record_type] ?? 0) + cell.records;
    maxCellTotal = Math.max(maxCellTotal, bucket.total);
  }

  const years: number[] = [];
  for (let y = minYear; y <= maxYear; y += 1) {
    years.push(y);
  }

  const regimeIds = [...byRegime.keys()].sort((a, b) =>
    regimeLabel(a).localeCompare(regimeLabel(b)),
  );

  return (
    <div className="overflow-x-auto">
      <div
        className="grid gap-[3px]"
        style={{
          gridTemplateColumns: `minmax(11rem, auto) repeat(${years.length}, minmax(2.25rem, 1fr))`,
        }}
      >
        <div />
        {years.map((y) => (
          <div
            key={y}
            className="adm-num pb-1 text-center text-[0.6875rem] text-[var(--adm-muted)]"
          >
            {y}
          </div>
        ))}
        {regimeIds.map((regimeId) => (
          <RegimeRow
            key={regimeId}
            label={regimeLabel(regimeId)}
            years={years}
            data={byRegime.get(regimeId)}
            maxCellTotal={maxCellTotal}
          />
        ))}
      </div>
    </div>
  );
}

function RegimeRow({
  label,
  years,
  data,
  maxCellTotal,
}: {
  label: string;
  years: readonly number[];
  data: Map<number, YearTotal> | undefined;
  maxCellTotal: number;
}) {
  return (
    <>
      <div className="truncate py-1 pr-3 text-xs text-[var(--adm-ink)]" title={label}>
        {label}
      </div>
      {years.map((y) => {
        const cell = data?.get(y);
        const total = cell?.total ?? 0;
        const intensity = maxCellTotal > 0 ? total / maxCellTotal : 0;
        const background =
          total === 0
            ? "var(--adm-surface-sunken)"
            : `color-mix(in srgb, var(--adm-accent) ${Math.round(10 + intensity * 85)}%, var(--adm-surface-sunken))`;
        const title =
          cell === undefined
            ? `${label} · ${y} — no dated records`
            : `${label} · ${y} — ${cell.total.toLocaleString()} record${cell.total === 1 ? "" : "s"} (${Object.entries(
                cell.byType,
              )
                .sort((a, b) => b[1] - a[1])
                .map(([type, count]) => `${type}: ${count.toLocaleString()}`)
                .join(", ")})`;
        return (
          <div key={y} title={title} style={{ aspectRatio: "1 / 1", background, borderRadius: "1px" }} />
        );
      })}
    </>
  );
}
