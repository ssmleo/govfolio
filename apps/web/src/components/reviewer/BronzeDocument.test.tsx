import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { BronzeDocument } from "@/components/reviewer/BronzeDocument";
import { makeProvenance } from "@/test/fixtures";

const PDF_URL =
  "https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2026/20033759.pdf";

describe("BronzeDocument (right half of the side-by-side)", () => {
  it("embeds the official PDF with a fallback link and the archived sha256", () => {
    render(<BronzeDocument provenance={makeProvenance()} />);
    expect(screen.getByTitle("Official source document")).toHaveAttribute(
      "src",
      PDF_URL,
    );
    expect(
      screen.getByRole("link", { name: "open the official PDF directly" }),
    ).toHaveAttribute("href", PDF_URL);
    expect(screen.getByTestId("bronze-sha256")).toHaveTextContent(
      "sha256:94781947c3975677a2fa8f7839f6c0f074b3d3a2ff6019b3cfd8ee4942f6262e",
    );
  });

  it("stays honest when no source URL was recorded", () => {
    const provenance = makeProvenance();
    provenance.raw_document = { ...provenance.raw_document, source_url: null };
    render(<BronzeDocument provenance={provenance} />);
    expect(screen.queryByTitle("Official source document")).not.toBeInTheDocument();
    expect(
      screen.getByText(/Source URL not recorded for this document/),
    ).toBeInTheDocument();
    expect(screen.getByTestId("bronze-sha256")).toBeInTheDocument();
  });
});
