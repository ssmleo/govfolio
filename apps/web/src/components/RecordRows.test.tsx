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

  it("shows one 'View filing' link per filing, not one per record row", () => {
    const sameFiling = [
      makeRecord({ id: "rec-1", filing_id: "filing-a" }),
      makeRecord({ id: "rec-2", filing_id: "filing-a" }),
    ];
    render(<RecordTable records={sameFiling} caption="test records" />);
    const links = screen.getAllByRole("link", { name: "View filing" });
    expect(links).toHaveLength(1);
    expect(links[0]).toHaveAttribute(
      "href",
      expect.stringContaining("/v1/filings/filing-a/document"),
    );
    expect(links[0]).toHaveAttribute("target", "_blank");
    expect(links[0]).toHaveAttribute("rel", "noopener noreferrer");
  });

  it("shows a separate 'View filing' link for each distinct filing", () => {
    const records = [
      makeRecord({ id: "rec-1", filing_id: "filing-a" }),
      makeRecord({ id: "rec-2", filing_id: "filing-b" }),
    ];
    render(<RecordTable records={records} caption="test records" />);
    expect(screen.getAllByRole("link", { name: "View filing" })).toHaveLength(2);
  });

  it("captions Brazil's filings as a bulk-source reconstruction, not a verbatim document", () => {
    const record = makeRecord({
      filing_id: "filing-br",
      regime_id: "0BRAREG0000000000000000001",
    });
    render(<RecordTable records={[record]} caption="test records" />);
    expect(
      screen.getByText(/reconstructed per-candidate from TSE's bulk disclosure files/),
    ).toBeInTheDocument();
  });
});
