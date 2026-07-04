//! DB-touching suite: gated behind `--ignored` (CI db job / local postgres on 5433).
#![allow(clippy::unwrap_used)]

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn migrator_is_idempotent(pool: sqlx::PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    govfolio_core::db::migrate(&pool).await.unwrap(); // second run: no-op, no error
    let n: i64 = sqlx::query_scalar("select count(*) from _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 2); // 0000_init + 0001_core
}
