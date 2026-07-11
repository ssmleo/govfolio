//! Release-1 receipt substrate DB contract. These tests are ignored locally unless
//! the Postgres integration suite is explicitly enabled.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use chrono::{TimeZone as _, Utc};
use govfolio_core::integration::{
    ApplyEvidence, ArtifactHash, CoveragePhase, IntegrationError, IntegrationReceipt,
    IntegrationState, ProducerProvider, RealSourceProof, TransitionEvidence, TransitionRequest,
    ValidationEvidence, apply_receipt, next_actionable_receipt, receipt_status, submit_receipt,
    transition_receipt,
};
use sqlx::PgPool;

fn sha(character: char) -> String {
    std::iter::repeat_n(character, 40).collect()
}

fn receipt(id: &str, from: CoveragePhase, to: Option<CoveragePhase>) -> IntegrationReceipt {
    IntegrationReceipt {
        id: id.to_owned(),
        work_key: format!("zz-work-{from:?}-{to:?}"),
        jurisdiction_id: "zz-receipt".to_owned(),
        from_phase: from,
        to_phase: to,
        blocked_reason: None,
        source_sha: sha('a'),
        base_sha: sha('b'),
        branch: "goal/fixture".to_owned(),
        lane_id: "lane-0".to_owned(),
        lease_generation: 7,
        provider: ProducerProvider::Claude,
        model: "fixture-model".to_owned(),
        attempt_id: "attempt-1".to_owned(),
        validation_evidence: vec![ValidationEvidence {
            name: "rust".to_owned(),
            command: "cargo test --workspace".to_owned(),
            exit_code: 0,
            output_sha256: std::iter::repeat_n('c', 64).collect(),
        }],
        artifact_hashes: vec![ArtifactHash {
            path: "artifact.json".to_owned(),
            sha256: std::iter::repeat_n('d', 64).collect(),
        }],
        real_source_proof: None,
        journal_summary: "fixture applied".to_owned(),
        repair_of: None,
        repair_ordinal: None,
    }
}

async fn setup(pool: &PgPool, phase: CoveragePhase) {
    govfolio_core::db::migrate(pool).await.unwrap();
    sqlx::query(
        "insert into jurisdiction \
         (id, name, level, coverage_phase, claimed_by, claimed_at, lease_generation) \
         values ('zz-receipt', 'Receipt fixture', 'national', $1, 'lane-0', now(), 7)",
    )
    .bind(phase.as_str())
    .execute(pool)
    .await
    .unwrap();
}

