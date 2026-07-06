//! Monthly sampling audit + precision report (design §7.4: "≥99% extraction
//! precision on value/side/instrument/date, monthly random-sample audit"; goal
//! 070). The automation-policy's expected.*.json auto-resolution flows through
//! this queue.
//!
//! Once a month the sampler draws a **stratified** sample of published Gold
//! records — each disclosure regime is a stratum — using a **deterministic,
//! seeded** draw so a batch is reproducible (tests pin the seed; a re-run of
//! the same month queues nothing new). Each drawn record is queued into
//! `sample_audit` as `pending`; an auditor marks it `confirmed` or
//! `discrepancy`; the per-regime precision estimate is computed from those
//! outcomes ([`precision_report`]).

use anyhow::Context as _;
use sha2::{Digest as _, Sha256};
use sqlx::PgPool;

/// Deterministic sample of up to `n` ids from `record_ids`, seeded by `seed`.
/// Reproducible and INPUT-ORDER-INDEPENDENT: records are ranked by a stable
/// seeded hash, so the same `(record_ids as a set, n, seed)` always yields the
/// same draw. The draw is the "random" axis; the caller passes ONE regime's
/// records (the stratum).
#[must_use]
pub fn select_sample(record_ids: &[String], n: usize, seed: u64) -> Vec<String> {
    let mut ranked: Vec<(&String, [u8; 32])> = record_ids
        .iter()
        .map(|id| (id, sample_hash(seed, id)))
        .collect();
    // (hash, id) key so ties never depend on the input order.
    ranked.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(b.0)));
    ranked
        .into_iter()
        .take(n)
        .map(|(id, _)| id.clone())
        .collect()
}

/// Stable seeded hash of one record id (`sha256(seed_be || ':' || id)`).
fn sample_hash(seed: u64, id: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(seed.to_be_bytes());
    hasher.update(b":");
    hasher.update(id.as_bytes());
    hasher.finalize().into()
}

/// One regime's slice of the precision report (design §7.4).
#[derive(Debug, Clone, PartialEq)]
pub struct RegimePrecision {
    /// `disclosure_regime.id` this stratum belongs to.
    pub regime_id: String,
    /// Human-readable regime body (`disclosure_regime.body`).
    pub body: String,
    /// Records sampled into this batch for the regime.
    pub sampled: i64,
    /// Sampled records an auditor has ruled on (`status <> 'pending'`).
    pub audited: i64,
    /// Audited records that turned out wrong (`status = 'discrepancy'`).
    pub discrepancies: i64,
    /// `(audited - discrepancies) / audited`; `None` until at least one sampled
    /// record has been audited (no estimate from zero observations).
    pub precision_estimate: Option<f64>,
}

/// A sampling batch's precision report (design §7.4).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PrecisionReport {
    /// `'YYYY-MM'` batch label the report covers.
    pub sample_month: String,
    /// Per-regime precision, ordered by regime id.
    pub regimes: Vec<RegimePrecision>,
}

/// Draws + queues the monthly sampling audit and returns the freshly computed
/// precision report. Stratified per regime: every regime with published Gold is
/// a stratum; up to `per_regime` records are drawn per stratum by a seeded hash
/// (`seed`) so the batch is reproducible. Idempotent per `month`: a record is
/// queued at most once (unique `(sample_month, record_id)`).
///
/// # Errors
/// Database failure, or a non-representable count (`usize`/`i64` overflow).
pub async fn run_sampling_audit(
    pool: &PgPool,
    month: &str,
    per_regime: usize,
    seed: i64,
) -> anyhow::Result<PrecisionReport> {
    // Every regime that has published Gold, in id order (deterministic strata).
    let regime_ids: Vec<String> =
        sqlx::query_scalar("select distinct regime_id from disclosure_record order by regime_id")
            .fetch_all(pool)
            .await
            .context("listing regimes with published Gold")?;

    for regime_id in &regime_ids {
        let record_ids: Vec<String> =
            sqlx::query_scalar("select id from disclosure_record where regime_id = $1 order by id")
                .bind(regime_id)
                .fetch_all(pool)
                .await
                .with_context(|| format!("listing records for regime {regime_id}"))?;

        #[allow(clippy::cast_sign_loss)] // seed is an opaque draw key, not a magnitude
        let drawn = select_sample(&record_ids, per_regime, seed as u64);
        for record_id in drawn {
            // Idempotent per month: a record is queued at most once per batch.
            sqlx::query(
                "insert into sample_audit (id, regime_id, record_id, sample_month, seed) \
                 values ($1, $2, $3, $4, $5) \
                 on conflict (sample_month, record_id) do nothing",
            )
            .bind(ulid::Ulid::new().to_string())
            .bind(regime_id)
            .bind(&record_id)
            .bind(month)
            .bind(seed)
            .execute(pool)
            .await
            .with_context(|| format!("queueing sample_audit for record {record_id}"))?;
        }
    }

    precision_report(pool, month).await
}

