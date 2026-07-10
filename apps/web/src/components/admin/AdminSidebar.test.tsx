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
