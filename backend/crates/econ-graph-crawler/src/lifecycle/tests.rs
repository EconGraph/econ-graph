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
    persist::persist_series(pool, SourceId::Fred, "new", &sample)
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
        assert_eq!(state.missing_refreshes, n);
        assert_eq!(state.status, if n < 3 { "active" } else { "retired" });
    }
    conn.transaction::<(), AppError, _>(async |conn| {
        record_restricted_conn(conn, SourceId::Fred, "retired").await
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
