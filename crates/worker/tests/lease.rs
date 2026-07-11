//! Race-proof suite for the atomic jurisdiction lease (goal 097, closes
//! `docs/runbooks/parallel-factory.md` pre-check 1). The claim path must be a
//! single atomic statement — a SELECT-then-UPDATE races and two lanes grab the
//! same jurisdiction. `FOR UPDATE SKIP LOCKED` in the claim subquery is
//! load-bearing: without it, a blocked concurrent UPDATE re-evaluates only the
//! outer row predicate after the winner commits and silently overwrites the
//! winner's lease (READ COMMITTED re-check semantics). Test 1 proves that
//! semantics deterministically with two open transactions; test 2 is the
//! empirical concurrent race.
#![allow(clippy::unwrap_used)]

use sqlx::PgPool;
use worker::lease::{
    Disposition, abandon, advance, claim_id, claim_next, claimable_count, release, renew, status,
};

async fn setup(pool: &PgPool) {
    govfolio_core::db::migrate(pool).await.unwrap();
}

/// Scratch registry rows; epoch 9 keeps these disjoint from anything a seed
/// could ever assign (real epochs are E1..E5).
async fn insert_jur(
    pool: &PgPool,
    id: &str,
    epoch: Option<i16>,
    phase: &str,
    priority: Option<f32>,
) {
    sqlx::query(
        "insert into jurisdiction (id, name, level, epoch, coverage_phase, priority_score)
         values ($1, $1, 'national', $2, $3, $4)",
    )
    .bind(id)
    .bind(epoch)
    .bind(phase)
    .bind(priority)
    .execute(pool)
    .await
    .unwrap();
}

