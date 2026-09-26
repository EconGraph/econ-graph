// REQUIREMENT: Comprehensive database integration tests for crawl queue
// PURPOSE: Test queue operations with real PostgreSQL database including SKIP LOCKED
// This ensures the crawler queue system works correctly with concurrent processing
//
// These tests need a reachable Postgres in DATABASE_URL. They run the embedded migrations once
// (idempotent), never drop the schema, and give every test its own unique `source` value so
// leftover rows from earlier runs can't interfere. They are #[serial] because
// `release_stuck` is table-wide.

use crate::database::DatabasePool;
use crate::models::{
    crawl_queue::{
        CrawlQueueItem, JobKind, LeaseOutcome, NewCrawlQueueItem, QueuePriority, QueueStatus,
        QueueTransition,
    },
    data_source::{DataSource, NewDataSource},
};
use crate::schema::{crawl_queue, data_sources};
use chrono::Utc;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serial_test::serial;
use std::collections::HashSet;
use std::time::Duration;
use uuid::Uuid;

static MIGRATED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

/// Pool on a throwaway test database with migrations applied (once per test binary).
///
/// Some tests call table-wide operations (`release_stuck`, `purge_finished`) that touch every
/// row, so this uses `TEST_DATABASE_URL`, or `DATABASE_URL` only when its database name contains
/// "test" (as CI's `econ_graph_test` does), and refuses anything else.
async fn test_pool() -> DatabasePool {
    let url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        let url = std::env::var("DATABASE_URL").expect(
            "set TEST_DATABASE_URL (or DATABASE_URL naming a *test* database) for crawl_queue tests",
        );
        let db_name = url.rsplit('/').next().unwrap_or("").split('?').next().unwrap_or("");
        assert!(
            db_name.contains("test"),
            "crawl_queue tests run table-wide queue operations; refusing to use database \
             {db_name:?} from DATABASE_URL. Point TEST_DATABASE_URL at a throwaway database."
        );
        url
    });
    MIGRATED
        .get_or_init(|| async {
            crate::database::run_migrations(&url)
                .await
                .expect("migrations failed");
        })
        .await;
    crate::database::create_pool(&url)
        .await
        .expect("Failed to connect to test database")
}

/// A source name unique to this test run (fits VARCHAR(50)).
fn unique_source(prefix: &str) -> String {
    format!("{}_{}", prefix, &Uuid::new_v4().simple().to_string()[..12])
}

fn new_item(source: &str, series_id: &str) -> NewCrawlQueueItem {
    NewCrawlQueueItem {
        source: source.to_string(),
        series_id: series_id.to_string(),
        ..Default::default()
    }
}

async fn reload(pool: &DatabasePool, id: Uuid) -> CrawlQueueItem {
    let mut conn = pool.get().await.unwrap();
    crawl_queue::table
        .find(id)
        .first::<CrawlQueueItem>(&mut conn)
        .await
        .unwrap()
}

// Simple unit tests that don't require complex database integration
#[cfg(test)]
mod simple_tests {
    use super::*;

    #[test]
    fn test_queue_priority_conversion() {
        // REQUIREMENT: Test queue priority enumeration
        // PURPOSE: Verify that queue priorities convert correctly to integers
        // This tests the basic enum functionality for job prioritization

        let high: i32 = QueuePriority::High.into();
        let normal: i32 = QueuePriority::Normal.into();
        let low: i32 = QueuePriority::Low.into();

        assert!(high > normal);
        assert!(normal > low);
    }

    #[test]
    fn test_queue_status_creation() {
        // REQUIREMENT: Test queue status enumeration
        // PURPOSE: Verify that queue status values work correctly
        // This tests the basic enum functionality for job tracking

        let pending = QueueStatus::Pending;
        let processing = QueueStatus::Processing;
        let completed = QueueStatus::Completed;
        let failed = QueueStatus::Failed;

        // Just verify they can be created and compared
        assert_ne!(format!("{:?}", pending), format!("{:?}", processing));
        assert_ne!(format!("{:?}", completed), format!("{:?}", failed));
    }
}