async fn merge_ready(pool: &PgPool, receipt_id: &str) -> i64 {
    let preparing = transition_receipt(
        pool,
        &TransitionRequest {
            receipt_id: receipt_id.to_owned(),
            expected_state: IntegrationState::Submitted,
            expected_version: 0,
            to_state: IntegrationState::Preparing,
            actor: "integrator".to_owned(),
            evidence: TransitionEvidence {
                candidate_base_sha: Some(sha('b')),
                ..TransitionEvidence::default()
            },
        },
    )
    .await
    .unwrap();
    let awaiting = transition_receipt(
        pool,
        &TransitionRequest {
            receipt_id: receipt_id.to_owned(),
            expected_state: preparing.state,
            expected_version: preparing.version,
            to_state: IntegrationState::AwaitingCi,
            actor: "integrator".to_owned(),
            evidence: TransitionEvidence {
                integration_branch: Some("integration/fixture".to_owned()),
                pr_number: Some(42),
                ..TransitionEvidence::default()
            },
        },
    )
    .await
    .unwrap();
    transition_receipt(
        pool,
        &TransitionRequest {
            receipt_id: receipt_id.to_owned(),
            expected_state: awaiting.state,
            expected_version: awaiting.version,
            to_state: IntegrationState::MergedUnapplied,
            actor: "integrator".to_owned(),
            evidence: TransitionEvidence {
                merge_sha: Some(sha('e')),
                ..TransitionEvidence::default()
            },
        },
    )
    .await
    .unwrap()
    .version
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn integration_receipt_submit_is_atomic_and_idempotent(pool: PgPool) {
    setup(&pool, CoveragePhase::Stub).await;
    let candidate = receipt(
        "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        CoveragePhase::Stub,
        Some(CoveragePhase::Scouted),
    );

    let first = submit_receipt(&pool, &candidate).await.unwrap();
    let second = submit_receipt(&pool, &candidate).await.unwrap();

    assert!(first.inserted);
    assert!(!second.inserted);
    assert_eq!(first.receipt_id, second.receipt_id);
    let counts: (i64, i64) = sqlx::query_as(
        "select (select count(*) from integration_receipt), \
                (select count(*) from integration_event)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (1, 1));
    let pending: Option<String> = sqlx::query_scalar(
        "select pending_integration_id from jurisdiction where id = 'zz-receipt'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pending.as_deref(), Some(candidate.id.as_str()));
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn integration_receipt_stale_generation_rejects_without_partial_receipt(pool: PgPool) {
    setup(&pool, CoveragePhase::Stub).await;
    let mut candidate = receipt(
        "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        CoveragePhase::Stub,
        Some(CoveragePhase::Scouted),
    );
    candidate.lease_generation = 6;

    assert!(matches!(
        submit_receipt(&pool, &candidate).await,
        Err(IntegrationError::LeaseMismatch { .. })
    ));
    let count: i64 = sqlx::query_scalar("select count(*) from integration_receipt")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn integration_receipt_immutable_rows_reject_update_and_delete(pool: PgPool) {
    setup(&pool, CoveragePhase::Stub).await;
    let candidate = receipt(
        "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        CoveragePhase::Stub,
        Some(CoveragePhase::Scouted),
    );
    submit_receipt(&pool, &candidate).await.unwrap();

    assert!(
        sqlx::query("update integration_receipt set journal_summary = 'changed'")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("delete from integration_event")
            .execute(&pool)
            .await
            .is_err()
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn integration_receipt_cas_transition_rejects_a_stale_version(pool: PgPool) {
    setup(&pool, CoveragePhase::Stub).await;
    let candidate = receipt(
        "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        CoveragePhase::Stub,
        Some(CoveragePhase::Scouted),
    );
    submit_receipt(&pool, &candidate).await.unwrap();
    let request = TransitionRequest {
        receipt_id: candidate.id.clone(),
        expected_state: IntegrationState::Submitted,
        expected_version: 0,
        to_state: IntegrationState::Preparing,
        actor: "integrator".to_owned(),
        evidence: TransitionEvidence {
            candidate_base_sha: Some(candidate.base_sha.clone()),
            ..TransitionEvidence::default()
        },
    };
    transition_receipt(&pool, &request).await.unwrap();
    let stale = TransitionRequest {
        expected_state: IntegrationState::Preparing,
        expected_version: 0,
        to_state: IntegrationState::Preparing,
        evidence: TransitionEvidence {
            candidate_base_sha: Some(sha('f')),
            ..TransitionEvidence::default()
        },
        ..request
    };
    assert!(matches!(
        transition_receipt(&pool, &stale).await,
        Err(IntegrationError::CasConflict { .. })
    ));
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn integration_receipt_bounded_repair_defers_and_replaces_pending(pool: PgPool) {
    setup(&pool, CoveragePhase::Stub).await;
    let original = receipt(
        "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        CoveragePhase::Stub,
        Some(CoveragePhase::Scouted),
    );
    submit_receipt(&pool, &original).await.unwrap();
    let preparing = transition_receipt(
        &pool,
        &TransitionRequest {
            receipt_id: original.id.clone(),
            expected_state: IntegrationState::Submitted,
            expected_version: 0,
            to_state: IntegrationState::Preparing,
            actor: "integrator".to_owned(),
            evidence: TransitionEvidence {
                candidate_base_sha: Some(original.base_sha.clone()),
                ..TransitionEvidence::default()
            },
        },
    )
    .await
    .unwrap();
    transition_receipt(
        &pool,
        &TransitionRequest {
            receipt_id: original.id.clone(),
            expected_state: preparing.state,
            expected_version: preparing.version,
            to_state: IntegrationState::ReworkRequired,
            actor: "integrator".to_owned(),
            evidence: TransitionEvidence {
                failure: Some("merge conflict".to_owned()),
                ..TransitionEvidence::default()
            },
        },
    )
    .await
    .unwrap();
    assert!(next_actionable_receipt(&pool).await.unwrap().is_none());

    let mut repair = original.clone();
    repair.id = "01BX5ZZKBKACTAV9WEVGEMMVA1".to_owned();
    repair.source_sha = sha('f');
    repair.branch = "goal/fixture-repair".to_owned();
    repair.attempt_id = "attempt-2".to_owned();
    repair.repair_of = Some(original.id.clone());
    repair.repair_ordinal = Some(1);
    submit_receipt(&pool, &repair).await.unwrap();

    assert_eq!(
        receipt_status(&pool, &original.id).await.unwrap().state,
        IntegrationState::Deferred
    );
    let (actionable, projection) = next_actionable_receipt(&pool)
        .await
        .unwrap()
        .expect("repair should become actionable");
    assert_eq!(actionable.id, repair.id);
    assert_eq!(projection.state, IntegrationState::Submitted);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn integration_receipt_apply_is_atomic_idempotent_and_renews_lease(pool: PgPool) {
    setup(&pool, CoveragePhase::Stub).await;
    let candidate = receipt(
        "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        CoveragePhase::Stub,
        Some(CoveragePhase::Scouted),
    );
    submit_receipt(&pool, &candidate).await.unwrap();
    let version = merge_ready(&pool, &candidate.id).await;
    let evidence = ApplyEvidence::successful(&candidate.source_sha, &sha('e'));

    let first = apply_receipt(&pool, &candidate.id, version, &evidence)
        .await
        .unwrap();
    let second = apply_receipt(&pool, &candidate.id, version, &evidence)
        .await
        .unwrap();

    assert!(!first.already_applied);
    assert!(second.already_applied);
    let row: (String, Option<String>, Option<String>) = sqlx::query_as(
        "select coverage_phase, claimed_by, pending_integration_id \
         from jurisdiction where id = 'zz-receipt'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, ("scouted".to_owned(), Some("lane-0".to_owned()), None));
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn integration_receipt_built_live_requires_real_source_and_releases_lease(pool: PgPool) {
    setup(&pool, CoveragePhase::Built).await;
    let mut candidate = receipt(
        "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        CoveragePhase::Built,
        Some(CoveragePhase::Live),
    );
    candidate.real_source_proof = Some(RealSourceProof {
        fetched_at: Utc
            .with_ymd_and_hms(2026, 7, 11, 12, 0, 0)
            .single()
            .unwrap(),
        source_url: "https://example.test/source".to_owned(),
        bronze_sha256: std::iter::repeat_n('f', 64).collect(),
        ingestion_run_id: "01BX5ZZKBKACTAV9WEVGEMMVA1".to_owned(),
        rows_ingested: 1,
    });
    submit_receipt(&pool, &candidate).await.unwrap();
    let version = merge_ready(&pool, &candidate.id).await;
    let mut evidence = ApplyEvidence::successful(&candidate.source_sha, &sha('e'));
    evidence.real_source_verified = true;

    apply_receipt(&pool, &candidate.id, version, &evidence)
        .await
        .unwrap();

    let row: (String, Option<String>) = sqlx::query_as(
        "select coverage_phase, claimed_by from jurisdiction where id = 'zz-receipt'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, ("live".to_owned(), None));
}
