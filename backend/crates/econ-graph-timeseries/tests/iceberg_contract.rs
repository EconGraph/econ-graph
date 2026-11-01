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
fn encoding_rejects_lossy_timestamps_and_nonobject_attributes() -> Result<()> {
    use chrono::Timelike;
    let schema = encoding::schema()?;
    let mut row = rows(Uuid::new_v4())[0].clone();
    row.created_at = row.created_at.with_nanosecond(123456789).unwrap();
    assert!(encoding::to_batch(&schema, &[row.clone()], 1).is_err());
    row.created_at = row.created_at.with_nanosecond(123456000).unwrap();
    for attributes in [
        Some(serde_json::Value::Null),
        Some(serde_json::json!([])),
        Some(serde_json::json!("bad")),
    ] {
        row.attributes = attributes;
        assert!(encoding::to_batch(&schema, &[row.clone()], 1).is_err());
    }
    for attributes in [
        None,
        Some(serde_json::json!({})),
        Some(serde_json::json!({"nested":{"explicit":null},"ordered":["b","a"]})),
    ] {
        row.attributes = attributes;
        let decoded = encoding::from_batches(&[encoding::to_batch(&schema, &[row.clone()], 1)?])?;
        assert_eq!(decoded[0], row);
    }
    Ok(())
}
#[test]
fn local_config_rejects_relative_warehouse() {
    assert!(StorageConfig::local("relative").file_io().is_err());
}