// Basic database integration test
#[tokio::test]
#[serial]
async fn test_basic_queue_operations() {
    // REQUIREMENT: Test basic crawl queue operations with database persistence
    // PURPOSE: Verify that crawler tasks can be queued and retrieved
    // This tests the core functionality of the background job queue system

    let pool = test_pool().await;
    let mut conn = pool.get().await.expect("Failed to get connection");

    // Create test data source (unique name: data_sources.name is UNIQUE and the DB is not wiped)
    let new_source = NewDataSource {
        name: format!("Queue Test Source {}", Uuid::new_v4()),
        description: Some("Source for testing queue operations".to_string()),
        base_url: "https://queue.example.com/api".to_string(),
        api_key_required: false,
        rate_limit_per_minute: 100,
        is_visible: true,
        is_enabled: true,
        requires_admin_approval: false,
        crawl_frequency_hours: 24,
        api_documentation_url: Some("https://queue.example.com/docs".to_string()),
        api_key_name: None,
    };

    let source: DataSource = diesel::insert_into(data_sources::table)
        .values(&new_source)
        .get_result(&mut conn)
        .await
        .expect("Failed to create data source");

    // Create a simple queue item with correct field structure
    let queue_item = NewCrawlQueueItem {
        source: source.id.to_string(),
        series_id: "TEST_SERIES_001".to_string(),
        priority: 1,
        max_retries: 3,
        scheduled_for: None,
        kind: JobKind::FetchSeries.to_string(),
    };

    // Insert the queue item
    let created_item: CrawlQueueItem = diesel::insert_into(crawl_queue::table)
        .values(&queue_item)
        .get_result(&mut conn)
        .await
        .expect("Failed to create queue item");

    // Verify the item was created correctly
    assert_eq!(created_item.source, source.id.to_string());
    assert_eq!(created_item.series_id, "TEST_SERIES_001");
    assert_eq!(created_item.max_retries, 3);
    assert_eq!(created_item.status, "pending"); // Status is stored as String
    assert_eq!(created_item.kind, "fetch_series");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial]
async fn test_two_workers_never_claim_same_item() {
    // REQUIREMENT: claim_next must be atomic across concurrent workers (bug 1: SELECT FOR UPDATE
    // SKIP LOCKED followed by a separate UPDATE outside a transaction allowed double claims).
    const N: usize = 60;
    let pool = test_pool().await;
    let source = unique_source("CONC");
    for i in 0..N {
        CrawlQueueItem::enqueue(&pool, &new_item(&source, &format!("S{i:03}")))
            .await
            .unwrap()
            .expect("fresh item must enqueue");
    }

    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let mut handles = Vec::new();
    for worker in ["worker-a", "worker-b"] {
        let pool = pool.clone();
        let source = source.clone();
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move {
            let sources = vec![source];
            barrier.wait().await;
            let mut claimed = Vec::new();
            while let Some(item) = CrawlQueueItem::claim_next(&pool, worker, Some(&sources))
                .await
                .unwrap()
            {
                assert_eq!(item.status, "processing");
                assert_eq!(item.locked_by.as_deref(), Some(worker));
                assert!(item.locked_at.is_some());
                claimed.push(item.id);
            }
            claimed
        }));
    }

    let a = handles.remove(0).await.unwrap();
    let b = handles.remove(0).await.unwrap();

    let set_a: HashSet<Uuid> = a.iter().copied().collect();
    let set_b: HashSet<Uuid> = b.iter().copied().collect();
    assert_eq!(set_a.len(), a.len(), "worker a claimed an item twice");
    assert_eq!(set_b.len(), b.len(), "worker b claimed an item twice");
    assert!(
        set_a.is_disjoint(&set_b),
        "both workers claimed the same item"
    );
    assert_eq!(a.len() + b.len(), N, "workers must claim every item");
    println!("worker-a claimed {}, worker-b claimed {}", a.len(), b.len());

    // DB agrees: every item is processing and locked by the worker that returned it.
    for id in &a {
        assert_eq!(
            reload(&pool, *id).await.locked_by.as_deref(),
            Some("worker-a")
        );
    }
    for id in &b {
        assert_eq!(
            reload(&pool, *id).await.locked_by.as_deref(),
            Some("worker-b")
        );
    }
}

