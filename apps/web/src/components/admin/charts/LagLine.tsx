"use client";

import { CartesianGrid, Legend, Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";

export interface LagLineSeries {
  key: string;
  label: string;
  color?: string;
}

export interface LagLineProps<T extends Record<string, unknown>> {
  data: readonly T[];
  categoryKey: keyof T & string;
  /** Usually two series (e.g. p50 vs p90) — the "dual-line comparison" shape. */
  series: readonly LagLineSeries[];
  height?: number;
}

const DEFAULT_COLORS: readonly string[] = ["var(--adm-accent)", "var(--adm-info-ink)"];

// Dual-line comparison over a shared category axis — p50 vs p90 lag,
// current vs prior period. Narrow on purpose: for one series see
// `TrendArea`, for buckets see `Histogram`.
export function LagLine<T extends Record<string, unknown>>({
  data,
  categoryKey,
  series,
  height = 200,
}: LagLineProps<T>) {
  return (
    <ResponsiveContainer width="100%" height={height}>
      <LineChart data={data as T[]} margin={{ top: 4, right: 8, bottom: 0, left: 0 }}>
        <CartesianGrid stroke="var(--adm-rule)" vertical={false} />
        <XAxis
          dataKey={categoryKey as string}
          tick={{ fontSize: 11, fill: "var(--adm-muted)" }}
          tickLine={false}
          axisLine={{ stroke: "var(--adm-rule-strong)" }}
        />
        <YAxis
          tick={{ fontSize: 11, fill: "var(--adm-muted)" }}
          tickLine={false}
          axisLine={false}
          width={40}
        />
        <Tooltip
          contentStyle={{
            background: "var(--adm-surface)",
            border: "1px solid var(--adm-rule-strong)",
            fontSize: 12,
          }}
        />
        <Legend wrapperStyle={{ fontSize: 11 }} />
        {series.map((s, i) => (
          <Line
            key={s.key}
            type="monotone"
            dataKey={s.key}
            name={s.label}
            stroke={s.color ?? DEFAULT_COLORS[i % DEFAULT_COLORS.length]}
            strokeWidth={1.5}
            dot={false}
          />
        ))}
      </LineChart>
    </ResponsiveContainer>
  );
}
