import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { TrendChart } from "@/components/admin/charts/TrendChart";

// Known series: max 60, 4 points.
const POINTS = [
  { label: "a", value: 10 },
  { label: "b", value: 40 },
  { label: "c", value: 25 },
  { label: "d", value: 60 },
] as const;

// mkTrend (dc.html:1587-1592), computed by hand.
//
// wide  (W=560, bottom=130, top=12, span=118):
//   x: 0, round(186.667)=187, round(373.333)=373, 560
//   y: round(130-(10/60)*118)=110, round(130-(40/60)*118)=51,
//      round(130-(25/60)*118)=81,  130-118=12
const WIDE_PATH = "M0,110 L187,51 L373,81 L560,12";
const WIDE_AREA = `${WIDE_PATH} L560,130 L0,130 Z`;
// small (W=420, bottom=95, top=10, span=85):
//   x: 0, 140, 280, 420
//   y: round(95-(10/60)*85)=81, round(95-(40/60)*85)=38,
//      round(95-(25/60)*85)=60, 95-85=10
const SMALL_PATH = "M0,81 L140,38 L280,60 L420,10";
const SMALL_AREA = `${SMALL_PATH} L420,95 L0,95 Z`;

function pathsOf(container: HTMLElement): { area: string | null; line: string | null } {
  const els = container.querySelectorAll("path");
  return {
    area: els.item(0).getAttribute("d"),
    line: els.item(1).getAttribute("d"),
  };
}

describe("TrendChart geometry (mkTrend port)", () => {
  it("wide: emits the exact rounded path, closed area, and endpoint dot coords", () => {
    const { container } = render(
      <TrendChart points={POINTS} size="wide" endpointDot ariaLabel="wide trend" />,
    );
    const svg = container.querySelector("svg");
    expect(svg).toHaveAttribute("viewBox", "0 0 560 150");
    expect(svg).toHaveAttribute("preserveAspectRatio", "none");

    const { area, line } = pathsOf(container);
    expect(line).toBe(WIDE_PATH);
    expect(area).toBe(WIDE_AREA);

    const dot = container.querySelector("circle");
    expect(dot).toHaveAttribute("cx", "560");
    expect(dot).toHaveAttribute("cy", "12");
    expect(dot).toHaveAttribute("r", "3");
  });

  it("small: emits the exact rounded path and closed area for the 420×110 frame", () => {
    const { container } = render(
      <TrendChart points={POINTS} size="small" ariaLabel="small trend" />,
    );
    expect(container.querySelector("svg")).toHaveAttribute("viewBox", "0 0 420 110");

    const { area, line } = pathsOf(container);
    expect(line).toBe(SMALL_PATH);
    expect(area).toBe(SMALL_AREA);
  });

  it("wide draws two interior hairlines plus the baseline; small only the baseline", () => {
    const wide = render(
      <TrendChart points={POINTS} size="wide" ariaLabel="wide trend" />,
    ).container;
    expect(wide.querySelectorAll("line")).toHaveLength(3);

    const small = render(
      <TrendChart points={POINTS} size="small" ariaLabel="small trend" />,
    ).container;
    expect(small.querySelectorAll("line")).toHaveLength(1);
  });

  it("omits the endpoint dot unless endpointDot is set", () => {
    const { container } = render(
      <TrendChart points={POINTS} size="wide" ariaLabel="wide trend" />,
    );
    expect(container.querySelector("circle")).toBeNull();
  });
});

describe("TrendChart degenerate-series guards", () => {
  it("a single point collapses to the flat baseline path with no NaN (wide)", () => {
    const { container } = render(
      <TrendChart
        points={[{ label: "only", value: 5 }]}
        size="wide"
        endpointDot
        ariaLabel="one point"
      />,
    );
    const { area, line } = pathsOf(container);
    expect(line).toBe("M0,130 L560,130");
    expect(area).toBe("M0,130 L560,130 L560,130 L0,130 Z");
    expect(line).not.toContain("NaN");
    expect(container.querySelector("circle")).toHaveAttribute("cy", "130");
  });

  it("an all-zero series collapses to the flat baseline path with no NaN (small)", () => {
    const { container } = render(
      <TrendChart
        points={[
          { label: "a", value: 0 },
          { label: "b", value: 0 },
          { label: "c", value: 0 },
        ]}
        size="small"
        ariaLabel="all zero"
      />,
    );
    const { area, line } = pathsOf(container);
    expect(line).toBe("M0,95 L420,95");
    expect(area).toBe("M0,95 L420,95 L420,95 L0,95 Z");
    expect(area).not.toContain("NaN");
  });
});

describe("TrendChart chrome", () => {
  it("exposes the aria label as the svg accessible name", () => {
    render(<TrendChart points={POINTS} size="wide" ariaLabel="Gold records per month" />);
    expect(screen.getByRole("img", { name: "Gold records per month" })).toBeInTheDocument();
  });

  it("renders the x-range labels row only when labels are given", () => {
    render(
      <TrendChart
        points={POINTS}
        size="wide"
        xLeftLabel="Aug 2025"
        xRightLabel="Jul 2026"
        ariaLabel="labeled"
      />,
    );
    expect(screen.getByText("Aug 2025")).toBeInTheDocument();
    expect(screen.getByText("Jul 2026")).toBeInTheDocument();

    const { container } = render(
      <TrendChart points={POINTS} size="small" ariaLabel="unlabeled" />,
    );
    // No labels given → the wrapper holds the svg alone, no labels row.
    expect(container.firstElementChild?.children).toHaveLength(1);
  });
});
