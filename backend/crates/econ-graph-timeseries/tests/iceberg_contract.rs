use anyhow::Result;
use chrono::{NaiveDate, TimeZone, Utc};
use diesel::prelude::*;
use diesel_async::{RunQueryDsl, SimpleAsyncConnection};
use econ_graph_core::{
    database::{create_pool, run_migration_set, MigrationSet},
    models::{Observation, ObservationRequest, PageContext, RevisionMode, TimeSeriesStore},
};
use econ_graph_timeseries::{encoding, IcebergStore, SqlCatalog, StorageConfig};
use uuid::Uuid;

fn rows(series: Uuid) -> Vec<Observation> {
    let timestamp = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
    [(1,1,true,Some("999")),(1,2,true,Some("12345678901234567890123456789012345678901234567890.1234567890123456789")),(1,3,false,None),(2,2,true,Some("0")),(3,3,true,Some("0.000000000000000000000000000000000000000000000000000000000001")),(3,3,false,Some("0.000000000000000000000000000000000000000000000000000000000001"))]
    .into_iter().enumerate().map(|(i,(date,revision,original,value))|serde_json::from_value(serde_json::json!({"id":Uuid::new_v5(&series,&i.to_be_bytes()),"series_id":series,"date":NaiveDate::from_ymd_opt(2020,date,1).unwrap(),"revision_date":NaiveDate::from_ymd_opt(2020,revision,1).unwrap(),"value":value,"is_original_release":original,"created_at":timestamp,"updated_at":timestamp,"attributes":{"source":"fixture","footnotes":[],"preliminary":false,"extra":{"unknown":null}}})).unwrap()).collect()
}
fn request(series: Uuid, mode: RevisionMode) -> ObservationRequest {
    ObservationRequest {
        series_id: series,
        start_date: None,
        end_date: None,
        revisions: mode,
        limit: None,
        offset: None,
        context: None,
    }
}

#[test]
fn arrow_encoding_is_lossless_for_exact_values_and_attributes() -> Result<()> {
    let rows = rows(Uuid::new_v4());
    let schema = encoding::schema()?;
    let batch = encoding::to_batch(&schema, &rows, 7)?;
    let round = encoding::from_batches(&[batch])?;
    assert_eq!(
        encoding::canonical_rows(&round)?,
        encoding::canonical_rows(&rows)?
    );
    Ok(())
}
#[test]
fn local_config_rejects_relative_warehouse() {
    assert!(StorageConfig::local("relative").file_io().is_err());
}

