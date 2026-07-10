//! One-off remediation: re-sniffs every `raw_document.mime_type` from the
//! document's ACTUAL archived bytes, correcting rows that were mislabeled by
//! the live-fetch mime-sniffing bug (fixed in `crates/pipeline/src/run.rs`'s
//! `fetch_remote`, see `docs/plans/2026-07-09-filing-document-viewer-design.md`
//! §3). That fix was forward-only: it makes NEWLY ingested documents get the
//! right `mime_type`, but does nothing for documents already sitting in
//! `raw_document` — every already-ingested row that went through the live
//! path was hardcoded to `application/pdf` regardless of actual content
//! (`br`'s ~22,000 historical rows are JSON, not PDF, and are the largest
//! affected slice as of 2026-07-09).
//!
//! This bin is the mechanical fix: for every `raw_document` row, read the
//! real bytes off `storage_uri`, re-sniff with
//! `pipeline::stages::ingest::sniff_mime` (the SAME function both the live
//! and offline fetch paths call), and `UPDATE raw_document SET mime_type`
//! when the sniffed value disagrees with what's stored. Only ever touches
//! the `mime_type` METADATA column — the Bronze bytes themselves are never
//! read back into anything but memory, never rewritten (invariant 2: raw is
//! sacred).
//!
//! NOT an identity-resolution fix (contrast
//! `fix-br-julio-cesar-santos-ba-2018.rs`): no blast-radius snapshot, no
//! FK rewrites, nothing politician/filing-shaped. Deliberately simple.
//!
//! Idempotent: a row whose stored `mime_type` already matches the sniffed
//! value is left untouched, so re-running reports 0 corrections once every
//! row has been fixed once.
//!
//! A `storage_uri` that can't be read (missing file — e.g. it points at a
//! purged temp directory from an old backfill run — or an unsupported
//! scheme) is SKIPPED and counted, never a hard failure; this is a known,
//! disclosed environment gap, not something this bin fixes.
//!
//! Dry-run by default (mirrors `fix-br-julio-cesar-santos-ba-2018.rs`'s own
//! dry-run/`--execute` convention): prints what WOULD change; `--execute`
//! applies the `UPDATE`s.
//!
//! *** DO NOT run `--execute` against a shared dev/prod Postgres without
//! *** explicit human/founder sign-off. This repo runs with other
//! *** concurrent agents against shared Postgres instances — an unreviewed
//! *** mass `UPDATE` is exactly the kind of blast radius that needs a human
//! *** in the loop first. Running this against the founder's real local dev
//! *** data (the ~22,000 affected `br` rows) is an explicit MANUAL
//! *** follow-up step, not something this bin's author (an agent) runs
//! *** unsupervised.
//!
//! Usage:
//! ```text
//! cargo run -p worker --bin resniff-raw-document-mime [-- --execute]
//! ```
//!
//! Env: `DATABASE_URL` (required).

use std::collections::BTreeMap;

use anyhow::Context as _;
use sqlx::PgPool;

use pipeline::stages::ingest::sniff_mime;

fn parse_args() -> anyhow::Result<bool> {
    let mut execute = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--execute" => execute = true,
            other => anyhow::bail!("unknown argument {other:?} (expected --execute)"),
        }
    }
    Ok(execute)
}

#[derive(sqlx::FromRow, Debug, Clone)]
struct RawDocumentRow {
    id: String,
    storage_uri: String,
    mime_type: String,
}

/// One row's outcome after comparing its stored `mime_type` against the
/// bytes' actual sniffed value. Pure — DB/filesystem-free — so it's
/// unit-testable without a live database or real files.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    /// Stored value already matches the sniffed value — nothing to do.
    AlreadyCorrect,
    /// Stored value disagrees; the caller should `UPDATE` to `to`.
    Corrected { to: String },
}

