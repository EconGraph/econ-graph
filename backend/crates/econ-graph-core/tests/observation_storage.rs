//! Requires disposable PostgreSQL via DATABASE_URL; exercises the real recovery SQL API.
use diesel::connection::SimpleConnection;
use econ_graph_core::observation_storage::{ObservationBackend, ObservationStorage};
use econ_graph_core::test_utils::ScratchDatabase;
use econ_graph_core::{create_pool, run_migration_set, MigrationSet};
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn readers_and_writers_refuse_stale_targets_but_pause_only_blocks_writes() {
    let db = ScratchDatabase::create("train4_selection");
    run_migration_set(&db.url, MigrationSet::Data)
        .await
        .unwrap();
    let pool = create_pool(&db.url).await.unwrap();
    let mut conn = pool.get().await.unwrap();
    let default = ObservationStorage {
        backend: ObservationBackend::Postgres,
        target_id: None,
        mirror_iceberg: false,
    };
    default.assert_read_selection_conn(&mut conn).await.unwrap();
    assert_eq!(
        default.assert_write_allowed_conn(&mut conn).await.unwrap(),
        0
    );

    let target = Uuid::new_v4();
    db.connect().batch_execute(&format!("INSERT INTO timeseries_recovery_state(target_id, target_label) VALUES ('{target}', 'disposable selection test')")).unwrap();
    assert!(default.assert_read_selection_conn(&mut conn).await.is_err());
    assert!(default.assert_write_allowed_conn(&mut conn).await.is_err());
    let matching = ObservationStorage {
        target_id: Some(target),
        ..default
    };
    matching
        .assert_read_selection_conn(&mut conn)
        .await
        .unwrap();
    matching.assert_write_allowed_conn(&mut conn).await.unwrap();
    let wrong = ObservationStorage {
        target_id: Some(Uuid::new_v4()),
        ..matching
    };
    assert!(wrong.assert_read_selection_conn(&mut conn).await.is_err());
    assert!(wrong.assert_write_allowed_conn(&mut conn).await.is_err());

    db.connect()
        .batch_execute(&format!(
            "UPDATE timeseries_recovery_state SET writes_paused=true, pause_token='{}', epoch=1",
            Uuid::new_v4()
        ))
        .unwrap();
    matching
        .assert_read_selection_conn(&mut conn)
        .await
        .unwrap();
    assert!(matching.assert_write_allowed_conn(&mut conn).await.is_err());
    db.connect()
        .batch_execute("UPDATE timeseries_recovery_state SET backend='iceberg', epoch=2")
        .unwrap();
    assert!(matching
        .assert_read_selection_conn(&mut conn)
        .await
        .is_err());
    let iceberg = ObservationStorage {
        backend: ObservationBackend::Iceberg,
        mirror_iceberg: true,
        ..matching
    };
    iceberg.assert_read_selection_conn(&mut conn).await.unwrap();
    assert!(iceberg.assert_write_allowed_conn(&mut conn).await.is_err());
    db.connect()
        .batch_execute(
            "UPDATE timeseries_recovery_state SET writes_paused=false, pause_token=NULL, epoch=3",
        )
        .unwrap();
    assert_eq!(
        iceberg.assert_write_allowed_conn(&mut conn).await.unwrap(),
        3
    );
    drop(conn);
    drop(pool);
}
