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
        confirmed_missing_ids: vec![],
        scope_ids: vec![],
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
    let states:Vec<State>=diesel::sql_query("SELECT external_id,status,since,missing_refreshes,last_seen_at,proposed_successor_id,successor_id FROM series_lifecycle WHERE external_id='0'").load(&mut conn).await.unwrap();
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
    let confirmed:State=diesel::sql_query("SELECT external_id,status,since,missing_refreshes,last_seen_at,proposed_successor_id,successor_id FROM series_lifecycle WHERE external_id='0'").get_result(&mut conn).await.unwrap();
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
        let state:State=diesel::sql_query("SELECT external_id,status,since,missing_refreshes,last_seen_at,proposed_successor_id,successor_id FROM series_lifecycle WHERE external_id='retired'").get_result(&mut conn).await.unwrap();
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
    let rows:Vec<State>=diesel::sql_query("SELECT external_id,status,since,missing_refreshes,last_seen_at,proposed_successor_id,successor_id FROM series_lifecycle ORDER BY external_id").load(&mut conn).await.unwrap();
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
    let enrolled:State=diesel::sql_query("SELECT external_id,status,since,missing_refreshes,last_seen_at,proposed_successor_id,successor_id FROM series_lifecycle WHERE external_id=$1")
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
        let state:State=diesel::sql_query("SELECT external_id,status,since,missing_refreshes,last_seen_at,proposed_successor_id,successor_id FROM series_lifecycle WHERE external_id=$1")
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
    let unrelated_state:Option<State>=diesel::sql_query("SELECT external_id,status,since,missing_refreshes,last_seen_at,proposed_successor_id,successor_id FROM series_lifecycle WHERE external_id=$1")
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
        diesel::sql_query("SELECT id FROM data_sources WHERE id=$1 FOR UPDATE")
            .bind::<SqlUuid,_>(source_id).execute(&mut *conn).await?;
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