#[test]
fn query_limits_reject_unbounded_or_invalid_budgets() -> Result<()> {
    use econ_graph_timeseries::query::QueryLimits;
    let defaults = QueryLimits::from_lookup(|_| None)?;
    assert_eq!(defaults.max_rows, 100_000);
    assert_eq!(defaults.memory_bytes, 256 * 1024 * 1024);
    for value in ["0", "invalid", &usize::MAX.to_string()] {
        assert!(QueryLimits::from_lookup(
            |name| (name == "ICEBERG_MAX_SCAN_ROWS").then(|| value.into())
        )
        .is_err());
    }
    assert!(QueryLimits::from_lookup(
        |name| (name == "ICEBERG_QUERY_MEMORY_BYTES").then(|| "1023".into())
    )
    .is_err());
    Ok(())
}
#[test]
fn endpoint_validation_is_pure_and_redacts_credential_input() -> Result<()> {
    for endpoint in [
        "https://user:credential-sentinel@example.test",
        "https://example.test?token=credential-sentinel",
        "https://example.test#credential-sentinel",
        "ftp://example.test",
    ] {
        let result = StorageConfig::from_lookup(|name| match name {
            "ICEBERG_WAREHOUSE" => Some("s3://bucket/prefix".into()),
            "S3_ENDPOINT" => Some(endpoint.into()),
            _ => None,
        });
        let error = result
            .err()
            .expect("invalid endpoint must be rejected")
            .to_string();
        assert!(
            !error.contains("credential-sentinel"),
            "error exposed URL credentials"
        );
    }
    assert!(StorageConfig::from_lookup(|name| match name {
        "ICEBERG_WAREHOUSE" => Some("s3://bucket/prefix".into()),
        "S3_ENDPOINT" => Some("http://localhost:9000".into()),
        _ => None,
    })
    .is_err());
    assert!(StorageConfig::from_lookup(|name| match name {
        "ICEBERG_WAREHOUSE" => Some("s3://bucket/prefix".into()),
        "S3_ENDPOINT" => Some("http://localhost:9000".into()),
        "AWS_ALLOW_HTTP" => Some("true".into()),
        _ => None,
    })
    .is_ok());
    Ok(())
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
    assert!(
        SqlCatalog::read_series_at_config(
            &catalog.config,
            &prepared.next,
            &[series],
            None,
            None,
            5
        )
        .await
        .is_err(),
        "bounded snapshot must error instead of truncating"
    );
    assert_eq!(
        SqlCatalog::read_series_at_config(
            &catalog.config,
            &prepared.next,
            &[series],
            None,
            None,
            6
        )
        .await?
        .len(),
        6
    );
    assert_eq!(
        catalog
            .read_series_at(
                &prepared.next,
                &[series],
                Some(NaiveDate::from_ymd_opt(2020, 2, 1).unwrap()),
                Some(NaiveDate::from_ymd_opt(2020, 2, 1).unwrap()),
                1
            )
            .await?
            .len(),
        1
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
    let page = store.read_page(request(series, RevisionMode::All)).await?;
    assert_eq!(
        page.revised.get(&rows[0].id),
        None,
        "later real original is unknown at the synthetic vintage"
    );
    assert_eq!(page.revised.get(&rows[1].id), Some(&false));
    assert_eq!(
        page.revised.get(&rows[2].id),
        Some(&true),
        "NULL differs from a known numeric original"
    );
    assert_eq!(page.revised.get(&rows[3].id), Some(&false));
    assert_eq!(
        page.revised.get(&rows[5].id),
        Some(&false),
        "equal exact numeric values are unchanged"
    );
    let mut conn = pool.get().await?;
    diesel::sql_query("INSERT INTO series_lifecycle(source_id,external_id,status,scope_fingerprint) VALUES($1,'contract','restricted','contract')")
        .bind::<diesel::sql_types::Uuid, _>(source).execute(&mut conn).await?;
    drop(conn);
    let restricted = store.read_page(request(series, RevisionMode::All)).await?;
    assert!(restricted.dataset_id.is_none());
    assert!(restricted.points.is_empty());
    assert!(restricted.context.is_empty());
    assert!(restricted.revised.is_empty());
    assert_eq!(restricted.total_count, 0);
    let mut conn = pool.get().await?;
    diesel::sql_query("DELETE FROM series_lifecycle WHERE source_id=$1 AND external_id='contract'")
        .bind::<diesel::sql_types::Uuid, _>(source)
        .execute(&mut conn)
        .await?;
    drop(conn);
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
    assert!(
        catalog
            .prepare_append(Uuid::new_v4(), dataset, &[rows[0].clone(), rows[0].clone()])
            .await
            .is_err(),
        "one batch cannot repeat an observation UUID"
    );
    let mut changed = rows[0].clone();
    changed.value = Some("12".parse()?);
    assert!(
        catalog
            .prepare_append(commit, dataset, &[changed])
            .await
            .is_err(),
        "a committed UUID cannot acknowledge different content"
    );
    let other_series = Uuid::new_v4();
    let mut conn = pool.get().await?;
    diesel::sql_query("INSERT INTO economic_series(id,source_id,external_id,title,frequency,dataset_id) VALUES($1,$2,'other','Other','monthly',$3)").bind::<diesel::sql_types::Uuid,_>(other_series).bind::<diesel::sql_types::Uuid,_>(source).bind::<diesel::sql_types::Uuid,_>(dataset).execute(&mut conn).await?;
    drop(conn);
    let mut retargeted = rows[0].clone();
    retargeted.series_id = other_series;
    assert!(
        catalog
            .prepare_append(Uuid::new_v4(), dataset, &[retargeted])
            .await
            .is_err(),
        "observation ID cannot move between series"
    );
    for field in 0..3 {
        let mut retargeted = rows[0].clone();
        match field {
            0 => retargeted.date += chrono::Duration::days(1),
            1 => retargeted.revision_date += chrono::Duration::days(1),
            _ => retargeted.is_original_release = !retargeted.is_original_release,
        }
        assert!(
            catalog
                .prepare_append(Uuid::new_v4(), dataset, &[retargeted])
                .await
                .is_err(),
            "observation natural identity is immutable"
        );
    }
    assert_eq!(
        store
            .read_page(request(series, RevisionMode::All))
            .await?
            .total_count,
        6
    );
    assert_eq!(
        store
            .read_page(request(other_series, RevisionMode::All))
            .await?
            .total_count,
        0
    );
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
            "../../../migrations/data/2026-11-01-000120_iceberg_sql_catalog/down.sql"
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
    let plan = catalog.maintenance_plan(dataset).await?;
    assert_eq!(plan.physical_rows, 7);
    assert_eq!(plan.logical_rows, 6);
    assert!(plan.compact_recommended);
    assert!(catalog
        .prepare_compaction(Uuid::new_v4(), dataset, 5)
        .await
        .is_err());
    let compact = catalog
        .prepare_compaction(Uuid::new_v4(), dataset, 6)
        .await?;
    let mut conn = pool.get().await?;
    conn.build_transaction()
        .run::<_, anyhow::Error, _>(async |conn| {
            SqlCatalog::commit_prepared_conn(conn, &compact).await
        })
        .await?;
    drop(conn);
    assert_eq!(
        encoding::canonical_rows(&catalog.read_rows(&compact.next).await?)?,
        encoding::canonical_rows(&latest)?
    );
    let plan = catalog.maintenance_plan(dataset).await?;
    assert_eq!(plan.physical_rows, 6);
    assert!(!plan.compact_recommended);
    assert_eq!(
        catalog
            .load_at(&compact.next)
            .await?
            .metadata()
            .current_snapshot()
            .unwrap()
            .summary()
            .operation,
        iceberg::spec::Operation::Replace
    );
    let expiry = catalog
        .prepare_snapshot_expiry(
            Uuid::new_v4(),
            dataset,
            Utc::now().timestamp_millis() + 1000,
            1,
        )
        .await;
    assert!(expiry.is_err(), "future cutoff is refused");
    let expiry = catalog
        .prepare_snapshot_expiry(Uuid::new_v4(), dataset, Utc::now().timestamp_millis(), 1)
        .await?;
    let mut conn = pool.get().await?;
    conn.build_transaction()
        .run::<_, anyhow::Error, _>(async |conn| {
            SqlCatalog::commit_prepared_conn(conn, &expiry).await
        })
        .await?;
    drop(conn);
    assert_eq!(
        catalog
            .load_at(&expiry.next)
            .await?
            .metadata()
            .snapshots()
            .len(),
        1
    );
    assert_eq!(
        encoding::canonical_rows(&catalog.read_rows(&expiry.next).await?)?,
        encoding::canonical_rows(&latest)?
    );
    let retained_objects = catalog.reachable_objects(&expiry.next).await?;
    // Shared snapshots repeat their manifest lists/manifests throughout metadata history.
    // Traversal may skip repeated loads, but must retain every previously reachable object
    // and its independent integrity digest even after compaction and snapshot expiry.
    for historical in &objects {
        let retained = retained_objects
            .iter()
            .find(|o| o.location == historical.location)
            .expect("historical recovery object must remain reachable");
        assert_eq!(retained.size_bytes, historical.size_bytes);
        assert_eq!(retained.sha256, historical.sha256);
    }
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
        &expiry.next
    );
    Ok(())
}
