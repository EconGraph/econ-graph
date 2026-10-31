//! Production-schema, verified-token parity with actual Iceberg files and a deliberately
//! divergent unselected store. Child processes isolate storage configuration without unsafe
//! mutation of the test runner's process-global environment.
use async_graphql::Request;
use chrono::{NaiveDate, TimeZone, Utc};
use diesel_async::SimpleAsyncConnection;
use econ_graph_auth::{authenticate, testkit::TestIssuer};
use econ_graph_core::{
    database::{
        create_pool, create_pool_with_size, run_migration_set, AppPool, DataPool, MigrationSet,
    },
    models::{DataSource, Observation},
    test_utils::ScratchDatabase,
};
use econ_graph_graphql::graphql::{
    context::GraphQLContext, dataloaders::DataLoaders, schema::create_schema,
};
use econ_graph_services::services::cross_section_service::{
    self, CrossSectionDate, CrossSectionRequest,
};
use econ_graph_timeseries::{SqlCatalog, StorageConfig};
use serde_json::{json, Value};
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

const EXACT: &str = "123456789012345678901234567890.123456789012345678901";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_compatibility_contract() {
    let Ok(case) = std::env::var("TRAIN4_COMPAT_CASE") else {
        for case in ["postgres", "iceberg", "budget"] {
            let warehouse = tempfile::tempdir().unwrap();
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child
                .args(["--exact", "selected_compatibility_contract", "--nocapture"])
                .env("TRAIN4_COMPAT_CASE", case)
                .env("ICEBERG_WAREHOUSE", warehouse.path())
                .env(
                    "ICEBERG_MAX_SCAN_ROWS",
                    if case == "budget" { "3" } else { "100000" },
                )
                .env(
                    "TIMESERIES_BACKEND",
                    if case == "postgres" {
                        "postgres"
                    } else {
                        "iceberg"
                    },
                )
                .env_remove("TIMESERIES_ICEBERG_MIRROR")
                .env_remove("TIMESERIES_TARGET_ID");
            if case != "postgres" {
                child.env("TIMESERIES_TARGET_ID", Uuid::new_v4().to_string());
            }
            let output = child.output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                output.status.success(),
                "{case} compatibility child failed: {stdout} {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored;"),
                "{case} child did not execute its required contract: {stdout}"
            );
        }
        return;
    };
    let app_db = ScratchDatabase::create("compat_app");
    let data_db = ScratchDatabase::create("compat_data");
    run_migration_set(&app_db.url, MigrationSet::App)
        .await
        .unwrap();
    run_migration_set(&data_db.url, MigrationSet::Data)
        .await
        .unwrap();
    let app = AppPool(create_pool(&app_db.url).await.unwrap());
    let data = create_pool(&data_db.url).await.unwrap();
    let fred = DataSource::find_by_name(&data, &DataSource::fred().name)
        .await
        .unwrap()
        .unwrap();
    let (dataset, series, canada) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let mut conn = data.get().await.unwrap();
    conn.batch_execute(&format!("INSERT INTO datasets(id,source_id,code,name,dimensions) VALUES('{dataset}','{}','compat','Compatibility','[{{\"name\":\"area\",\"label\":\"Area\",\"type\":\"string\"}}]'); INSERT INTO economic_series(id,source_id,dataset_id,external_id,title,frequency,end_date,dimensions) VALUES('{series}','{}','{dataset}','USA','Compatibility USA','Monthly','2024-03-01','{{\"area\":\"USA\"}}'),('{canada}','{}','{dataset}','CAN','Compatibility CAN','Monthly','2024-01-01','{{\"area\":\"CAN\"}}')",fred.id,fred.id,fred.id)).await.unwrap();
    let stamp = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let mut rows: Vec<Observation> = [
        (1, 1, 1, true, Some("999")),
        (1, 2, 1, true, Some(EXACT)),
        (1, 3, 1, false, None),
        (2, 2, 15, true, Some("20")),
        (3, 3, 15, true, Some("25")),
        (3, 3, 15, false, None),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (date, month, day, original, value))| Observation {
        id: Uuid::new_v5(&series, &i.to_be_bytes()),
        series_id: series,
        date: NaiveDate::from_ymd_opt(2024, date, 1).unwrap(),
        revision_date: NaiveDate::from_ymd_opt(2024, month, day).unwrap(),
        is_original_release: original,
        value: value.map(|v| v.parse().unwrap()),
        created_at: stamp,
        updated_at: stamp,
        attributes: Some(json!({"footnotes":[],"unknown":null})),
    })
    .collect();
    let mut ca = rows[0].clone();
    ca.id = Uuid::new_v5(&canada, b"observation");
    ca.series_id = canada;
    ca.value = Some(7.into());
    rows.push(ca);
    {
        use diesel_async::RunQueryDsl;
        for row in &rows {
            diesel::sql_query("INSERT INTO data_points(id,series_id,date,value,revision_date,is_original_release,created_at,updated_at,attributes) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
            .bind::<diesel::sql_types::Uuid,_>(row.id).bind::<diesel::sql_types::Uuid,_>(row.series_id)
            .bind::<diesel::sql_types::Date,_>(row.date).bind::<diesel::sql_types::Nullable<diesel::sql_types::Numeric>,_>(&row.value)
            .bind::<diesel::sql_types::Date,_>(row.revision_date).bind::<diesel::sql_types::Bool,_>(row.is_original_release)
            .bind::<diesel::sql_types::Timestamptz,_>(stamp).bind::<diesel::sql_types::Timestamptz,_>(stamp)
            .bind::<diesel::sql_types::Nullable<diesel::sql_types::Jsonb>,_>(&row.attributes).execute(&mut conn).await.unwrap();
        }
    }
    drop(conn);
    let catalog = SqlCatalog::new(data.clone(), StorageConfig::from_env().unwrap());
    // Bound ingestion as well: the budget case needs a real snapshot larger than a single
    // read budget, built from several individually valid writer batches.
    for chunk in rows.chunks(3) {
        let prepared = catalog
            .prepare_append(Uuid::new_v4(), dataset, chunk)
            .await
            .unwrap();
        let mut conn = data.get().await.unwrap();
        conn.build_transaction()
            .run::<_, anyhow::Error, _>(async |conn| {
                SqlCatalog::commit_prepared_conn(conn, &prepared).await
            })
            .await
            .unwrap();
    }
    let mut conn = data.get().await.unwrap();
    if case != "postgres" {
        let target = std::env::var("TIMESERIES_TARGET_ID").unwrap();
        conn.batch_execute(&format!("INSERT INTO timeseries_recovery_state(target_id,target_label,backend) VALUES('{target}','compatibility fixture','iceberg'); UPDATE data_points SET value=-42; DELETE FROM data_points WHERE id='{}'",rows[0].id)).await.unwrap();
    } else {
        // PostgreSQL must remain authoritative even when a different Iceberg value exists.
        let mut changed = rows[5].clone();
        changed.value = Some(777.into());
        let extra = catalog
            .prepare_append(Uuid::new_v4(), dataset, &[changed])
            .await
            .unwrap();
        conn.build_transaction()
            .run::<_, anyhow::Error, _>(async |conn| {
                SqlCatalog::commit_prepared_conn(conn, &extra).await
            })
            .await
            .unwrap();
    }
    drop(conn);
    let schema = create_schema(app.clone(), Some(DataPool(data.clone())));
    let issuer = TestIssuer::start().await;
    let token = issuer.mint_token(Uuid::new_v4(), &["catalog.fred:read"]);
    let verifier = issuer.verifier();
    let caller = authenticate(&app, Some(&verifier), Some(&format!("Bearer {token}")))
        .await
        .unwrap()
        .unwrap();
    let context = Arc::new(GraphQLContext::new(Some(caller)));
    let query = format!(
        r#"{{series(id:"{series}"){{id dataPointCount dataPoints{{id value attributes}} latest:dataPoints(filter:{{latestRevisionOnly:true}}){{value}} original:dataPoints(filter:{{originalOnly:true}}){{value}} early:dataPoints(filter:{{asOf:"2024-01-15"}}){{id}} vintage:dataPoints(filter:{{asOf:"2024-02-15"}}){{value}} transformed:dataPoints(transformation:PERCENT_CHANGE){{value}} recentDataPoints(limit:2){{date value}} latestObservation{{date value attributes}} }} crossSection(datasetId:"{dataset}",across:"area",latest:true){{key date value}} earlyCross:crossSection(datasetId:"{dataset}",across:"area",date:"2024-01-01",asOf:"2024-01-15"){{key date value}} onDate:crossSection(datasetId:"{dataset}",across:"area",date:"2024-03-01"){{key date value}} }}"#
    );
    let result = schema
        .execute(Request::new(query).data(context.clone()))
        .await;
    if case == "budget" {
        assert!(
            !result.errors.is_empty(),
            "over-budget reads must not return the PG shadow"
        );
        let narrow=schema.execute(Request::new(format!(r#"{{series(id:"{series}"){{dataPoints(filter:{{startDate:"2024-03-01",endDate:"2024-03-01"}}){{value}} zero:recentDataPoints(limit:0){{id}}}}}}"#)).data(context.clone())).await;
        assert!(narrow.errors.is_empty(), "{:?}", narrow.errors);
        assert_eq!(
            narrow.data.into_json().unwrap()["series"]["dataPoints"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        return;
    }
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let json = result.data.into_json().unwrap();
    assert_eq!(json["series"]["dataPointCount"], 6);
    assert_eq!(json["series"]["dataPoints"].as_array().unwrap().len(), 6);
    assert_eq!(json["series"]["dataPoints"][1]["value"], EXACT);
    assert_eq!(
        json["series"]["dataPoints"][1]["id"],
        rows[1].id.to_string()
    );
    assert_eq!(
        json["series"]["dataPoints"][1]["attributes"],
        rows[1].attributes.clone().unwrap()
    );
    assert_eq!(json["series"]["latest"].as_array().unwrap().len(), 3);
    assert_eq!(json["series"]["latest"][0]["value"], Value::Null);
    assert_eq!(json["series"]["original"][0]["value"], EXACT);
    assert_eq!(json["series"]["early"], json!([]));
    assert_eq!(json["series"]["vintage"][0]["value"], EXACT);
    assert_eq!(json["series"]["vintage"].as_array().unwrap().len(), 2);
    assert_eq!(json["series"]["transformed"].as_array().unwrap().len(), 3);
    assert_eq!(
        json["series"]["recentDataPoints"],
        json!([{"date":"2024-03-01","value":null},{"date":"2024-02-01","value":"20"}])
    );
    assert_eq!(json["series"]["latestObservation"]["value"], Value::Null);
    assert_eq!(json["series"]["latestObservation"]["date"], "2024-03-01");
    assert_eq!(
        json["crossSection"],
        json!([{"key":"CAN","date":"2024-01-01","value":"7"},{"key":"USA","date":"2024-02-01","value":"20"}])
    );

    assert_eq!(
        json["earlyCross"],
        json!([{"key":"CAN","date":"2024-01-01","value":"7"},{"key":"USA","date":"2024-01-01","value":null}])
    );
    assert_eq!(
        json["onDate"],
        json!([{"key":"CAN","date":"2024-03-01","value":null},{"key":"USA","date":"2024-03-01","value":null}])
    );

    let loaders = DataLoaders::new(DataPool(data.clone()));
    let (us, ca) = tokio::join!(
        loaders.data_points_by_series_loader.try_load(series),
        loaders.data_points_by_series_loader.try_load(canada)
    );
    assert_eq!(us.unwrap().len(), 3);
    assert_eq!(ca.unwrap()[0].value, Some(7.into()));
    let (us, ca) = tokio::join!(
        loaders.data_point_count_loader.try_load(series),
        loaders.data_point_count_loader.try_load(canada)
    );
    assert_eq!((us.unwrap(), ca.unwrap()), (6, 1));
    let (us, ca) = tokio::join!(
        loaders.latest_observation_loader.try_load(series),
        loaders.latest_observation_loader.try_load(canada)
    );
    assert_eq!(us.unwrap().unwrap().value, None);
    assert_eq!(ca.unwrap().unwrap().value, Some(7.into()));

    let denied_token = issuer.mint_token(Uuid::new_v4(), &["catalog.bea:read"]);
    let denied = authenticate(
        &app,
        Some(&verifier),
        Some(&format!("Bearer {denied_token}")),
    )
    .await
    .unwrap()
    .unwrap();
    let denied=schema.execute(Request::new(format!(r#"{{series(id:"{series}"){{dataPointCount dataPoints{{value}} latestObservation{{value}}}}}}"#)).data(Arc::new(GraphQLContext::new(Some(denied))))).await;
    assert!(denied.errors.is_empty());
    assert_eq!(denied.data.into_json().unwrap()["series"], Value::Null);

    // The legacy direct field is not the paginated store's 10000-point page. Exercise
    // an actual API result above that boundary without changing the bounded scan budget.
    let (large_dataset, large_series) = (Uuid::new_v4(), Uuid::new_v4());
    let mut conn = data.get().await.unwrap();
    conn.batch_execute(&format!("INSERT INTO datasets(id,source_id,code,name) VALUES('{large_dataset}','{}','large','Large compatibility'); INSERT INTO economic_series(id,source_id,dataset_id,external_id,title,frequency,end_date) VALUES('{large_series}','{}','{large_dataset}','LARGE','Large compatibility','Daily','1927-05-20')", fred.id,fred.id)).await.unwrap();
    if case == "postgres" {
        conn.batch_execute(&format!("INSERT INTO data_points(series_id,date,revision_date,value) SELECT '{large_series}', DATE '1900-01-01' + n, DATE '1900-01-01' + n, 1 FROM generate_series(0,10000) n")).await.unwrap();
    }
    drop(conn);
    if case == "iceberg" {
        let start = NaiveDate::from_ymd_opt(1900, 1, 1).unwrap();
        let large: Vec<_> = (0_i64..10001)
            .map(|i| {
                let mut row = rows[0].clone();
                row.id = Uuid::new_v5(&large_series, &i.to_be_bytes());
                row.series_id = large_series;
                row.date = start + chrono::Duration::days(i);
                row.revision_date = row.date;
                row.value = Some(1.into());
                row
            })
            .collect();
        let prepared = catalog
            .prepare_append(Uuid::new_v4(), large_dataset, &large)
            .await
            .unwrap();
        let mut conn = data.get().await.unwrap();
        conn.build_transaction()
            .run::<_, anyhow::Error, _>(async |conn| {
                SqlCatalog::commit_prepared_conn(conn, &prepared).await
            })
            .await
            .unwrap();
    }
    let large = schema
        .execute(
            Request::new(format!(
                r#"{{series(id:"{large_series}"){{dataPointCount dataPoints{{id}}}}}}"#
            ))
            .data(context.clone()),
        )
        .await;
    assert!(large.errors.is_empty(), "{:?}", large.errors);
    let large = large.data.into_json().unwrap();
    assert_eq!(large["series"]["dataPointCount"], 10001);
    assert_eq!(
        large["series"]["dataPoints"].as_array().unwrap().len(),
        10001
    );

    if case == "iceberg" {
        // A one-connection compatibility reader must never check out a nested connection.
        // Publishing a newer pointer after its RR snapshot starts cannot retarget that read.
        let mut changed = rows[5].clone();
        changed.value = Some(777.into());
        let next = catalog
            .prepare_append(Uuid::new_v4(), dataset, &[changed])
            .await
            .unwrap();
        let one = create_pool_with_size(&data_db.url, 1).await.unwrap();
        let mut pinned = one.get().await.unwrap();
        let areas = econ_graph_core::reference::areas().unwrap();
        let request = CrossSectionRequest {
            dataset_id: dataset,
            measure: None,
            filter: BTreeMap::new(),
            across: "area".into(),
            date: CrossSectionDate::Latest,
            as_of: None,
        };
        pinned
            .build_transaction()
            .repeatable_read()
            .read_only()
            .run::<_, anyhow::Error, _>(async |conn| {
                SqlCatalog::pointer_conn(conn, dataset).await?.unwrap();
                let mut writer = data.get().await?;
                writer
                    .build_transaction()
                    .run::<_, anyhow::Error, _>(async |conn| {
                        SqlCatalog::commit_prepared_conn(conn, &next).await
                    })
                    .await?;
                let old = cross_section_service::cross_section(conn, areas, &request).await?;
                assert_eq!(
                    old.iter().find(|r| r.key == "USA").unwrap().value,
                    Some(20.into())
                );
                Ok(())
            })
            .await
            .unwrap();
        drop(pinned);
        drop(one);
    }
    // Even an existing loader must revalidate the durable selection, not return cached counts.
    let mut conn = data.get().await.unwrap();
    if case == "postgres" {
        conn.batch_execute(&format!("INSERT INTO timeseries_recovery_state(target_id,target_label,backend) VALUES('{}','stale process','iceberg')",Uuid::new_v4())).await.unwrap();
    } else {
        conn.batch_execute("UPDATE timeseries_recovery_state SET backend='postgres'")
            .await
            .unwrap();
    }
    drop(conn);
    assert!(loaders
        .data_point_count_loader
        .try_load(series)
        .await
        .is_err());
    assert!(loaders
        .latest_observation_loader
        .try_load(series)
        .await
        .is_err());
    assert!(loaders
        .data_points_by_series_loader
        .try_load(series)
        .await
        .is_err());
}
