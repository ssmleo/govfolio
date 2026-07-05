import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { QueueTable } from "@/components/reviewer/QueueTable";
import { makeQueueItem, makeTask } from "@/test/fixtures";

const NOW = new Date("2026-07-05T01:00:00Z"); // 3h after the fixture created_at

describe("QueueTable", () => {
  it("renders the API's ranking order VERBATIM — no re-sorting", () => {
    // Deliberately "unsorted" input: low priority first. The queue must show
    // rows exactly as given (the ranking is the API's contract).
    const items = [
      makeQueueItem({
        task: makeTask({ id: "01TASKLOWPRIORITY000000001", priority_score: 1.5 }),
      }),
      makeQueueItem({
        task: makeTask({ id: "01TASKHIGHPRIORITY00000002", priority_score: 9.9 }),
      }),
    ];
    render(<QueueTable items={items} now={NOW} />);
    const rows = screen.getAllByRole("row").slice(1); // drop the header row
    expect(rows[0]).toHaveTextContent("1.5");
    expect(rows[1]).toHaveTextContent("9.9");
  });

  it("shows reason (linked to the task), target summary, confidence, extractor and age", () => {
    render(<QueueTable items={[makeQueueItem()]} now={NOW} />);
    expect(screen.getByRole("link", { name: "ptr_amendment_unlinked" })).toHaveAttribute(
      "href",
      "/review/01KWRTASK0000000000000001A",
    );
    expect(screen.getByText("David Rouzer")).toBeInTheDocument();
    expect(screen.getByText(/Boeing Company \(BA\) \[ST\]/)).toBeInTheDocument();
    expect(screen.getByText(/\$1,001 – \$15,000/)).toBeInTheDocument();
    expect(screen.getByText("98%")).toBeInTheDocument();
    expect(screen.getByText("us_house_ptr/text@1")).toBeInTheDocument();
    expect(screen.getByText("3h")).toBeInTheDocument();
    expect(screen.getByText("Unverified")).toBeInTheDocument();
  });

  it("is honest about tasks that do not target a disclosure record", () => {
    const items = [
      makeQueueItem({
        task: makeTask({ target_kind: "filing", target_id: "us_house:9115811" }),
        record: null,
      }),
    ];
    render(<QueueTable items={items} now={NOW} />);
    expect(
      screen.getByText(/filing us_house:9115811 \(not a disclosure record\)/),
    ).toBeInTheDocument();
  });

  it("renders an honest empty state", () => {
    render(<QueueTable items={[]} now={NOW} />);
    expect(screen.getByText("No review tasks in this view.")).toBeInTheDocument();
  });
});
