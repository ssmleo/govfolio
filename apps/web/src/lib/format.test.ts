import { describe, expect, it } from "vitest";

import {
  formatAmount,
  formatConfidence,
  formatDate,
  formatDateTime,
  formatValueInterval,
} from "@/lib/format";

describe("formatAmount", () => {
  it("formats decimal strings as currency, stripping integer cents", () => {
    expect(formatAmount("1001.00", "USD")).toBe("$1,001");
    expect(formatAmount("70000.00", "GBP")).toBe("£70,000");
    expect(formatAmount("123456.78", "EUR")).toBe("€123,456.78");
  });

  it("keeps non-integer cents", () => {
    expect(formatAmount("19.99", "USD")).toBe("$19.99");
  });

  it("is exact past 2^53 where parseFloat would corrupt (invariant 7)", () => {
    // The float path destroys this value — proof the string path matters:
    expect(String(Number("90071992547409931.55"))).toBe("90071992547409940");
    // The Intl string path keeps every digit:
    expect(formatAmount("90071992547409931.55", "USD")).toBe(
      "$90,071,992,547,409,931.55",
    );
  });
});

describe("formatValueInterval", () => {
  it("formats a declared band as a range", () => {
    expect(
      formatValueInterval({ low: "1001.00", high: "15000.00", currency: "USD" }),
    ).toBe("$1,001 – $15,000");
  });

  it("formats an exact figure (low == high) as one amount", () => {
    expect(
      formatValueInterval({ low: "2500.00", high: "2500.00", currency: "EUR" }),
    ).toBe("€2,500");
  });

  it("formats an open-ended threshold (high null) as 'or more'", () => {
    expect(formatValueInterval({ low: "70000.00", high: null, currency: "GBP" })).toBe(
      "£70,000 or more",
    );
    expect(formatValueInterval({ low: "70000.00", currency: "GBP" })).toBe(
      "£70,000 or more",
    );
  });
});

describe("dates and confidence", () => {
  it("formats date-only strings in UTC (no timezone drift)", () => {
    expect(formatDate("2025-12-09")).toBe("Dec 9, 2025");
    expect(formatDate("2026-01-01")).toBe("Jan 1, 2026");
  });

  it("formats timestamps labeled UTC", () => {
    expect(formatDateTime("2026-07-05T00:43:48.798177Z")).toContain("Jul 5, 2026");
    expect(formatDateTime("2026-07-05T00:43:48.798177Z")).toContain("UTC");
  });

  it("formats confidence as a percentage", () => {
    expect(formatConfidence(0.98)).toBe("98%");
    expect(formatConfidence(0.5)).toBe("50%");
  });
});