#[tokio::test]
#[serial]
async fn test_full_lifecycle_retry_complete_reenqueue() {
    // REQUIREMENT: enqueue -> claim -> retry_later -> (not claimable until due) -> claim ->
    // complete -> the same (source, series_id, kind) can be enqueued again
    // (bugs 2, 3, 4: retried items were never re-picked, and a series could be enqueued only once).
    let pool = test_pool().await;
    let source = unique_source("LIFE");
    let sources = vec![source.clone()];

    let item = CrawlQueueItem::enqueue(&pool, &new_item(&source, "GDP"))
        .await
        .unwrap()
        .expect("enqueue");
    assert_eq!(item.status, "pending");

    let claimed = CrawlQueueItem::claim_next(&pool, "w1", Some(&sources))
        .await
        .unwrap()
        .expect("claim");
    assert_eq!(claimed.id, item.id);

    let before = Utc::now();
    assert!(claimed.claim_token.is_some());
    let transition =
        CrawlQueueItem::retry_later(&pool, &claimed, "timeout", Duration::from_secs(3600), true)
            .await
            .unwrap();
    let QueueTransition::Rescheduled { at } = transition else {
        panic!("expected Rescheduled, got {transition:?}");
    };
    assert!(at > before + chrono::Duration::minutes(59));

    let row = reload(&pool, item.id).await;
    assert_eq!(row.status, "retrying");
    assert_eq!(row.retry_count, 1);
    assert_eq!(row.error_message.as_deref(), Some("timeout"));
    assert!(row.locked_by.is_none() && row.locked_at.is_none());
    assert!(row.claim_token.is_none());

    // Not due yet -> not claimable.
    assert!(CrawlQueueItem::claim_next(&pool, "w2", Some(&sources))
        .await
        .unwrap()
        .is_none());

    // Make it due.
    {
        let mut conn = pool.get().await.unwrap();
        diesel::update(crawl_queue::table.find(item.id))
            .set(crawl_queue::scheduled_for.eq(Some(Utc::now() - chrono::Duration::seconds(1))))
            .execute(&mut conn)
            .await
            .unwrap();
    }

    let reclaimed = CrawlQueueItem::claim_next(&pool, "w2", Some(&sources))
        .await
        .unwrap()
        .expect("retrying item must be claimable once due");
    assert_eq!(reclaimed.id, item.id);
    assert_eq!(reclaimed.locked_by.as_deref(), Some("w2"));
    assert_eq!(reclaimed.retry_count, 1);

    assert_eq!(
        CrawlQueueItem::complete(&pool, &reclaimed).await.unwrap(),
        LeaseOutcome::Applied
    );
    let row = reload(&pool, item.id).await;
    assert_eq!(row.status, "completed");
    assert!(row.locked_by.is_none() && row.locked_at.is_none());

    // Finished rows don't block re-enqueueing the same work.
    let again = CrawlQueueItem::enqueue(&pool, &new_item(&source, "GDP"))
        .await
        .unwrap()
        .expect("re-enqueue after completion must succeed");
    assert_ne!(again.id, item.id);
    assert_eq!(again.status, "pending");
}

