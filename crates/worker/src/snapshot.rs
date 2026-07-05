//! Monthly snapshot export (goal 050; design §6.2 "Bulk export: monthly
//! snapshot" — the free tier's bulk door). Full Gold `disclosure_record`
//! export, verified AND unverified (`verification_state` travels with every
//! row — honesty in bulk too), as gzipped CSV + JSONL with a sha256
//! manifest and a CC BY license note.
//!
//! Visibility runs through the ONE evaluator (`core::query::RecordFilter`)
//! with the same 24h `filing.discovered_at` bound the free API tier gets:
//! the snapshot is a free-tier artifact, so it must not tunnel under the
//! freemium delay (design §6.2).
//!
//! Output lands in a local directory (GCS upload is deploy plumbing, later).
//! Money stays decimal strings (invariant 7 — `rust_decimal` serde), and
//! `details` stays real JSON in JSONL / compact JSON text in CSV.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use const_format::concatcp;
use flate2::Compression;
use flate2::write::GzEncoder;
use rust_decimal::Decimal;
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use sqlx::PgPool;

use govfolio_core::query::RecordFilter;

/// The free tier's freshness bound, mirrored here (the API twin lives in
/// `api::auth`; both feed the same `core::query` slot).
const FREE_DELAY_HOURS: i64 = 24;

/// Export projection — every Gold column, nothing invented.
const EXPORT_SQL: &str = concatcp!(
    "select id, filing_id, politician_id, regime_id, instrument_id, \
     asset_description_raw, record_type, asset_class, side, transaction_date, \
     as_of_date, notified_date, event_date, value_low, value_high, currency, \
     owner, verification_state, extraction_confidence, extracted_by, \
     fingerprint, supersedes_record_id, details, created_at \
     from disclosure_record where ",
    RecordFilter::SQL_WHERE,
    " order by id"
);

/// One exported record (CSV column order == field order here).
#[derive(Debug, Serialize, sqlx::FromRow)]
struct ExportRecord {
    id: String,
    filing_id: String,
    politician_id: String,
    regime_id: String,
    instrument_id: Option<String>,
    asset_description_raw: String,
    record_type: String,
    asset_class: String,
    side: Option<String>,
    transaction_date: Option<NaiveDate>,
    as_of_date: Option<NaiveDate>,
    notified_date: Option<NaiveDate>,
    event_date: Option<NaiveDate>,
    value_low: Option<Decimal>,
    value_high: Option<Decimal>,
    currency: Option<String>,
    owner: Option<String>,
    verification_state: String,
    extraction_confidence: Option<f32>,
    extracted_by: String,
    fingerprint: String,
    supersedes_record_id: Option<String>,
    details: serde_json::Value,
    created_at: DateTime<Utc>,
}

const CSV_HEADER: [&str; 24] = [
    "id",
    "filing_id",
    "politician_id",
    "regime_id",
    "instrument_id",
    "asset_description_raw",
    "record_type",
    "asset_class",
    "side",
    "transaction_date",
    "as_of_date",
    "notified_date",
    "event_date",
    "value_low",
    "value_high",
    "currency",
    "owner",
    "verification_state",
    "extraction_confidence",
    "extracted_by",
    "fingerprint",
    "supersedes_record_id",
    "details",
    "created_at",
];

impl ExportRecord {
    /// CSV cells, in [`CSV_HEADER`] order. Options render empty; `details`
    /// renders as compact JSON text.
    fn csv_cells(&self) -> Vec<String> {
        fn opt<T: ToString>(value: Option<&T>) -> String {
            value.map(ToString::to_string).unwrap_or_default()
        }
        vec![
            self.id.clone(),
            self.filing_id.clone(),
            self.politician_id.clone(),
            self.regime_id.clone(),
            opt(self.instrument_id.as_ref()),
            self.asset_description_raw.clone(),
            self.record_type.clone(),
            self.asset_class.clone(),
            opt(self.side.as_ref()),
            opt(self.transaction_date.as_ref()),
            opt(self.as_of_date.as_ref()),
            opt(self.notified_date.as_ref()),
            opt(self.event_date.as_ref()),
            opt(self.value_low.as_ref()),
            opt(self.value_high.as_ref()),
            opt(self.currency.as_ref()),
            opt(self.owner.as_ref()),
            self.verification_state.clone(),
            opt(self.extraction_confidence.as_ref()),
            self.extracted_by.clone(),
            self.fingerprint.clone(),
            opt(self.supersedes_record_id.as_ref()),
            self.details.to_string(),
            self.created_at.to_rfc3339(),
        ]
    }
}

