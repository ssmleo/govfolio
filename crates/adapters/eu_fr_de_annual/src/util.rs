//! Small shared parsing helpers (dates, money) used across the three sub-adapters.

use chrono::NaiveDate;
use rust_decimal::Decimal;

/// Parses a `DD/MM/YYYY` date (optionally trailing `HH:MM:SS`) — the shared
/// European day-first convention (EU DPI footer date, FR `dateDepot`). Returns
/// `None` on any deviation (the raw string survives verbatim in `details`).
#[must_use]
pub(crate) fn parse_ddmmyyyy(raw: &str) -> Option<NaiveDate> {
    let date_part = raw.split_whitespace().next()?;
    let mut parts = date_part.split('/');
    let day: u32 = parts.next()?.trim().parse().ok()?;
    let month: u32 = parts.next()?.trim().parse().ok()?;
    let year: i32 = parts.next()?.trim().parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    NaiveDate::from_ymd_opt(year, month, day)
}

/// A whole-euro (or space/dot-grouped) numeral → `Decimal` at two decimal places
/// (invariant 7). `separators` are stripped before parsing (FR space thousands,
/// DE dot thousands). A German decimal comma is converted to a point when
/// `comma_decimal` is set.
#[must_use]
pub(crate) fn parse_amount(raw: &str, comma_decimal: bool) -> Option<Decimal> {
    let mut cleaned = String::with_capacity(raw.len());
    for c in raw.chars() {
        match c {
            '0'..='9' => cleaned.push(c),
            '-' if cleaned.is_empty() => cleaned.push(c),
            ',' if comma_decimal => cleaned.push('.'),
            // grouping separators (space, thin space, dot) are dropped
            _ => {}
        }
    }
    if cleaned.is_empty() {
        return None;
    }
    let mut amount: Decimal = cleaned.parse().ok()?;
    amount.rescale(2);
    Some(amount)
}
