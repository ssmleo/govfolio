//! Bronze → `raw_document` ingestion (invariant 2: raw is sacred, sha256-
//! addressed) and Silver run-linkage (`stg_meta`, design §4.2 supporting
//! tables). All writes `ON CONFLICT DO NOTHING` (invariant 4).

use anyhow::Context as _;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::adapter::RawDocRef;
use crate::run::StagedSilver;

/// Best-effort mime sniff for the `raw_document.mime_type` column; the byte
/// content is the authority, not the URL suffix. Covers every live adapter's
/// real byte shape: PDF (Australia/US-House), HTML (US-Senate/Canada report
/// pages), JSON (UK's API response; Brazil's synthesized per-candidate join).
#[must_use]
pub fn sniff_mime(bytes: &[u8]) -> &'static str {
    let trimmed = leading_ascii_whitespace_trimmed(bytes);
    if trimmed.starts_with(b"%PDF-") {
        "application/pdf"
    } else if starts_with_html(trimmed) {
        "text/html"
    } else if looks_like_json(trimmed) {
        "application/json"
    } else {
        "application/octet-stream"
    }
}

fn leading_ascii_whitespace_trimmed(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    &bytes[start..]
}

fn starts_with_html(bytes: &[u8]) -> bool {
    const PREFIXES: [&[u8]; 4] = [b"<!DOCTYPE", b"<!doctype", b"<html", b"<HTML"];
    PREFIXES.iter().any(|prefix| bytes.starts_with(prefix))
}

fn looks_like_json(bytes: &[u8]) -> bool {
    matches!(bytes.first(), Some(b'{' | b'[')) && std::str::from_utf8(bytes).is_ok()
}

/// Ensures the `raw_document` row for a Bronze document and returns its id —
/// the existing row's id when the sha256 was seen before (dedup key, design
/// §5.2), so downstream linkage is stable across replays.
///
/// # Errors
/// Database failure.
pub async fn ensure_raw_document(
    pool: &PgPool,
    doc: &RawDocRef,
    storage_uri: &str,
    mime_type: &str,
    source_url: Option<&str>,
    fetched_at: DateTime<Utc>,
    fetch_run_id: Option<&str>,
) -> anyhow::Result<String> {
    let minted = ulid::Ulid::new().to_string();
    sqlx::query(
        "insert into raw_document \
           (id, storage_uri, sha256, mime_type, source_url, fetched_at, fetch_run_id) \
         values ($1, $2, $3, $4, $5, $6, $7) \
         on conflict (sha256) do nothing",
    )
    .bind(&minted)
    .bind(storage_uri)
    .bind(&doc.sha256)
    .bind(mime_type)
    .bind(source_url)
    .bind(fetched_at)
    .bind(fetch_run_id)
    .execute(pool)
    .await
    .with_context(|| format!("inserting raw_document {}", doc.sha256))?;
    sqlx::query_scalar("select id from raw_document where sha256 = $1")
        .bind(&doc.sha256)
        .fetch_one(pool)
        .await
        .with_context(|| format!("resolving raw_document id for {}", doc.sha256))
}

/// Links staged Silver rows to the pipeline run that produced them
/// (`stg_meta`; first writer wins, replays add nothing).
///
/// # Errors
/// Database failure.
pub async fn link_stg_meta(
    pool: &PgPool,
    stg_table: &str,
    staged: &[StagedSilver],
    raw_document_id: &str,
    pipeline_run_id: &str,
) -> anyhow::Result<()> {
    for row in staged {
        sqlx::query(
            "insert into stg_meta (stg_table, stg_id, raw_document_id, pipeline_run_id) \
             values ($1, $2, $3, $4) \
             on conflict (stg_table, stg_id) do nothing",
        )
        .bind(stg_table)
        .bind(&row.stg_id)
        .bind(raw_document_id)
        .bind(pipeline_run_id)
        .execute(pool)
        .await
        .with_context(|| format!("linking stg_meta for {stg_table}/{}", row.stg_id))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::sniff_mime;

    #[test]
    fn sniffs_pdf_magic_and_falls_back_to_octet_stream() {
        assert_eq!(sniff_mime(b"%PDF-1.7 rest"), "application/pdf");
        assert_eq!(sniff_mime(b"<?xml?>"), "application/octet-stream");
        assert_eq!(sniff_mime(b""), "application/octet-stream");
    }

    #[test]
    fn sniffs_html_by_doctype_or_tag() {
        assert_eq!(sniff_mime(b"<!DOCTYPE html><html></html>"), "text/html");
        assert_eq!(sniff_mime(b"<html><body>report</body></html>"), "text/html");
        assert_eq!(sniff_mime(b"  <!DOCTYPE html>\n<html></html>"), "text/html");
    }

    #[test]
    fn sniffs_json_objects_and_arrays_but_not_other_leading_brackets() {
        assert_eq!(sniff_mime(br#"{"a":1}"#), "application/json");
        assert_eq!(sniff_mime(b"[1,2,3]"), "application/json");
        assert_eq!(
            sniff_mime(b"<?xml version=\"1.0\"?>"),
            "application/octet-stream"
        );
    }
}
