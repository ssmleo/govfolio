import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ResolveActionResult, ResolveInput } from "@/lib/api";
import { ResolvePanel, type ResolvePanelProps } from "@/components/reviewer/ResolvePanel";
import { makeRecord } from "@/test/fixtures";

const { refresh } = vi.hoisted(() => ({ refresh: vi.fn() }));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ refresh }),
}));

const APPLIED: ResolveActionResult = {
  kind: "applied",
  recordId: "01KWQVPG6B08S4VX92NZED3C16",
  supersedingRecordId: null,
};

function renderPanel(overrides: Partial<ResolvePanelProps> = {}) {
  const action =
    vi.fn<(taskId: string, input: ResolveInput) => Promise<ResolveActionResult>>();
  action.mockResolvedValue(APPLIED);
  const props: ResolvePanelProps = {
    taskId: "01KWRTASK0000000000000001A",
    status: "open",
    targetKind: "disclosure_record",
    targetId: "01KWQVPG6B08S4VX92NZED3C16",
    record: makeRecord(),
    action,
    ...overrides,
  };
  render(<ResolvePanel {...props} />);
  return action;
}

beforeEach(() => {
  refresh.mockClear();
});

describe("ResolvePanel validation", () => {
  it("requires a reviewer name before any verdict reaches the endpoint", async () => {
    const action = renderPanel();
    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
    expect(await screen.findByTestId("resolve-error")).toHaveTextContent(
      "Reviewer name is required.",
    );
    expect(action).not.toHaveBeenCalled();
  });

  it("requires a regime code for an edit", async () => {
    const action = renderPanel();
    fireEvent.change(screen.getByLabelText("Reviewer"), { target: { value: "jane" } });
    fireEvent.click(screen.getByRole("button", { name: "Edit…" }));
    fireEvent.click(screen.getByRole("button", { name: "Submit correction" }));
    expect(await screen.findByTestId("resolve-error")).toHaveTextContent(
      "Regime code is required",
    );
    expect(action).not.toHaveBeenCalled();
  });

  it("rejects value bounds that are not decimal strings", async () => {
    const action = renderPanel();
    fireEvent.change(screen.getByLabelText("Reviewer"), { target: { value: "jane" } });
    fireEvent.click(screen.getByRole("button", { name: "Edit…" }));
    fireEvent.change(screen.getByLabelText("Regime code"), {
      target: { value: "us_house" },
    });
    fireEvent.change(screen.getByLabelText("Value low"), {
      target: { value: "12,000" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Submit correction" }));
    expect(await screen.findByTestId("resolve-error")).toHaveTextContent(
      "decimal strings",
    );
    expect(action).not.toHaveBeenCalled();
  });

  it("rejects a regime payload that is not a JSON object", async () => {
    const action = renderPanel();
    fireEvent.change(screen.getByLabelText("Reviewer"), { target: { value: "jane" } });
    fireEvent.click(screen.getByRole("button", { name: "Edit…" }));
    fireEvent.change(screen.getByLabelText("Regime code"), {
      target: { value: "us_house" },
    });
    fireEvent.change(screen.getByLabelText("Regime payload (details, JSON)"), {
      target: { value: "not json" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Submit correction" }));
    expect(await screen.findByTestId("resolve-error")).toHaveTextContent("JSON object");
    expect(action).not.toHaveBeenCalled();
  });
});

describe("ResolvePanel verdicts (one door: the resolve action)", () => {
  it("confirm sends the confirm verdict with reviewer and note", async () => {
    const action = renderPanel();
    fireEvent.change(screen.getByLabelText("Reviewer"), { target: { value: "jane" } });
    fireEvent.change(screen.getByLabelText("Note"), {
      target: { value: "matches the PDF" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
    await waitFor(() => {
      expect(action).toHaveBeenCalledWith("01KWRTASK0000000000000001A", {
        reviewer: "jane",
        note: "matches the PDF",
        verdict: "confirm",
      });
    });
    expect(await screen.findByTestId("resolve-outcome")).toHaveTextContent(
      "Verdict applied.",
    );
    expect(
      screen.getByRole("link", { name: "01KWQVPG6B08S4VX92NZED3C16" }),
    ).toHaveAttribute("href", "/r/01KWQVPG6B08S4VX92NZED3C16");
    expect(refresh).toHaveBeenCalled();
  });

  it("reject sends the reject verdict", async () => {
    const action = renderPanel();
    fireEvent.change(screen.getByLabelText("Reviewer"), { target: { value: "jane" } });
    fireEvent.click(screen.getByRole("button", { name: "Reject" }));
    await waitFor(() => {
      expect(action).toHaveBeenCalledWith(
        "01KWRTASK0000000000000001A",
        expect.objectContaining({ verdict: "reject", reviewer: "jane", note: null }),
      );
    });
  });

  it("edit form is seeded with the record's current values", () => {
    renderPanel();
    fireEvent.click(screen.getByRole("button", { name: "Edit…" }));
    expect(screen.getByLabelText("Asset description (as filed)")).toHaveValue(
      "Boeing Company (BA) [ST]",
    );
    expect(screen.getByLabelText("Record type")).toHaveValue("transaction");
    expect(screen.getByLabelText("Asset class")).toHaveValue("equity");
    expect(screen.getByLabelText("Side")).toHaveValue("sell");
    expect(screen.getByLabelText("Owner")).toHaveValue("self");
    expect(screen.getByLabelText("Transaction date")).toHaveValue("2025-12-09");
    expect(screen.getByLabelText("Value low")).toHaveValue("1001.00");
    expect(screen.getByLabelText("Value high")).toHaveValue("15000.00");
    expect(screen.getByLabelText("Currency")).toHaveValue("USD");
    expect(screen.getByLabelText("Regime payload (details, JSON)")).toHaveValue("{}");
  });

  it("edit submits money as UNTOUCHED decimal strings (invariant 7)", async () => {
    const action = renderPanel();
    fireEvent.change(screen.getByLabelText("Reviewer"), { target: { value: "jane" } });
    fireEvent.click(screen.getByRole("button", { name: "Edit…" }));
    fireEvent.change(screen.getByLabelText("Regime code"), {
      target: { value: "us_house" },
    });
    fireEvent.change(screen.getByLabelText("Value high"), {
      target: { value: "50000.00" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Submit correction" }));
    await waitFor(() => {
      expect(action).toHaveBeenCalledWith(
        "01KWRTASK0000000000000001A",
        expect.objectContaining({
          verdict: "edit",
          regime_code: "us_house",
          corrected: expect.objectContaining({
            asset_description_raw: "Boeing Company (BA) [ST]",
            // Decimal STRINGS end to end — the panel never runs money
            // through Number().
            value: { low: "1001.00", high: "50000.00", currency: "USD" },
            // A human correction, not an extractor guess.
            extracted_by: "review:web@1",
            extraction_confidence: null,
          }),
        }),
      );
    });
  });

  it("surfaces a 409 honestly: nothing changed, state reloaded", async () => {
    const action = renderPanel();
    action.mockResolvedValue({
      kind: "conflict",
      message: "review task is already resolved.",
    });
    fireEvent.change(screen.getByLabelText("Reviewer"), { target: { value: "late" } });
    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
    expect(await screen.findByTestId("resolve-conflict")).toHaveTextContent(
      "Already resolved",
    );
    expect(refresh).toHaveBeenCalled();
  });

  it("shows an API error verbatim and keeps the forms usable", async () => {
    const action = renderPanel();
    action.mockResolvedValue({
      kind: "error",
      code: "internal",
      message: "corrected record fails the details contract",
    });
    fireEvent.change(screen.getByLabelText("Reviewer"), { target: { value: "jane" } });
    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
    expect(await screen.findByTestId("resolve-error")).toHaveTextContent(
      "internal: corrected record fails the details contract",
    );
    expect(screen.getByRole("button", { name: "Confirm" })).toBeInTheDocument();
    expect(refresh).not.toHaveBeenCalled();
  });
});

describe("ResolvePanel closed states", () => {
  it("shows no forms for a task that does not target a disclosure record", () => {
    renderPanel({ record: null, targetKind: "filing", targetId: "us_house:9115811" });
    expect(screen.getByTestId("no-adjudication")).toHaveTextContent(
      "filing us_house:9115811",
    );
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("shows no forms for an already-resolved task", () => {
    renderPanel({ status: "resolved" });
    expect(screen.getByTestId("task-closed")).toHaveTextContent("This task is resolved");
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});
