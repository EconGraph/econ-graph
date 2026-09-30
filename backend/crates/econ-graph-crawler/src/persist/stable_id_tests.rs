//! DB-backed tests for stable series ids and retirement. They need `DATABASE_URL` and are skipped
//! when it is unset. Each test creates its own databases next to `DATABASE_URL`'s (same server,
//! a name unique to the test and process), migrates them and drops them at the end, so rows of
//! other tests can't interfere and they don't take the shared database's test lock.

use std::collections::BTreeMap;

use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use econ_graph_core::schema::{data_sources, economic_series, series_metadata};
use econ_graph_core::DatabasePool;
use uuid::Uuid;

use super::*;
use crate::adapter::SourceAdapter;
use crate::series_id::stable_series_id;
use crate::testkit::echo::EchoAdapter;
use crate::testkit::{test_ctx, MockSource, Reply, Route};

/// A migrated database of its own, dropped by [`FreshDb::drop`].
struct FreshDb {
    admin_url: String,
    name: String,
    pool: DatabasePool,
}

impl FreshDb {
    /// `base` plus this process id, so concurrent test runs on one server don't collide.
    async fn create(admin_url: &str, base: &str) -> Self {
        let name = format!("{base}_{}", std::process::id());
        let mut admin = AsyncPgConnection::establish(admin_url).await.unwrap();
        for sql in [
            format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"),
            format!("CREATE DATABASE {name}"),
        ] {
            diesel::sql_query(sql).execute(&mut admin).await.unwrap();
        }
        let mut url = reqwest::Url::parse(admin_url).unwrap();
        url.set_path(&name);
        econ_graph_core::run_migrations(url.as_str()).await.unwrap();
        let pool = econ_graph_core::create_pool(url.as_str()).await.unwrap();
        Self {
            admin_url: admin_url.to_string(),
            name,
            pool,
        }
    }

    async fn drop(self) {
        drop(self.pool);
        let mut admin = AsyncPgConnection::establish(&self.admin_url).await.unwrap();
        diesel::sql_query(format!(
            "DROP DATABASE IF EXISTS {} WITH (FORCE)",
            self.name
        ))
        .execute(&mut admin)
        .await
        .unwrap();
    }
}

fn database_url() -> Option<String> {
    let url = std::env::var("DATABASE_URL").ok();
    if url.is_none() {
        eprintln!("DATABASE_URL not set; skipping DB-backed stable id test");
    }
    url
}

/// `(source name, external_id) -> id` of every row of `economic_series` and `series_metadata`.
type Ids = BTreeMap<(String, String), Uuid>;

async fn ids(pool: &DatabasePool) -> (Ids, Ids) {
    let mut conn = pool.get().await.unwrap();
    let series: Vec<(String, String, Uuid)> = economic_series::table
        .inner_join(data_sources::table)
        .select((
            data_sources::name,
            economic_series::external_id,
            economic_series::id,
        ))
        .load(&mut conn)
        .await
        .unwrap();
    let metadata: Vec<(String, String, Uuid)> = series_metadata::table
        .inner_join(data_sources::table.on(data_sources::id.eq(series_metadata::source_id)))
        .select((
            data_sources::name,
            series_metadata::external_id,
            series_metadata::id,
        ))
        .load(&mut conn)
        .await
        .unwrap();
    let map = |rows: Vec<(String, String, Uuid)>| {
        rows.into_iter()
            .map(|(source, external_id, id)| ((source, external_id), id))
            .collect()
    };
    (map(series), map(metadata))
}

/// The stable id every row should have, by data source name.
fn expected_id(source_name: &str, external_id: &str) -> Uuid {
    let source = SourceId::ALL
        .into_iter()
        .find(|s| data_source_template(*s).name == source_name)
        .unwrap_or_else(|| panic!("no SourceId for data source {source_name:?}"));
    stable_series_id(source, external_id)
}

const CATALOG: &str = r#"{"series": [
    {"id": "GDP", "title": "Gross Domestic Product"},
    {"id": "PAYEMS", "title": "All Employees, Total Nonfarm"}
]}"#;

async fn mock_source() -> MockSource {
    let mock = MockSource::start().await;
    mock.mount(&Route::get("/catalog"), Reply::json_str(CATALOG))
        .await;
    for id in ["GDP", "PAYEMS", "UNDISCOVERED"] {
        mock.mount(
            &Route::get(format!("/series/{id}")),
            Reply::json(serde_json::json!({
                "title": format!("Series {id}"),
                "points": [
                    {"date": "2024-01-01", "value": "1.5"},
                    {"date": "2024-04-01", "value": "2.5"},
                ],
            })),
        )
        .await;
    }
    mock
}

