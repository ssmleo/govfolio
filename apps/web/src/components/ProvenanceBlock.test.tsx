import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ProvenanceBlock } from "@/components/ProvenanceBlock";
import { makeProvenance } from "@/test/fixtures";

describe("ProvenanceBlock", () => {
  it("shows the official source link, sha256, and fetch time", () => {
    const provenance = makeProvenance();
    render(<ProvenanceBlock provenance={provenance} />);

    const sourceUrl = provenance.raw_document.source_url ?? "";
    expect(screen.getByRole("link", { name: sourceUrl })).toHaveAttribute(
      "href",
      sourceUrl,
    );
    expect(screen.getByTestId("sha256")).toHaveTextContent(
      `sha256:${provenance.raw_document.sha256}`,
    );
    expect(screen.getByText(/fetched/)).toHaveTextContent("Jul 5, 2026");
    expect(screen.getByText(/fetched/)).toHaveTextContent("UTC");
  });

  it("links the regime to its jurisdiction page", () => {
    render(<ProvenanceBlock provenance={makeProvenance()} />);
    expect(screen.getByRole("link", { name: "US House" })).toHaveAttribute(
      "href",
      "/jurisdictions/us",
    );
  });

  it("says so plainly when no source URL was recorded", () => {
    const provenance = makeProvenance();
    provenance.raw_document = { ...provenance.raw_document, source_url: null };
    render(<ProvenanceBlock provenance={provenance} />);
    expect(
      screen.getByText("Source URL not recorded for this document"),
    ).toBeInTheDocument();
  });
});
