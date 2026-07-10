import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { FunnelRows } from "@/components/admin/charts/FunnelRows";

// us_house from the design (dc.html:1541), max = maxCand = 1842.
const ROWS = [
  {
    adapter: "us_house",
    segments: [
      { value: 1731, color: "#3F8E68" },
      { value: 64, color: "#B98A3B" },
      { value: 47, color: "#A34E45" },
    ],
    totalLabel: "1,842",
  },
  {
    adapter: "us_senate",
    segments: [
      { value: 0, color: "#3F8E68" },
      { value: 0, color: "#B98A3B" },
      { value: 0, color: "#A34E45" },
    ],
    totalLabel: "frozen",
    totalTone: "danger" as const,
  },
] as const;

function trackOf(container: HTMLElement, rowIndex: number): HTMLElement {
  const root = container.firstElementChild as HTMLElement;
  const row = root.children[rowIndex] as HTMLElement;
  return row.children[1] as HTMLElement;
}

function segmentWidths(track: HTMLElement): string[] {
  return Array.from(track.children).map((seg) => (seg as HTMLElement).style.width);
}

describe("FunnelRows width math", () => {
  it("segments take the exact unrounded (100 * v / max)% of the shared scale", () => {
    const { container } = render(<FunnelRows rows={ROWS} max={1842} />);
    expect(segmentWidths(trackOf(container, 0))).toEqual([
      `${(100 * 1731) / 1842}%`,
      `${(100 * 64) / 1842}%`,
      `${(100 * 47) / 1842}%`,
    ]);
  });

  it("scales against the given max, not the row total", () => {
    const { container } = render(
      <FunnelRows
        rows={[
          {
            adapter: "solo",
            segments: [{ value: 25, color: "#3F8E68" }],
            totalLabel: "25",
          },
        ]}
        max={100}
      />,
    );
    expect(segmentWidths(trackOf(container, 0))).toEqual(["25%"]);
  });
});

describe("FunnelRows zero/frozen handling", () => {
  it("zero-value segments collapse to 0% (frozen adapters draw an empty track)", () => {
    const { container } = render(<FunnelRows rows={ROWS} max={1842} />);
    expect(segmentWidths(trackOf(container, 1))).toEqual(["0%", "0%", "0%"]);
  });

  it("renders the frozen total in danger ink and normal totals in secondary ink", () => {
    render(<FunnelRows rows={ROWS} max={1842} />);
    expect(screen.getByText("frozen").style.color).toBe("var(--adm-danger-ink)");
    expect(screen.getByText("1,842").style.color).toBe("var(--adm-text-secondary)");
  });

  it("a non-positive max never yields NaN widths", () => {
    const { container } = render(
      <FunnelRows
        rows={[
          {
            adapter: "empty",
            segments: [{ value: 0, color: "#3F8E68" }],
            totalLabel: "0",
          },
        ]}
        max={0}
      />,
    );
    expect(segmentWidths(trackOf(container, 0))).toEqual(["0%"]);
  });
});