/// Pure decision: does `stored_mime` need correcting to `sniffed_mime`?
fn classify(stored_mime: &str, sniffed_mime: &str) -> Verdict {
    if stored_mime == sniffed_mime {
        Verdict::AlreadyCorrect
    } else {
        Verdict::Corrected {
            to: sniffed_mime.to_owned(),
        }
    }
}

/// Strips the `file://` scheme prefix a `storage_uri` carries — the only
/// scheme any adapter writes today (mirrors `crates/api/src/bronze.rs`'s
/// `read_document`, reimplemented here since this is a separate crate and
/// the remediation is a two-line strip+read, not worth a cross-crate dep).
fn local_path(storage_uri: &str) -> Option<&str> {
    storage_uri.strip_prefix("file://")
}

/// A run's tally: what was found and what (would be/was) changed.
#[derive(Debug, Default)]
struct Summary {
    scanned: usize,
    corrected: usize,
    /// target mime type -> how many rows were (would be) corrected to it.
    corrected_to: BTreeMap<String, usize>,
    skipped_unreadable: usize,
}

impl Summary {
    fn print(&self, execute: bool) {
        println!(
            "resniff-raw-document-mime ({}) summary:",
            if execute { "EXECUTE" } else { "DRY-RUN" }
        );
        println!("  scanned:            {}", self.scanned);
        println!(
            "  corrected{}:  {}",
            if execute { "        " } else { " (would be)" },
            self.corrected
        );
        for (mime, count) in &self.corrected_to {
            println!("    -> {mime}: {count}");
        }
        println!(
            "  skipped (unreadable storage_uri): {}",
            self.skipped_unreadable
        );
    }
}