/// One exported file as manifested.
#[derive(Debug, Serialize)]
pub struct ManifestFile {
    /// File name within the snapshot directory.
    pub name: String,
    /// Size in bytes (as written, i.e. gzipped).
    pub bytes: u64,
    /// sha256 hex of the file bytes (integrity + citability, same hasher
    /// family as Bronze).
    pub sha256: String,
}

/// The snapshot manifest (written as `MANIFEST.json`).
#[derive(Debug, Serialize)]
pub struct Manifest {
    /// When the export ran.
    pub generated_at: DateTime<Utc>,
    /// Records with `filing.discovered_at` after this instant are NOT in the
    /// snapshot (the free-tier freshness bound).
    pub max_discovered_at: DateTime<Utc>,
    /// Exported record count (rows in each data file).
    pub record_count: u64,
    /// The data files with checksums.
    pub files: Vec<ManifestFile>,
    /// SPDX license identifier for the exported data.
    pub license: String,
}

/// Where a snapshot landed.
#[derive(Debug)]
pub struct SnapshotOutcome {
    /// Directory containing the four files.
    pub dir: PathBuf,
    /// The manifest as written.
    pub manifest: Manifest,
}

fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> anyhow::Result<ManifestFile> {
    std::fs::write(dir.join(name), bytes).with_context(|| format!("writing {name}"))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(ManifestFile {
        name: name.to_owned(),
        bytes: bytes.len() as u64,
        sha256: format!("{:x}", hasher.finalize()),
    })
}

/// Runs one full export into `dir` (created if absent): `records.csv.gz`,
/// `records.jsonl.gz`, `MANIFEST.json`, `NOTE.txt`.
///
/// # Errors
/// Database or filesystem failure; a query-layer failure binding the
/// visibility bound (structurally impossible).
pub async fn run_snapshot(pool: &PgPool, dir: &Path) -> anyhow::Result<SnapshotOutcome> {
    let generated_at = Utc::now();
    let max_discovered_at = generated_at - Duration::hours(FREE_DELAY_HOURS);
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;

    let visibility = RecordFilter::default().with_max_discovered_at(Some(max_discovered_at));
    let rows: Vec<ExportRecord> = visibility
        .bind_query_as(sqlx::query_as(EXPORT_SQL))
        .context("binding the visibility bound")?
        .fetch_all(pool)
        .await
        .context("reading disclosure records")?;

    // CSV (gzip).
    let mut csv_gz = GzEncoder::new(Vec::new(), Compression::default());
    {
        let mut writer = csv::Writer::from_writer(&mut csv_gz);
        writer.write_record(CSV_HEADER).context("csv header")?;
        for row in &rows {
            writer.write_record(row.csv_cells()).context("csv row")?;
        }
        writer.flush().context("flushing csv")?;
    }
    let csv_bytes = csv_gz.finish().context("gzipping csv")?;

    // JSONL (gzip): one JSON object per line, details as real JSON, money as
    // decimal strings.
    let mut jsonl_gz = GzEncoder::new(Vec::new(), Compression::default());
    for row in &rows {
        let line = serde_json::to_string(row).context("serializing jsonl row")?;
        jsonl_gz.write_all(line.as_bytes()).context("jsonl row")?;
        jsonl_gz.write_all(b"\n").context("jsonl newline")?;
    }
    let jsonl_bytes = jsonl_gz.finish().context("gzipping jsonl")?;

    let files = vec![
        write_file(dir, "records.csv.gz", &csv_bytes)?,
        write_file(dir, "records.jsonl.gz", &jsonl_bytes)?,
    ];
    let manifest = Manifest {
        generated_at,
        max_discovered_at,
        record_count: rows.len() as u64,
        files,
        license: "CC-BY-4.0".to_owned(),
    };
    let manifest_json =
        serde_json::to_string_pretty(&manifest).context("serializing the manifest")?;
    std::fs::write(dir.join("MANIFEST.json"), format!("{manifest_json}\n"))
        .context("writing MANIFEST.json")?;

    // License note: factual one-liner + license identifier ONLY (public
    // legal/marketing copy is a human lane — automation-policy residual).
    let note = format!(
        "govfolio.io disclosure-record snapshot generated {} ({} records, \
         verified and unverified). License: CC BY 4.0 \
         (https://creativecommons.org/licenses/by/4.0/).\n",
        generated_at.format("%Y-%m-%d"),
        manifest.record_count
    );
    std::fs::write(dir.join("NOTE.txt"), note).context("writing NOTE.txt")?;

    Ok(SnapshotOutcome {
        dir: dir.to_path_buf(),
        manifest,
    })
}
