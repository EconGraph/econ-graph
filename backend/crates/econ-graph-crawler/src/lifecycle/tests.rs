use super::*;
use crate::SeriesDataset;

fn found(id: &str) -> DiscoveredSeries {
    DiscoveredSeries {
        external_id: id.into(),
        title: format!("Title {id}"),
        description: None,
        units: None,
        frequency: Some("Annual".into()),
        data_url: None,
        dataset: SeriesDataset {
            code: "bea_regional".into(),
            dimensions: Default::default(),
        },
    }
}
fn tracked(id: &str) -> State {
    State {
        external_id: id.into(),
        status: "active".into(),
        since: None,
        missing_refreshes: 0,
        last_seen_at: None,
        proposed_successor_id: None,
        successor_id: None,
        in_accepted_catalog: true,
    }
}
#[test]
fn needs_three_absences_and_recovers_without_losing_confirmed_successors() {
    let now = Utc::now();
    let baseline: Vec<_> = (0..5).map(|i| tracked(&i.to_string())).collect();
    let selected: Vec<_> = (1..5).map(|i| found(&i.to_string())).collect();
    let mut states = baseline;
    for n in 1..=3 {
        let (next, guarded) = plan(
            &states,
            &selected,
            &CatalogLifecycle::default(),
            true,
            now,
            LifecyclePolicy::default(),
        );
        assert!(!guarded);
        states = next;
        let missing = &states[0];
        assert_eq!(missing.missing_refreshes, n);
        assert_eq!(
            missing.status,
            if n < 3 {
                "active"
            } else {
                "missing_from_catalog"
            }
        );
    }
    let (restored, _) = plan(
        &states,
        &(0..5).map(|i| found(&i.to_string())).collect::<Vec<_>>(),
        &CatalogLifecycle::default(),
        true,
        now,
        LifecyclePolicy::default(),
    );
    assert_eq!(restored[0].status, "active");
    assert_eq!(restored[0].missing_refreshes, 0);
    assert_eq!(restored[0].since, None);
    states[0].successor_id = Some(Uuid::new_v4());
    states[0].status = "superseded".into();
    let (restored, _) = plan(
        &states,
        &(0..5).map(|i| found(&i.to_string())).collect::<Vec<_>>(),
        &CatalogLifecycle::default(),
        true,
        now,
        LifecyclePolicy::default(),
    );
    assert_eq!(restored[0].status, "superseded");
    assert!(restored[0].successor_id.is_some());
}
#[test]
fn mass_disappearance_and_empty_snapshots_mark_nothing() {
    let baseline: Vec<_> = (0..5).map(|i| tracked(&i.to_string())).collect();
    for selected in [vec![], vec![found("3"), found("4")]] {
        let (next, guarded) = plan(
            &baseline,
            &selected,
            &CatalogLifecycle::default(),
            true,
            Utc::now(),
            LifecyclePolicy::default(),
        );
        assert!(guarded);
        assert!(next.is_empty());
    }
}
#[test]
fn changed_rules_do_not_turn_deselection_into_removal() {
    let baseline = vec![tracked("old")];
    let (states, guarded) = plan(
        &baseline,
        &[found("new")],
        &CatalogLifecycle::default(),
        false,
        Utc::now(),
        LifecyclePolicy::default(),
    );
    assert!(!guarded);
    let old = states.iter().find(|s| s.external_id == "old").unwrap();
    assert_eq!(old.status, "active");
    assert_eq!(old.missing_refreshes, 0);
}
#[test]
fn explicit_restrictions_and_discontinued_titles_are_distinct_from_absence() {
    let facts = CatalogLifecycle {
        restricted_ids: vec!["r".into()],
        discontinued_ids: vec!["d".into()],
        ..Default::default()
    };
    let (states, guarded) = plan(
        &[],
        &[found("r"), found("d")],
        &facts,
        false,
        Utc::now(),
        LifecyclePolicy::default(),
    );
    assert!(!guarded);
    assert_eq!(states[0].status, "discontinued");
    assert_eq!(states[1].status, "restricted");
}

#[test]
fn gradual_retirements_do_not_freeze_future_complete_catalogs() {
    let mut existing: Vec<_> = (0..10).map(|i| tracked(&i.to_string())).collect();
    for retired in 1..=5 {
        let found: Vec<_> = (retired..10).map(|i| found(&i.to_string())).collect();
        let (next, guarded) = plan(
            &existing,
            &found,
            &CatalogLifecycle::default(),
            true,
            Utc::now(),
            LifecyclePolicy::default(),
        );
        assert!(
            !guarded,
            "sequential one-series retirement {retired} was mistaken for a mass disappearance"
        );
        existing = next;
    }
    assert_eq!(existing[0].status, "missing_from_catalog");
}

#[tokio::test]
async fn retained_history_survives_discovery_fetch_and_confirmed_successor() {
    use crate::{
        persist::stable_id_tests::FreshDb, testkit::echo::EchoAdapter, DatasetCatalog,
        FetchedPoint, FetchedSeries, NewSeriesMetadataLite,
    };
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return;
    };
    let db = FreshDb::create(&url, "eco464_lifecycle_history").await;
    let pool = &db.pool;
    let mut catalog = DatasetCatalog::empty();
    catalog
        .load_adapter(&EchoAdapter::new("https://unused.invalid"))
        .unwrap();
    persist::sync_datasets(pool, &catalog).await.unwrap();
    let mut initial: Vec<_> = (0..5).map(|i| found(&i.to_string())).collect();
    for d in &mut initial {
        d.dataset = SeriesDataset::new("FRED", Vec::<(String, String)>::new());
    }
    initial[0].title = "Retained old history".into();
    let sample = FetchedSeries {
        metadata: Some(NewSeriesMetadataLite {
            title: "Retained old history".into(),
            ..Default::default()
        }),
        points: vec![FetchedPoint {
            date: chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
            value: Some(42.into()),
            revision_date: chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
            is_original_release: true,
        }],
        dataset: initial[0].dataset.clone(),
        validators: None,
    };
    let original = persist::persist_series(pool, SourceId::Fred, "0", &sample)
        .await
        .unwrap()
        .series_id;
    let mut conn = pool.get().await.unwrap();
    let mut now = Utc::now();
    conn.transaction::<(), AppError, _>(async |conn| {
        persist::persist_discovered_conn(conn, SourceId::Fred, &initial).await?;
        reconcile_conn(
            conn,
            SourceId::Fred,
            &initial,
            &CatalogLifecycle::default(),
            "scope-v1",
            now,
        )
        .await?;
        Ok(())
    })
    .await
    .unwrap();
    let mut renamed = initial[1..].to_vec();
    let mut new = initial[0].clone();
    new.external_id = "new".into();
    renamed.push(new);
    let empty_target = FetchedSeries {
        points: Vec::new(),
        ..sample.clone()
    };
    persist::persist_series(pool, SourceId::Fred, "new", &empty_target)
        .await
        .unwrap();
    for _ in 0..3 {
        now += chrono::Duration::seconds(1);
        conn.transaction::<(), AppError, _>(async |conn| {
            persist::persist_discovered_conn(conn, SourceId::Fred, &renamed).await?;
            reconcile_conn(
                conn,
                SourceId::Fred,
                &renamed,
                &CatalogLifecycle::default(),
                "scope-v1",
                now,
            )
            .await?;
            Ok(())
        })
        .await
        .unwrap();
    }
    let states:Vec<State>=diesel::sql_query("SELECT external_id,status,since,missing_refreshes,last_seen_at,proposed_successor_id,successor_id,in_accepted_catalog FROM series_lifecycle WHERE external_id='0'").load(&mut conn).await.unwrap();
    assert_eq!(states[0].status, "missing_from_catalog");
    assert_eq!(
        states[0].proposed_successor_id,
        Some(crate::stable_series_id(SourceId::Fred, "new"))
    );
    // History still exists and is browsable. A successful historic fetch is not
    // evidence of renewed publication and cannot clear its badge.
    let after = persist::persist_series(pool, SourceId::Fred, "0", &sample)
        .await
        .unwrap();
    assert_eq!(original, after.series_id);
    #[derive(QueryableByName)]
    struct Stored {
        #[diesel(sql_type=Text)]
        status: String,
        #[diesel(sql_type=diesel::sql_types::Bool)]
        active: bool,
        #[diesel(sql_type=diesel::sql_types::BigInt)]
        points: i64,
    }
    let stored:Stored=diesel::sql_query("SELECT crawl_status AS status,is_active AS active,(SELECT count(*) FROM data_points WHERE series_id=es.id) AS points FROM economic_series es WHERE id=$1")
        .bind::<SqlUuid,_>(original).get_result(&mut conn).await.unwrap();
    assert_eq!(stored.status, "missing_from_catalog");
    assert!(stored.active);
    assert_eq!(stored.points, 1);
    // A wrong target cannot silently become a successor.
    assert!(confirm_successor(pool, SourceId::Fred, "0", "1")
        .await
        .is_err());
    assert!(
        confirm_successor(pool, SourceId::Fred, "0", "new")
            .await
            .is_err(),
        "metadata/empty-history target is not a usable successor"
    );
    persist::persist_series(pool, SourceId::Fred, "new", &sample)
        .await
        .unwrap();
    for blocked in [
        "restricted",
        "discontinued",
        "missing_from_catalog",
        "retired",
    ] {
        diesel::sql_query("UPDATE series_lifecycle SET status=$1 WHERE external_id='new'")
            .bind::<Text, _>(blocked)
            .execute(&mut conn)
            .await
            .unwrap();
        assert!(
            confirm_successor(pool, SourceId::Fred, "0", "new")
                .await
                .is_err(),
            "non-published successor {blocked} was incorrectly accepted"
        );
    }
    diesel::sql_query("UPDATE series_lifecycle SET status='active' WHERE external_id='new'")
        .execute(&mut conn)
        .await
        .unwrap();
    confirm_successor(pool, SourceId::Fred, "0", "new")
        .await
        .unwrap();
    let confirmed:State=diesel::sql_query("SELECT external_id,status,since,missing_refreshes,last_seen_at,proposed_successor_id,successor_id,in_accepted_catalog FROM series_lifecycle WHERE external_id='0'").get_result(&mut conn).await.unwrap();
    assert_eq!(confirmed.status, "superseded");
    assert_eq!(
        confirmed.successor_id,
        Some(crate::stable_series_id(SourceId::Fred, "new"))
    );
    drop(conn);
    db.drop().await;
}

