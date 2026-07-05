import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { AccessNotice } from "@/components/reviewer/AccessNotice";
import { ApiError } from "@/lib/api";

describe("AccessNotice", () => {
  it("surfaces the API's 401 envelope verbatim: status, code, message", () => {
    render(
      <AccessNotice error={new ApiError(401, "admin_token_required", "X-Admin-Token required")} />,
    );

    expect(screen.getByText("Review surface unavailable")).toBeInTheDocument();
    expect(screen.getByText("401")).toBeInTheDocument();
    expect(screen.getByTestId("access-error-code")).toHaveTextContent(
      "admin_token_required",
    );
    expect(screen.getByTestId("access-error-message")).toHaveTextContent(
      "X-Admin-Token required",
    );
    // The operator hint names the exact env pair.
    expect(screen.getByText("GOVFOLIO_ADMIN_TOKEN")).toBeInTheDocument();
    expect(screen.getByText("ADMIN_TOKEN")).toBeInTheDocument();
  });

  it("surfaces a 403 mismatch envelope the same way", () => {
    render(
      <AccessNotice
        error={new ApiError(403, "admin_token_invalid", "X-Admin-Token does not match")}
      />,
    );

    expect(screen.getByText("403")).toBeInTheDocument();
    expect(screen.getByTestId("access-error-code")).toHaveTextContent(
      "admin_token_invalid",
    );
  });
});
