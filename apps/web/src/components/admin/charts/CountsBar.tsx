"use client";

import { Bar, BarChart, CartesianGrid, Legend, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";

export interface CountsBarSeries {
  key: string;
  label: string;
  /** Defaults to a neutral rotation; pass a `var(--adm-*-ink)` token to make one series a status claim (e.g. failures in danger). */
  color?: string;
}

export interface CountsBarProps<T extends Record<string, unknown>> {
  data: readonly T[];
  categoryKey: keyof T & string;
  series: readonly CountsBarSeries[];
  /** Stack series into one bar per category instead of grouping side by side. */
  stacked?: boolean;
  height?: number;
}

const DEFAULT_COLORS: readonly string[] = [
  "var(--adm-accent)",
  "var(--adm-info-ink)",
  "var(--adm-muted)",
  "var(--adm-warning-ink)",
];

// Grouped or stacked counts per category — funnel stage counts, tier
// breakdowns, that shape. Page agents supply a category key and one row
// per series; this never exposes raw Recharts props.
export function CountsBar<T extends Record<string, unknown>>({
  data,
  categoryKey,
  series,
  stacked = false,
  height = 220,
}: CountsBarProps<T>) {
  return (
    <ResponsiveContainer width="100%" height={height}>
      <BarChart data={data as T[]} margin={{ top: 4, right: 8, bottom: 0, left: 0 }}>
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
          width={36}
          allowDecimals={false}
        />
        <Tooltip
          contentStyle={{
            background: "var(--adm-surface)",
            border: "1px solid var(--adm-rule-strong)",
            fontSize: 12,
          }}
        />
        {series.length > 1 && <Legend wrapperStyle={{ fontSize: 11 }} />}
        {series.map((s, i) => (
          <Bar
            key={s.key}
            dataKey={s.key}
            name={s.label}
            stackId={stacked ? "stack" : undefined}
            fill={s.color ?? DEFAULT_COLORS[i % DEFAULT_COLORS.length]}
            radius={stacked ? undefined : [1, 1, 0, 0]}
          />
        ))}
      </BarChart>
    </ResponsiveContainer>
  );
}
