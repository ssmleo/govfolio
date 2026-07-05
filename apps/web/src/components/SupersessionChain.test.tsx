import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { SupersessionChain } from "@/components/SupersessionChain";
import { makeRecord } from "@/test/fixtures";

describe("SupersessionChain", () => {
  it("states plainly when there are no corrections", () => {
    render(<SupersessionChain supersedes={[]} supersededBy={[]} />);
    expect(screen.getByTestId("no-supersession")).toHaveTextContent(
      "only version of this record",
    );
  });

  it("lists corrections that supersede this record, with links", () => {
    const correction = makeRecord({
      id: "01KWQVPG6B08S4VX92NZED3C99",
      verification_state: "corrected",
      asset_description_raw: "Boeing Company (BA) [ST] — corrected",
    });
    render(<SupersessionChain supersedes={[]} supersededBy={[correction]} />);
    const arm = screen.getByTestId("superseded-by");
    expect(arm).toHaveTextContent("Superseded by");
    expect(
      screen.getByRole("link", { name: correction.asset_description_raw }),
    ).toHaveAttribute("href", `/r/${correction.id}`);
    expect(screen.getByText("Corrected")).toBeInTheDocument();
  });

  it("lists the corrected history this record supersedes", () => {
    const original = makeRecord({ id: "01KWQVPG6B08S4VX92NZED3C00" });
    render(<SupersessionChain supersedes={[original]} supersededBy={[]} />);
    const arm = screen.getByTestId("supersedes");
    expect(arm).toHaveTextContent("Supersedes");
    expect(
      screen.getByRole("link", { name: original.asset_description_raw }),
    ).toHaveAttribute("href", `/r/${original.id}`);
  });
});