#[tokio::test]
#[serial]
async fn test_enqueue_duplicate_while_active_returns_none() {
    let pool = test_pool().await;
    let source = unique_source("DUP");
    let sources = vec![source.clone()];

    let first = CrawlQueueItem::enqueue(&pool, &new_item(&source, "CPI"))
        .await
        .unwrap()
        .expect("first enqueue");

    // pending duplicate
    assert!(CrawlQueueItem::enqueue(&pool, &new_item(&source, "CPI"))
        .await
        .unwrap()
        .is_none());

    // processing duplicate
    let claimed = CrawlQueueItem::claim_next(&pool, "w", Some(&sources))
        .await
        .unwrap()
        .expect("claim");
    assert!(CrawlQueueItem::enqueue(&pool, &new_item(&source, "CPI"))
        .await
        .unwrap()
        .is_none());

    // retrying duplicate
    let t = CrawlQueueItem::retry_later(&pool, &claimed, "x", Duration::from_secs(60), true)
        .await
        .unwrap();
    assert!(matches!(t, QueueTransition::Rescheduled { .. }));
    assert!(CrawlQueueItem::enqueue(&pool, &new_item(&source, "CPI"))
        .await
        .unwrap()
        .is_none());

    // A different kind for the same series is independent work.
    let discover = NewCrawlQueueItem {
        kind: JobKind::DiscoverCatalog.to_string(),
        ..new_item(&source, "CPI")
    };
    let other = CrawlQueueItem::enqueue(&pool, &discover)
        .await
        .unwrap()
        .expect("different kind must enqueue");
    assert_eq!(other.job_kind(), JobKind::DiscoverCatalog);

    // After a permanent failure the series can be enqueued again. (The item is `retrying`, so
    // no worker holds a lease on it: only the admin override can fail it.)
    assert_eq!(
        CrawlQueueItem::fail(&pool, &claimed, "gone").await.unwrap(),
        LeaseOutcome::LostLease
    );
    CrawlQueueItem::force_fail(&pool, first.id, "gone")
        .await
        .unwrap();
    assert!(CrawlQueueItem::enqueue(&pool, &new_item(&source, "CPI"))
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
#[serial]
async fn test_retry_later_attempt_counting_and_failure() {
    let pool = test_pool().await;
    let source = unique_source("RETRY");
    let sources = vec![source.clone()];

    let item = CrawlQueueItem::enqueue(
        &pool,
        &NewCrawlQueueItem {
            max_retries: 2,
            ..new_item(&source, "UNRATE")
        },
    )
    .await
    .unwrap()
    .unwrap();
    let (pool_ref, sources_ref) = (&pool, &sources);
    let claim = move || async move {
        CrawlQueueItem::claim_next(pool_ref, "w", Some(sources_ref))
            .await
            .unwrap()
            .expect("zero-delay retry is immediately claimable")
    };
    let mut claimed = claim().await;
    assert_eq!(claimed.id, item.id);

    // Rate limiting doesn't consume the attempt budget.
    for _ in 0..3 {
        let t = CrawlQueueItem::retry_later(&pool, &claimed, "429", Duration::ZERO, false)
            .await
            .unwrap();
        assert!(matches!(t, QueueTransition::Rescheduled { .. }));
        let row = reload(&pool, item.id).await;
        assert_eq!(row.status, "retrying");
        // No longer processing -> a second transition from the same worker is a lost lease.
        assert_eq!(
            CrawlQueueItem::retry_later(&pool, &claimed, "dup", Duration::ZERO, false)
                .await
                .unwrap(),
            QueueTransition::LostLease
        );
        claimed = claim().await;
        assert_eq!(claimed.id, item.id);
    }
    let row = reload(&pool, item.id).await;
    assert_eq!(row.retry_count, 0);
    assert_eq!(row.error_message.as_deref(), Some("429"));

    // First counted attempt: 1 < 2 -> rescheduled.
    let t = CrawlQueueItem::retry_later(&pool, &claimed, "503", Duration::ZERO, true)
        .await
        .unwrap();
    assert!(matches!(t, QueueTransition::Rescheduled { .. }));
    assert_eq!(reload(&pool, item.id).await.retry_count, 1);

    // Claim again (zero delay -> due now), then the second counted attempt reaches max_retries.
    claimed = claim().await;
    assert_eq!(claimed.id, item.id);
    let t = CrawlQueueItem::retry_later(&pool, &claimed, "503 again", Duration::ZERO, true)
        .await
        .unwrap();
    assert_eq!(t, QueueTransition::Failed);

    let row = reload(&pool, item.id).await;
    assert_eq!(row.status, "failed");
    assert_eq!(row.retry_count, 2);
    assert_eq!(row.error_message.as_deref(), Some("503 again"));
    assert!(row.locked_by.is_none() && row.locked_at.is_none());

    // Failed items are not claimable and cannot be rescheduled.
    assert!(CrawlQueueItem::claim_next(&pool, "w", Some(&sources))
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        CrawlQueueItem::retry_later(&pool, &claimed, "x", Duration::ZERO, true)
            .await
            .unwrap(),
        QueueTransition::LostLease
    );
    assert!(
        CrawlQueueItem::force_retry_later(&pool, item.id, "x", Duration::ZERO, true)
            .await
            .is_err()
    );
}

#[tokio::test]
#[serial]
async fn test_force_retry_later_ignores_lease_owner() {
    let pool = test_pool().await;
    let source = unique_source("FORCE");
    let item = CrawlQueueItem::enqueue(&pool, &new_item(&source, "A"))
        .await
        .unwrap()
        .unwrap();
    // Pending (unclaimed) items can be rescheduled by the admin path.
    let t = CrawlQueueItem::force_retry_later(&pool, item.id, "admin", Duration::ZERO, true)
        .await
        .unwrap();
    assert!(matches!(t, QueueTransition::Rescheduled { .. }));
    let row = reload(&pool, item.id).await;
    assert_eq!((row.status.as_str(), row.retry_count), ("retrying", 1));
}

#[tokio::test]
#[serial]
async fn test_release_stuck_clears_locks() {
    let pool = test_pool().await;
    let source = unique_source("STUCK");
    let sources = vec![source.clone()];

    let item = CrawlQueueItem::enqueue(&pool, &new_item(&source, "M2"))
        .await
        .unwrap()
        .unwrap();
    CrawlQueueItem::claim_next(&pool, "crashed-worker", Some(&sources))
        .await
        .unwrap()
        .unwrap();

    // A generous threshold leaves a fresh lock alone.
    CrawlQueueItem::release_stuck(&pool, Duration::from_secs(3600))
        .await
        .unwrap();
    assert_eq!(reload(&pool, item.id).await.status, "processing");

    tokio::time::sleep(Duration::from_millis(20)).await;
    let released = CrawlQueueItem::release_stuck(&pool, Duration::from_millis(10))
        .await
        .unwrap();
    assert!(released >= 1);

    let row = reload(&pool, item.id).await;
    assert_eq!(row.status, "pending");
    assert!(row.locked_by.is_none());
    assert!(row.locked_at.is_none());
    assert_eq!(row.retry_count, 1, "a release counts as an attempt");
    assert!(row.finished_at.is_none());
    let msg = row.error_message.unwrap_or_default();
    assert!(
        msg.contains("lease expired") && msg.contains("crashed-worker"),
        "{msg}"
    );

    let reclaimed = CrawlQueueItem::claim_next(&pool, "healthy-worker", Some(&sources))
        .await
        .unwrap()
        .expect("released item must be claimable");
    assert_eq!(reclaimed.id, item.id);
    assert_eq!(reclaimed.locked_by.as_deref(), Some("healthy-worker"));
}

#[tokio::test]
#[serial]
async fn test_claim_priority_order_and_source_filter() {
    let pool = test_pool().await;
    let src_a = unique_source("PRIA");
    let src_b = unique_source("PRIB");

    let low = CrawlQueueItem::enqueue(
        &pool,
        &NewCrawlQueueItem {
            priority: 2,
            ..new_item(&src_a, "LOW")
        },
    )
    .await
    .unwrap()
    .unwrap();
    let high = CrawlQueueItem::enqueue(
        &pool,
        &NewCrawlQueueItem {
            priority: 9,
            ..new_item(&src_a, "HIGH")
        },
    )
    .await
    .unwrap()
    .unwrap();
    let other = CrawlQueueItem::enqueue(&pool, &new_item(&src_b, "OTHER"))
        .await
        .unwrap()
        .unwrap();
    let future = CrawlQueueItem::enqueue(
        &pool,
        &NewCrawlQueueItem {
            priority: 10,
            scheduled_for: Some(Utc::now() + chrono::Duration::hours(1)),
            ..new_item(&src_a, "FUTURE")
        },
    )
    .await
    .unwrap()
    .unwrap();

    let only_a = vec![src_a.clone()];
    let first = CrawlQueueItem::claim_next(&pool, "w", Some(&only_a))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.id, high.id, "highest due priority first");
    let second = CrawlQueueItem::claim_next(&pool, "w", Some(&only_a))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second.id, low.id);
    assert!(CrawlQueueItem::claim_next(&pool, "w", Some(&only_a))
        .await
        .unwrap()
        .is_none());
    assert_eq!(reload(&pool, future.id).await.status, "pending");
    assert_eq!(reload(&pool, other.id).await.status, "pending");

    let only_b = vec![src_b.clone()];
    let b = CrawlQueueItem::claim_next(&pool, "w", Some(&only_b))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(b.id, other.id);
}

