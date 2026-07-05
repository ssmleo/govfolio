import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { DisclosureRecord } from "@/lib/api";
import { TaskFields } from "@/components/reviewer/TaskFields";
import { makeRecord } from "@/test/fixtures";

describe("TaskFields (left half of the side-by-side)", () => {
  it("maps every extracted field of the record", () => {
    const record = makeRecord({
      details: JSON.parse(
        '{"doc_id":"20033759","amount_band_raw":"$1,001 - $15,000"}',
      ) as DisclosureRecord["details"],
    });
    render(<TaskFields record={record} />);

    // Raw description, exactly as filed (invariant 2).
    expect(screen.getByTestId("field-asset")).toHaveTextContent(
      "Boeing Company (BA) [ST]",
    );
    // Declared band from decimal strings (invariant 7).
    expect(screen.getByTestId("field-value")).toHaveTextContent("$1,001 – $15,000");
    // Dates.
    expect(screen.getAllByText("Dec 9, 2025")).toHaveLength(2); // transaction + notified
    // Owner.
    expect(screen.getByTestId("field-owner")).toHaveTextContent("self");
    // Type and class.
    expect(screen.getByText(/transaction · sell/)).toBeInTheDocument();
    expect(screen.getByText("equity")).toBeInTheDocument();
    // Extraction provenance.
    expect(screen.getByText("us_house_ptr/text@1")).toBeInTheDocument();
    expect(screen.getByText("98%")).toBeInTheDocument();
    expect(screen.getByText(record.fingerprint)).toBeInTheDocument();
    expect(screen.getByText(record.id)).toBeInTheDocument();
    // Verification state travels with the record.
    expect(screen.getByText("Unverified")).toBeInTheDocument();
    // The contract-typed details payload, verbatim.
    expect(screen.getByTestId("field-details")).toHaveTextContent('"doc_id": "20033759"');
    expect(screen.getByTestId("field-details")).toHaveTextContent(
      '"amount_band_raw": "$1,001 - $15,000"',
    );
  });

  it("is honest about an unresolved instrument (never guessed)", () => {
    render(<TaskFields record={makeRecord({ instrument_id: null })} />);
    expect(
      screen.getByText("Not resolved — left unlinked rather than guessed"),
    ).toBeInTheDocument();
  });
});
