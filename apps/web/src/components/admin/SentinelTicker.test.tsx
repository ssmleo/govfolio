import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { SentinelTicker } from "@/components/admin/SentinelTicker";
import { makeAdminOverview } from "@/test/fixtures";
import type { AdminOverview } from "@/lib/api";

interface MockQueryResult {
  data?: AdminOverview;
  isLoading: boolean;
  isError: boolean;
}

const { useQueryMock } = vi.hoisted(() => ({
  useQueryMock: vi.fn<() => MockQueryResult>(),
}));

vi.mock("@tanstack/react-query", () => ({
  useQuery: useQueryMock,
}));

function mockOverview(overrides: Partial<AdminOverview> = {}): void {
  useQueryMock.mockReturnValue({
    data: makeAdminOverview(overrides),
    isLoading: false,
    isError: false,
  });
}

describe("SentinelTicker loading/error states", () => {
  it("shows a loading state while the query is in flight", () => {
    useQueryMock.mockReturnValue({ data: undefined, isLoading: true, isError: false });
    render(<SentinelTicker />);
    expect(screen.getByText("loading sentinel…")).toBeInTheDocument();
  });

  it("shows an unavailable state on query error", () => {
    useQueryMock.mockReturnValue({ data: undefined, isLoading: false, isError: true });
    render(<SentinelTicker />);
    expect(screen.getByText("sentinel ticker unavailable")).toBeInTheDocument();
  });
});

describe("SentinelTicker state-word derivation", () => {
  it("is NOMINAL when nothing is frozen, nothing failed, and no drift is open", () => {
    mockOverview({ frozen_regimes: [], runs_24h: { failed: 0, running: 2, succeeded: 10 } });
    render(<SentinelTicker />);
    expect(screen.getByText("NOMINAL")).toBeInTheDocument();
  });

  it("is WATCH when drift is open but nothing is frozen or failing", () => {
    mockOverview({
      frozen_regimes: [],
      runs_24h: { failed: 0, running: 0, succeeded: 5 },
      queue_depths: {
        delivery_dlq: 0,
        drift_open: 3,
        outbox_undispatched: 0,
        pipeline_failed: 0,
        pipeline_running: 0,
        review_open: 0,
        sample_pending: 0,
        usage_unbilled: 0,
      },
    });
    render(<SentinelTicker />);
    expect(screen.getByText("WATCH")).toBeInTheDocument();
  });

  it("is INCIDENT when any regime is frozen, even with no open drift or failures", () => {
    mockOverview({
      frozen_regimes: [{ regime_code: "br", frozen_at: "2026-07-01T00:00:00Z", frozen_kind: "layout_shift" }],
    });
    render(<SentinelTicker />);
    expect(screen.getByText("INCIDENT")).toBeInTheDocument();
  });

  it("is INCIDENT when a run failed in the last 24h, even with nothing frozen", () => {
    mockOverview({
      frozen_regimes: [],
      runs_24h: { failed: 1, running: 0, succeeded: 4 },
    });
    render(<SentinelTicker />);
    expect(screen.getByText("INCIDENT")).toBeInTheDocument();
  });

  it("prefers INCIDENT over WATCH when both a failure and open drift are present", () => {
    mockOverview({
      frozen_regimes: [],
      runs_24h: { failed: 2, running: 0, succeeded: 0 },
      queue_depths: {
        delivery_dlq: 0,
        drift_open: 5,
        outbox_undispatched: 0,
        pipeline_failed: 0,
        pipeline_running: 0,
        review_open: 0,
        sample_pending: 0,
        usage_unbilled: 0,
      },
    });
    render(<SentinelTicker />);
    expect(screen.getByText("INCIDENT")).toBeInTheDocument();
    expect(screen.queryByText("WATCH")).not.toBeInTheDocument();
  });
});

describe("SentinelTicker rendered stats", () => {
  it("renders the real frozen/running/failed/queue counts, not fabricated ones", () => {
    mockOverview({
      frozen_regimes: [
        { regime_code: "br" },
        { regime_code: "us_house" },
      ],
      gold_records_estimate: 874,
      runs_24h: { failed: 1, running: 3, succeeded: 20 },
      queue_depths: {
        delivery_dlq: 4,
        drift_open: 2,
        outbox_undispatched: 0,
        pipeline_failed: 0,
        pipeline_running: 0,
        review_open: 7,
        sample_pending: 0,
        usage_unbilled: 0,
      },
    });
    render(<SentinelTicker />);

    expect(screen.getByText("frozen").nextElementSibling).toHaveTextContent("2");
    expect(screen.getByText("running").nextElementSibling).toHaveTextContent("3");
    expect(screen.getByText("failed 24h").nextElementSibling).toHaveTextContent("1");
    expect(screen.getByText("review open").nextElementSibling).toHaveTextContent("7");
    expect(screen.getByText("drift open").nextElementSibling).toHaveTextContent("2");
    expect(screen.getByText("dlq").nextElementSibling).toHaveTextContent("4");
    // Sub-1000 value: toLocaleString() output is locale-independent.
    expect(screen.getByText("gold records").nextElementSibling).toHaveTextContent("874");
  });

  it("renders an honest dash when the planner estimate is null (never-analyzed table)", () => {
    mockOverview({ gold_records_estimate: null });
    render(<SentinelTicker />);
    expect(screen.getByText("gold records").nextElementSibling).toHaveTextContent("—");
  });
});
