//! Pre-publication gates (goal 070, design §7.5 redaction + §5.6 drift freeze).
//! DB-gated like the other sqlx suites (`--ignored`, postgres on `DATABASE_URL`).
//! Exercises `publish_filing` directly — the atomic Gold-writing chokepoint both
//! gates live in — proving:
//! - redaction STRIPS flagged `details` keys from the published Gold row while
//!   the as-filed raw + Bronze document stay intact (invariant 2);
//! - redaction SUPPRESSES an un-republishable record (FR patrimony) — no Gold
//!   row, a filing `review_task` instead;
//! - a FROZEN regime refuses to publish (0 Gold rows + a `review_task`), and
//!   once the freeze clears, the retry publishes normally.
#![allow(clippy::unwrap_used)]

use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;

use govfolio_core::domain::enums::{AssetClass, RecordType};
use govfolio_core::domain::gold::GoldCandidate;
use pipeline::run::FilingIdentity;
use pipeline::stages::publish::{FilingSpec, publish_filing};

// publish_filing parses regime_id/politician_id into ULID domain types
// (bind_identity), so the seeded ids must be canonical ULIDs.
const REGIME_ID: &str = "01BX5ZZKBKACTAV9WEVGEMMVS0";
const REGIME_CODE: &str = "fr_hatvp_dia";
const POLITICIAN_ID: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";
const RAW_DOC_ID: &str = "raw-fr";

/// jurisdiction + `fr_hatvp_dia` regime + politician + `raw_document` — the FK
/// parents `publish_filing` needs before it can insert a filing + Gold rows.
const SEED_PARENTS: &str = r"
insert into jurisdiction (id, name, iso_code, level) values
  ('jur-fr', 'France', 'FR', 'national');

insert into disclosure_regime
  (id, jurisdiction_id, body, regime_type, value_precision, effective_from) values
  ('01BX5ZZKBKACTAV9WEVGEMMVS0', 'jur-fr', 'HATVP DIA', 'periodic_declaration', 'exact',
   '2020-01-01');

insert into politician (id, canonical_name) values
  ('01BX5ZZKBKACTAV9WEVGEMMVRZ', 'Test Deputé');

insert into raw_document (id, storage_uri, sha256, mime_type, fetched_at) values
  ('raw-fr', 'file:///raw-fr', 'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
   'application/xml', now());
";

async fn seed(pool: &PgPool) {
    govfolio_core::db::migrate(pool).await.unwrap();
    sqlx::raw_sql(SEED_PARENTS).execute(pool).await.unwrap();
}

fn identity() -> FilingIdentity {
    FilingIdentity {
        external_id: "dia-firmin-2026".to_owned(),
        filer_name: "Test Deputé".to_owned(),
        district: "FR-75".to_owned(),
        filing_type: "dia".to_owned(),
        filed_date: None,
    }
}

fn spec(identity: &FilingIdentity) -> FilingSpec<'_> {
    FilingSpec {
        regime_id: REGIME_ID,
        regime_code: REGIME_CODE,
        politician_id: POLITICIAN_ID,
        raw_document_id: RAW_DOC_ID,
        identity,
        discovered_at: Utc::now(),
        backfill: false,
    }
}

/// A contract-valid `fr_hatvp_dia` interest candidate; `extra` is merged into
/// `details` (identity fields are nil ULIDs — publish binds them from the spec).
fn fr_candidate(type_declaration: &str, extra: serde_json::Value) -> GoldCandidate {
    let mut details = json!({
        "declaration_stem": "dia-firmin-2026",
        "declaration_uuid": "11111111-1111-1111-1111-111111111111",
        "type_declaration": type_declaration,
        "is_modificative": false,
        "section_tag": "activitesProfessionnellesDto",
        "row_ordinal": 1,
        "motif": "CREATION",
        "entry_fields": {},
        "montants": [],
        "language": "fr",
        "value_source": "none"
    });
    if let (serde_json::Value::Object(map), serde_json::Value::Object(add)) = (&mut details, extra)
    {
        map.extend(add);
    }
    GoldCandidate {
        filing_id: "00000000000000000000000000".parse().unwrap(),
        politician_id: "00000000000000000000000000".parse().unwrap(),
        regime_id: "00000000000000000000000000".parse().unwrap(),
        instrument_id: None,
        asset_description_raw: "Activité de conseil — Cabinet Firmin".to_owned(),
        record_type: RecordType::Interest,
        asset_class: AssetClass::Other,
        side: None,
        transaction_date: None,
        as_of_date: None,
        notified_date: None,
        value: None,
        owner: None,
        extraction_confidence: Some(0.99),
        extracted_by: "fixture:gates@0".to_owned(),
        fingerprint: None,
        details,
    }
}