async fn claimed_by(pool: &PgPool, id: &str) -> Option<String> {
    sqlx::query_scalar("select claimed_by from jurisdiction where id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn lease_generation(pool: &PgPool, id: &str) -> i64 {
    sqlx::query_scalar("select lease_generation from jurisdiction where id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn set_pending(pool: &PgPool, id: &str, receipt_id: &str) {
    sqlx::query(
        "insert into integration_receipt (
           id, work_key, jurisdiction_id, from_phase, to_phase, blocked_reason,
           source_sha, base_sha, source_branch, lane_id, lease_generation,
           provider, model, attempt_id, validation_evidence, artifact_hashes,
           real_source_proof, journal_summary, repair_of, repair_ordinal,
           payload_sha256
         ) values (
           $1, 'worker-test:' || $2, $2, 'surveyed', null, null,
           repeat('a', 40), repeat('b', 40), 'test/worker-lease', 'lane-a', 1,
           'claude', 'fixture-model', 'attempt-1', '[{}]'::jsonb, '[]'::jsonb,
           null, 'worker lease pending fixture', null, null, repeat('c', 64)
         )",
    )
    .bind(receipt_id)
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("update jurisdiction set pending_integration_id = $2 where id = $1")
        .bind(id)
        .bind(receipt_id)
        .execute(pool)
        .await
        .unwrap();
}

async fn backdate_claim(pool: &PgPool, id: &str, hours: i32) {
    sqlx::query(
        "update jurisdiction set claimed_at = now() - make_interval(hours => $2) where id = $1",
    )
    .bind(id)
    .bind(hours)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn two_open_transactions_claim_distinct_rows_or_none(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-only", Some(9), "stub", Some(90.0)).await;

    let mut tx_a = pool.begin().await.unwrap();
    let mut tx_b = pool.begin().await.unwrap();

    let a = claim_next(&mut *tx_a, "lane-a", 9).await.unwrap();
    assert_eq!(a.as_ref().map(|l| l.id.as_str()), Some("zz-only"));
    assert_eq!(a.as_ref().map(|l| l.generation), Some(1));

    // A holds the row lock uncommitted; SKIP LOCKED must make B pass over it,
    // not queue behind it and steal the lease after A commits.
    let b = claim_next(&mut *tx_b, "lane-b", 9).await.unwrap();
    assert!(
        b.is_none(),
        "SKIP LOCKED: B must not see or wait on A's row"
    );

    tx_a.commit().await.unwrap();

    // Post-commit, the row is freshly claimed by A — still nothing for B.
    let b_retry = claim_next(&mut *tx_b, "lane-b", 9).await.unwrap();
    assert!(b_retry.is_none(), "fresh foreign lease is not claimable");
    tx_b.commit().await.unwrap();

    assert_eq!(
        claimed_by(&pool, "zz-only").await.as_deref(),
        Some("lane-a")
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn concurrent_claims_yield_exactly_one_winner(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-race", Some(9), "stub", Some(90.0)).await;

    let (a, b) = tokio::join!(
        claim_next(&pool, "lane-a", 9),
        claim_next(&pool, "lane-b", 9)
    );
    let (a, b) = (a.unwrap(), b.unwrap());

    assert!(
        a.is_some() ^ b.is_some(),
        "exactly one lane must win the single claimable row (a={a:?}, b={b:?})"
    );
    let winner = if a.is_some() { "lane-a" } else { "lane-b" };
    assert_eq!(claimed_by(&pool, "zz-race").await.as_deref(), Some(winner));
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn stale_lease_reclaimable_fresh_is_not(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-stale", Some(9), "scouted", Some(90.0)).await;

    let held = claim_next(&pool, "lane-a", 9).await.unwrap();
    assert_eq!(held.as_ref().map(|lease| lease.generation), Some(1));

    // Fresh foreign lease: not claimable.
    assert!(claim_next(&pool, "lane-b", 9).await.unwrap().is_none());

    // >24h old = free per source-exploration.md's stale-lease convention.
    backdate_claim(&pool, "zz-stale", 25).await;
    let stolen = claim_next(&pool, "lane-b", 9).await.unwrap();
    assert_eq!(stolen.as_ref().map(|l| l.id.as_str()), Some("zz-stale"));
    assert_eq!(stolen.as_ref().map(|l| l.generation), Some(2));
    assert!(
        !renew(
            &pool,
            "lane-a",
            "zz-stale",
            held.as_ref().unwrap().generation
        )
        .await
        .unwrap()
    );
    assert!(
        !abandon(
            &pool,
            "lane-a",
            "zz-stale",
            held.as_ref().unwrap().generation
        )
        .await
        .unwrap()
    );
    assert_eq!(
        claimed_by(&pool, "zz-stale").await.as_deref(),
        Some("lane-b")
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn claim_next_resumes_own_lease_never_holds_two(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-mine", Some(9), "scouted", Some(50.0)).await;
    insert_jur(&pool, "zz-other", Some(9), "stub", Some(90.0)).await;

    // First claim takes the higher-priority row.
    let first = claim_next(&pool, "lane-a", 9).await.unwrap().unwrap();
    assert_eq!(first.id, "zz-other");
    assert_eq!(first.generation, 1);
    let t0: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("select claimed_at from jurisdiction where id = 'zz-other'")
            .fetch_one(&pool)
            .await
            .unwrap();

    // A fresh session of the same lane resumes its own in-flight jurisdiction
    // (renewing claimed_at as the heartbeat) instead of claiming a second one.
    let resumed = claim_next(&pool, "lane-a", 9).await.unwrap().unwrap();
    assert_eq!(
        resumed.id, "zz-other",
        "own lease outranks any unclaimed row"
    );
    assert_eq!(resumed.generation, first.generation);
    let t1: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("select claimed_at from jurisdiction where id = 'zz-other'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(t1 >= t0, "resume must renew the heartbeat");
    assert_eq!(claimed_by(&pool, "zz-mine").await, None, "never two leases");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn claim_next_orders_by_priority_and_skips_live_blocked_other_epoch(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-live", Some(9), "live", Some(99.0)).await;
    insert_jur(&pool, "zz-blocked", Some(9), "blocked", Some(98.0)).await;
    insert_jur(&pool, "zz-e8", Some(8), "stub", Some(97.0)).await;
    insert_jur(&pool, "zz-noepoch", None, "stub", Some(96.0)).await;
    insert_jur(&pool, "zz-low", Some(9), "stub", Some(10.0)).await;
    insert_jur(&pool, "zz-high", Some(9), "stub", Some(80.0)).await;

    let first = claim_next(&pool, "lane-a", 9).await.unwrap().unwrap();
    assert_eq!(
        first.id, "zz-high",
        "highest priority_score in the epoch wins"
    );
    let second = claim_next(&pool, "lane-b", 9).await.unwrap().unwrap();
    assert_eq!(second.id, "zz-low");
    assert!(
        claim_next(&pool, "lane-c", 9).await.unwrap().is_none(),
        "live/blocked/other-epoch/no-epoch rows are never claimable"
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn renew_and_abandon_are_generation_compare_and_swap(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-cas", Some(9), "stub", Some(90.0)).await;
    let claimed = claim_next(&pool, "lane-a", 9).await.unwrap().unwrap();

    assert!(
        !renew(&pool, "lane-b", "zz-cas", claimed.generation)
            .await
            .unwrap()
    );
    assert!(
        !renew(&pool, "lane-a", "zz-cas", claimed.generation + 1)
            .await
            .unwrap()
    );
    assert!(
        renew(&pool, "lane-a", "zz-cas", claimed.generation)
            .await
            .unwrap()
    );
    assert!(
        !abandon(&pool, "lane-a", "zz-cas", claimed.generation + 1)
            .await
            .unwrap()
    );
    assert!(
        abandon(&pool, "lane-a", "zz-cas", claimed.generation)
            .await
            .unwrap()
    );
    assert_eq!(claimed_by(&pool, "zz-cas").await, None);
    assert_eq!(lease_generation(&pool, "zz-cas").await, claimed.generation);

    let reclaimed = claim_next(&pool, "lane-b", 9).await.unwrap().unwrap();
    assert_eq!(reclaimed.generation, claimed.generation + 1);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn pending_integration_excludes_claim_resume_reclaim_target_renew_and_abandon(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-pending", Some(9), "surveyed", Some(90.0)).await;
    let claimed = claim_next(&pool, "lane-a", 9).await.unwrap().unwrap();
    set_pending(&pool, "zz-pending", "01KXTESTRECEIPT0000000000").await;
    backdate_claim(&pool, "zz-pending", 25).await;

    assert_eq!(claimable_count(&pool, Some("lane-a"), 9).await.unwrap(), 0);
    assert!(claim_next(&pool, "lane-a", 9).await.unwrap().is_none());
    assert!(claim_next(&pool, "lane-b", 9).await.unwrap().is_none());
    assert!(
        claim_id(&pool, "lane-a", "zz-pending")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        claim_id(&pool, "lane-b", "zz-pending")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        !renew(&pool, "lane-a", "zz-pending", claimed.generation)
            .await
            .unwrap()
    );
    assert!(
        !abandon(&pool, "lane-a", "zz-pending", claimed.generation)
            .await
            .unwrap()
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn claim_id_targets_and_status_reports(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-id", Some(9), "sampled", Some(42.0)).await;

    let got = claim_id(&pool, "lane-a", "zz-id").await.unwrap().unwrap();
    assert_eq!(got.id, "zz-id");
    assert_eq!(got.coverage_phase, "sampled");
    assert_eq!(got.generation, 1);

    // Foreign fresh lease: targeted claim also refuses.
    assert!(claim_id(&pool, "lane-b", "zz-id").await.unwrap().is_none());
    // Own lease: renewable.
    let resumed = claim_id(&pool, "lane-a", "zz-id").await.unwrap().unwrap();
    assert_eq!(resumed.generation, got.generation);

    let live = status(&pool).await.unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].id, "zz-id");
    assert_eq!(live[0].claimed_by, "lane-a");
    assert_eq!(live[0].coverage_phase, "sampled");
    assert_eq!(live[0].generation, got.generation);
    assert_eq!(live[0].pending_integration_id, None);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn direct_phase_advance_live_and_block_paths_are_retired(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-bad", Some(9), "stub", Some(90.0)).await;
    claim_next(&pool, "lane-a", 9).await.unwrap().unwrap();

    // Client-side validation fails closed before the DB CHECK would.
    assert!(advance(&pool, "lane-a", "zz-bad", "scouted").await.is_err());
    // `live` is release-only: an advanced-to-live row would keep its lease
    // while being invisible to every claim path — an unreclaimable ghost.
    assert!(
        release(
            &pool,
            "lane-a",
            "zz-bad",
            Disposition::Advance("live".to_owned())
        )
        .await
        .is_err()
    );
    assert!(
        release(
            &pool,
            "lane-a",
            "zz-bad",
            Disposition::Block("review_failed:survey".to_owned())
        )
        .await
        .is_err()
    );
    let (phase, holder): (String, Option<String>) =
        sqlx::query_as("select coverage_phase, claimed_by from jurisdiction where id = 'zz-bad'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(phase, "stub");
    assert_eq!(holder.as_deref(), Some("lane-a"));
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn epoch_flip_resumes_held_lease_instead_of_claiming_second(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-old-epoch", Some(8), "surveyed", Some(90.0)).await;
    insert_jur(&pool, "zz-new-epoch", Some(9), "stub", Some(95.0)).await;

    let held = claim_next(&pool, "lane-a", 8).await.unwrap().unwrap();
    assert_eq!(held.id, "zz-old-epoch");

    // Epoch flips to 9 while lane-a still holds its epoch-8 row: the claim
    // must resume the held lease (any epoch), never hand out a second one.
    let resumed = claim_next(&pool, "lane-a", 9).await.unwrap().unwrap();
    assert_eq!(resumed.id, "zz-old-epoch", "own lease binds across epochs");
    assert_eq!(
        claimed_by(&pool, "zz-new-epoch").await,
        None,
        "never two leases"
    );

    // Once released, the same lane claims fresh work in the new epoch.
    assert!(
        abandon(&pool, "lane-a", "zz-old-epoch", held.generation)
            .await
            .unwrap()
    );
    let fresh = claim_next(&pool, "lane-a", 9).await.unwrap().unwrap();
    assert_eq!(fresh.id, "zz-new-epoch");
}

// --- claimable probe (goal 104) --------------------------------------------
// The zero-spend read-only pre-check run-loop.sh lanes run before spawning a
// claude session. Every test also asserts probe/claim AGREEMENT: a probe that
// drifts from `claim_next` either starves lanes (probe says none, claim would
// succeed) or burns sessions (probe says yes, claim returns none).

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn claimable_count_empty_registry_is_zero(pool: PgPool) {
    setup(&pool).await;
    assert_eq!(claimable_count(&pool, None, 9).await.unwrap(), 0);
    assert_eq!(claimable_count(&pool, Some("lane-a"), 9).await.unwrap(), 0);
    // Agreement: claim_next finds nothing either.
    assert!(claim_next(&pool, "lane-a", 9).await.unwrap().is_none());
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn claimable_count_counts_unclaimed_and_stale_only(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-un", Some(9), "stub", Some(90.0)).await;
    insert_jur(&pool, "zz-live", Some(9), "live", Some(99.0)).await;
    insert_jur(&pool, "zz-blocked", Some(9), "blocked", Some(98.0)).await;
    insert_jur(&pool, "zz-held", Some(9), "scouted", Some(80.0)).await;
    claim_id(&pool, "lane-b", "zz-held").await.unwrap().unwrap();
    insert_jur(&pool, "zz-stale", Some(9), "surveyed", Some(70.0)).await;
    claim_id(&pool, "lane-c", "zz-stale")
        .await
        .unwrap()
        .unwrap();
    backdate_claim(&pool, "zz-stale", 25).await;

    // Unclaimed + stale count; live/blocked/fresh-foreign never do.
    assert_eq!(claimable_count(&pool, None, 9).await.unwrap(), 2);

    // Agreement: exactly two successful claims, then both sides say none.
    assert!(claim_next(&pool, "lane-x", 9).await.unwrap().is_some());
    assert!(claim_next(&pool, "lane-y", 9).await.unwrap().is_some());
    assert_eq!(claimable_count(&pool, None, 9).await.unwrap(), 0);
    assert!(claim_next(&pool, "lane-z", 9).await.unwrap().is_none());
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn claimable_count_identity_leg_mirrors_resume_own(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-mine", Some(9), "scouted", Some(90.0)).await;
    claim_next(&pool, "lane-a", 9).await.unwrap().unwrap();

    // Fresh foreign lease: invisible to the identity-less probe and to
    // other lanes' probes — exactly like their claim would find nothing.
    assert_eq!(claimable_count(&pool, None, 9).await.unwrap(), 0);
    assert_eq!(claimable_count(&pool, Some("lane-b"), 9).await.unwrap(), 0);
    assert!(claim_next(&pool, "lane-b", 9).await.unwrap().is_none());

    // The holder's probe sees its own resumable in-flight row — and, like
    // claim_next's resume-own path, regardless of the probed epoch (a
    // mid-walk lane must never idle-sleep on its own unfinished work).
    assert_eq!(claimable_count(&pool, Some("lane-a"), 9).await.unwrap(), 1);
    assert_eq!(claimable_count(&pool, Some("lane-a"), 7).await.unwrap(), 1);
    let resumed = claim_next(&pool, "lane-a", 9).await.unwrap();
    assert_eq!(resumed.map(|l| l.id), Some("zz-mine".to_owned()));

    // Simulate the integrator's terminal apply projection: it alone advances
    // phase and releases the lease after the receipt is green on main.
    sqlx::query(
        "update jurisdiction
         set coverage_phase = 'live', claimed_by = null, claimed_at = null
         where id = 'zz-mine'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(claimable_count(&pool, Some("lane-a"), 9).await.unwrap(), 0);
    assert!(claim_next(&pool, "lane-a", 9).await.unwrap().is_none());
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn claimable_count_filters_by_epoch(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-e8", Some(8), "stub", Some(90.0)).await;
    insert_jur(&pool, "zz-noepoch", None, "stub", Some(95.0)).await;

    // Wrong epoch (and NULL epoch): not claimable — probe and claim agree.
    assert_eq!(claimable_count(&pool, None, 9).await.unwrap(), 0);
    assert!(claim_next(&pool, "lane-a", 9).await.unwrap().is_none());

    // Right epoch: claimable — probe and claim agree.
    assert_eq!(claimable_count(&pool, None, 8).await.unwrap(), 1);
    let got = claim_next(&pool, "lane-a", 8).await.unwrap();
    assert_eq!(got.map(|l| l.id), Some("zz-e8".to_owned()));
}