#[tokio::test]
#[serial]
async fn test_release_and_changeset_clear_lock() {
    // Bug 2: UpdateCrawlQueueItem skipped `None`, so locks were never cleared.
    let pool = test_pool().await;
    let source = unique_source("REL");
    let item = CrawlQueueItem::enqueue(&pool, &new_item(&source, "X"))
        .await
        .unwrap()
        .unwrap();
    let claimed = CrawlQueueItem::claim_by_id(&pool, item.id, "w")
        .await
        .unwrap()
        .expect("claim_by_id");
    assert!(claimed.is_locked());

    assert!(CrawlQueueItem::release(&pool, item.id).await.unwrap());
    let row = reload(&pool, item.id).await;
    assert_eq!(row.status, "pending");
    assert!(row.locked_by.is_none() && row.locked_at.is_none());

    // Finished items are not resurrected.
    CrawlQueueItem::force_complete(&pool, item.id)
        .await
        .unwrap();
    assert!(!CrawlQueueItem::release(&pool, item.id).await.unwrap());
    assert_eq!(reload(&pool, item.id).await.status, "completed");

    // Missing ids: the admin overrides error rather than silently succeeding; the guarded
    // worker transitions report that no lease is held.
    assert!(CrawlQueueItem::force_complete(&pool, Uuid::new_v4())
        .await
        .is_err());
    assert!(CrawlQueueItem::force_fail(&pool, Uuid::new_v4(), "x")
        .await
        .is_err());
    assert_eq!(
        CrawlQueueItem::complete(
            &pool,
            &CrawlQueueItem {
                id: Uuid::new_v4(),
                ..claimed.clone()
            }
        )
        .await
        .unwrap(),
        LeaseOutcome::LostLease
    );
}

