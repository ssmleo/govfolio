import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { RecordTable } from "@/components/RecordRows";
import { makeRecord } from "@/test/fixtures";

describe("RecordTable", () => {
  it("renders money from decimal strings via the string-safe Intl path", () => {
    render(<RecordTable records={[makeRecord()]} caption="test records" />);
    expect(screen.getByText("$1,001 – $15,000")).toBeInTheDocument();
  });

  it("keeps giant declared values exact (no parseFloat in the render path)", () => {
    const record = makeRecord({
      value: {
        low: "90071992547409931.55",
        high: "90071992547409931.55",
        currency: "USD",
      },
    });
    render(<RecordTable records={[record]} caption="test records" />);
    expect(screen.getByText("$90,071,992,547,409,931.55")).toBeInTheDocument();
  });

  it("links each row to its record page and shows the verification state", () => {
    const record = makeRecord();
    render(<RecordTable records={[record]} caption="test records" />);
    expect(
      screen.getByRole("link", { name: record.asset_description_raw }),
    ).toHaveAttribute("href", `/r/${record.id}`);
    expect(screen.getByText("Unverified")).toBeInTheDocument();
  });

  it("renders an honest empty state", () => {
    render(<RecordTable records={[]} caption="test records" />);
    expect(screen.getByText(/No disclosure records yet/)).toBeInTheDocument();
  });
});
