//! Regime seeding: the `jurisdiction` + `disclosure_regime` rows an adapter
//! publishes under. Ids are adapter-provided stable constants (one regime row
//! per adapter code, design §4.2), so seeding is idempotent by primary key —
//! `ON CONFLICT DO NOTHING`, replays insert nothing.

use anyhow::Context as _;
use chrono::NaiveDate;
use sqlx::PgPool;

/// The `jurisdiction` row a regime hangs off (design §4.2).
#[derive(Debug, Clone)]
pub struct JurisdictionSeed {
    /// Stable jurisdiction id (ISO 3166-1 alpha-2 lowercase by convention).
    pub id: String,
    /// Display name.
    pub name: String,
    /// ISO 3166-1 alpha-2 where applicable.
    pub iso_code: Option<String>,
    /// `supranational` | `national` | `subnational`.
    pub level: String,
}

/// One `disclosure_regime` row plus its jurisdiction (regime doc §1 metadata).
#[derive(Debug, Clone)]
pub struct RegimeSeed {
    /// Jurisdiction the regime belongs to.
    pub jurisdiction: JurisdictionSeed,
    /// Stable regime row id (adapter constant).
    pub regime_id: String,
    /// Body, e.g. `US House`.
    pub body: String,
    /// `transaction_report` | `periodic_declaration` | `change_notification` | `none`.
    pub regime_type: String,
    /// `exact` | `banded` | `categorical` | `none`.
    pub value_precision: String,
    /// Free-form cadence description.
    pub cadence: Option<String>,
    /// Statutory maximum disclosure lag in days.
    pub disclosure_lag_days: Option<i32>,
    /// Official source landing page.
    pub source_url: Option<String>,
    /// Date the regime's rules took effect.
    pub effective_from: NaiveDate,
}

/// Seeds the jurisdiction + regime rows; idempotent by stable ids.
///
/// # Errors
/// Database failure.
pub async fn seed_regime(pool: &PgPool, seed: &RegimeSeed) -> anyhow::Result<()> {
    sqlx::query(
        "insert into jurisdiction (id, name, iso_code, level) \
         values ($1, $2, $3, $4) on conflict do nothing",
    )
    .bind(&seed.jurisdiction.id)
    .bind(&seed.jurisdiction.name)
    .bind(&seed.jurisdiction.iso_code)
    .bind(&seed.jurisdiction.level)
    .execute(pool)
    .await
    .with_context(|| format!("seeding jurisdiction {}", seed.jurisdiction.id))?;
    sqlx::query(
        "insert into disclosure_regime \
           (id, jurisdiction_id, body, regime_type, value_precision, cadence, \
            disclosure_lag_days, source_url, effective_from) \
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9) on conflict do nothing",
    )
    .bind(&seed.regime_id)
    .bind(&seed.jurisdiction.id)
    .bind(&seed.body)
    .bind(&seed.regime_type)
    .bind(&seed.value_precision)
    .bind(&seed.cadence)
    .bind(seed.disclosure_lag_days)
    .bind(&seed.source_url)
    .bind(seed.effective_from)
    .execute(pool)
    .await
    .with_context(|| format!("seeding disclosure_regime {}", seed.regime_id))?;
    Ok(())
}
