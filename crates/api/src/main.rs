//! The `/v1` API server (plan M1 exit: `cargo run -p api` serves real House
//! records at `localhost:8080/v1/records`).

use anyhow::Context as _;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must be set (see .env.example)")?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url)
        .await
        .context("connecting to postgres")?;
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .context("binding 0.0.0.0:8080")?;
    println!("govfolio api listening on http://localhost:8080 (try /v1/records)");
    axum::serve(listener, api::app(pool))
        .await
        .context("serving /v1")
}
