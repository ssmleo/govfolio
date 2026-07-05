//! billing-sync (goal 050, design §6.4): `usage_event` -> Stripe metered
//! billing, exactly-once.
//!
//! The ledger discipline (same shape as the review-audit pattern): a
//! `usage_report` row is created and its events stamped IN ONE TRANSACTION,
//! then the report is sent to Stripe with the report ULID as the
//! idempotency identifier. A crash between commit and send leaves an
//! unreported row that the next pass resends under the SAME identifier —
//! Stripe dedups, so usage is never double-billed and never lost.
//!
//! Eligibility: users with a live (`active`/`trialing`) mirrored
//! subscription AND a `stripe_customer_id`. Everyone else's events stay
//! unbilled in the ledger (they still count against quotas).

use anyhow::Context as _;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::stripe::StripeClient;

/// What a billing-sync pass did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BillingStats {
    /// New `usage_report` rows created this pass.
    pub reports_created: u64,
    /// Reports accepted by Stripe this pass (new + resent).
    pub reports_sent: u64,
    /// `usage_event` rows newly rolled into a report.
    pub events_billed: u64,
}

/// One unreported report joined with its billing coordinates.
#[derive(Debug, sqlx::FromRow)]
struct PendingReport {
    id: String,
    quantity: i64,
    stripe_customer_id: String,
}

/// Billable users with unbilled events.
#[derive(Debug, sqlx::FromRow)]
struct BillableUser {
    id: String,
}

/// Runs one billing-sync pass: roll unbilled usage into reports, send every
/// unreported report. Idempotent and crash-safe at every step.
///
/// # Errors
/// Database failure. A Stripe send failure is NOT fatal: the report stays
/// unreported for the next pass (logged loudly).
pub async fn billing_sync_pass(
    pool: &PgPool,
    stripe: &dyn StripeClient,
) -> anyhow::Result<BillingStats> {
    let mut stats = BillingStats::default();
    let cutoff: DateTime<Utc> = Utc::now();

    // 1) Roll unbilled events into per-user reports (atomic with stamping).
    let billable: Vec<BillableUser> = sqlx::query_as(
        "select u.id from user_account u \
         where u.stripe_customer_id is not null \
           and exists (select 1 from subscription s \
                        where s.user_id = u.id and s.status in ('active','trialing')) \
           and exists (select 1 from usage_event e \
                        where e.user_id = u.id and e.report_id is null \
                          and e.occurred_at <= $1) \
         order by u.id",
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await
    .context("selecting billable users")?;

    for user in &billable {
        if let Some(stamped) = roll_up_user(pool, &user.id, cutoff).await? {
            stats.reports_created += 1;
            stats.events_billed += stamped;
        }
    }

    // 2) Send every unreported report (new ones + crash leftovers) under its
    //    stable identifier.
    let pending: Vec<PendingReport> = sqlx::query_as(
        "select r.id, r.quantity, u.stripe_customer_id \
         from usage_report r join user_account u on u.id = r.user_id \
         where r.reported_at is null and u.stripe_customer_id is not null \
         order by r.id",
    )
    .fetch_all(pool)
    .await
    .context("selecting unreported usage reports")?;

    for report in &pending {
        let quantity = u64::try_from(report.quantity).context("report quantity fits u64")?;
        match stripe
            .report_usage(&report.stripe_customer_id, quantity, &report.id)
            .await
        {
            Ok(()) => {
                sqlx::query("update usage_report set reported_at = now() where id = $1")
                    .bind(&report.id)
                    .execute(pool)
                    .await
                    .context("stamping reported_at")?;
                stats.reports_sent += 1;
            }
            Err(err) => {
                // Loud, non-fatal: the report stays unreported and the next
                // pass resends the SAME identifier.
                eprintln!("billing-sync: report {} not accepted: {err:#}", report.id);
            }
        }
    }
    Ok(stats)
}

/// Rolls one user's unbilled events (up to `cutoff`) into a fresh
/// `usage_report`, stamping the events IN THE SAME TRANSACTION. Returns the
/// number of events rolled up; `None` when there was nothing to bill.
///
/// # Errors
/// Database failure, or a count/stamp mismatch (single-writer invariant
/// broken — fail the pass rather than misbill).
async fn roll_up_user(
    pool: &PgPool,
    user_id: &str,
    cutoff: DateTime<Utc>,
) -> anyhow::Result<Option<u64>> {
    let report_id = ulid::Ulid::new().to_string();
    let mut tx = pool.begin().await.context("opening billing txn")?;
    // Period bounds are the honest min/max of the batch being billed.
    let (period_start, period_end, quantity): (Option<DateTime<Utc>>, Option<DateTime<Utc>>, i64) =
        sqlx::query_as(
            "select min(occurred_at), max(occurred_at), count(*) from usage_event \
             where user_id = $1 and report_id is null and occurred_at <= $2",
        )
        .bind(user_id)
        .bind(cutoff)
        .fetch_one(&mut *tx)
        .await
        .context("measuring unbilled usage")?;
    if quantity == 0 {
        return Ok(None); // raced away; nothing to bill
    }
    let (Some(period_start), Some(period_end)) = (period_start, period_end) else {
        anyhow::bail!("count>0 but no period bounds for user {user_id}");
    };
    sqlx::query(
        "insert into usage_report (id, user_id, period_start, period_end, quantity) \
         values ($1, $2, $3, $4, $5)",
    )
    .bind(&report_id)
    .bind(user_id)
    .bind(period_start)
    .bind(period_end)
    .bind(quantity)
    .execute(&mut *tx)
    .await
    .context("inserting usage_report")?;
    let stamped = sqlx::query(
        "update usage_event set report_id = $1 \
         where user_id = $2 and report_id is null and occurred_at <= $3",
    )
    .bind(&report_id)
    .bind(user_id)
    .bind(cutoff)
    .execute(&mut *tx)
    .await
    .context("stamping usage events")?
    .rows_affected();
    anyhow::ensure!(
        i64::try_from(stamped) == Ok(quantity),
        "usage stamp mismatch for user {user_id}: counted {quantity}, stamped {stamped}"
    );
    tx.commit().await.context("committing billing txn")?;
    Ok(Some(stamped))
}