fn no_reasons() -> impl Fn(&GoldCandidate) -> Vec<String> + Sync {
    |_: &GoldCandidate| Vec::new()
}

async fn gold_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("select count(*) from disclosure_record")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn redaction_strips_flagged_details_from_gold_but_keeps_raw(pool: PgPool) {
    seed(&pool).await;
    let identity = identity();
    let candidate = fr_candidate(
        "dia",
        json!({ "declarant_address": "12 rue Privée, 75000 Paris" }),
    );

    let stats = publish_filing(
        &pool,
        &spec(&identity),
        std::slice::from_ref(&candidate),
        &no_reasons(),
    )
    .await
    .unwrap();
    assert_eq!(stats.gold_inserted, 1);
    assert_eq!(stats.suppressed, 0);

    // The flagged PII key is GONE from the published Gold row...
    let has_address: bool = sqlx::query_scalar(
        "select jsonb_exists(details, 'declarant_address') from disclosure_record",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !has_address,
        "declarant_address is redacted from public Gold"
    );

    // ...but the record itself, its as-filed raw, and the unflagged detail keys
    // are intact (invariant 2: raw is sacred).
    let (raw, type_decl): (String, String) = sqlx::query_as(
        "select asset_description_raw, details->>'type_declaration' from disclosure_record",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(raw, "Activité de conseil — Cabinet Firmin");
    assert_eq!(type_decl, "dia");

    // The in-memory source candidate (the Silver-equivalent) still carries it —
    // redaction copied, it did not mutate upstream.
    assert!(candidate.details.get("declarant_address").is_some());

    // Bronze is untouched: the raw_document row survives with its sha.
    let raw_docs: i64 = sqlx::query_scalar("select count(*) from raw_document where id = $1")
        .bind(RAW_DOC_ID)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(raw_docs, 1, "Bronze raw_document is never redacted");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn redaction_suppresses_fr_patrimony_and_writes_no_gold(pool: PgPool) {
    seed(&pool).await;
    let identity = identity();
    // A patrimony (dsp*) record that somehow reached publish: un-republishable.
    let patrimony = fr_candidate("dspm", json!({}));

    let stats = publish_filing(&pool, &spec(&identity), &[patrimony], &no_reasons())
        .await
        .unwrap();
    assert_eq!(stats.gold_inserted, 0, "patrimony never reaches Gold");
    assert_eq!(stats.suppressed, 1);
    assert_eq!(gold_count(&pool).await, 0);

    // The drop is surfaced, not silent: a filing review_task records it.
    let tasks: i64 = sqlx::query_scalar(
        "select count(*) from review_task \
         where reason = 'redaction_fr_patrimony_unrepublishable' and status = 'open'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        tasks, 1,
        "the belt-and-suspenders drop opened a review_task"
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn drift_frozen_regime_refuses_publish_then_publishes_on_recovery(pool: PgPool) {
    seed(&pool).await;
    let identity = identity();
    let candidate = fr_candidate("dia", json!({}));

    // Freeze the regime (as the sentinel would on a layout shift, §5.6).
    sqlx::query("insert into sentinel_watch (regime_code, frozen) values ($1, true)")
        .bind(REGIME_CODE)
        .execute(&pool)
        .await
        .unwrap();

    // Publish is REFUSED: fails closed, writes no Gold, opens a block task.
    let err = publish_filing(
        &pool,
        &spec(&identity),
        std::slice::from_ref(&candidate),
        &no_reasons(),
    )
    .await
    .unwrap_err();
    assert!(
        format!("{err:#}").contains("frozen"),
        "the refusal names the freeze: {err:#}"
    );
    assert_eq!(gold_count(&pool).await, 0, "a frozen regime writes no Gold");
    let block_tasks: i64 = sqlx::query_scalar(
        "select count(*) from review_task where reason = 'publish_blocked_frozen' and status = 'open'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(block_tasks, 1);

    // A second frozen attempt does NOT multiply the block task (idempotent).
    let _ = publish_filing(
        &pool,
        &spec(&identity),
        std::slice::from_ref(&candidate),
        &no_reasons(),
    )
    .await;
    let block_tasks: i64 = sqlx::query_scalar(
        "select count(*) from review_task where reason = 'publish_blocked_frozen' and status = 'open'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(block_tasks, 1, "the block task is opened at most once");

    // Recovery: unfreeze, and the retry publishes normally.
    sqlx::query("update sentinel_watch set frozen = false where regime_code = $1")
        .bind(REGIME_CODE)
        .execute(&pool)
        .await
        .unwrap();
    let stats = publish_filing(&pool, &spec(&identity), &[candidate], &no_reasons())
        .await
        .unwrap();
    assert_eq!(stats.gold_inserted, 1, "recovered regime publishes");
    assert_eq!(gold_count(&pool).await, 1);
}
