import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { CorrectionItem } from "@/components/CorrectionsLog";
import { CorrectionsLog } from "@/components/CorrectionsLog";
import { makeRecord } from "@/test/fixtures";

const ORIGINAL_ID = "01KWQVPG6B08S4VX92NZED3C16";
const CORRECTION_ID = "01KWQVPG6B08S4VX92NZED3C99";

function correctedPair(): CorrectionItem {
  const superseded = makeRecord({
    id: ORIGINAL_ID,
    verification_state: "unverified",
    value: { low: "1001.00", high: "15000.00", currency: "USD" },
  });
  const correction = makeRecord({
    id: CORRECTION_ID,
    verification_state: "corrected",
    supersedes_record_id: ORIGINAL_ID,
    value: { low: "15001.00", high: "50000.00", currency: "USD" },
  });
  return { correction, superseded };
}

describe("CorrectionsLog", () => {
  it("lists a corrected record linking to it and the original it supersedes", () => {
    render(<CorrectionsLog items={[correctedPair()]} />);

    const entry = screen.getByTestId("correction-entry");
    // Links to the correction's own record page (the full history + provenance).
    expect(within(entry).getByRole("link", { name: /Boeing/ })).toHaveAttribute(
      "href",
      `/r/${CORRECTION_ID}`,
    );
    // Links to the earlier record it supersedes (invariant 1: original kept).
    expect(within(entry).getByTestId("superseded-link")).toHaveAttribute(
      "href",
      `/r/${ORIGINAL_ID}`,
    );
    // The corrected badge travels with the entry, labeled honestly by text.
    expect(within(entry).getByText("Corrected")).toHaveClass("badge", "badge-corrected");
  });

  it("shows what changed at a glance: the declared value before and after", () => {
    const { container } = render(<CorrectionsLog items={[correctedPair()]} />);

    const before = container.querySelector(".diff-before");
    const after = container.querySelector(".diff-after");
    expect(before?.textContent).toContain("$1,001");
    expect(before?.textContent).toContain("$15,000");
    expect(after?.textContent).toContain("$15,001");
    expect(after?.textContent).toContain("$50,000");
  });

  it("omits fields that did not change (states only what was corrected)", () => {
    // Same asset and event date on both versions — only the value differs.
    render(<CorrectionsLog items={[correctedPair()]} />);

    const rowHeaders = screen.getAllByRole("rowheader").map((cell) => cell.textContent);
    expect(rowHeaders).toEqual(["Declared value"]);
  });

  it("renders an honest empty state when nothing has been corrected", () => {
    render(<CorrectionsLog items={[]} />);

    const empty = screen.getByTestId("corrections-empty");
    expect(empty).toHaveTextContent(/No corrections on record/);
    expect(screen.queryByTestId("correction-entry")).toBeNull();
  });
});
