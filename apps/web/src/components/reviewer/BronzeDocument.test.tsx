import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { BronzeDocument } from "@/components/reviewer/BronzeDocument";
import { makeProvenance } from "@/test/fixtures";

describe("BronzeDocument (right half of the side-by-side)", () => {
  it("embeds OUR archived copy via the same-origin reviewer proxy, with a fallback link and the archived sha256", () => {
    const provenance = makeProvenance();
    render(<BronzeDocument provenance={provenance} />);
    // NOT the public /v1/filings/{id}/document endpoint directly — that's
    // tier-gated behind the 24h free-tier embargo, which would 404 a fresh
    // filing. Goes through the same-origin proxy instead (real-time,
    // admin-gated server-side).
    const expectedUrl = `/review/document/${provenance.filing.id}`;
    expect(screen.getByTitle("Archived source document")).toHaveAttribute(
      "src",
      expectedUrl,
    );
    expect(
      screen.getByRole("link", { name: "open the archived document directly" }),
    ).toHaveAttribute("href", expectedUrl);
    expect(screen.getByTestId("bronze-sha256")).toHaveTextContent(
      "sha256:94781947c3975677a2fa8f7839f6c0f074b3d3a2ff6019b3cfd8ee4942f6262e",
    );
  });
});
