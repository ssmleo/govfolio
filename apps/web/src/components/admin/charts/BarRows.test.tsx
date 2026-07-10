import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { BarRows } from "@/components/admin/charts/BarRows";

const ROWS = [
  { label: "below_threshold_match", value: 6 },
  { label: "ocr_low_confidence", value: 3 },
  { label: "zero_row_parse", value: 0 },
] as const;

function rowOf(label: string): HTMLElement {
  return screen.getByText(label).parentElement as HTMLElement;
}

function fillOf(label: string): HTMLElement {
  const track = rowOf(label).children[1] as HTMLElement;
  return track.children[0] as HTMLElement;
}

describe("BarRows width math", () => {
  it("fills Math.round(100 * v / max)% with max defaulting to the largest value", () => {
    render(<BarRows rows={ROWS} labelWidth={186} barHeight={12} fill="#B98A3B" valueWidth={28} />);
    expect(fillOf("below_threshold_match").style.width).toBe("100%");
    expect(fillOf("ocr_low_confidence").style.width).toBe("50%");
    expect(fillOf("zero_row_parse").style.width).toBe("0%");
  });

  it("honors an explicit max override", () => {
    render(
      <BarRows
        rows={ROWS}
        max={12}
        labelWidth={186}
        barHeight={12}
        fill="#B98A3B"
        valueWidth={28}
      />,
    );
    expect(fillOf("below_threshold_match").style.width).toBe("50%");
    expect(fillOf("ocr_low_confidence").style.width).toBe("25%");
  });

  it("all-zero rows render 0% fills, never NaN", () => {
    render(
      <BarRows
        rows={[
          { label: "a", value: 0 },
          { label: "b", value: 0 },
        ]}
        labelWidth={150}
        barHeight={6}
        fill="#8C7853"
        valueWidth={66}
      />,
    );
    expect(fillOf("a").style.width).toBe("0%");
    expect(fillOf("b").style.width).toBe("0%");
  });
});

describe("BarRows value column", () => {
  it("formats values with grouping by default and prefers a pre-formatted display", () => {
    render(
      <BarRows
        rows={[
          { label: "application/pdf", value: 29412 },
          { label: "qid coverage", value: 879, display: "87.9%" },
        ]}
        labelWidth={150}
        barHeight={6}
        fill="#8C7853"
        valueWidth={66}
      />,
    );
    expect(screen.getByText("29,412")).toBeInTheDocument();
    expect(screen.getByText("87.9%")).toBeInTheDocument();
    expect(screen.queryByText("879")).not.toBeInTheDocument();
  });

  it("reads values in full ink at 12px bars and secondary ink at 6px bars", () => {
    render(
      <BarRows
        rows={[{ label: "twelve", value: 5 }]}
        labelWidth={186}
        barHeight={12}
        fill="#B98A3B"
        valueWidth={28}
      />,
    );
    const value12 = rowOf("twelve").children[2] as HTMLElement;
    expect(value12.style.color).toBe("var(--adm-ink)");

    render(
      <BarRows
        rows={[{ label: "six", value: 5 }]}
        labelWidth={150}
        barHeight={6}
        fill="#8C7853"
        valueWidth={66}
      />,
    );
    const value6 = rowOf("six").children[2] as HTMLElement;
    expect(value6.style.color).toBe("var(--adm-text-secondary)");
  });
});

describe("BarRows layout variants", () => {
  it("track height follows barHeight and label width/alignment follow the props", () => {
    render(
      <BarRows
        rows={[{ label: "endpoint", value: 5 }]}
        labelWidth={236}
        labelAlign="left"
        barHeight={12}
        fill="#C2A15E"
        valueWidth={56}
      />,
    );
    const row = rowOf("endpoint");
    const label = row.children[0] as HTMLElement;
    const track = row.children[1] as HTMLElement;
    expect(label.style.width).toBe("236px");
    expect(label.style.textAlign).toBe("left");
    expect(track.style.height).toBe("12px");
  });

  it("ruled rows use border-top + 9px padding instead of a column gap (mime variant)", () => {
    const { container } = render(
      <BarRows
        rows={[
          { label: "application/pdf", value: 4 },
          { label: "text/html", value: 2 },
        ]}
        labelWidth={150}
        barHeight={6}
        fill="#8C7853"
        valueWidth={66}
        ruled
      />,
    );
    const wrapper = container.firstElementChild as HTMLElement;
    expect(wrapper.getAttribute("style")).toBeNull();
    const rowStyle = rowOf("application/pdf").getAttribute("style") ?? "";
    expect(rowStyle).toContain("border-top");
    expect(rowStyle).toContain("padding");
  });

  it("unruled rows stack in a flex column with the given gap", () => {
    const { container } = render(
      <BarRows
        rows={ROWS}
        labelWidth={186}
        barHeight={12}
        fill="#B98A3B"
        valueWidth={28}
        gap={10}
      />,
    );
    const wrapper = container.firstElementChild as HTMLElement;
    expect(wrapper.style.flexDirection).toBe("column");
    expect(wrapper.style.gap).toBe("10px");
  });
});
