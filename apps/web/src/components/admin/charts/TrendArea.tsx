"use client";

import {
  Area,
  AreaChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

export interface TrendAreaPoint {
  x: string;
  y: number;
}

export interface TrendAreaProps {
  data: readonly TrendAreaPoint[];
  /** Tooltip value label, e.g. "records" or "filings". */
  valueLabel?: string;
  height?: number;
}

// One time series as a filled area, brand-accent by default — growth days,
// usage-by-day, that shape. A thin, purpose-built wrapper: page agents pass
// {x, y} points, never raw Recharts props.
export function TrendArea({ data, valueLabel = "value", height = 180 }: TrendAreaProps) {
  return (
    <ResponsiveContainer width="100%" height={height}>
      <AreaChart data={data as TrendAreaPoint[]} margin={{ top: 4, right: 8, bottom: 0, left: 0 }}>
        <CartesianGrid stroke="var(--adm-rule)" vertical={false} />
        <XAxis
          dataKey="x"
          tick={{ fontSize: 11, fill: "var(--adm-muted)" }}
          tickLine={false}
          axisLine={{ stroke: "var(--adm-rule-strong)" }}
        />
        <YAxis
          tick={{ fontSize: 11, fill: "var(--adm-muted)" }}
          tickLine={false}
          axisLine={false}
          width={40}
          allowDecimals={false}
        />
        <Tooltip
          contentStyle={{
            background: "var(--adm-surface)",
            border: "1px solid var(--adm-rule-strong)",
            fontSize: 12,
          }}
          formatter={(v) => [v, valueLabel]}
        />
        <Area
          type="monotone"
          dataKey="y"
          stroke="var(--adm-accent)"
          fill="var(--adm-accent)"
          fillOpacity={0.15}
          strokeWidth={1.5}
        />
      </AreaChart>
    </ResponsiveContainer>
  );
}
