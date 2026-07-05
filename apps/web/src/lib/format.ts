// Display formatting. Money is DECIMAL STRINGS from the API (invariant 7).
// The strings go straight into Intl.NumberFormat — never through Number()/
// parseFloat, which corrupt values past 2^53 (see format.test.ts for proof).

import type { ValueInterval } from "@/lib/api";

type Currency = ValueInterval["currency"];

const currencyFormatters = new Map<Currency, Intl.NumberFormat>();

function currencyFormatter(currency: Currency): Intl.NumberFormat {
  let formatter = currencyFormatters.get(currency);
  if (!formatter) {
    formatter = new Intl.NumberFormat("en-US", {
      style: "currency",
      currency,
      trailingZeroDisplay: "stripIfInteger",
    });
    currencyFormatters.set(currency, formatter);
  }
  return formatter;
}

/** Formats one decimal-string amount, exactly (string-safe Intl path). */
export function formatAmount(decimal: string, currency: Currency): string {
  // The API contract guarantees decimal strings (invariant 7), which is
  // exactly Intl's StringNumericLiteral shape; assert rather than round-trip
  // through Number (which corrupts past 2^53 — proven in format.test.ts).
  return currencyFormatter(currency).format(decimal as Intl.StringNumericLiteral);
}

/**
 * Formats a declared value band:
 * exact (`low == high`), open-ended (`high == null`, "X or more"), or a range.
 */
export function formatValueInterval(value: ValueInterval): string {
  if (value.high === null || value.high === undefined) {
    return `${formatAmount(value.low, value.currency)} or more`;
  }
  if (value.high === value.low) {
    return formatAmount(value.low, value.currency);
  }
  return `${formatAmount(value.low, value.currency)} – ${formatAmount(value.high, value.currency)}`;
}

const dateFormatter = new Intl.DateTimeFormat("en-US", {
  dateStyle: "medium",
  timeZone: "UTC",
});

const dateTimeFormatter = new Intl.DateTimeFormat("en-US", {
  dateStyle: "medium",
  timeStyle: "short",
  timeZone: "UTC",
});

/** Formats a date-only ISO string (UTC; date-only strings parse as UTC midnight). */
export function formatDate(isoDate: string): string {
  return dateFormatter.format(new Date(isoDate));
}

/** Formats an ISO timestamp in UTC, labeled as such. */
export function formatDateTime(isoDateTime: string): string {
  return `${dateTimeFormatter.format(new Date(isoDateTime))} UTC`;
}

/** Formats extraction confidence `[0,1]` as a percentage, e.g. `98%`. */
export function formatConfidence(confidence: number): string {
  return `${Math.round(confidence * 100)}%`;
}

/**
 * Formats how long ago `isoDateTime` was, coarsely (`12m`, `5h`, `3d`).
 * `now` is a parameter so renders are deterministic under test.
 */
export function formatAge(isoDateTime: string, now: Date): string {
  const elapsedMs = now.getTime() - new Date(isoDateTime).getTime();
  const minutes = Math.max(0, Math.floor(elapsedMs / 60_000));
  if (minutes < 60) {
    return `${minutes}m`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 48) {
    return `${hours}h`;
  }
  return `${Math.floor(hours / 24)}d`;
}