/// Discovers the catalog and fetches every discovered series plus one that discovery doesn't
/// list, persisting everything, the same calls the worker makes.
async fn crawl(pool: &DatabasePool, mock: &MockSource) {
    let adapter = EchoAdapter::new(mock.base_url());
    let ctx = test_ctx();
    let found = adapter.discover(&ctx).await.unwrap();
    persist_discovered(pool, adapter.id(), &found)
        .await
        .unwrap();
    let mut external_ids: Vec<String> = found.iter().map(|d| d.external_id.clone()).collect();
    external_ids.push("UNDISCOVERED".to_string());
    for external_id in external_ids {
        let fetched = adapter
            .fetch_series(&ctx, &external_id, None)
            .await
            .unwrap();
        persist_series(pool, adapter.id(), &external_id, &fetched)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn crawling_fixtures_into_two_fresh_databases_gives_identical_ids() {
    let Some(url) = database_url() else { return };
    let mock = mock_source().await;
    let a = FreshDb::create(&url, "econ_graph_test_stable_ids_a").await;
    let b = FreshDb::create(&url, "econ_graph_test_stable_ids_b").await;
    crawl(&a.pool, &mock).await;
    // A second crawl of the same database changes nothing.
    crawl(&b.pool, &mock).await;
    crawl(&b.pool, &mock).await;

    let (series_a, metadata_a) = ids(&a.pool).await;
    let (series_b, metadata_b) = ids(&b.pool).await;
    a.drop().await;
    b.drop().await;

    let fred = data_source_template(SourceId::Fred).name;
    assert_eq!(
        series_a.keys().cloned().collect::<Vec<_>>(),
        ["GDP", "PAYEMS", "UNDISCOVERED"].map(|id| (fred.clone(), id.to_string()))
    );
    assert_eq!(series_a, series_b);
    assert_eq!(metadata_a, metadata_b);
    for ((source, external_id), id) in series_a.iter().chain(&metadata_a) {
        assert_eq!(
            *id,
            expected_id(source, external_id),
            "{source}:{external_id}"
        );
    }
    // Same natural key, same id, in both tables; FRED:GDP is the pinned example.
    let gdp = (fred, "GDP".to_string());
    assert_eq!(series_a[&gdp], metadata_a[&gdp]);
    assert_eq!(
        series_a[&gdp].to_string(),
        "d8124fe6-ef1c-52dd-8d22-c1976625064c"
    );
}

/// The migration moves the rows seeded by the initial migration onto their stable ids.
#[tokio::test]
async fn seeded_series_metadata_has_stable_ids() {
    let Some(url) = database_url() else { return };
    let db = FreshDb::create(&url, "econ_graph_test_stable_ids_seeds").await;
    let (_, metadata) = ids(&db.pool).await;
    db.drop().await;

    assert_eq!(metadata.len(), 11, "seeds: {metadata:?}");
    for ((source, external_id), id) in &metadata {
        assert_eq!(
            *id,
            expected_id(source, external_id),
            "{source}:{external_id}"
        );
    }
}

/// A row created before stable ids is moved onto its stable id when it's discovered again.
#[tokio::test]
async fn rediscovery_moves_old_metadata_rows_onto_stable_ids() {
    let Some(url) = database_url() else { return };
    let mock = mock_source().await;
    let db = FreshDb::create(&url, "econ_graph_test_stable_ids_realign").await;
    let source_id = data_source_id(&db.pool, SourceId::Fred).await.unwrap();
    {
        let mut conn = db.pool.get().await.unwrap();
        diesel::sql_query(
            "INSERT INTO series_metadata (id, source_id, external_id, title, is_active) \
             VALUES (gen_random_uuid(), $1, 'PAYEMS', 'old', TRUE)",
        )
        .bind::<SqlUuid, _>(source_id)
        .execute(&mut conn)
        .await
        .unwrap();
    }
    crawl(&db.pool, &mock).await;
    let (_, metadata) = ids(&db.pool).await;
    db.drop().await;

    let key = (data_source_template(SourceId::Fred).name, "PAYEMS".into());
    assert_eq!(metadata[&key], stable_series_id(SourceId::Fred, "PAYEMS"));
}

async fn active(pool: &DatabasePool, external_id: &str) -> (Option<bool>, Option<bool>) {
    let mut conn = pool.get().await.unwrap();
    let series = economic_series::table
        .filter(economic_series::external_id.eq(external_id))
        .select(economic_series::is_active)
        .first::<bool>(&mut conn)
        .await
        .optional()
        .unwrap();
    let metadata = series_metadata::table
        .filter(series_metadata::external_id.eq(external_id))
        .select(series_metadata::is_active)
        .first::<bool>(&mut conn)
        .await
        .optional()
        .unwrap();
    (series, metadata)
}

fn listed(ids: &[&str]) -> Vec<DiscoveredSeries> {
    ids.iter()
        .map(|id| DiscoveredSeries {
            external_id: id.to_string(),
            title: id.to_string(),
            description: None,
            units: None,
            frequency: None,
            data_url: None,
            dataset: None,
        })
        .collect()
}

#[tokio::test]
async fn unlisted_series_are_retired_not_deleted_and_come_back() {
    let Some(url) = database_url() else { return };
    let mock = mock_source().await;
    let db = FreshDb::create(&url, "econ_graph_test_stable_ids_retire").await;
    let p = &db.pool;
    crawl(p, &mock).await;
    let (before, _) = ids(p).await;

    // An empty catalog is ignored.
    let none = retire_unlisted(p, SourceId::Fred, &[], None).await.unwrap();
    assert_eq!(none, Retirement::default());
    assert_eq!(active(p, "GDP").await, (Some(true), Some(true)));

    // The source stops listing PAYEMS (and never listed UNDISCOVERED).
    let retired = retire_unlisted(p, SourceId::Fred, &listed(&["GDP"]), None)
        .await
        .unwrap();
    // Seeded FRED metadata (UNRATE, CPIAUCSL) isn't listed either.
    assert_eq!(retired.series_retired, 2);
    assert_eq!(retired.metadata_retired, 3, "{retired:?}");
    assert_eq!(retired.series_reactivated, 0);
    assert_eq!(active(p, "GDP").await, (Some(true), Some(true)));
    assert_eq!(active(p, "PAYEMS").await, (Some(false), Some(false)));
    assert_eq!(active(p, "UNDISCOVERED").await, (Some(false), None));
    // Other sources are untouched.
    assert_eq!(active(p, "CES0000000001").await, (None, Some(true)));

    // Nothing was deleted: same rows, same ids, points kept.
    let (after, _) = ids(p).await;
    assert_eq!(before, after);
    let payems = stable_series_id(SourceId::Fred, "PAYEMS");
    assert_eq!(
        latest_point_date_by_id(p, payems).await.unwrap(),
        NaiveDate::from_ymd_opt(2024, 4, 1)
    );

    // PAYEMS is listed again: it comes back with the same id.
    persist_discovered(p, SourceId::Fred, &listed(&["GDP", "PAYEMS"]))
        .await
        .unwrap();
    let back = retire_unlisted(p, SourceId::Fred, &listed(&["GDP", "PAYEMS"]), None)
        .await
        .unwrap();
    assert_eq!(back.series_reactivated, 1);
    assert_eq!(active(p, "PAYEMS").await, (Some(true), Some(true)));
    let (again, _) = ids(p).await;
    db.drop().await;
    assert_eq!(before, again);
}

/// Regression for the review finding on #219: Census discovery only ever lists `CENSUS_BDS_*`
/// series, but the Census data source also holds ACS `series_metadata` rows seeded by the initial
/// migration (`B01001001`, `B19013_001E`). A complete-catalog retirement for Census must not
/// retire those, and `CensusAdapter::retirement_scope_prefix` (`ID_PREFIX`, `"CENSUS_BDS_"`)
/// exists to stop it. See `unlisted_series_are_retired_not_deleted_and_come_back` for the
/// unscoped (whole-source) case this specializes.
#[tokio::test]
async fn census_retirement_is_scoped_to_bds_and_never_touches_seeded_acs_rows() {
    use crate::sources::census::CensusAdapter;

    let Some(url) = database_url() else { return };
    let db = FreshDb::create(&url, "econ_graph_test_stable_ids_census_scope").await;
    let p = &db.pool;
    let adapter = CensusAdapter::default();
    assert!(adapter.discovery_is_complete());
    let scope = adapter.retirement_scope_prefix();
    assert_eq!(scope, Some("CENSUS_BDS_"));

    // The seeded ACS rows exist and are active before any Census discovery runs.
    assert_eq!(active(p, "B01001001").await, (None, Some(true)));
    assert_eq!(active(p, "B19013_001E").await, (None, Some(true)));

    // A BDS discovery that lists one national series and nothing else.
    let bds = listed(&["CENSUS_BDS_ESTAB_us"]);
    persist_discovered(p, SourceId::Census, &bds).await.unwrap();
    let retired = retire_unlisted(p, SourceId::Census, &bds, scope)
        .await
        .unwrap();

    // Nothing BDS doesn't own is touched, however unlisted it is.
    assert_eq!(retired.metadata_retired, 0, "{retired:?}");
    assert_eq!(retired.series_retired, 0, "{retired:?}");
    assert_eq!(active(p, "B01001001").await, (None, Some(true)));
    assert_eq!(active(p, "B19013_001E").await, (None, Some(true)));
    assert_eq!(active(p, "CENSUS_BDS_ESTAB_us").await, (None, Some(true)));

    // An unscoped call (as if the adapter forgot to scope itself) would retire them: proves the
    // scope parameter, not some other accident, is what protects the ACS rows above.
    let unscoped = retire_unlisted(p, SourceId::Census, &bds, None)
        .await
        .unwrap();
    assert_eq!(unscoped.metadata_retired, 2, "{unscoped:?}");
    db.drop().await;
}
