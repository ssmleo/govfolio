//! Snapshot export (goal 050): full Gold to gzipped CSV + JSONL with a
//! sha256 manifest and the CC BY note; visibility through the ONE evaluator
//! (the free-tier 24h bound — a snapshot must not tunnel under the delay).
//!
//! DB-gated like the other sqlx suites: `--ignored` + postgres on `DATABASE_URL`.
#![allow(clippy::unwrap_used)]

use std::io::Read as _;

use chrono::{Duration, Utc};
use flate2::read::GzDecoder;
use sha2::{Digest as _, Sha256};
use sqlx::PgPool;

use worker::snapshot::run_snapshot;

/// Seeds `old_n` records past the 24h bound and one fresh record inside it.
async fn seed(pool: &PgPool, old_n: usize) {
    govfolio_core::db::migrate(pool).await.unwrap();
    let politician_id = ulid::Ulid::new().to_string();
    let regime_id = ulid::Ulid::new().to_string();
    sqlx::query(
        "insert into jurisdiction (id, name, level) values ('us', 'United States', 'national')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into disclosure_regime \
           (id, jurisdiction_id, body, regime_type, value_precision, effective_from) \
         values ($1, 'us', 'US House', 'transaction_report', 'banded', '2012-01-01')",
    )
    .bind(&regime_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("insert into politician (id, canonical_name) values ($1, 'Test Person')")
        .bind(&politician_id)
        .execute(pool)
        .await
        .unwrap();
    for (i, age_hours) in std::iter::repeat_n(25i64, old_n)
        .chain(std::iter::once(0))
        .enumerate()
    {
        let raw_id = ulid::Ulid::new().to_string();
        let filing_id = ulid::Ulid::new().to_string();
        sqlx::query(
            "insert into raw_document (id, storage_uri, sha256, mime_type, fetched_at) \
             values ($1, $2, $3, 'application/pdf', now())",
        )
        .bind(&raw_id)
        .bind(format!("file:///bronze/{i}.pdf"))
        .bind(format!("sha-{i}"))
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "insert into filing \
               (id, regime_id, politician_id, raw_document_id, external_id, filing_type, \
                discovered_at) \
             values ($1, $2, $3, $4, $5, 'ptr', $6)",
        )
        .bind(&filing_id)
        .bind(&regime_id)
        .bind(&politician_id)
        .bind(&raw_id)
        .bind(format!("ext-{i}"))
        .bind(Utc::now() - Duration::hours(age_hours))
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "insert into disclosure_record \
               (id, filing_id, politician_id, regime_id, asset_description_raw, record_type, \
                asset_class, side, transaction_date, value_low, value_high, currency, \
                extracted_by, fingerprint) \
             values ($1, $2, $3, $4, $5, 'transaction', 'equity', 'buy', '2026-06-01', \
                     1001.00, 15000.00, 'USD', 'snapshot-test', $6)",
        )
        .bind(ulid::Ulid::new().to_string())
        .bind(&filing_id)
        .bind(&politician_id)
        .bind(&regime_id)
        .bind(format!("asset {i}"))
        .bind(format!("fp-{i}"))
        .execute(pool)
        .await
        .unwrap();
    }
}

fn gunzip(bytes: &[u8]) -> String {
    let mut out = String::new();
    GzDecoder::new(bytes).read_to_string(&mut out).unwrap();
    out
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn snapshot_exports_verified_and_unverified_behind_the_free_boundary(pool: PgPool) {
    seed(&pool, 2).await;
    let dir = std::env::temp_dir().join(format!(
        "govfolio-snapshot-test-{}-{}",
        std::process::id(),
        ulid::Ulid::new()
    ));
    let outcome = run_snapshot(&pool, &dir).await.unwrap();

    // The fresh record (discovered < 24h ago) is NOT in the free-tier
    // artifact; the two aged ones are — same ONE evaluator as /v1.
    assert_eq!(outcome.manifest.record_count, 2);
    assert_eq!(outcome.manifest.license, "CC-BY-4.0");

    // Manifest checksums match the bytes on disk, byte for byte.
    assert_eq!(outcome.manifest.files.len(), 2);
    for file in &outcome.manifest.files {
        let bytes = std::fs::read(dir.join(&file.name)).unwrap();
        assert_eq!(bytes.len() as u64, file.bytes);
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        assert_eq!(
            format!("{:x}", hasher.finalize()),
            file.sha256,
            "{}",
            file.name
        );
    }

    // JSONL: one JSON object per record; money = decimal strings
    // (invariant 7); verification_state on every row.
    let jsonl = gunzip(&std::fs::read(dir.join("records.jsonl.gz")).unwrap());
    let rows: Vec<serde_json::Value> = jsonl
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 2);
    for row in &rows {
        assert_eq!(row["value_low"], serde_json::json!("1001.00"));
        assert_eq!(row["verification_state"], serde_json::json!("unverified"));
        assert!(row["details"].is_object(), "details stays real JSON");
    }

    // CSV: header + 2 rows, same column count throughout.
    let csv_text = gunzip(&std::fs::read(dir.join("records.csv.gz")).unwrap());
    let lines: Vec<&str> = csv_text.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("id,filing_id,"));

    // The note is a factual one-liner + the license identifier, nothing more.
    let note = std::fs::read_to_string(dir.join("NOTE.txt")).unwrap();
    assert!(note.contains("CC BY 4.0"));
    assert!(note.contains("2 records"));
    assert_eq!(note.lines().count(), 1, "one line, no copy");

    std::fs::remove_dir_all(&dir).ok();
}