#[tokio::test]
async fn fetch_only_not_found_and_restriction_evidence_is_durable() {
    use crate::{persist::stable_id_tests::FreshDb, testkit::echo::EchoAdapter, DatasetCatalog};
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return;
    };
    let db = FreshDb::create(&url, "eco464_fetch_lifecycle").await;
    let pool = &db.pool;
    let mut catalog = DatasetCatalog::empty();
    catalog
        .load_adapter(&EchoAdapter::new("https://unused.invalid"))
        .unwrap();
    persist::sync_datasets(pool, &catalog).await.unwrap();
    let mut d = found("retired");
    d.dataset = SeriesDataset::new("FRED", Vec::<(String, String)>::new());
    persist::persist_discovered(pool, SourceId::Fred, &[d])
        .await
        .unwrap();
    let mut conn = pool.get().await.unwrap();
    for n in 1..=3 {
        conn.transaction::<(), AppError, _>(async |conn| {
            record_not_found_conn(conn, SourceId::Fred, "retired").await?;
            Ok(())
        })
        .await
        .unwrap();
        let state:State=diesel::sql_query("SELECT external_id,status,since,missing_refreshes,last_seen_at,proposed_successor_id,successor_id,in_accepted_catalog FROM series_lifecycle WHERE external_id='retired'").get_result(&mut conn).await.unwrap();
        assert_eq!(state.missing_refreshes, 0);
        assert_eq!(fetch_evidence(pool, "retired").await.fetch_misses, n);
        assert_eq!(state.status, if n < 3 { "active" } else { "retired" });
    }
    conn.transaction::<(), AppError, _>(async |conn| {
        record_restricted_conn(conn, SourceId::Fred, "retired").await?;
        Ok(())
    })
    .await
    .unwrap();
    #[derive(QueryableByName)]
    struct Active {
        #[diesel(sql_type=diesel::sql_types::Bool)]
        active: bool,
    }
    let a: Active = diesel::sql_query(
        "SELECT is_active AS active FROM series_metadata WHERE external_id='retired'",
    )
    .get_result(&mut conn)
    .await
    .unwrap();
    assert!(!a.active);
    drop(conn);
    db.drop().await;
}

#[test]
fn positive_legal_evidence_is_preserved_across_selection_changes() {
    let mut old = tracked("same");
    old.status = "restricted".into();
    old.since = Some(Utc::now());
    let mut superseded = tracked("linked");
    superseded.status = "superseded".into();
    superseded.successor_id = Some(Uuid::new_v4());
    let original_link = superseded.successor_id;
    let (states, guarded) = plan(
        &[old, superseded],
        &[found("same"), found("linked")],
        &CatalogLifecycle::default(),
        false,
        Utc::now(),
        LifecyclePolicy::default(),
    );
    assert!(!guarded);
    assert_eq!(
        states
            .iter()
            .find(|s| s.external_id == "same")
            .unwrap()
            .status,
        "restricted"
    );
    let linked = states.iter().find(|s| s.external_id == "linked").unwrap();
    assert_eq!(linked.status, "superseded");
    assert_eq!(linked.successor_id, original_link);
}

#[tokio::test]
async fn guarded_catalog_still_hides_explicitly_restricted_history() {
    use crate::{persist::stable_id_tests::FreshDb, testkit::echo::EchoAdapter, DatasetCatalog};
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return;
    };
    let db = FreshDb::create(&url, "eco464_guard_restricted").await;
    let pool = &db.pool;
    let mut catalog = DatasetCatalog::empty();
    catalog
        .load_adapter(&EchoAdapter::new("https://unused.invalid"))
        .unwrap();
    persist::sync_datasets(pool, &catalog).await.unwrap();
    let mut initial: Vec<_> = (0..5).map(|i| found(&i.to_string())).collect();
    for d in &mut initial {
        d.dataset = SeriesDataset::new("FRED", Vec::<(String, String)>::new());
    }
    let now = Utc::now();
    let mut conn = pool.get().await.unwrap();
    conn.transaction::<(), AppError, _>(async |conn| {
        persist::persist_discovered_conn(conn, SourceId::Fred, &initial).await?;
        reconcile_conn(
            conn,
            SourceId::Fred,
            &initial,
            &CatalogLifecycle::default(),
            "same",
            now,
        )
        .await?;
        Ok(())
    })
    .await
    .unwrap();
    let small = vec![initial[0].clone()];
    let facts = CatalogLifecycle {
        restricted_ids: vec!["0".into()],
        ..Default::default()
    };
    conn.transaction::<(), AppError, _>(async |conn| {
        let result = reconcile_conn(
            conn,
            SourceId::Fred,
            &small,
            &facts,
            "same",
            now + chrono::Duration::seconds(1),
        )
        .await?;
        assert!(result.guarded);
        Ok(())
    })
    .await
    .unwrap();
    let rows:Vec<State>=diesel::sql_query("SELECT external_id,status,since,missing_refreshes,last_seen_at,proposed_successor_id,successor_id,in_accepted_catalog FROM series_lifecycle ORDER BY external_id").load(&mut conn).await.unwrap();
    assert_eq!(rows[0].status, "restricted");
    assert!(rows[1..]
        .iter()
        .all(|r| r.status == "active" && r.missing_refreshes == 0));
    // An older full snapshot cannot reverse evidence committed by a newer guarded one.
    conn.transaction::<(), AppError, _>(async |conn| {
        let result = reconcile_conn(
            conn,
            SourceId::Fred,
            &initial,
            &CatalogLifecycle::default(),
            "same",
            now,
        )
        .await?;
        assert_eq!(result, Reconciled::default());
        Ok(())
    })
    .await
    .unwrap();
    drop(conn);
    db.drop().await;
}