#[tokio::test]
#[serial]
async fn test_lost_lease_after_release_stuck() {
    // REQUIREMENT: a job that outlives its lease must not finish an item another worker has
    // since claimed (the first worker's `complete` used to mark it done while B still ran it).
    let pool = test_pool().await;
    let source = unique_source("LEASE");
    let sources = vec![source.clone()];
    let item = CrawlQueueItem::enqueue(&pool, &new_item(&source, "GDP"))
        .await
        .unwrap()
        .unwrap();

    let a = CrawlQueueItem::claim_next(&pool, "worker-a", Some(&sources))
        .await
        .unwrap()
        .expect("A claims");
    assert_eq!(a.id, item.id);

    // A's job outlives stuck_after (0 here): the lease is taken away and B claims the item.
    assert!(
        CrawlQueueItem::release_stuck(&pool, Duration::ZERO)
            .await
            .unwrap()
            >= 1
    );
    let b = CrawlQueueItem::claim_next(&pool, "worker-b", Some(&sources))
        .await
        .unwrap()
        .expect("B claims the released item");
    assert_eq!(b.id, item.id);

    // Every transition A attempts is refused and leaves B's claim untouched.
    assert_eq!(
        CrawlQueueItem::complete(&pool, &a).await.unwrap(),
        LeaseOutcome::LostLease
    );
    assert_eq!(
        CrawlQueueItem::fail(&pool, &a, "boom").await.unwrap(),
        LeaseOutcome::LostLease
    );
    assert_eq!(
        CrawlQueueItem::retry_later(&pool, &a, "503", Duration::ZERO, true)
            .await
            .unwrap(),
        QueueTransition::LostLease
    );
    let row = reload(&pool, item.id).await;
    assert_eq!(row.status, "processing");
    assert_eq!(row.locked_by.as_deref(), Some("worker-b"));
    assert!(row.finished_at.is_none());
    assert_eq!(row.retry_count, 1, "only the release counted as an attempt");

    // B still owns the lease and finishes normally.
    assert_eq!(
        CrawlQueueItem::complete(&pool, &b).await.unwrap(),
        LeaseOutcome::Applied
    );
    let row = reload(&pool, item.id).await;
    assert_eq!(row.status, "completed");
    assert!(row.locked_by.is_none() && row.locked_at.is_none());
    assert!(row.finished_at.is_some());

    // ...and a late duplicate from B is a lost lease too, not a second transition.
    assert_eq!(
        CrawlQueueItem::complete(&pool, &b).await.unwrap(),
        LeaseOutcome::LostLease
    );
}