#[tokio::test]
#[ignore = "requires explicitly disposable DATA_DATABASE_URL, creates real Iceberg objects in tempdir"]
async fn real_iceberg_roundtrip_parity_upserts_conflicts_and_atomicity() -> Result<()> {
    let url = std::env::var("DATA_DATABASE_URL")
        .expect("DATA_DATABASE_URL required for explicit Iceberg integration");
    run_migration_set(&url, MigrationSet::Data).await?;
    let pool = create_pool(&url).await?;
    let temp = tempfile::tempdir()?;
    let catalog = SqlCatalog::new(
        pool.clone(),
        StorageConfig::local(temp.path().to_str().unwrap()),
    );
    let source = Uuid::new_v4();
    let dataset = Uuid::new_v4();
    let series = Uuid::new_v4();
    let mut conn = pool.get().await?;
    conn.batch_execute(&format!("INSERT INTO data_sources(id,name,description,base_url) VALUES('{source}','Iceberg contract {source}','test','https://example.test'); INSERT INTO datasets(id,source_id,code,name,dimensions) VALUES('{dataset}','{source}','contract','Contract','[{{\"name\":\"area\",\"label\":\"Area\",\"type\":\"string\"}}]'); INSERT INTO economic_series(id,source_id,external_id,title,frequency,dataset_id,dimensions) VALUES('{series}','{source}','contract','Contract','monthly','{dataset}','{{\"area\":\"USA\"}}')")).await?;
    let rows = rows(series);
    for r in &rows {
        diesel::sql_query("INSERT INTO data_points(id,series_id,date,revision_date,value,is_original_release,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
            .bind::<diesel::sql_types::Uuid,_>(r.id).bind::<diesel::sql_types::Uuid,_>(series).bind::<diesel::sql_types::Date,_>(r.date).bind::<diesel::sql_types::Date,_>(r.revision_date).bind::<diesel::sql_types::Nullable<diesel::sql_types::Numeric>,_>(&r.value).bind::<diesel::sql_types::Bool,_>(r.is_original_release).bind::<diesel::sql_types::Timestamptz,_>(r.created_at).bind::<diesel::sql_types::Timestamptz,_>(r.updated_at).execute(&mut conn).await?;
    }
    drop(conn);
    let commit = Uuid::new_v4();
    let prepared = catalog.prepare_append(commit, dataset, &rows).await?;
    // A crashed writer cannot publish by merely writing files and metadata.
    assert_eq!(catalog.pointer(dataset).await?.unwrap().generation, 0);
    let mut outside = pool.get().await?;
    assert!(SqlCatalog::commit_prepared_conn(&mut outside, &prepared)
        .await
        .is_err());
    drop(outside);

    assert!(catalog
        .read_rows(&catalog.pointer(dataset).await?.unwrap())
        .await?
        .is_empty());
    let mut conn = pool.get().await?;
    conn.build_transaction()
        .run::<_, anyhow::Error, _>(async |conn| {
            SqlCatalog::commit_prepared_conn(conn, &prepared).await
        })
        .await?;
    drop(conn);
    assert_eq!(
        encoding::canonical_rows(&catalog.read_rows(&prepared.next).await?)?,
        encoding::canonical_rows(&rows)?
    );
    let store = IcebergStore::new(catalog.clone());
    for mode in [
        RevisionMode::All,
        RevisionMode::Latest,
        RevisionMode::Original,
        RevisionMode::OriginalLatest,
        RevisionMode::AsOf(NaiveDate::from_ymd_opt(2020, 2, 1).unwrap()),
        RevisionMode::OriginalAsOf(NaiveDate::from_ymd_opt(2020, 2, 1).unwrap()),
    ] {
        let page = store.read_page(request(series, mode)).await?;
        assert_eq!(
            page.total_count,
            if mode == RevisionMode::All {
                6
            } else if matches!(mode, RevisionMode::AsOf(_) | RevisionMode::OriginalAsOf(_)) {
                2
            } else {
                3
            }
        );
        assert_eq!(page.dataset_id, Some(dataset));
    }
    for context in [
        PageContext::PreviousPoint,
        PageContext::FirstPoint,
        PageContext::Days(32),
    ] {
        let mut req = request(series, RevisionMode::All);
        req.offset = Some(3);
        req.limit = Some(1);
        req.context = Some(context);
        let page = store.read_page(req).await?;
        assert_eq!(page.total_count, 6);
        assert_eq!(page.points.len(), 1);
        assert_eq!(
            page.context.len(),
            if matches!(context, PageContext::Days(_)) {
                3
            } else {
                1
            }
        );
    }
    // Identical committed retries do not append duplicates or advance the catalog.
    let repeat = catalog.prepare_append(commit, dataset, &rows).await?;
    let mut conn = pool.get().await?;
    let changed = conn
        .build_transaction()
        .run::<_, anyhow::Error, _>(async |conn| {
            SqlCatalog::commit_prepared_conn(conn, &repeat).await
        })
        .await?;
    assert!(!changed);
    drop(conn);
    // Two writers based on the same pointer: only one can publish. A conflicting pointer
    // cannot partially update the caller's series summary in the surrounding transaction.
    let mut correction = rows[1].clone();
    correction.value = Some("42.00000000000000000000000000000000000001".parse()?);
    let left = catalog
        .prepare_append(Uuid::new_v4(), dataset, &[correction.clone()])
        .await?;
    let right = catalog
        .prepare_append(Uuid::new_v4(), dataset, &[rows[0].clone()])
        .await?;
    let mut conn = pool.get().await?;
    conn.build_transaction()
        .run::<_, anyhow::Error, _>(async |conn| {
            SqlCatalog::commit_prepared_conn(conn, &left).await
        })
        .await?;
    let failed = conn
        .build_transaction()
        .run::<_, anyhow::Error, _>(async |conn| {
            diesel::sql_query("UPDATE economic_series SET title='must rollback' WHERE id=$1")
                .bind::<diesel::sql_types::Uuid, _>(series)
                .execute(conn)
                .await?;
            SqlCatalog::commit_prepared_conn(conn, &right).await
        })
        .await;
    assert!(failed.is_err());
    let title = econ_graph_core::schema::economic_series::table
        .find(series)
        .select(econ_graph_core::schema::economic_series::title)
        .first::<String>(&mut conn)
        .await?;
    assert_eq!(title, "Contract");
    drop(conn);
    let objects = catalog.reachable_objects(&left.next).await?;
    assert!(objects.iter().any(|o| o.location.ends_with(".parquet")));
    assert!(objects.iter().any(|o| o.location.ends_with(".avro")));
    assert!(objects
        .iter()
        .any(|o| o.location == prepared.next.metadata_location));
    assert!(objects
        .iter()
        .all(|o| o.sha256.len() == 64 && o.size_bytes > 0));
    let mut down_conn = pool.get().await?;
    assert!(down_conn
        .batch_execute(include_str!(
            "../../../migrations/data/2026-10-31-020000_iceberg_sql_catalog/down.sql"
        ))
        .await
        .is_err());
    drop(down_conn);
    let latest = catalog.read_rows(&left.next).await?;
    assert_eq!(latest.len(), 6);
    assert_eq!(
        latest.iter().find(|r| r.id == correction.id).unwrap().value,
        correction.value
    );
    // A read pinned before the correction keeps its original exact value and metadata.
    assert_eq!(
        encoding::canonical_rows(&catalog.read_rows(&prepared.next).await?)?,
        encoding::canonical_rows(&rows)?
    );
    assert_eq!(
        catalog
            .checkpoint()
            .await?
            .tables
            .iter()
            .find(|p| p.dataset_id == dataset)
            .unwrap(),
        &left.next
    );
    Ok(())
}