#[tokio::test]
async fn bea_renamed_table_keeps_old_dimension_codes_labels_units_and_descriptions() {
    use crate::{persist::stable_id_tests::FreshDb, DatasetCatalog};
    use econ_graph_core::models::{Code, DatasetComponents};
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return;
    };
    let db = FreshDb::create(&url, "eco464_dimension_retention").await;
    let pool = &db.pool;
    let mut catalog = DatasetCatalog::empty();
    catalog
        .insert(
            SourceId::Bea,
            &["bea_regional"],
            crate::dataset::parse_dataset_file(
                r#"
[[dataset]]
code="bea_regional"
name="Regional"
[[dataset.dimensions]]
name="table_name"
label="Table"
"#,
            )
            .unwrap(),
        )
        .unwrap();
    persist::sync_datasets(pool, &catalog).await.unwrap();
    let mut old = Code::new("SAGDP2N", "Historic regional GDP");
    old.unit = Some("Millions of dollars".into());
    old.description = Some("Original upstream table".into());
    persist::merge_dataset_dimension_code_entries(
        pool,
        SourceId::Bea,
        "bea_regional",
        "table_name",
        &[old.clone()],
    )
    .await
    .unwrap();
    let new = Code::new("SAGDP2", "Current regional GDP");
    persist::merge_dataset_dimension_code_entries(
        pool,
        SourceId::Bea,
        "bea_regional",
        "table_name",
        std::slice::from_ref(&new),
    )
    .await
    .unwrap();
    // Even an empty complete source catalog or later refreshed current title cannot
    // prune the old code: historical series still need its original labels.
    persist::merge_dataset_dimension_code_entries(
        pool,
        SourceId::Bea,
        "bea_regional",
        "table_name",
        &[],
    )
    .await
    .unwrap();
    #[derive(QueryableByName)]
    struct Dims {
        #[diesel(sql_type=Jsonb)]
        dimensions: DatasetComponents,
    }
    let mut conn = pool.get().await.unwrap();
    let dims: Dims = diesel::sql_query("SELECT dimensions FROM datasets WHERE code='bea_regional'")
        .get_result(&mut conn)
        .await
        .unwrap();
    let codes = dims.dimensions.0[0].codes.as_ref().unwrap();
    assert!(codes.contains(&old));
    assert!(codes.contains(&new));
    drop(conn);
    db.drop().await;
}

#[tokio::test]
async fn first_rules_baseline_enrolls_only_proven_legacy_scope_members() {
    use crate::{persist::stable_id_tests::FreshDb, DatasetCatalog, FetchedPoint, FetchedSeries};
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return;
    };
    let db = FreshDb::create(&url, "eco464_legacy_scope_adoption").await;
    let pool = &db.pool;
    let mut catalog = DatasetCatalog::empty();
    catalog
        .insert(
            SourceId::Bea,
            &["bea_regional"],
            crate::dataset::parse_dataset_file(
                r#"
[[dataset]]
code="bea_regional"
name="Regional"
"#,
            )
            .unwrap(),
        )
        .unwrap();
    persist::sync_datasets(pool, &catalog).await.unwrap();
    let old_id = "bea_regional/SAGDP2N.1.06000";
    let new_id = "bea_regional/SAGDP2.1.06000";
    let unrelated_id = "bea_regional/SAINC1.1.06000";
    let old = found(old_id);
    let unrelated = found(unrelated_id);
    persist::persist_discovered(pool, SourceId::Bea, &[old.clone(), unrelated.clone()])
        .await
        .unwrap();
    let point_date = chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
    let sample = FetchedSeries {
        metadata: None,
        points: vec![FetchedPoint {
            date: point_date,
            value: Some(7.into()),
            revision_date: point_date,
            is_original_release: true,
        }],
        dataset: old.dataset.clone(),
        validators: None,
    };
    let old_series = persist::persist_series(pool, SourceId::Bea, old_id, &sample)
        .await
        .unwrap()
        .series_id;
    let unrelated_series = persist::persist_series(pool, SourceId::Bea, unrelated_id, &sample)
        .await
        .unwrap()
        .series_id;
    let mut current: Vec<_> = (0..4)
        .map(|i| found(&format!("bea_regional/SAGDP2.{i}.00000")))
        .collect();
    current.push(found(new_id));
    // The adapter, rather than the generic lifecycle layer, proves membership using
    // canonical BEA ids + retained table titles. SAINC1 is outside the selected family.
    let facts = CatalogLifecycle {
        scope_ids: vec![old_id.into()],
        ..Default::default()
    };
    let mut conn = pool.get().await.unwrap();
    let mut now = Utc::now();
    conn.transaction::<(), AppError, _>(async |conn| {
        persist::persist_discovered_conn(conn, SourceId::Bea, &current).await?;
        reconcile_conn(
            conn,
            SourceId::Bea,
            &current,
            &facts,
            "bea-SAGDP2-family",
            now,
        )
        .await?;
        Ok(())
    })
    .await
    .unwrap();
    let enrolled:State=diesel::sql_query("SELECT external_id,status,since,missing_refreshes,last_seen_at,proposed_successor_id,successor_id,in_accepted_catalog FROM series_lifecycle WHERE external_id=$1")
        .bind::<Text,_>(old_id).get_result(&mut conn).await.unwrap();
    assert_eq!(enrolled.status, "active");
    assert_eq!(enrolled.missing_refreshes, 0);
    for refresh in 1..=3 {
        now += chrono::Duration::seconds(1);
        conn.transaction::<(), AppError, _>(async |conn| {
            reconcile_conn(
                conn,
                SourceId::Bea,
                &current,
                &facts,
                "bea-SAGDP2-family",
                now,
            )
            .await?;
            Ok(())
        })
        .await
        .unwrap();
        let state:State=diesel::sql_query("SELECT external_id,status,since,missing_refreshes,last_seen_at,proposed_successor_id,successor_id,in_accepted_catalog FROM series_lifecycle WHERE external_id=$1")
            .bind::<Text,_>(old_id).get_result(&mut conn).await.unwrap();
        assert_eq!(state.missing_refreshes, refresh);
        assert_eq!(
            state.status,
            if refresh < 3 {
                "active"
            } else {
                "missing_from_catalog"
            }
        );
    }
    let unrelated_state:Option<State>=diesel::sql_query("SELECT external_id,status,since,missing_refreshes,last_seen_at,proposed_successor_id,successor_id,in_accepted_catalog FROM series_lifecycle WHERE external_id=$1")
        .bind::<Text,_>(unrelated_id).get_result(&mut conn).await.optional().unwrap();
    assert!(unrelated_state.is_none());
    #[derive(QueryableByName)]
    struct Retained {
        #[diesel(sql_type=diesel::sql_types::BigInt)]
        n: i64,
    }
    let retained: Retained =
        diesel::sql_query("SELECT count(*) AS n FROM data_points WHERE series_id=ANY($1)")
            .bind::<diesel::sql_types::Array<SqlUuid>, _>(vec![old_series, unrelated_series])
            .get_result(&mut conn)
            .await
            .unwrap();
    assert_eq!(retained.n, 2);
    drop(conn);
    db.drop().await;
}

#[derive(Debug, QueryableByName)]
struct FetchEvidence {
    #[diesel(sql_type=Text)]
    status: String,
    #[diesel(sql_type=diesel::sql_types::Integer)]
    catalog_misses: i32,
    #[diesel(sql_type=diesel::sql_types::Integer)]
    fetch_misses: i32,
    #[diesel(sql_type=diesel::sql_types::Bool)]
    fetch_retired: bool,
}

