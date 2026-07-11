//! Registry DONE / DOING / LEFT (read-only SQL). Degrades when PG is down.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::lease::{self, LeaseStatus};

#[derive(Debug, Clone)]
pub enum RegistryView {
    Unavailable(String),
    Ok(RegistryBoard),
}

#[derive(Debug, Clone, Default)]
pub struct RegistryBoard {
    pub doing: Vec<DoingLease>,
    pub phase_counts: Vec<(String, i64)>,
    pub epoch_counts: Vec<(Option<i16>, i64)>,
    pub claimable_at_epoch: i64,
    pub blocked_reasons: Vec<(String, i64)>,
    pub left_sample: Vec<LeftRow>,
}

#[derive(Debug, Clone)]
pub struct DoingLease {
    pub id: String,
    pub coverage_phase: String,
    pub claimed_by: String,
    pub claimed_at: DateTime<Utc>,
    pub age_min: i64,
}

#[derive(Debug, Clone)]
pub struct LeftRow {
    pub id: String,
    pub coverage_phase: String,
    pub epoch: Option<i16>,
    pub priority_score: Option<f32>,
}

pub async fn collect(epoch: i16) -> RegistryView {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(u) if !u.trim().is_empty() => u,
        _ => {
            return RegistryView::Unavailable(
                "DATABASE_URL unset (default: postgres://postgres:postgres@localhost:5433/govfolio)"
                    .into(),
            );
        }
    };
    let pool = match PgPool::connect(&database_url).await {
        Ok(p) => p,
        Err(e) => return RegistryView::Unavailable(format!("connect: {e}")),
    };
    match load_board(&pool, epoch).await {
        Ok(b) => RegistryView::Ok(b),
        Err(e) => RegistryView::Unavailable(format!("query: {e}")),
    }
}

async fn load_board(pool: &PgPool, epoch: i16) -> anyhow::Result<RegistryBoard> {
    let now = Utc::now();
    let live = lease::status(pool).await?;
    let doing = live
        .into_iter()
        .map(|l: LeaseStatus| {
            let age_min = (now - l.claimed_at).num_minutes().max(0);
            DoingLease {
                id: l.id,
                coverage_phase: l.coverage_phase,
                claimed_by: l.claimed_by,
                claimed_at: l.claimed_at,
                age_min,
            }
        })
        .collect();

    let phase_counts = sqlx::query_as::<_, (String, i64)>(
        "select coverage_phase, count(*)::bigint
         from jurisdiction
         group by coverage_phase
         order by coverage_phase",
    )
    .fetch_all(pool)
    .await?;

    let epoch_counts = sqlx::query_as::<_, (Option<i16>, i64)>(
        "select epoch, count(*)::bigint
         from jurisdiction
         group by epoch
         order by epoch nulls first",
    )
    .fetch_all(pool)
    .await?;

    // SELECT-only twin of claim_next's pick filter (lease.rs) — keep in sync.
    let claimable_at_epoch: i64 = sqlx::query_scalar(
        "select count(*)::bigint from jurisdiction
         where epoch = $1
           and coverage_phase not in ('live', 'blocked')
           and (claimed_by is null
                or claimed_at < now() - interval '24 hours')",
    )
    .bind(epoch)
    .fetch_one(pool)
    .await?;

    let blocked_reasons = sqlx::query_as::<_, (Option<String>, i64)>(
        "select blocked_reason, count(*)::bigint
         from jurisdiction
         where coverage_phase = 'blocked'
         group by blocked_reason
         order by count(*) desc
         limit 8",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(r, n)| (r.unwrap_or_else(|| "(null)".into()), n))
    .collect();

    // Incomplete work sample: prefer epoch-assigned rows (factory-relevant);
    // null-epoch stubs are bulk queue noise and bury real LEFT signal.
    let left_sample = sqlx::query_as::<_, (String, String, Option<i16>, Option<f32>)>(
        "select id, coverage_phase, epoch, priority_score
         from jurisdiction
         where coverage_phase not in ('live', 'blocked')
           and epoch is not null
         order by priority_score desc nulls last, id
         limit 8",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(id, coverage_phase, epoch, priority_score)| LeftRow {
        id,
        coverage_phase,
        epoch,
        priority_score,
    })
    .collect();

    Ok(RegistryBoard {
        doing,
        phase_counts,
        epoch_counts,
        claimable_at_epoch,
        blocked_reasons,
        left_sample,
    })
}