#[tokio::test]
#[serial]
async fn test_stale_claim_with_same_worker_id_is_lost_lease() {
    // REQUIREMENT: leases are per claim, not per worker id. A worker whose job outlived its lease
    // may claim the same item again under the same id; the stale task must not finish the new claim.
    let pool = test_pool().await;
    let source = unique_source("FENCE");
    let sources = vec![source.clone()];
    let item = CrawlQueueItem::enqueue(&pool, &new_item(&source, "GDP"))
        .await
        .unwrap()
        .unwrap();

    let stale = CrawlQueueItem::claim_next(&pool, "worker", Some(&sources))
        .await
        .unwrap()
        .expect("first claim");
    CrawlQueueItem::release_stuck(&pool, Duration::ZERO)
        .await
        .unwrap();
    let fresh = CrawlQueueItem::claim_next(&pool, "worker", Some(&sources))
        .await
        .unwrap()
        .expect("same worker id claims the released item again");
    assert_eq!(fresh.id, item.id);
    assert_eq!(fresh.locked_by, stale.locked_by);
    assert_ne!(fresh.claim_token, stale.claim_token);

    assert_eq!(
        CrawlQueueItem::complete(&pool, &stale).await.unwrap(),
        LeaseOutcome::LostLease
    );
    assert_eq!(
        CrawlQueueItem::fail(&pool, &stale, "boom").await.unwrap(),
        LeaseOutcome::LostLease
    );
    assert_eq!(
        CrawlQueueItem::retry_later(&pool, &stale, "503", Duration::ZERO, true)
            .await
            .unwrap(),
        QueueTransition::LostLease
    );
    let row = reload(&pool, item.id).await;
    assert_eq!(row.status, "processing");
    assert_eq!(row.claim_token, fresh.claim_token);

    // An unclaimed row carries no lease at all.
    assert_eq!(
        CrawlQueueItem::complete(&pool, &item).await.unwrap(),
        LeaseOutcome::LostLease
    );

    assert_eq!(
        CrawlQueueItem::complete(&pool, &fresh).await.unwrap(),
        LeaseOutcome::Applied
    );
    assert!(reload(&pool, item.id).await.claim_token.is_none());
}