async fn fetch_evidence(pool: &DatabasePool, id: &str) -> FetchEvidence {
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query("SELECT status,missing_refreshes AS catalog_misses,fetch_not_found_count AS fetch_misses,fetch_retired
        FROM series_lifecycle WHERE external_id=$1")
        .bind::<Text,_>(id).get_result(&mut conn).await.unwrap()
}

async fn fetch_counter_db(tag: &str) -> Option<crate::persist::stable_id_tests::FreshDb> {
    use crate::{testkit::echo::EchoAdapter, DatasetCatalog};
    let url = std::env::var("DATABASE_URL").ok()?;
    let db = crate::persist::stable_id_tests::FreshDb::create(&url, tag).await;
    let mut catalog = DatasetCatalog::empty();
    catalog
        .load_adapter(&EchoAdapter::new("https://unused.invalid"))
        .unwrap();
    persist::sync_datasets(&db.pool, &catalog).await.unwrap();
    Some(db)
}

fn counter_series(id: &str) -> DiscoveredSeries {
    DiscoveredSeries {
        dataset: SeriesDataset::new("FRED", Vec::<(String, String)>::new()),
        ..found(id)
    }
}

fn successful_history() -> crate::FetchedSeries {
    let date = chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
    crate::FetchedSeries {
        metadata: None,
        points: vec![crate::FetchedPoint {
            date,
            value: Some(7.into()),
            revision_date: date,
            is_original_release: true,
        }],
        dataset: SeriesDataset::new("FRED", Vec::<(String, String)>::new()),
        validators: None,
    }
}

async fn fetch_miss(pool: &DatabasePool, id: &str) {
    let mut conn = pool.get().await.unwrap();
    conn.transaction::<(), AppError, _>(async |conn| {
        record_not_found_conn(conn, SourceId::Fred, id).await?;
        Ok(())
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn direct_fetch_success_resets_a_worker_miss_committed_while_it_waited() {
    let Some(db) = fetch_counter_db("eco464_direct_fetch_order").await else {
        return;
    };
    let pool = &db.pool;
    let id = "first-success";
    persist::persist_discovered(pool, SourceId::Fred, &[counter_series(id)])
        .await
        .unwrap();
    let source_id = persist::data_source_id(pool, SourceId::Fred).await.unwrap();
    let mut conn = pool.get().await.unwrap();
    #[derive(QueryableByName)]
    struct Pid {
        #[diesel(sql_type=diesel::sql_types::Integer)]
        pid: i32,
    }
    #[derive(QueryableByName)]
    struct Blocked {
        #[diesel(sql_type=diesel::sql_types::BigInt)]
        n: i64,
    }

    let fetch = conn.transaction::<_, AppError, _>(async |conn| {
        lock_series(conn, source_id, id).await?;
        let blocker: Pid = diesel::sql_query("SELECT pg_backend_pid() AS pid")
            .get_result(&mut *conn).await?;
        let direct_pool = pool.clone();
        let fetch = tokio::spawn(async move {
            persist::persist_series(&direct_pool, SourceId::Fred, id, &successful_history()).await
        });
        // With the old ordering, the direct fetch read zero misses before blocking on
        // its first economic_series INSERT's parent FK. This wait exposes that race.
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                // PostgreSQL caches activity statistics until this transaction ends.
                // Refresh before polling so a wait that began after our first read is visible.
                diesel::sql_query("SELECT pg_stat_clear_snapshot()")
                    .execute(&mut *conn).await.unwrap();
                let blocked: Blocked = diesel::sql_query(
                    "SELECT count(*) AS n FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid))")
                    .bind::<diesel::sql_types::Integer,_>(blocker.pid)
                    .get_result(&mut *conn).await.unwrap();
                if blocked.n > 0 { break; }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }).await.expect("the direct fetch waits for this source transaction");
        record_not_found_conn(conn, SourceId::Fred, id).await?;
        Ok(fetch)
    }).await.unwrap();
    fetch.await.unwrap().unwrap();
    let state = fetch_evidence(pool, id).await;
    assert_eq!(
        (
            state.fetch_misses,
            state.catalog_misses,
            state.status.as_str()
        ),
        (0, 0, "active")
    );
    drop(conn);
    db.drop().await;
}

#[tokio::test]
async fn successful_fetch_breaks_not_found_streak_without_catalog_changes() {
    let Some(db) = fetch_counter_db("eco464_nonconsecutive_fetch_misses").await else {
        return;
    };
    let p = &db.pool;
    persist::persist_discovered(p, SourceId::Fred, &[counter_series("intermittent")])
        .await
        .unwrap();
    for _ in 0..3 {
        fetch_miss(p, "intermittent").await;
        let e = fetch_evidence(p, "intermittent").await;
        assert_eq!(
            (
                e.status.as_str(),
                e.fetch_misses,
                e.catalog_misses,
                e.fetch_retired
            ),
            ("active", 1, 0, false)
        );
        persist::persist_series(p, SourceId::Fred, "intermittent", &successful_history())
            .await
            .unwrap();
        let e = fetch_evidence(p, "intermittent").await;
        assert_eq!(
            (
                e.status.as_str(),
                e.fetch_misses,
                e.catalog_misses,
                e.fetch_retired
            ),
            ("active", 0, 0, false)
        );
    }
    db.drop().await;
}

#[tokio::test]
async fn mixed_catalog_and_fetch_absences_never_share_their_confirmation_threshold() {
    let Some(db) = fetch_counter_db("eco464_independent_catalog_fetch_misses").await else {
        return;
    };
    let p = &db.pool;
    let baseline: Vec<_> = ["target", "a", "b", "c", "d"]
        .into_iter()
        .map(counter_series)
        .collect();
    let present = baseline[1..].to_vec();
    let mut now = Utc::now();
    let mut conn = p.get().await.unwrap();
    conn.transaction::<(), AppError, _>(async |conn| {
        persist::persist_discovered_conn(conn, SourceId::Fred, &baseline).await?;
        reconcile_conn(
            conn,
            SourceId::Fred,
            &baseline,
            &CatalogLifecycle::default(),
            "scope",
            now,
        )
        .await?;
        Ok(())
    })
    .await
    .unwrap();
    now += chrono::Duration::seconds(1);
    conn.transaction::<(), AppError, _>(async |conn| {
        reconcile_conn(
            conn,
            SourceId::Fred,
            &present,
            &CatalogLifecycle::default(),
            "scope",
            now,
        )
        .await?;
        Ok(())
    })
    .await
    .unwrap();
    fetch_miss(p, "target").await;
    fetch_miss(p, "target").await;
    let e = fetch_evidence(p, "target").await;
    assert_eq!(
        (e.status.as_str(), e.catalog_misses, e.fetch_misses),
        ("active", 1, 2),
        "one catalog miss plus two fetch misses is not three of either"
    );
    persist::persist_series(p, SourceId::Fred, "target", &successful_history())
        .await
        .unwrap();
    let e = fetch_evidence(p, "target").await;
    assert_eq!(
        (e.status.as_str(), e.catalog_misses, e.fetch_misses),
        ("active", 1, 0)
    );
    // Historical data remains fetchable while publication is gone. Successes between
    // catalog observations must not prevent the third complete-catalog absence badge.
    for n in 2..=3 {
        now += chrono::Duration::seconds(1);
        conn.transaction::<(), AppError, _>(async |conn| {
            reconcile_conn(
                conn,
                SourceId::Fred,
                &present,
                &CatalogLifecycle::default(),
                "scope",
                now,
            )
            .await?;
            Ok(())
        })
        .await
        .unwrap();
        persist::persist_series(p, SourceId::Fred, "target", &successful_history())
            .await
            .unwrap();
        let e = fetch_evidence(p, "target").await;
        assert_eq!(e.catalog_misses, n);
        assert_eq!(e.fetch_misses, 0);
        assert_eq!(
            e.status,
            if n < 3 {
                "active"
            } else {
                "missing_from_catalog"
            }
        );
    }
    for _ in 0..3 {
        fetch_miss(p, "target").await;
    }
    let e = fetch_evidence(p, "target").await;
    assert_eq!(e.status, "missing_from_catalog");
    assert!(!e.fetch_retired);
    persist::persist_series(p, SourceId::Fred, "target", &successful_history())
        .await
        .unwrap();
    let e = fetch_evidence(p, "target").await;
    assert_eq!(
        (
            e.status.as_str(),
            e.catalog_misses,
            e.fetch_misses,
            e.fetch_retired
        ),
        ("missing_from_catalog", 3, 0, false)
    );
    drop(conn);
    db.drop().await;
}

#[tokio::test]
async fn first_successful_fetch_recovers_fetch_only_retirement_but_never_legal_state() {
    let Some(db) = fetch_counter_db("eco464_fetch_only_recovery").await else {
        return;
    };
    let p = &db.pool;
    persist::persist_discovered(p, SourceId::Fred, &[counter_series("metadata-only")])
        .await
        .unwrap();
    for _ in 0..3 {
        fetch_miss(p, "metadata-only").await;
    }
    let e = fetch_evidence(p, "metadata-only").await;
    assert_eq!(
        (
            e.status.as_str(),
            e.catalog_misses,
            e.fetch_misses,
            e.fetch_retired
        ),
        ("retired", 0, 3, true)
    );
    // No economic_series row exists yet. Recovery must lock the source before the
    // lifecycle row so inserting the FK-backed series cannot invert catalog lock order.
    let written =
        persist::persist_series(p, SourceId::Fred, "metadata-only", &successful_history())
            .await
            .unwrap();
    let e = fetch_evidence(p, "metadata-only").await;
    assert_eq!(
        (
            e.status.as_str(),
            e.catalog_misses,
            e.fetch_misses,
            e.fetch_retired
        ),
        ("active", 0, 0, false)
    );
    let mut conn = p.get().await.unwrap();
    #[derive(QueryableByName)]
    struct Es {
        #[diesel(sql_type=Nullable<Text>)]
        status: Option<String>,
        #[diesel(sql_type=diesel::sql_types::Bool)]
        active: bool,
    }
    let es: Es = diesel::sql_query(
        "SELECT crawl_status AS status,is_active AS active FROM economic_series WHERE id=$1",
    )
    .bind::<SqlUuid, _>(written.series_id)
    .get_result(&mut conn)
    .await
    .unwrap();
    assert_eq!(es.status.as_deref(), Some("success"));
    assert!(es.active);
    conn.transaction::<(), AppError, _>(async |conn| {
        record_restricted_conn(conn, SourceId::Fred, "metadata-only").await?;
        Ok(())
    })
    .await
    .unwrap();
    for _ in 0..3 {
        fetch_miss(p, "metadata-only").await;
    }
    persist::persist_series(p, SourceId::Fred, "metadata-only", &successful_history())
        .await
        .unwrap();
    let e = fetch_evidence(p, "metadata-only").await;
    assert_eq!(e.status, "restricted");
    assert_eq!(e.fetch_misses, 0);
    drop(conn);
    db.drop().await;
}

#[tokio::test]
async fn catalog_confirmed_retirement_survives_fetch_success_and_rule_baseline_reset() {
    let Some(db) = fetch_counter_db("eco464_catalog_retirement_reason").await else {
        return;
    };
    let p = &db.pool;
    let baseline: Vec<_> = ["gone", "a", "b", "c", "d"]
        .into_iter()
        .map(counter_series)
        .collect();
    let present = baseline[1..].to_vec();
    let mut now = Utc::now();
    let mut conn = p.get().await.unwrap();
    conn.transaction::<(), AppError, _>(async |conn| {
        persist::persist_discovered_conn(conn, SourceId::Fred, &baseline).await?;
        reconcile_conn(
            conn,
            SourceId::Fred,
            &baseline,
            &CatalogLifecycle::default(),
            "old-scope",
            now,
        )
        .await?;
        Ok(())
    })
    .await
    .unwrap();
    let facts = CatalogLifecycle {
        confirmed_missing_ids: vec!["gone".into()],
        ..Default::default()
    };
    for _ in 0..3 {
        now += chrono::Duration::seconds(1);
        conn.transaction::<(), AppError, _>(async |conn| {
            reconcile_conn(conn, SourceId::Fred, &present, &facts, "old-scope", now).await?;
            Ok(())
        })
        .await
        .unwrap();
    }
    let e = fetch_evidence(p, "gone").await;
    assert_eq!(e.status, "retired");
    assert!(!e.fetch_retired);
    // Changing the rule fingerprint resets absence evidence without revoking a
    // previously confirmed catalog state. The explicit reason flag preserves it.
    now += chrono::Duration::seconds(1);
    conn.transaction::<(), AppError, _>(async |conn| {
        reconcile_conn(conn, SourceId::Fred, &present, &facts, "new-scope", now).await?;
        Ok(())
    })
    .await
    .unwrap();
    fetch_miss(p, "gone").await;
    persist::persist_series(p, SourceId::Fred, "gone", &successful_history())
        .await
        .unwrap();
    let e = fetch_evidence(p, "gone").await;
    assert_eq!(
        (
            e.status.as_str(),
            e.catalog_misses,
            e.fetch_misses,
            e.fetch_retired
        ),
        ("retired", 1, 0, false)
    );
    drop(conn);
    db.drop().await;
}

/// Scope members already absent when a rules baseline is taken were never listed by an
/// accepted catalog, so they must not count as a mass disappearance on the next run.
#[test]
fn absent_scope_members_at_baseline_do_not_lock_the_guard() {
    let now = Utc::now();
    let listed: Vec<_> = (0..5).map(|i| found(&format!("listed{i}"))).collect();
    let facts = CatalogLifecycle {
        scope_ids: (0..5)
            .map(|i| format!("listed{i}"))
            .chain((0..5).map(|i| format!("absent{i}")))
            .collect(),
        ..Default::default()
    };
    let (mut states, guarded) = plan(&[], &listed, &facts, false, now, LifecyclePolicy::default());
    assert!(!guarded);
    for run in 2..=5 {
        let (next, guarded) = plan(
            &states,
            &listed,
            &facts,
            true,
            now,
            LifecyclePolicy::default(),
        );
        assert!(!guarded, "run {run} was guarded");
        states = next;
    }
    let absent: Vec<_> = states
        .iter()
        .filter(|s| s.external_id.starts_with("absent"))
        .collect();
    assert_eq!(absent.len(), 5);
    assert!(absent
        .iter()
        .all(|s| s.status == "missing_from_catalog" && !s.in_accepted_catalog));
    assert!(states
        .iter()
        .filter(|s| s.external_id.starts_with("listed"))
        .all(|s| s.status == "active" && s.in_accepted_catalog));
}

async fn reconcile_at(
    pool: &DatabasePool,
    listed: &[DiscoveredSeries],
    facts: &CatalogLifecycle,
    at: DateTime<Utc>,
) -> Reconciled {
    let mut conn = pool.get().await.unwrap();
    conn.transaction::<_, AppError, _>(async |conn| {
        reconcile_conn(conn, SourceId::Fred, listed, facts, "rules", at).await
    })
    .await
    .unwrap()
}

#[derive(QueryableByName)]
struct Pending {
    #[diesel(sql_type = diesel::sql_types::Bool)]
    pending: bool,
}

async fn guarded_snapshot_pending(pool: &DatabasePool) -> bool {
    let mut conn = pool.get().await.unwrap();
    let row: Pending = diesel::sql_query(
        "SELECT guarded_snapshot IS NOT NULL AS pending FROM catalog_lifecycle_scopes",
    )
    .get_result(&mut conn)
    .await
    .unwrap();
    row.pending
}

#[tokio::test]
async fn baseline_absent_scope_members_do_not_guard_stored_catalogs() {
    let Some(db) = fetch_counter_db("eco464_baseline_absent").await else {
        return;
    };
    let pool = &db.pool;
    let listed: Vec<_> = (0..5)
        .map(|i| counter_series(&format!("listed{i}")))
        .collect();
    let facts = CatalogLifecycle {
        scope_ids: (0..5).map(|i| format!("absent{i}")).collect(),
        ..Default::default()
    };
    let start = Utc::now();
    for run in 0..5 {
        let result = reconcile_at(
            pool,
            &listed,
            &facts,
            start + chrono::Duration::seconds(run),
        )
        .await;
        assert!(!result.guarded, "run {} was guarded", run + 1);
    }
    assert!(!guarded_snapshot_pending(pool).await);
    let absent = fetch_evidence(pool, "absent0").await;
    assert_eq!(
        (absent.status.as_str(), absent.catalog_misses),
        ("missing_from_catalog", 4)
    );
    assert_eq!(fetch_evidence(pool, "listed0").await.status, "active");
    db.drop().await;
}

#[tokio::test]
async fn operator_accepts_a_reviewed_guarded_catalog() {
    let Some(db) = fetch_counter_db("eco464_accept_guarded").await else {
        return;
    };
    let pool = &db.pool;
    let all: Vec<_> = (0..10).map(|i| counter_series(&format!("s{i}"))).collect();
    let facts = CatalogLifecycle::default();
    let start = Utc::now();
    assert!(!reconcile_at(pool, &all, &facts, start).await.guarded);
    let err = review_guarded_catalog(pool, SourceId::Fred, None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("no guarded catalog snapshot"));

    let half = &all[..5];
    let guarded = reconcile_at(pool, half, &facts, start + chrono::Duration::seconds(1)).await;
    assert!(guarded.guarded);
    assert!(guarded_snapshot_pending(pool).await);

    let first = review_guarded_catalog(pool, SourceId::Fred, None)
        .await
        .unwrap();
    // A newer guarded snapshot replaces the reviewed one; accepting the old review refuses.
    assert!(
        reconcile_at(pool, half, &facts, start + chrono::Duration::seconds(2))
            .await
            .guarded
    );
    let err = review_guarded_catalog(pool, SourceId::Fred, Some(first.guarded_at))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("review it again"), "{err}");
    let review = review_guarded_catalog(pool, SourceId::Fred, None)
        .await
        .unwrap();
    assert!(review.guarded_at > first.guarded_at);
    assert_eq!((review.listed, review.previously_listed), (5, 10));
    assert_eq!(
        review.newly_missing,
        (5..10).map(|i| format!("s{i}")).collect::<Vec<_>>()
    );
    assert_eq!(fetch_evidence(pool, "s9").await.catalog_misses, 0);
    assert!(guarded_snapshot_pending(pool).await);

    let accepted = review_guarded_catalog(pool, SourceId::Fred, Some(review.guarded_at))
        .await
        .unwrap();
    assert_eq!(accepted.newly_missing.len(), 5);
    assert_eq!(accepted.transitions, 0);
    let s9 = fetch_evidence(pool, "s9").await;
    assert_eq!((s9.status.as_str(), s9.catalog_misses), ("active", 1));
    assert_eq!(fetch_evidence(pool, "s0").await.catalog_misses, 0);
    assert!(!guarded_snapshot_pending(pool).await);

    // The accepted snapshot is the new baseline: the same catalog is no longer guarded,
    // and the absent series need two more accepted absences.
    for run in 3..=4 {
        let next = reconcile_at(pool, half, &facts, start + chrono::Duration::seconds(run)).await;
        assert!(!next.guarded);
    }
    let s9 = fetch_evidence(pool, "s9").await;
    assert_eq!(
        (s9.status.as_str(), s9.catalog_misses),
        ("missing_from_catalog", 3)
    );
    db.drop().await;
}

/// The per-series lock lets different series of one source persist concurrently.
#[tokio::test]
async fn successful_fetches_of_different_series_do_not_wait_for_each_other() {
    let Some(db) = fetch_counter_db("eco464_concurrent_fetches").await else {
        return;
    };
    let pool = &db.pool;
    persist::persist_discovered(
        pool,
        SourceId::Fred,
        &[counter_series("held"), counter_series("other")],
    )
    .await
    .unwrap();
    fetch_miss(pool, "other").await;
    let mut conn = pool.get().await.unwrap();
    conn.transaction::<(), AppError, _>(async |conn| {
        persist::persist_series_conn(conn, SourceId::Fred, "held", &successful_history(), true)
            .await?;
        let other_pool = pool.clone();
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            persist::persist_series(&other_pool, SourceId::Fred, "other", &successful_history()),
        )
        .await
        .expect("a different series' fetch waited for the open transaction")
        .unwrap();
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(fetch_evidence(pool, "other").await.fetch_misses, 0);
    drop(conn);
    db.drop().await;
}

/// The reviewed case: 50 old series enrolled at adoption without being listed upstream (a
/// renamed table family), 50 renamed and 10 other listed series. No refresh may be guarded,
/// and the old series reach `missing_from_catalog` after three refreshes.
#[test]
fn a_fifty_series_rename_enrolled_at_adoption_is_badged_not_guarded() {
    let now = Utc::now();
    let old: Vec<String> = (0..50)
        .map(|i| format!("bea_regional/SAGDP2N.{i}.06000"))
        .collect();
    let current: Vec<_> = (0..50)
        .map(|i| found(&format!("bea_regional/SAGDP2.{i}.06000")))
        .chain((0..10).map(|i| found(&format!("bea_nipa/T10101.{i}.A"))))
        .collect();
    let facts = CatalogLifecycle {
        scope_ids: old.clone(),
        ..Default::default()
    };
    let (mut states, guarded) = plan(
        &[],
        &current,
        &facts,
        false,
        now,
        LifecyclePolicy::default(),
    );
    assert!(!guarded);
    assert_eq!(states.len(), 110);
    let mut guarded_runs = 0;
    for refresh in 1..=3 {
        let (next, guarded) = plan(
            &states,
            &current,
            &facts,
            true,
            now,
            LifecyclePolicy::default(),
        );
        guarded_runs += usize::from(guarded);
        states = next;
        for id in &old {
            let s = states.iter().find(|s| &s.external_id == id).unwrap();
            assert_eq!(s.missing_refreshes, refresh);
        }
    }
    assert_eq!(guarded_runs, 0);
    for id in &old {
        let s = states.iter().find(|s| &s.external_id == id).unwrap();
        assert_eq!(s.status, "missing_from_catalog");
    }
    assert!(current.iter().all(|d| {
        states
            .iter()
            .find(|s| s.external_id == d.external_id)
            .is_some_and(|s| s.status == "active")
    }));
}

/// Rule drift: a tracked series the walked catalog still lists, but the rules no longer
/// select (exact ids for FRED, table prefixes for BEA), is never absent and never trips the
/// guard; a series missing from the whole walked catalog still is.
#[test]
fn a_series_listed_upstream_but_outside_the_rules_is_never_absent() {
    let now = Utc::now();
    let baseline: Vec<_> = (0..5).map(|i| tracked(&i.to_string())).collect();
    let selected: Vec<_> = (2..5).map(|i| found(&i.to_string())).collect();
    for facts in [
        CatalogLifecycle {
            unselected_ids: vec!["0".into(), "1".into(), "not-tracked".into()],
            ..Default::default()
        },
        CatalogLifecycle {
            unselected_prefixes: vec!["0".into(), "1".into()],
            ..Default::default()
        },
    ] {
        let mut states = baseline.clone();
        states[0].status = "missing_from_catalog".into();
        states[0].missing_refreshes = 3;
        states[0].since = Some(now);
        for refresh in 1..=4 {
            // Two of five drifting out of the rules would trip the 20% guard if counted.
            let (next, guarded) = plan(
                &states,
                &selected,
                &facts,
                true,
                now,
                LifecyclePolicy::default(),
            );
            assert!(!guarded, "refresh {refresh} was guarded");
            states = next;
            for id in ["0", "1"] {
                let s = states.iter().find(|s| s.external_id == id).unwrap();
                assert_eq!(
                    (
                        s.status.as_str(),
                        s.missing_refreshes,
                        s.since,
                        s.in_accepted_catalog
                    ),
                    ("active", 0, None, true)
                );
            }
        }
        assert!(states.iter().all(|s| s.external_id != "not-tracked"));
    }
    // Once the walked catalog no longer lists it at all, absence counts again.
    let mut states = baseline.clone();
    let listed: Vec<_> = (1..5).map(|i| found(&i.to_string())).collect();
    for _ in 0..3 {
        let (next, guarded) = plan(
            &states,
            &listed,
            &CatalogLifecycle::default(),
            true,
            now,
            LifecyclePolicy::default(),
        );
        assert!(!guarded);
        states = next;
    }
    assert_eq!(states[0].status, "missing_from_catalog");
}

#[tokio::test]
async fn rule_drift_is_counted_for_review_and_keeps_publication_state() {
    let Some(db) = fetch_counter_db("eco464_rule_drift").await else {
        return;
    };
    let pool = &db.pool;
    let all: Vec<_> = (0..5)
        .map(|i| counter_series(&format!("GDP_{i}")))
        .collect();
    persist::persist_discovered(pool, SourceId::Fred, &all)
        .await
        .unwrap();
    let start = Utc::now();
    reconcile_at(pool, &all, &CatalogLifecycle::default(), start).await;
    let drifted = CatalogLifecycle {
        unselected_ids: vec!["GDP_0".into()],
        ..Default::default()
    };
    for run in 1..=4 {
        let result = reconcile_at(
            pool,
            &all[1..],
            &drifted,
            start + chrono::Duration::seconds(run),
        )
        .await;
        assert_eq!(result.unselected_tracked, Some(1));
        let e = fetch_evidence(pool, "GDP_0").await;
        assert_eq!((e.status.as_str(), e.catalog_misses), ("active", 0));
    }
    db.drop().await;
}

/// The set-based proposal query returns what the former per-series query returned: only a
/// unique listed, public match on normalized title and frequency.
#[tokio::test]
async fn successor_proposals_match_the_per_series_query() {
    let Some(db) = fetch_counter_db("eco464_successor_set").await else {
        return;
    };
    let pool = &db.pool;
    let titled = |id: &str, title: &str, frequency: &str| DiscoveredSeries {
        title: title.into(),
        frequency: Some(frequency.into()),
        ..counter_series(id)
    };
    persist::persist_discovered(
        pool,
        SourceId::Fred,
        &[
            titled("old_unique", "Gross output", "Annual"),
            titled("new_unique", "  gross OUTPUT ", "Annual"),
            titled("old_dup", "Twice", "Annual"),
            titled("new_dup1", "Twice", "Annual"),
            titled("new_dup2", "twice", "Annual"),
            titled("old_restricted", "Hidden", "Annual"),
            titled("new_restricted", "Hidden", "Annual"),
            titled("old_freq", "Rate", "Annual"),
            titled("new_freq", "Rate", "Monthly"),
            titled("old_unlisted", "Elsewhere", "Annual"),
            titled("new_unlisted", "Elsewhere", "Annual"),
        ],
    )
    .await
    .unwrap();
    // The stored history's id wins over the metadata id when both exist.
    persist::persist_series(pool, SourceId::Fred, "new_unique", &successful_history())
        .await
        .unwrap();
    let source_id = persist::data_source_id(pool, SourceId::Fred).await.unwrap();
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query("INSERT INTO series_lifecycle(source_id,external_id,status,scope_fingerprint) VALUES($1,'new_restricted','restricted','fetch-only')")
        .bind::<SqlUuid,_>(source_id).execute(&mut conn).await.unwrap();
    let ended: Vec<String> = [
        "old_unique",
        "old_dup",
        "old_restricted",
        "old_freq",
        "old_unlisted",
    ]
    .map(String::from)
    .to_vec();
    let listed: Vec<String> = [
        "new_unique",
        "new_dup1",
        "new_dup2",
        "new_restricted",
        "new_freq",
    ]
    .map(String::from)
    .to_vec();
    let proposals = successor_proposals(&mut conn, source_id, &ended, &listed)
        .await
        .unwrap();
    let mut per_series = BTreeMap::new();
    for id in &ended {
        let candidates: Vec<Uuid> = diesel::sql_query(
            "SELECT DISTINCT COALESCE(es.id,sm.id) AS id FROM series_metadata old \
             JOIN series_metadata sm ON sm.source_id=old.source_id \
                AND lower(trim(sm.title))=lower(trim(old.title)) \
                AND sm.frequency IS NOT DISTINCT FROM old.frequency \
             LEFT JOIN economic_series es ON es.source_id=sm.source_id AND es.external_id=sm.external_id \
             WHERE old.source_id=$1 AND old.external_id=$2 AND sm.external_id<>$2 \
               AND sm.external_id = ANY($3) AND NOT EXISTS (SELECT 1 FROM series_lifecycle candidate \
                   WHERE candidate.source_id=sm.source_id AND candidate.external_id=sm.external_id \
                     AND candidate.status IN ('restricted','superseded')) ORDER BY id LIMIT 2",
        )
        .bind::<SqlUuid, _>(source_id)
        .bind::<Text, _>(id)
        .bind::<diesel::sql_types::Array<Text>, _>(&listed)
        .load::<Id>(&mut conn)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.id)
        .collect();
        if candidates.len() == 1 {
            per_series.insert(id.clone(), candidates[0]);
        }
    }
    assert_eq!(proposals, per_series);
    assert_eq!(
        proposals.keys().collect::<Vec<_>>(),
        vec!["old_unique"],
        "only the unique, public, same-frequency listed match is proposed"
    );
    let stored = persist::find_series_id_conn(&mut conn, SourceId::Fred, "new_unique")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(proposals["old_unique"], stored);
    drop(conn);
    db.drop().await;
}

async fn absent_miss(pool: &DatabasePool, id: &str) {
    let mut conn = pool.get().await.unwrap();
    conn.transaction::<(), AppError, _>(async |conn| {
        record_absent_conn(conn, SourceId::Fred, id).await?;
        Ok(())
    })
    .await
    .unwrap();
}

#[derive(QueryableByName)]
struct AbsentCount {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    n: i32,
}

async fn absent_count(pool: &DatabasePool, id: &str) -> i32 {
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query("SELECT fetch_absent_count AS n FROM series_lifecycle WHERE external_id=$1")
        .bind::<Text, _>(id)
        .get_result::<AbsentCount>(&mut conn)
        .await
        .unwrap()
        .n
}

/// A line missing from complete whole-table responses (for example under a table the rules
/// no longer select) is weaker evidence: six in a row retire it from fetching, it never
/// touches the NotFound counter, and a later success recovers it.
#[tokio::test]
async fn omitted_lines_fetch_retire_only_after_six_responses() {
    let Some(db) = fetch_counter_db("eco464_absent_lines").await else {
        return;
    };
    let pool = &db.pool;
    let id = "outside_rules";
    persist::persist_discovered(pool, SourceId::Fred, &[counter_series(id)])
        .await
        .unwrap();
    for _ in 0..5 {
        absent_miss(pool, id).await;
    }
    let state = fetch_evidence(pool, id).await;
    assert_eq!(
        (
            state.status.as_str(),
            state.fetch_misses,
            state.fetch_retired
        ),
        ("active", 0, false)
    );
    assert_eq!(absent_count(pool, id).await, 5);
    absent_miss(pool, id).await;
    let state = fetch_evidence(pool, id).await;
    assert_eq!(
        (state.status.as_str(), state.fetch_retired),
        ("retired", true)
    );
    persist::persist_series(pool, SourceId::Fred, id, &successful_history())
        .await
        .unwrap();
    let state = fetch_evidence(pool, id).await;
    assert_eq!(
        (state.status.as_str(), state.fetch_retired),
        ("active", false)
    );
    assert_eq!(absent_count(pool, id).await, 0);
    db.drop().await;
}

/// When a catalog lists a fetch-retired series again, its fetch evidence restarts, and while
/// it stays listed, NotFound is not counted at all.
#[tokio::test]
async fn catalog_reactivation_resets_fetch_miss_counters() {
    let Some(db) = fetch_counter_db("eco464_catalog_reactivates").await else {
        return;
    };
    let pool = &db.pool;
    let id = "comes_back";
    let listed = vec![counter_series(id)];
    persist::persist_discovered(pool, SourceId::Fred, &listed)
        .await
        .unwrap();
    for _ in 0..3 {
        fetch_miss(pool, id).await;
    }
    absent_miss(pool, id).await;
    let state = fetch_evidence(pool, id).await;
    assert_eq!(
        (
            state.status.as_str(),
            state.fetch_retired,
            state.fetch_misses
        ),
        ("retired", true, 3)
    );
    reconcile_at(pool, &listed, &CatalogLifecycle::default(), Utc::now()).await;
    let state = fetch_evidence(pool, id).await;
    assert_eq!(
        (
            state.status.as_str(),
            state.fetch_retired,
            state.fetch_misses
        ),
        ("active", false, 0)
    );
    assert_eq!(absent_count(pool, id).await, 0);
    // Listed outright now, so the catalog alone decides: a later NotFound is not counted.
    for _ in 0..NOT_FOUND_RETIRE_AFTER {
        fetch_miss(pool, id).await;
    }
    let state = fetch_evidence(pool, id).await;
    assert_eq!((state.status.as_str(), state.fetch_misses), ("active", 0));
    db.drop().await;
}

/// A success between omissions restarts the Absent streak: 3, success, 5 never retires.
#[tokio::test]
async fn a_successful_fetch_restarts_the_absent_streak() {
    let Some(db) = fetch_counter_db("eco464_absent_streak").await else {
        return;
    };
    let pool = &db.pool;
    let id = "intermittent";
    persist::persist_discovered(pool, SourceId::Fred, &[counter_series(id)])
        .await
        .unwrap();
    for _ in 0..3 {
        absent_miss(pool, id).await;
    }
    persist::persist_series(pool, SourceId::Fred, id, &successful_history())
        .await
        .unwrap();
    assert_eq!(absent_count(pool, id).await, 0);
    for _ in 0..5 {
        absent_miss(pool, id).await;
    }
    let state = fetch_evidence(pool, id).await;
    assert_eq!(
        (state.status.as_str(), state.fetch_retired),
        ("active", false)
    );
    assert_eq!(absent_count(pool, id).await, 5);
    db.drop().await;
}

/// A series the accepted catalog lists never accumulates Absent evidence, so a selected
/// pair the catalog lists but GetData omits stays active instead of flipping between
/// fetch-retired and catalog-reactivated. An unlisted series still counts.
#[tokio::test]
async fn absent_counts_only_for_series_the_accepted_catalog_does_not_list() {
    let Some(db) = fetch_counter_db("eco464_absent_listed").await else {
        return;
    };
    let pool = &db.pool;
    let listed = vec![counter_series("listed")];
    persist::persist_discovered(
        pool,
        SourceId::Fred,
        &[counter_series("listed"), counter_series("legacy")],
    )
    .await
    .unwrap();
    reconcile_at(pool, &listed, &CatalogLifecycle::default(), Utc::now()).await;
    for _ in 0..ABSENT_RETIRE_AFTER {
        absent_miss(pool, "listed").await;
        absent_miss(pool, "legacy").await;
    }
    let state = fetch_evidence(pool, "listed").await;
    assert_eq!(
        (state.status.as_str(), state.fetch_retired),
        ("active", false)
    );
    assert_eq!(absent_count(pool, "listed").await, 0);
    let legacy = fetch_evidence(pool, "legacy").await;
    assert_eq!(
        (legacy.status.as_str(), legacy.fetch_retired),
        ("retired", true)
    );
    db.drop().await;
}

/// Weaker evidence needs more misses and at least as long a wait between them, so Absent
/// never fetch-retires a series sooner than NotFound would. Checked at compile time.
const _: () = {
    use crate::scheduler::{ABSENT_BACKOFF_FACTOR, NOT_FOUND_BACKOFF_FACTOR};
    assert!(ABSENT_RETIRE_AFTER > NOT_FOUND_RETIRE_AFTER);
    assert!(ABSENT_BACKOFF_FACTOR >= NOT_FOUND_BACKOFF_FACTOR);
    assert!(
        (ABSENT_RETIRE_AFTER as i64 - 1) * ABSENT_BACKOFF_FACTOR
            > (NOT_FOUND_RETIRE_AFTER as i64 - 1) * NOT_FOUND_BACKOFF_FACTOR
    );
};

async fn reconcile_scope(
    pool: &DatabasePool,
    listed: &[DiscoveredSeries],
    facts: &CatalogLifecycle,
    fingerprint: &str,
    at: DateTime<Utc>,
) {
    let mut conn = pool.get().await.unwrap();
    conn.transaction::<_, AppError, _>(async |conn| {
        reconcile_conn(conn, SourceId::Fred, listed, facts, fingerprint, at).await
    })
    .await
    .unwrap();
}

/// A series still published upstream under a table the rules no longer select stays in the
/// guard's baseline but is not listed, so omitted-line evidence still counts for it.
#[tokio::test]
async fn unselected_series_still_count_absent_evidence() {
    let Some(db) = fetch_counter_db("eco464_absent_unselected").await else {
        return;
    };
    let pool = &db.pool;
    let kept = counter_series("kept");
    persist::persist_discovered(
        pool,
        SourceId::Fred,
        &[kept.clone(), counter_series("drifted")],
    )
    .await
    .unwrap();
    let start = Utc::now();
    let both = vec![kept.clone(), counter_series("drifted")];
    reconcile_scope(pool, &both, &CatalogLifecycle::default(), "rules", start).await;
    let facts = CatalogLifecycle {
        unselected_ids: vec!["drifted".into()],
        ..Default::default()
    };
    reconcile_scope(
        pool,
        std::slice::from_ref(&kept),
        &facts,
        "rules",
        start + chrono::Duration::seconds(1),
    )
    .await;
    absent_miss(pool, "drifted").await;
    absent_miss(pool, "kept").await;
    assert_eq!(absent_count(pool, "drifted").await, 1);
    assert_eq!(absent_count(pool, "kept").await, 0);
    db.drop().await;
}

/// Listing by an older accepted catalog under different rules does not protect a series.
#[tokio::test]
async fn listing_under_older_rules_does_not_exempt_absent_evidence() {
    let Some(db) = fetch_counter_db("eco464_absent_old_scope").await else {
        return;
    };
    let pool = &db.pool;
    let old = counter_series("old_rules");
    let new = counter_series("new_rules");
    persist::persist_discovered(pool, SourceId::Fred, &[old.clone(), new.clone()])
        .await
        .unwrap();
    let start = Utc::now();
    reconcile_scope(
        pool,
        std::slice::from_ref(&old),
        &CatalogLifecycle::default(),
        "v1",
        start,
    )
    .await;
    reconcile_scope(
        pool,
        std::slice::from_ref(&new),
        &CatalogLifecycle::default(),
        "v2",
        start + chrono::Duration::seconds(1),
    )
    .await;
    absent_miss(pool, "old_rules").await;
    absent_miss(pool, "new_rules").await;
    assert_eq!(absent_count(pool, "old_rules").await, 1);
    assert_eq!(absent_count(pool, "new_rules").await, 0);
    db.drop().await;
}

/// A series still published outside the rules (drifted) that fetch evidence retired stays
/// retired across catalogs; only an outright listing (or a successful fetch) reactivates it.
#[tokio::test]
async fn drifted_fetch_retired_series_stays_retired_until_listed() {
    let Some(db) = fetch_counter_db("eco464_drifted_retired").await else {
        return;
    };
    let pool = &db.pool;
    let (kept, drifted) = (counter_series("kept"), counter_series("drifted"));
    persist::persist_discovered(pool, SourceId::Fred, &[kept.clone(), drifted.clone()])
        .await
        .unwrap();
    let start = Utc::now();
    let at = |n: i64| start + chrono::Duration::seconds(n);
    let both = vec![kept.clone(), drifted.clone()];
    let only_kept = vec![kept.clone()];
    let unselected = CatalogLifecycle {
        unselected_ids: vec!["drifted".into()],
        ..Default::default()
    };
    reconcile_scope(pool, &both, &CatalogLifecycle::default(), "rules", at(0)).await;
    reconcile_scope(pool, &only_kept, &unselected, "rules", at(1)).await;
    for _ in 0..NOT_FOUND_RETIRE_AFTER {
        fetch_miss(pool, "drifted").await;
    }
    assert_eq!(fetch_evidence(pool, "drifted").await.status, "retired");
    for n in 2..5 {
        reconcile_scope(pool, &only_kept, &unselected, "rules", at(n)).await;
        let state = fetch_evidence(pool, "drifted").await;
        assert_eq!(
            (
                state.status.as_str(),
                state.fetch_retired,
                state.fetch_misses
            ),
            ("retired", true, NOT_FOUND_RETIRE_AFTER),
            "catalog {n}"
        );
    }
    reconcile_scope(pool, &both, &CatalogLifecycle::default(), "rules", at(5)).await;
    let state = fetch_evidence(pool, "drifted").await;
    assert_eq!(
        (
            state.status.as_str(),
            state.fetch_retired,
            state.fetch_misses
        ),
        ("active", false, 0)
    );
    db.drop().await;
}

/// Any return to active clears stale fetch counters, even without an outright listing.
#[tokio::test]
async fn return_to_active_clears_fetch_counters() {
    let Some(db) = fetch_counter_db("eco464_return_active").await else {
        return;
    };
    let pool = &db.pool;
    let (kept, back) = (counter_series("kept"), counter_series("back"));
    persist::persist_discovered(pool, SourceId::Fred, &[kept.clone(), back.clone()])
        .await
        .unwrap();
    let start = Utc::now();
    let at = |n: i64| start + chrono::Duration::seconds(n);
    // Two series so losing one is 50%: accept each snapshot after review.
    reconcile_scope(
        pool,
        &[kept.clone(), back.clone()],
        &CatalogLifecycle::default(),
        "rules",
        at(0),
    )
    .await;
    for n in 1..=3 {
        reconcile_scope(
            pool,
            std::slice::from_ref(&kept),
            &CatalogLifecycle::default(),
            "rules",
            at(n),
        )
        .await;
        if let Ok(review) = review_guarded_catalog(pool, SourceId::Fred, None).await {
            review_guarded_catalog(pool, SourceId::Fred, Some(review.guarded_at))
                .await
                .unwrap();
        }
    }
    assert_eq!(
        fetch_evidence(pool, "back").await.status,
        "missing_from_catalog"
    );
    fetch_miss(pool, "back").await;
    fetch_miss(pool, "back").await;
    assert_eq!(fetch_evidence(pool, "back").await.fetch_misses, 2);
    let drifted = CatalogLifecycle {
        unselected_ids: vec!["back".into()],
        ..Default::default()
    };
    reconcile_scope(pool, std::slice::from_ref(&kept), &drifted, "rules", at(4)).await;
    let state = fetch_evidence(pool, "back").await;
    assert_eq!((state.status.as_str(), state.fetch_misses), ("active", 0));
    db.drop().await;
}

/// A stored row that was already inactive (for example deactivated by the earlier
/// unlisted-series sweep) and that the rules enroll as a scope member stays inactive until a
/// catalog lists it outright.
#[tokio::test]
async fn enrolled_inactive_rows_stay_inactive_until_listed() {
    let Some(db) = fetch_counter_db("eco464_inactive_enrolled").await else {
        return;
    };
    let pool = &db.pool;
    let (kept, old) = (counter_series("kept"), counter_series("old"));
    persist::persist_discovered(pool, SourceId::Fred, &[kept.clone(), old.clone()])
        .await
        .unwrap();
    for id in ["kept", "old"] {
        persist::persist_series(pool, SourceId::Fred, id, &successful_history())
            .await
            .unwrap();
    }
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query("UPDATE economic_series SET is_active=false WHERE external_id='old'")
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("UPDATE series_metadata SET is_active=false WHERE external_id='old'")
        .execute(&mut conn)
        .await
        .unwrap();
    #[derive(QueryableByName)]
    struct Active {
        #[diesel(sql_type = diesel::sql_types::Bool)]
        es: bool,
        #[diesel(sql_type = diesel::sql_types::Bool)]
        sm: bool,
    }
    const ACTIVE_OLD: &str =
        "SELECT es.is_active AS es, sm.is_active AS sm FROM economic_series es \
         JOIN series_metadata sm ON sm.source_id=es.source_id AND sm.external_id=es.external_id \
         WHERE es.external_id='old'";
    let start = Utc::now();
    let scope = CatalogLifecycle {
        scope_ids: vec!["old".into()],
        ..Default::default()
    };
    reconcile_scope(pool, std::slice::from_ref(&kept), &scope, "rules", start).await;
    let a: Active = diesel::sql_query(ACTIVE_OLD)
        .get_result(&mut conn)
        .await
        .unwrap();
    assert_eq!((a.es, a.sm), (false, false));
    reconcile_scope(
        pool,
        &[kept.clone(), old.clone()],
        &scope,
        "rules",
        start + chrono::Duration::seconds(1),
    )
    .await;
    let a: Active = diesel::sql_query(ACTIVE_OLD)
        .get_result(&mut conn)
        .await
        .unwrap();
    assert_eq!((a.es, a.sm), (true, true));
    drop(conn);
    db.drop().await;
}
