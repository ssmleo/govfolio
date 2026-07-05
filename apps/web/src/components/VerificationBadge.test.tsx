import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { VerificationState } from "@/lib/api";
import { VerificationBadge } from "@/components/VerificationBadge";

const STATES: VerificationState[] = ["unverified", "verified", "corrected", "disputed"];

describe("VerificationBadge", () => {
  it("renders a visually distinct, honestly labeled badge per state", () => {
    render(
      <>
        {STATES.map((state) => (
          <VerificationBadge key={state} state={state} />
        ))}
      </>,
    );
    expect(screen.getByText("Unverified")).toHaveClass("badge", "badge-unverified");
    expect(screen.getByText("Verified")).toHaveClass("badge", "badge-verified");
    expect(screen.getByText("Corrected")).toHaveClass("badge", "badge-corrected");
    expect(screen.getByText("Disputed")).toHaveClass("badge", "badge-disputed");
  });

  it("labels differ by text, not only by color", () => {
    render(
      <>
        {STATES.map((state) => (
          <VerificationBadge key={state} state={state} />
        ))}
      </>,
    );
    const labels = STATES.map(
      (state) => screen.getByText(new RegExp(`^${state}$`, "i")).textContent,
    );
    expect(new Set(labels).size).toBe(STATES.length);
  });

  it("explains unverified honestly (published as filed, not reviewed)", () => {
    render(<VerificationBadge state="unverified" />);
    expect(screen.getByText("Unverified")).toHaveAttribute(
      "title",
      expect.stringContaining("not yet reviewed"),
    );
  });
});
