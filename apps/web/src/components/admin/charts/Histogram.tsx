"use client";

import { Bar, BarChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";

export interface HistogramBucket {
  bucket: string;
  count: number;
}

export interface HistogramProps {
  data: readonly HistogramBucket[];
  color?: string;
  height?: number;
}

// A single-series distribution — delivery attempts, age buckets, that
// shape. Deliberately narrower than `CountsBar` (one series, no legend):
// this is the shape for "how many things fall in each bucket".
export function Histogram({ data, color = "var(--adm-accent)", height = 160 }: HistogramProps) {
  return (
    <ResponsiveContainer width="100%" height={height}>
      <BarChart data={data as HistogramBucket[]} margin={{ top: 4, right: 8, bottom: 0, left: 0 }}>
        <CartesianGrid stroke="var(--adm-rule)" vertical={false} />
        <XAxis
          dataKey="bucket"
          tick={{ fontSize: 11, fill: "var(--adm-muted)" }}
          tickLine={false}
          axisLine={{ stroke: "var(--adm-rule-strong)" }}
        />
        <YAxis
          tick={{ fontSize: 11, fill: "var(--adm-muted)" }}
          tickLine={false}
          axisLine={false}
          width={32}
          allowDecimals={false}
        />
        <Tooltip
          contentStyle={{
            background: "var(--adm-surface)",
            border: "1px solid var(--adm-rule-strong)",
            fontSize: 12,
          }}
        />
        <Bar dataKey="count" fill={color} radius={[1, 1, 0, 0]} />
      </BarChart>
    </ResponsiveContainer>
  );
}
