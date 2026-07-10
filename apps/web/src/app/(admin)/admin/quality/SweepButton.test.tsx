import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { SweepButton } from "./SweepButton";

const { push } = vi.hoisted(() => ({ push: vi.fn() }));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push }),
}));

// useTransition's `isPending` flip is transient: a synchronous fireEvent
// click flushes both the pending-true and pending-false renders before the
// assertion runs, so it can't be observed by clicking and then reading the
// DOM. Instead, partial-mock `useTransition` itself so a test can force
// `isPending` open and assert the real conditional JSX (the pulsing scan
// line replacing the explainer + button) that the component renders for
// that state.
const { isPendingRef, startTransitionMock } = vi.hoisted(() => ({
  isPendingRef: { current: false },
  startTransitionMock: vi.fn((callback: () => void) => callback()),
}));

vi.mock("react", async (importOriginal) => {
  const actual = await importOriginal<typeof import("react")>();
  return {
    ...actual,
    useTransition: () => [isPendingRef.current, startTransitionMock] as const,
  };
});

beforeEach(() => {
  push.mockClear();
  startTransitionMock.mockClear();
  isPendingRef.current = false;
});

describe("SweepButton", () => {
  it("navigates to the br collision sweep query param on click", () => {
    render(<SweepButton />);
    fireEvent.click(screen.getByRole("button"));
    expect(push).toHaveBeenCalledWith("/admin/quality?sweep=br");
  });

  it("shows the idle explainer copy and an enabled run button when not pending", () => {
    render(<SweepButton />);
    const button = screen.getByRole("button", { name: "Run collision sweep" });
    expect(button).toBeEnabled();
    expect(
      screen.getByText(/Zero rows is a pass; any row needs investigation\./),
    ).toBeInTheDocument();
  });

  it("swaps the whole block to the scanning line while the transition is in flight", () => {
    isPendingRef.current = true;
    render(<SweepButton />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    expect(
      screen.getByText("scanning br staged rows — comparing CPFs per politician…"),
    ).toBeInTheDocument();
  });
});
