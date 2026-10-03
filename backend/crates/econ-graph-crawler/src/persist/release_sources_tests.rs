//! ECO-392 regression: on a freshly migrated database, every source the release build registers
//! (`FRED`, `BLS`, `CENSUS`, `BEA`, `WORLD_BANK`, `FHFA`; see `sources::default_registry`) must be
//! due for crawling and counted by coverage. The consolidated baseline migration seeded World
//! Bank and BEA as `is_enabled = false` behind an admin-approval step that release 1 never shipped
//! (the admin frontend is out of scope) -- Census was seeded disabled the same way but the same
//! migration re-enables it further down. The scheduler's due and discovery queries, and the
//! coverage report, all skip a source whose `data_sources` row has `is_enabled = false`, so World
//! Bank and BEA were never crawled on a fresh v4.0 install.
//!
//! This uses its own migrated database ([`FreshDb`]), not the shared one `scheduler`'s and
//! `coverage`'s own DB-backed tests use: those share process-wide mutable `data_sources` rows
//! across tests, which would make an assertion about the *default*, just-migrated state of these
//! flags order-dependent on whatever another test left behind.

use diesel::sql_types::{Bool, Text, Uuid as SqlUuid};
use diesel_async::RunQueryDsl;

use super::*;
use crate::coverage::crawl_coverage;
use crate::persist::stable_id_tests::{database_url, FreshDb};
use crate::scheduler::{RefreshScheduler, SchedulerConfig};
use crate::source::SourceId;
use crate::sources::static_catalogs::is_static_catalog_source;

/// The live, non-static-catalog sources `default_registry()` registers: the ones a fresh v4.0
/// install must actually crawl.
fn release_sources() -> Vec<SourceId> {
    crate::sources::default_registry()
        .ids()
        .into_iter()
        .filter(|&s| !is_static_catalog_source(s))
        .collect()
}

/// A Census id the scheduler's and coverage's fetchable-id filter accepts (national-level BDS).
const CENSUS_FETCHABLE_ID: &str = "bds/national..T15CX";

#[derive(diesel::QueryableByName)]
struct Enabled {
    #[diesel(sql_type = Bool)]
    is_enabled: bool,
}

#[derive(diesel::QueryableByName, Debug)]
struct Queued {
    #[diesel(sql_type = Text)]
    source: String,
}

async fn seed_series(pool: &DatabasePool, source: SourceId) {
    let external_id = if source == SourceId::Census {
        CENSUS_FETCHABLE_ID.to_string()
    } else {
        format!("{}_release_source_test", source.as_str())
    };
    let source_id = data_source_id(pool, source).await.unwrap();
    let mut conn = pool.get().await.unwrap();
    let dataset_id = econ_graph_core::test_utils::test_dataset_id(&mut conn, source_id).await;
    diesel::sql_query(
        "INSERT INTO economic_series (source_id, external_id, title, frequency, is_active, dataset_id) \
         VALUES ($1, $2, $2, 'Annual', TRUE, $3)",
    )
    .bind::<SqlUuid, _>(source_id)
    .bind::<Text, _>(&external_id)
    .bind::<SqlUuid, _>(dataset_id)
    .execute(&mut conn)
    .await
    .unwrap();
}

#[tokio::test]
async fn fresh_migration_enables_every_release_source() {
    let Some(admin_url) = database_url() else {
        return;
    };
    let release_sources = release_sources();
    let db = FreshDb::create(&admin_url, "econgraph_release_sources").await;

    // The flag the scheduler's due/discovery queries and crawl_coverage filter on, straight from
    // the just-migrated `data_sources` rows (BEA and World Bank come from the baseline migration;
    // FHFA has no seed row and is created here by `data_source_id`'s template fallback).
    let mut enabled = Vec::with_capacity(release_sources.len());
    for &source in &release_sources {
        let source_id = data_source_id(&db.pool, source).await.unwrap();
        let mut conn = db.pool.get().await.unwrap();
        let row: Enabled = diesel::sql_query("SELECT is_enabled FROM data_sources WHERE id = $1")
            .bind::<SqlUuid, _>(source_id)
            .get_result(&mut conn)
            .await
            .unwrap();
        enabled.push((source, row.is_enabled));
    }

    for &source in &release_sources {
        seed_series(&db.pool, source).await;
    }

    let scheduler = RefreshScheduler::new(
        db.pool.clone(),
        release_sources.clone(),
        SchedulerConfig::default(),
    );
    scheduler.tick().await.unwrap();

    // The baseline migration also seeds a few `series_metadata` rows of its own for some of
    // these sources, so compare per source (via the queued jobs) rather than a total count.
    let queued: Vec<Queued> = {
        let mut conn = db.pool.get().await.unwrap();
        diesel::sql_query("SELECT source FROM crawl_queue WHERE kind = 'fetch_series'")
            .get_results(&mut conn)
            .await
            .unwrap()
    };

    let coverage = crawl_coverage(&db.pool, &release_sources).await.unwrap();

    db.drop().await;

    for (source, is_enabled) in enabled {
        assert!(is_enabled, "{source} should be enabled by default");
    }
    for &source in &release_sources {
        assert!(
            queued.iter().any(|q| q.source == source.as_str()),
            "{source} has no fetch_series job queued: {queued:?}"
        );
    }
    for &source in &release_sources {
        let c = coverage
            .iter()
            .find(|c| c.source == source.as_str())
            .unwrap_or_else(|| panic!("{source} missing from coverage: {coverage:?}"));
        assert!(c.discovered >= 1, "{source}: {c:?}");
    }
}