/// Computes the per-regime precision report for one sampling batch from its
/// queued+audited `sample_audit` rows (design §7.4). Callable independently of
/// [`run_sampling_audit`] to re-read the report after auditing.
///
/// # Errors
/// Database failure.
pub async fn precision_report(pool: &PgPool, month: &str) -> anyhow::Result<PrecisionReport> {
    let rows: Vec<(String, String, i64, i64, i64)> = sqlx::query_as(
        "select sa.regime_id, dr.body, \
                count(*) as sampled, \
                count(*) filter (where sa.status <> 'pending') as audited, \
                count(*) filter (where sa.status = 'discrepancy') as discrepancies \
         from sample_audit sa \
         join disclosure_regime dr on dr.id = sa.regime_id \
         where sa.sample_month = $1 \
         group by sa.regime_id, dr.body \
         order by sa.regime_id",
    )
    .bind(month)
    .fetch_all(pool)
    .await
    .with_context(|| format!("computing precision report for {month}"))?;

    let regimes = rows
        .into_iter()
        .map(
            |(regime_id, body, sampled, audited, discrepancies)| RegimePrecision {
                regime_id,
                body,
                sampled,
                audited,
                discrepancies,
                precision_estimate: precision_estimate(audited, discrepancies),
            },
        )
        .collect();
    Ok(PrecisionReport {
        sample_month: month.to_owned(),
        regimes,
    })
}

/// `(audited - discrepancies) / audited`, or `None` when nothing is audited yet.
#[allow(clippy::cast_precision_loss)] // audit counts are tiny; f64 is exact here
fn precision_estimate(audited: i64, discrepancies: i64) -> Option<f64> {
    if audited <= 0 {
        return None;
    }
    Some((audited - discrepancies) as f64 / audited as f64)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("record-{i:04}")).collect()
    }

    #[test]
    fn sampler_select_is_deterministic_and_order_independent() {
        let forward = ids(50);
        let mut reversed = forward.clone();
        reversed.reverse();

        let a = select_sample(&forward, 7, 42);
        let b = select_sample(&reversed, 7, 42);
        assert_eq!(a.len(), 7);
        // Same set + seed -> same draw regardless of input order.
        assert_eq!(a, b, "the seeded draw is input-order-independent");
        // Re-running is identical (reproducible).
        assert_eq!(a, select_sample(&forward, 7, 42));
    }

    #[test]
    fn sampler_select_varies_with_seed() {
        let pool = ids(50);
        let a = select_sample(&pool, 7, 1);
        let b = select_sample(&pool, 7, 2);
        assert_ne!(a, b, "a different seed draws a different sample");
    }

    #[test]
    fn sampler_select_caps_at_available() {
        assert_eq!(select_sample(&ids(3), 10, 42).len(), 3);
        assert_eq!(select_sample(&[], 10, 42).len(), 0);
    }

    #[test]
    fn precision_estimate_math() {
        assert_eq!(
            precision_estimate(0, 0),
            None,
            "no observations -> no estimate"
        );
        assert_eq!(precision_estimate(10, 0), Some(1.0));
        assert_eq!(precision_estimate(10, 1), Some(0.9));
        assert_eq!(precision_estimate(4, 4), Some(0.0));
    }
}
