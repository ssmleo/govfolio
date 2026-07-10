import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DensityColumns } from "@/components/admin/charts/DensityColumns";

// Two hours; segments arrive BOTTOM-UP (first = baseline series).
const HOURS = [
  {
    title: "03:00 UTC · 15 docs",
    segments: [
      { value: 10, color: "#C2A15E" },
      { value: 5, color: "#8FB2E8" },
    ],
  },
  {
    title: "04:00 UTC · 60 docs",
    segments: [
      { value: 20, color: "#C2A15E" },
      { value: 40, color: "#8FB2E8" },
    ],
  },
] as const;

function hourDiv(container: HTMLElement, index: number): HTMLElement {
  const root = container.firstElementChild as HTMLElement;
  const columnsRow = root.children[0] as HTMLElement;
  return columnsRow.children[index] as HTMLElement;
}

function segmentHeights(hour: HTMLElement): string[] {
  return Array.from(hour.children).map((seg) => (seg as HTMLElement).style.height);
}

describe("DensityColumns scaling", () => {
  it("scales segments to Math.round(114 * v / maxTotal) px against the max hour total", () => {
    const { container } = render(<DensityColumns hours={HOURS} />);
    // Max total = 20 + 40 = 60.
    // Hour A: 10 → round(114*10/60)=19px, 5 → round(9.5)=10px.
    // Hour B: 20 → 38px, 40 → 76px.
    // DOM order is reversed (see stacking test) — heights listed top-down.
    expect(segmentHeights(hourDiv(container, 0))).toEqual(["10px", "19px"]);
    expect(segmentHeights(hourDiv(container, 1))).toEqual(["76px", "38px"]);
  });

  it("honors an explicit maxTotal override", () => {
    const { container } = render(<DensityColumns hours={HOURS} maxTotal={120} />);
    // 40 → round(114*40/120)=38px, 20 → round(114*20/120)=19px.
    expect(segmentHeights(hourDiv(container, 1))).toEqual(["38px", "19px"]);
  });
});

describe("DensityColumns stacking order", () => {
  it("renders the first-given (baseline) segment as the LAST child of the column-justify-end flex", () => {
    const { container } = render(<DensityColumns hours={HOURS} />);
    const hourA = hourDiv(container, 0);
    expect(hourA.style.flexDirection).toBe("column");
    expect(hourA.style.justifyContent).toBe("flex-end");
    // First given segment (value 10 → 19px) must be the LAST DOM child so it
    // sits on the baseline (dc.html:621-626: house is the last child).
    const last = hourA.children[hourA.children.length - 1] as HTMLElement;
    expect(last.style.height).toBe("19px");
  });
});

describe("DensityColumns titles and labels", () => {
  it("puts the hour readout in the native title attribute", () => {
    render(<DensityColumns hours={HOURS} />);
    expect(screen.getByTitle("03:00 UTC · 15 docs")).toBeInTheDocument();
    expect(screen.getByTitle("04:00 UTC · 60 docs")).toBeInTheDocument();
  });

  it("defaults the x-labels to 48h ago / 24h ago / now", () => {
    render(<DensityColumns hours={HOURS} />);
    expect(screen.getByText("48h ago")).toBeInTheDocument();
    expect(screen.getByText("24h ago")).toBeInTheDocument();
    expect(screen.getByText("now")).toBeInTheDocument();
  });
});
