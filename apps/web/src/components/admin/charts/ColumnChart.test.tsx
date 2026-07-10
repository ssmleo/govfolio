import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ColumnChart } from "@/components/admin/charts/ColumnChart";

// Delivery-attempt distribution from the design (dc.html:1766-1769).
const SQRT_COLUMNS = [
  { bucket: "1", count: 29412 },
  { bucket: "2", count: 1206 },
  { bucket: "3", count: 244 },
  { bucket: "≥4", count: 19 },
] as const;

// Review-age buckets from the design (dc.html:1671/1678).
const LINEAR_COLUMNS = [
  { bucket: "<1d", count: 4 },
  { bucket: "1–7d", count: 7 },
  { bucket: "7–30d", count: 2 },
  { bucket: ">30d", count: 1 },
] as const;

function barHeights(container: HTMLElement): string[] {
  const root = container.firstElementChild as HTMLElement;
  const columnsRow = root.children[0] as HTMLElement;
  return Array.from(columnsRow.children).map(
    (col) => (col.children[1] as HTMLElement).style.height,
  );
}

describe("ColumnChart scaling", () => {
  it("sqrt: Math.max(3, Math.round(80 * sqrt(v/max))) px per bar, incl. the 3px floor", () => {
    const { container } = render(<ColumnChart columns={SQRT_COLUMNS} scale="sqrt" />);
    // 29412 → 80 (max); 1206 → round(80*0.20249)=16; 244 → round(80*0.09108)=7;
    // 19 → round(80*0.02542)=2, floored to 3.
    expect(barHeights(container)).toEqual(["80px", "16px", "7px", "3px"]);
  });

  it("linear: Math.round(80 * v/max) px per bar, no floor", () => {
    const { container } = render(<ColumnChart columns={LINEAR_COLUMNS} scale="linear" />);
    // max 7 → round(80*4/7)=46, 80, round(80*2/7)=23, round(80*1/7)=11.
    expect(barHeights(container)).toEqual(["46px", "80px", "23px", "11px"]);
  });

  it("respects a custom maxBarPx", () => {
    const { container } = render(
      <ColumnChart columns={LINEAR_COLUMNS} scale="linear" maxBarPx={40} />,
    );
    expect(barHeights(container)).toEqual(["23px", "40px", "11px", "6px"]);
  });

  it("an all-zero series renders 0px bars linear / 3px-floored bars sqrt, no NaN", () => {
    const zeros = [
      { bucket: "a", count: 0 },
      { bucket: "b", count: 0 },
    ];
    const linear = render(<ColumnChart columns={zeros} scale="linear" />).container;
    expect(barHeights(linear)).toEqual(["0px", "0px"]);

    const sqrt = render(<ColumnChart columns={zeros} scale="sqrt" />).container;
    expect(barHeights(sqrt)).toEqual(["3px", "3px"]);
  });
});

describe("ColumnChart labels and counts", () => {
  it("formats counts with grouping and repeats buckets in the ruled label row", () => {
    render(<ColumnChart columns={SQRT_COLUMNS} scale="sqrt" />);
    expect(screen.getByText("29,412")).toBeInTheDocument();
    expect(screen.getByText("1,206")).toBeInTheDocument();
    expect(screen.getByText("≥4")).toBeInTheDocument();
  });

  it("uses the 110px default frame height", () => {
    const { container } = render(<ColumnChart columns={LINEAR_COLUMNS} scale="linear" />);
    const columnsRow = (container.firstElementChild as HTMLElement).children[0] as HTMLElement;
    expect(columnsRow.style.height).toBe("110px");
  });
});