#[tokio::test]
#[serial]
async fn test_release_stuck_fails_item_at_max_retries() {
    // REQUIREMENT: a job that keeps crashing workers (never finishes) must not loop forever.
    let pool = test_pool().await;
    let source = unique_source("CRASHY");
    let sources = vec![source.clone()];
    let item = CrawlQueueItem::enqueue(
        &pool,
        &NewCrawlQueueItem {
            max_retries: 2,
            ..new_item(&source, "BOOM")
        },
    )
    .await
    .unwrap()
    .unwrap();

    for (round, expected_status) in [(1, "pending"), (2, "failed")] {
        CrawlQueueItem::claim_next(&pool, &format!("crasher-{round}"), Some(&sources))
            .await
            .unwrap()
            .expect("claimable");
        CrawlQueueItem::release_stuck(&pool, Duration::ZERO)
            .await
            .unwrap();
        let row = reload(&pool, item.id).await;
        assert_eq!(row.retry_count, round);
        assert_eq!(row.status, expected_status, "after release {round}");
        assert!(row.locked_by.is_none() && row.locked_at.is_none());
    }
    let row = reload(&pool, item.id).await;
    assert!(row.finished_at.is_some());
    assert!(row
        .error_message
        .as_deref()
        .is_some_and(|m| m.contains("lease expired") && m.contains("crasher-2")));
    assert!(CrawlQueueItem::claim_next(&pool, "w", Some(&sources))
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
#[serial]
async fn test_purge_finished_deletes_only_old_finished_rows() {
    let pool = test_pool().await;
    let source = unique_source("PURGE");
    let sources = vec![source.clone()];
    let mk = |series: &'static str| {
        let pool = pool.clone();
        let source = source.clone();
        async move {
            CrawlQueueItem::enqueue(&pool, &new_item(&source, series))
                .await
                .unwrap()
                .unwrap()
                .id
        }
    };
    let old_done = mk("OLD_DONE").await;
    let old_failed = mk("OLD_FAILED").await;
    let recent_done = mk("RECENT_DONE").await;
    let old_pending = mk("OLD_PENDING").await;
    let old_processing = mk("OLD_PROCESSING").await;

    for id in [old_done, recent_done] {
        CrawlQueueItem::force_complete(&pool, id).await.unwrap();
    }
    CrawlQueueItem::force_fail(&pool, old_failed, "x")
        .await
        .unwrap();
    CrawlQueueItem::claim_by_id(&pool, old_processing, "w")
        .await
        .unwrap()
        .unwrap();

    // Age everything except `recent_done` by 10 days (a trigger bumps `updated_at` on every
    // UPDATE, so ages are set through created_at / finished_at).
    let legacy_done: Uuid = {
        let mut conn = pool.get().await.unwrap();
        let ten_days_ago = Utc::now() - chrono::Duration::days(10);
        diesel::update(
            crawl_queue::table
                .filter(crawl_queue::source.eq(&source))
                .filter(crawl_queue::id.ne(recent_done)),
        )
        .set(crawl_queue::created_at.eq(ten_days_ago))
        .execute(&mut conn)
        .await
        .unwrap();
        diesel::update(crawl_queue::table.filter(crawl_queue::id.eq_any([old_done, old_failed])))
            .set(crawl_queue::finished_at.eq(Some(ten_days_ago)))
            .execute(&mut conn)
            .await
            .unwrap();
        // A row finished before `finished_at` existed: only `updated_at` tells its age.
        diesel::insert_into(crawl_queue::table)
            .values((
                crawl_queue::source.eq(&source),
                crawl_queue::series_id.eq("LEGACY_DONE"),
                crawl_queue::status.eq("completed"),
                crawl_queue::created_at.eq(ten_days_ago),
                crawl_queue::updated_at.eq(ten_days_ago),
            ))
            .returning(crawl_queue::id)
            .get_result(&mut conn)
            .await
            .unwrap()
    };
    assert!(reload(&pool, legacy_done).await.finished_at.is_none());

    let deleted = CrawlQueueItem::purge_finished(&pool, Duration::from_secs(5 * 86_400))
        .await
        .unwrap();
    assert!(deleted >= 3, "deleted {deleted}");

    let mut conn = pool.get().await.unwrap();
    let left: HashSet<Uuid> = crawl_queue::table
        .filter(crawl_queue::source.eq_any(&sources))
        .select(crawl_queue::id)
        .load::<Uuid>(&mut conn)
        .await
        .unwrap()
        .into_iter()
        .collect();
    assert_eq!(
        left,
        HashSet::from([recent_done, old_pending, old_processing]),
        "only old completed/failed rows are purged"
    );
}

#[tokio::test]
#[serial]
async fn test_purge_finished_works_in_batches() {
    let pool = test_pool().await;
    let source = unique_source("PURGEB");
    let n = super::PURGE_BATCH_SIZE + 5;
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(
        "INSERT INTO crawl_queue (source, series_id, priority, max_retries, status, kind, \
                                  finished_at, updated_at) \
         SELECT $1, 'S' || g, 5, 3, \
                CASE WHEN g % 3 = 0 THEN 'cancelled' ELSE 'completed' END, 'fetch_series', \
                NOW() - INTERVAL '30 days', NOW() - INTERVAL '30 days' \
         FROM generate_series(1, $2) AS g",
    )
    .bind::<diesel::sql_types::Text, _>(&source)
    .bind::<diesel::sql_types::BigInt, _>(n)
    .execute(&mut conn)
    .await
    .unwrap();

    let deleted = CrawlQueueItem::purge_finished(&pool, Duration::from_secs(86_400))
        .await
        .unwrap();
    assert!(deleted >= n, "deleted {deleted}, expected at least {n}");
    let left: i64 = crawl_queue::table
        .filter(crawl_queue::source.eq(&source))
        .count()
        .get_result(&mut conn)
        .await
        .unwrap();
    assert_eq!(left, 0);
}
