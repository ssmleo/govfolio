//! CLI migrator: reads `DATABASE_URL`, connects, applies embedded migrations.
//! Usage: `DATABASE_URL=postgres://... cargo run -p core --bin migrate`

use anyhow::Context;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL is not set")?;
    let pool = sqlx::PgPool::connect(&url)
        .await
        .context("failed to connect to DATABASE_URL")?;
    govfolio_core::db::migrate(&pool)
        .await
        .context("failed to apply migrations")?;
    println!("migrations up to date");
    // Seed the worldwide jurisdiction registry (design §5.7/§5.8; goal 065).
    // Idempotent (ON CONFLICT DO NOTHING) — safe on every deploy.
    govfolio_core::seed::seed_registry(&pool)
        .await
        .context("failed to seed jurisdiction registry")?;
    println!("jurisdiction registry seeded");
    Ok(())
}
