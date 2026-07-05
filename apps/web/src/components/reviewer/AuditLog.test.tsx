import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { AuditLog } from "@/components/reviewer/AuditLog";
import { makeAuditEntry } from "@/test/fixtures";

describe("AuditLog", () => {
  it("lists every resolve attempt: reviewer, verdict, outcome, note, records, time", () => {
    const entries = [
      makeAuditEntry(),
      makeAuditEntry({
        id: "01KWRAUDIT000000000000002B",
        reviewer: "reviewer-late",
        verdict: "reject",
        outcome: "conflict",
        note: null,
        affected_record_ids: [],
        created_at: "2026-07-04T23:30:00Z",
      }),
    ];
    render(<AuditLog entries={entries} />);

    const rows = screen.getAllByRole("row").slice(1); // drop the header row
    expect(rows).toHaveLength(2);

    expect(rows[0]).toHaveTextContent("reviewer-jane");
    expect(rows[0]).toHaveTextContent("confirm");
    expect(rows[0]).toHaveTextContent("applied");
    expect(rows[0]).toHaveTextContent("matches the source document");
    expect(rows[0]).toHaveTextContent("Jul 4, 2026, 11:00 PM UTC");
    expect(
      screen.getByRole("link", { name: "01KWQVPG6B08S4VX92NZED3C16" }),
    ).toHaveAttribute("href", "/r/01KWQVPG6B08S4VX92NZED3C16");

    // A conflicting attempt is a real audit row too — with no affected records.
    expect(rows[1]).toHaveTextContent("reviewer-late");
    expect(rows[1]).toHaveTextContent("conflict");
    expect(rows[1]).toHaveTextContent("none");
  });

  it("renders an honest empty state", () => {
    render(<AuditLog entries={[]} />);
    expect(screen.getByText("No resolve attempts yet.")).toBeInTheDocument();
  });
});
