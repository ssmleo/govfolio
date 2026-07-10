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
use worker::lease::{Disposition, advance, claim_id, claim_next, release, status};

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
    assert!(held.is_some());

    // Fresh foreign lease: not claimable.
    assert!(claim_next(&pool, "lane-b", 9).await.unwrap().is_none());

    // >24h old = free per source-exploration.md's stale-lease convention.
    backdate_claim(&pool, "zz-stale", 25).await;
    let stolen = claim_next(&pool, "lane-b", 9).await.unwrap();
    assert_eq!(stolen.map(|l| l.id), Some("zz-stale".to_owned()));
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
async fn advance_keeps_lease_release_clears_it(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-adv", Some(9), "stub", Some(90.0)).await;
    claim_next(&pool, "lane-a", 9).await.unwrap().unwrap();

    // Non-holder can neither advance nor release.
    assert!(!advance(&pool, "lane-b", "zz-adv", "scouted").await.unwrap());
    assert!(
        !release(&pool, "lane-b", "zz-adv", Disposition::Keep)
            .await
            .unwrap()
    );

    // Holder advances through an intermediate phase boundary; lease survives.
    assert!(advance(&pool, "lane-a", "zz-adv", "scouted").await.unwrap());
    assert_eq!(claimed_by(&pool, "zz-adv").await.as_deref(), Some("lane-a"));
    let phase: String =
        sqlx::query_scalar("select coverage_phase from jurisdiction where id = 'zz-adv'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(phase, "scouted");

    // Release with a final advance clears the lease.
    assert!(
        release(
            &pool,
            "lane-a",
            "zz-adv",
            Disposition::Advance("live".to_owned())
        )
        .await
        .unwrap()
    );
    assert_eq!(claimed_by(&pool, "zz-adv").await, None);
    let (phase, reason): (String, Option<String>) = sqlx::query_as(
        "select coverage_phase, blocked_reason from jurisdiction where id = 'zz-adv'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(phase, "live");
    assert_eq!(reason, None);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn release_block_sets_phase_and_reason(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-blk", Some(9), "surveyed", Some(90.0)).await;
    claim_next(&pool, "lane-a", 9).await.unwrap().unwrap();

    assert!(
        release(
            &pool,
            "lane-a",
            "zz-blk",
            Disposition::Block("review_failed:survey".to_owned())
        )
        .await
        .unwrap()
    );
    let (phase, reason, holder): (String, Option<String>, Option<String>) = sqlx::query_as(
        "select coverage_phase, blocked_reason, claimed_by from jurisdiction where id = 'zz-blk'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(phase, "blocked");
    assert_eq!(reason.as_deref(), Some("review_failed:survey"));
    assert_eq!(holder, None);

    // Blocked rows never come back through claim --next.
    assert!(claim_next(&pool, "lane-b", 9).await.unwrap().is_none());
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn claim_id_targets_and_status_reports(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-id", Some(9), "sampled", Some(42.0)).await;

    let got = claim_id(&pool, "lane-a", "zz-id").await.unwrap().unwrap();
    assert_eq!(got.id, "zz-id");
    assert_eq!(got.coverage_phase, "sampled");

    // Foreign fresh lease: targeted claim also refuses.
    assert!(claim_id(&pool, "lane-b", "zz-id").await.unwrap().is_none());
    // Own lease: renewable.
    assert!(claim_id(&pool, "lane-a", "zz-id").await.unwrap().is_some());

    let live = status(&pool).await.unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].id, "zz-id");
    assert_eq!(live[0].claimed_by, "lane-a");
    assert_eq!(live[0].coverage_phase, "sampled");
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn advance_rejects_phase_outside_contract(pool: PgPool) {
    setup(&pool).await;
    insert_jur(&pool, "zz-bad", Some(9), "stub", Some(90.0)).await;
    claim_next(&pool, "lane-a", 9).await.unwrap().unwrap();

    // Client-side validation fails closed before the DB CHECK would.
    assert!(advance(&pool, "lane-a", "zz-bad", "shipped").await.is_err());
    assert!(advance(&pool, "lane-a", "zz-bad", "blocked").await.is_err());
    // `live` is release-only: an advanced-to-live row would keep its lease
    // while being invisible to every claim path — an unreclaimable ghost.
    assert!(advance(&pool, "lane-a", "zz-bad", "live").await.is_err());
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
        release(&pool, "lane-a", "zz-old-epoch", Disposition::Keep)
            .await
            .unwrap()
    );
    let fresh = claim_next(&pool, "lane-a", 9).await.unwrap().unwrap();
    assert_eq!(fresh.id, "zz-new-epoch");
}
