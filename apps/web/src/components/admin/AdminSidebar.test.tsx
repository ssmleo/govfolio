import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { AdminSidebar } from "@/components/admin/AdminSidebar";

const LAST_SCREEN_KEY = "govfolio-admin-last-screen";

const { push, pathnameMock } = vi.hoisted(() => ({
  push: vi.fn(),
  pathnameMock: vi.fn(() => "/admin"),
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push }),
  usePathname: () => pathnameMock(),
}));

beforeEach(() => {
  push.mockClear();
  pathnameMock.mockReturnValue("/admin");
  window.localStorage.clear();
});

describe("AdminSidebar markup (design-exact shell)", () => {
  it("renders the letter chip to the LEFT of each label (◆ for Overview, A-H for sections)", () => {
    render(<AdminSidebar />);
    // textContent order proves chip-first: the glyph precedes the label.
    expect(screen.getByRole("link", { name: /Overview/ })).toHaveTextContent(/^◆Overview$/);
    expect(screen.getByRole("link", { name: /Coverage/ })).toHaveTextContent(/^ACoverage$/);
    expect(screen.getByRole("link", { name: /Loop/ })).toHaveTextContent(/^HLoop$/);
  });

  it("renders NO digit shortcut numbers in the DOM (the design has none)", () => {
    render(<AdminSidebar />);
    for (let digit = 1; digit <= 9; digit++) {
      expect(screen.queryByText(String(digit))).not.toBeInTheDocument();
    }
  });

  it("titles every link with its route", () => {
    render(<AdminSidebar />);
    expect(screen.getByRole("link", { name: /Overview/ })).toHaveAttribute("title", "/admin");
    expect(screen.getByRole("link", { name: /Coverage/ })).toHaveAttribute(
      "title",
      "/admin/coverage",
    );
    expect(screen.getByRole("link", { name: /Loop/ })).toHaveAttribute("title", "/admin/loop");
  });

  it("marks only the current path's link as active (aria-current + gold chip ink)", () => {
    pathnameMock.mockReturnValue("/admin/coverage");
    render(<AdminSidebar />);

    const coverage = screen.getByRole("link", { name: /Coverage/ });
    expect(coverage).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("link", { name: /Overview/ })).not.toHaveAttribute("aria-current");

    // Active chip lights up gold-deep; an inactive chip stays dim.
    expect(screen.getByText("A").style.color).toBe("var(--adm-accent-deep)");
    expect(screen.getByText("◆").style.color).toBe("var(--adm-nav-chip-inactive)");
  });

  it("renders the Access panel pinned after the groups", () => {
    render(<AdminSidebar />);
    expect(screen.getByText("Access")).toBeInTheDocument();
    expect(screen.getByText("Founder token · full scope")).toBeInTheDocument();
    expect(screen.getByText("all reads are logged")).toBeInTheDocument();
  });
});

describe("AdminSidebar keyboard shortcuts", () => {
  it("digit 1 navigates to the first flattened link (Overview)", () => {
    render(<AdminSidebar />);
    fireEvent.keyDown(window, { key: "1" });
    expect(push).toHaveBeenCalledWith("/admin");
  });

  it("digit 3 navigates to the third flattened link (Backfill)", () => {
    render(<AdminSidebar />);
    fireEvent.keyDown(window, { key: "3" });
    expect(push).toHaveBeenCalledWith("/admin/backfill");
  });

  it("digit 9 navigates to the ninth flattened link (Loop)", () => {
    render(<AdminSidebar />);
    fireEvent.keyDown(window, { key: "9" });
    expect(push).toHaveBeenCalledWith("/admin/loop");
  });

  it("ignores a digit outside the 1-9 range", () => {
    render(<AdminSidebar />);
    fireEvent.keyDown(window, { key: "0" });
    expect(push).not.toHaveBeenCalled();
  });

  it("ignores the shortcut when a modifier key is held (does not fight Ctrl/Cmd+digit)", () => {
    render(<AdminSidebar />);
    fireEvent.keyDown(window, { key: "3", ctrlKey: true });
    expect(push).not.toHaveBeenCalled();
  });

  it("does not navigate when an adjacent form field has focus", () => {
    render(
      <div>
        <input aria-label="adjacent field" />
        <AdminSidebar />
      </div>,
    );
    const input = screen.getByLabelText("adjacent field");
    input.focus();
    expect(document.activeElement).toBe(input);
    fireEvent.keyDown(window, { key: "3" });
    expect(push).not.toHaveBeenCalled();
  });
});

describe("AdminSidebar last-screen bookkeeping", () => {
  it("writes the current path to localStorage on mount", () => {
    pathnameMock.mockReturnValue("/admin/coverage");
    render(<AdminSidebar />);
    expect(window.localStorage.getItem(LAST_SCREEN_KEY)).toBe("/admin/coverage");
  });

  it("updates localStorage when the path changes", () => {
    pathnameMock.mockReturnValue("/admin");
    const { rerender } = render(<AdminSidebar />);
    expect(window.localStorage.getItem(LAST_SCREEN_KEY)).toBe("/admin");

    pathnameMock.mockReturnValue("/admin/quality");
    rerender(<AdminSidebar />);
    expect(window.localStorage.getItem(LAST_SCREEN_KEY)).toBe("/admin/quality");
  });
});
