import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { makeRegimeDossierData } from "@/test/fixtures";

import { RegimeDossier } from "./RegimeDossier";

describe("RegimeDossier closed state (never opened)", () => {
  it("renders nothing when data is null and it has never been opened", () => {
    const { container } = render(<RegimeDossier data={null} onClose={vi.fn()} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("does not attach an Escape listener while closed", () => {
    const onClose = vi.fn();
    render(<RegimeDossier data={null} onClose={onClose} />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe("RegimeDossier stays mounted after being opened once (slide-out transition)", () => {
  it("keeps the panel in the DOM, hidden and inert, after data goes back to null", () => {
    const data = makeRegimeDossierData();
    const { container, rerender } = render(<RegimeDossier data={data} onClose={vi.fn()} />);
    expect(container.querySelector("aside")).not.toBeNull();

    rerender(<RegimeDossier data={null} onClose={vi.fn()} />);
    const aside = container.querySelector("aside");
    expect(aside).not.toBeNull();
    expect(aside).toHaveStyle({ visibility: "hidden" });
    expect(aside).toHaveAttribute("inert");
    // The last-known content stays rendered (it's what the slide-out
    // animates away, not an empty shell) — see `cached` in RegimeDossier.tsx.
    expect(screen.getByText("US House — United States")).toBeInTheDocument();
  });

  it("ignores Escape once closed, even though the listener stays attached", () => {
    const onClose = vi.fn();
    const { rerender } = render(
      <RegimeDossier data={makeRegimeDossierData()} onClose={onClose} />,
    );
    rerender(<RegimeDossier data={null} onClose={onClose} />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe("RegimeDossier open state — renders the assembled fixture data", () => {
  it("renders title, facts, tier composition, notes, and the adapter crate footer", () => {
    const data = makeRegimeDossierData();
    render(<RegimeDossier data={data} onClose={vi.fn()} />);

    expect(screen.getByRole("heading", { name: "US House — United States" })).toBeInTheDocument();
    expect(screen.getByText("United States (us)")).toBeInTheDocument();
    expect(screen.getByText("us_house")).toBeInTheDocument();
    expect(screen.getByText("Not frozen; no open drift reports.")).toBeInTheDocument();
    expect(screen.getByText("Discovery lag p50 1.0h, p90 2.0h.")).toBeInTheDocument();
    expect(screen.getByText("transaction_report regime, banded values.")).toBeInTheDocument();
    expect(
      screen.getByText("crates/adapters/us_house", { exact: false }),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "See full pipeline detail →" })).toHaveAttribute(
      "href",
      "/admin/pipeline",
    );
  });

  it("omits the regime note section when no Regime row joined (honest null, not fabricated)", () => {
    const data = makeRegimeDossierData({ regimeNote: null });
    render(<RegimeDossier data={data} onClose={vi.fn()} />);
    expect(screen.queryByText("Regime note")).not.toBeInTheDocument();
  });

  it("shows the honest empty state for years with no dated Gold records", () => {
    const data = makeRegimeDossierData({ goldByYear: [] });
    render(<RegimeDossier data={data} onClose={vi.fn()} />);
    expect(
      screen.getByText("No dated Gold records for this regime yet."),
    ).toBeInTheDocument();
  });

  it("shows 'unbridged' in the footer when the regime has no adapter crates", () => {
    const data = makeRegimeDossierData({ adapterCrates: [] });
    render(<RegimeDossier data={data} onClose={vi.fn()} />);
    // The footer sentence's "unbridged" text is a sibling text node next to
    // "Adapter crates:" within the same <p>, not its own element — match on
    // substring rather than the whole paragraph's concatenated text.
    expect(screen.getByText("unbridged", { exact: false })).toBeInTheDocument();
  });
});

describe("RegimeDossier close affordances", () => {
  it("calls onClose when the × button is clicked", () => {
    const onClose = vi.fn();
    render(<RegimeDossier data={makeRegimeDossierData()} onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: "Close dossier" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("calls onClose when the backdrop is clicked", () => {
    const onClose = vi.fn();
    const { container } = render(
      <RegimeDossier data={makeRegimeDossierData()} onClose={onClose} />,
    );
    const backdrop = container.querySelector('[aria-hidden="true"]');
    expect(backdrop).not.toBeNull();
    fireEvent.click(backdrop as Element);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("calls onClose on Escape keydown", () => {
    const onClose = vi.fn();
    render(<RegimeDossier data={makeRegimeDossierData()} onClose={onClose} />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("ignores other keys", () => {
    const onClose = vi.fn();
    render(<RegimeDossier data={makeRegimeDossierData()} onClose={onClose} />);
    fireEvent.keyDown(window, { key: "Enter" });
    expect(onClose).not.toHaveBeenCalled();
  });
});
