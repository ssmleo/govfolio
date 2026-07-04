/// Apply all embedded migrations (`crates/core/migrations/`) to the pool's database.
///
/// Safe to call repeatedly: already-applied migrations are skipped by checksum.
///
/// # Errors
///
/// Returns [`sqlx::migrate::MigrateError`] if a migration fails to apply, a
/// previously applied migration's checksum no longer matches, or the
/// connection is lost mid-run.
pub async fn migrate(pool: &sqlx::PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