/// Runs the resniff over every `raw_document` row in `pool`. `execute` gates
/// whether corrections are actually written (`false` = dry-run report only).
async fn run_resniff(pool: &PgPool, execute: bool) -> anyhow::Result<Summary> {
    let rows: Vec<RawDocumentRow> =
        sqlx::query_as("select id, storage_uri, mime_type from raw_document")
            .fetch_all(pool)
            .await
            .context("selecting raw_document rows")?;

    let mut summary = Summary::default();
    for row in rows {
        summary.scanned += 1;
        let Some(path) = local_path(&row.storage_uri) else {
            eprintln!(
                "skip {}: unsupported storage_uri scheme {:?}",
                row.id, row.storage_uri
            );
            summary.skipped_unreadable += 1;
            continue;
        };
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) => {
                eprintln!("skip {}: could not read {path:?}: {err}", row.id);
                summary.skipped_unreadable += 1;
                continue;
            }
        };
        let sniffed = sniff_mime(&bytes);
        match classify(&row.mime_type, sniffed) {
            Verdict::AlreadyCorrect => {}
            Verdict::Corrected { to } => {
                summary.corrected += 1;
                *summary.corrected_to.entry(to.clone()).or_insert(0) += 1;
                if execute {
                    sqlx::query("update raw_document set mime_type = $1 where id = $2")
                        .bind(&to)
                        .bind(&row.id)
                        .execute(pool)
                        .await
                        .with_context(|| {
                            format!("updating mime_type for raw_document {}", row.id)
                        })?;
                }
            }
        }
    }
    Ok(summary)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let execute = parse_args()?;
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let pool = PgPool::connect(&database_url)
        .await
        .context("connecting to Postgres")?;

    println!(
        "== resniff-raw-document-mime ({}) ==",
        if execute { "EXECUTE" } else { "DRY-RUN" }
    );
    let summary = run_resniff(&pool, execute).await?;
    summary.print(execute);
    if !execute && summary.corrected > 0 {
        println!("Re-run with --execute to apply the corrections above.");
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn matching_mime_needs_no_correction() {
        assert_eq!(
            classify("application/pdf", "application/pdf"),
            Verdict::AlreadyCorrect
        );
    }

    #[test]
    fn mismatched_mime_is_corrected_to_the_sniffed_value() {
        assert_eq!(
            classify("application/pdf", "application/json"),
            Verdict::Corrected {
                to: "application/json".to_owned()
            }
        );
    }

    #[test]
    fn local_path_strips_the_file_scheme() {
        assert_eq!(local_path("file:///tmp/foo.pdf"), Some("/tmp/foo.pdf"));
        assert_eq!(local_path("gs://bucket/object"), None);
    }

    /// Seeds a few `raw_document` rows (no FK dependencies — the table
    /// stands alone, see `crates/core/migrations/0001_core.sql`) with
    /// wrong-but-known `mime_type`s and REAL temp files, then proves:
    /// dry-run reports the corrections without writing; `--execute` writes
    /// them; and a second `--execute` run is a no-op (idempotent).
    #[sqlx::test(migrations = false)]
    #[ignore = "needs postgres"]
    async fn corrects_wrong_mime_types_and_is_idempotent(pool: sqlx::PgPool) {
        govfolio_core::db::migrate(&pool).await.unwrap();

        // Row 1: stored as PDF, bytes are actually JSON (the live bug's
        // real-world shape for br) — should be corrected.
        let json_path =
            std::env::temp_dir().join(format!("govfolio-resniff-test-{}.json", ulid::Ulid::new()));
        std::fs::write(&json_path, br#"{"a":1}"#).unwrap();
        let json_id = ulid::Ulid::new().to_string();
        insert_raw_document(&pool, &json_id, &json_path, "application/pdf").await;

        // Row 2: stored as PDF, bytes really are PDF — already correct, must
        // not be touched.
        let pdf_path =
            std::env::temp_dir().join(format!("govfolio-resniff-test-{}.pdf", ulid::Ulid::new()));
        std::fs::write(&pdf_path, b"%PDF-1.7 test").unwrap();
        let pdf_id = ulid::Ulid::new().to_string();
        insert_raw_document(&pool, &pdf_id, &pdf_path, "application/pdf").await;

        // Row 3: storage_uri points at a file that doesn't exist (a purged
        // temp backfill dir) — must be skipped, not crash the run.
        let missing_path = std::env::temp_dir().join(format!(
            "govfolio-resniff-test-missing-{}.bin",
            ulid::Ulid::new()
        ));
        let missing_id = ulid::Ulid::new().to_string();
        insert_raw_document(&pool, &missing_id, &missing_path, "application/pdf").await;

        // Dry-run: reports the one correction, writes nothing.
        let dry = run_resniff(&pool, false).await.unwrap();
        assert_eq!(dry.scanned, 3);
        assert_eq!(dry.corrected, 1);
        assert_eq!(dry.skipped_unreadable, 1);
        assert_eq!(stored_mime(&pool, &json_id).await, "application/pdf");

        // Execute: writes the correction.
        let applied = run_resniff(&pool, true).await.unwrap();
        assert_eq!(applied.scanned, 3);
        assert_eq!(applied.corrected, 1);
        assert_eq!(applied.skipped_unreadable, 1);
        assert_eq!(stored_mime(&pool, &json_id).await, "application/json");
        assert_eq!(stored_mime(&pool, &pdf_id).await, "application/pdf");

        // Second execute: idempotent, 0 corrections.
        let second = run_resniff(&pool, true).await.unwrap();
        assert_eq!(second.corrected, 0);
        assert_eq!(second.skipped_unreadable, 1);

        std::fs::remove_file(&json_path).unwrap();
        std::fs::remove_file(&pdf_path).unwrap();
    }

    async fn insert_raw_document(
        pool: &sqlx::PgPool,
        id: &str,
        path: &std::path::Path,
        mime: &str,
    ) {
        let storage_uri = format!("file://{}", path.display());
        sqlx::query(
            "insert into raw_document (id, storage_uri, sha256, mime_type, fetched_at) \
             values ($1, $2, $3, $4, now())",
        )
        .bind(id)
        .bind(&storage_uri)
        .bind(format!("{id}-sha256"))
        .bind(mime)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn stored_mime(pool: &sqlx::PgPool, id: &str) -> String {
        sqlx::query_scalar("select mime_type from raw_document where id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap()
    }
}
