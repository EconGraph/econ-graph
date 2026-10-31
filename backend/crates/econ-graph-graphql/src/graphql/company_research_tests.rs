//! Real data-plane regression tests; every fixture owns an independently migrated DB.
use super::*;
use async_graphql::{Request, Variables};
use diesel::sql_types::{Date, Nullable, Numeric, Text, Uuid as SqlUuid};
use diesel_async::RunQueryDsl;
use econ_graph_core::database::{
    create_pool, run_migration_set, AppPool, DataPool, DatabasePool, MigrationSet,
};
use econ_graph_core::test_utils::ScratchDatabase;
use serde_json::{json, Value};

fn unreachable_pool() -> DatabasePool {
    let manager = diesel_async::pooled_connection::AsyncDieselConnectionManager::<
        diesel_async::AsyncPgConnection,
    >::new("postgres://nobody@127.0.0.1:1/none");
    DatabasePool::builder()
        .connection_timeout(std::time::Duration::from_secs(1))
        .build_unchecked(manager)
}
fn schema(
    data: Option<DatabasePool>,
    granted: bool,
) -> async_graphql::Schema<
    super::super::query::Query,
    super::super::mutation::Mutation,
    async_graphql::EmptySubscription,
> {
    let auth = GraphQLContext::anonymous().with_anonymous_catalog_roles(if granted {
        vec![Role::CatalogSecRead]
    } else {
        vec![]
    });
    super::super::schema::create_schema_with_data(
        AppPool(unreachable_pool()),
        data.map(DataPool),
        Arc::new(auth),
    )
}
async fn result(
    schema: &async_graphql::Schema<
        super::super::query::Query,
        super::super::mutation::Mutation,
        async_graphql::EmptySubscription,
    >,
    query: &str,
    variables: Value,
) -> Value {
    let response = schema
        .execute(Request::new(query).variables(Variables::from_json(variables)))
        .await;
    assert!(response.errors.is_empty(), "{query}: {:?}", response.errors);
    response.data.into_json().unwrap()
}

#[tokio::test]
async fn company_research_gate_and_permission_checks_precede_database_access() {
    let queries = [
        r#"{ companies { nodes { cik } } }"#,
        r#"{ company(cik:"320193") { cik } }"#,
        r#"{ companyFacts(cik:"320193") { nodes { value } } }"#,
        r#"{ financialStatement(cik:"320193",kind:BALANCE_SHEET) { lines { value } } }"#,
        r#"{ companyRatios(cik:"320193",periodEnd:"2024-12-31") { name } }"#,
        r#"{ companyPeers(cik:"320193",periodEnd:"2024-12-31") { company { cik } } }"#,
        r#"{ companyResearchTargetExists(anchor:"{}") }"#,
    ];
    for granted in [false, true] {
        let schema = schema(Some(unreachable_pool()), granted);
        for query in queries {
            // With the flag on, the granted case would reach the data DB, tested below.
            if cfg!(flag_financial_components) && granted {
                continue;
            }
            let response = (&schema).execute(query).await;
            let expected = if cfg!(flag_financial_components) {
                "FORBIDDEN"
            } else {
                "FEATURE_DISABLED"
            };
            assert_eq!(response.errors.len(), 1, "{query}: {:?}", response.errors);
            assert_eq!(
                response.errors[0].extensions.as_ref().unwrap().get("code"),
                Some(&async_graphql::Value::from(expected)),
                "{query}"
            );
        }
    }
}

#[cfg(flag_financial_components)]
#[tokio::test]
async fn company_research_app_only_schema_returns_remote_data_contract() {
    let app_schema = schema(None, true);
    let response = (&app_schema)
        .execute(r#"{ company(cik:"320193") { cik } }"#)
        .await;
    assert_eq!(
        response.errors[0].extensions.as_ref().unwrap().get("code"),
        Some(&async_graphql::Value::from(context::DATA_SERVED_REMOTELY))
    );
}

#[cfg(flag_financial_components)]
async fn insert_fact(
    pool: &DatabasePool,
    cik: &str,
    concept: &str,
    start: Option<&str>,
    end: &str,
    accession: &str,
    filed: &str,
    value: &str,
) -> CompanyFact {
    let subject = CompanyFactSubject {
        cik: cik.into(),
        taxonomy: "us-gaap".into(),
        concept: concept.into(),
        period: match start {
            Some(start) => ResearchPeriod::Duration {
                start: start.parse().unwrap(),
                end: end.parse().unwrap(),
            },
            None => ResearchPeriod::Instant {
                date: end.parse().unwrap(),
            },
        },
        unit: "USD".into(),
    };
    let fact = CompanyFact::new(
        subject,
        accession.into(),
        filed.parse().unwrap(),
        value.parse().unwrap(),
        "10-K".into(),
    )
    .unwrap();
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query("INSERT INTO company_facts(id,cik,taxonomy,concept,period_start,period_end,unit,accession,filed,value,form,label) VALUES($1,$2,'us-gaap',$3,$4,$5,'USD',$6,$7,$8,'10-K',$3)")
      .bind::<SqlUuid,_>(fact.id).bind::<Text,_>(cik).bind::<Text,_>(concept).bind::<Nullable<Date>,_>(start.map(|s|s.parse::<NaiveDate>().unwrap())).bind::<Date,_>(end.parse::<NaiveDate>().unwrap()).bind::<Text,_>(accession).bind::<Date,_>(filed.parse::<NaiveDate>().unwrap()).bind::<Numeric,_>(&fact.value).execute(&mut conn).await.unwrap();
    fact
}

#[cfg(flag_financial_components)]
#[tokio::test]
async fn company_research_postgres_revision_pagination_statement_peer_and_anchor_contracts() {
    let db = tokio::task::spawn_blocking(|| ScratchDatabase::create("company_research"))
        .await
        .unwrap();
    // Data migrations only: the data API must never require users/annotations tables.
    run_migration_set(&db.url, MigrationSet::Data)
        .await
        .unwrap();
    let pool = create_pool(&db.url).await.unwrap();
    {
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query("INSERT INTO companies(cik,name,ticker,sic_code) VALUES ('0000320193','Apple research fixture','AAPL','3571'),('0000789019','Compatible peer fixture','PEER','3571'),('0000000002','Metadata-only issuer','META','9999')").execute(&mut conn).await.unwrap();
        diesel::sql_query("INSERT INTO company_fact_filings(id,cik,accession,filed,form,is_xbrl) VALUES('00000000-0000-0000-0000-000000000111','0000320193','0000320193-24-000099','2024-04-01','10-K',true)").execute(&mut conn).await.unwrap();
    }
    let original = insert_fact(
        &pool,
        "0000320193",
        "Assets",
        None,
        "2024-12-31",
        "0000320193-25-000001",
        "2025-02-01",
        "100",
    )
    .await;
    insert_fact(
        &pool,
        "0000320193",
        "Assets",
        None,
        "2024-12-31",
        "0000320193-25-000002",
        "2025-03-01",
        "100",
    )
    .await;
    let changed = insert_fact(
        &pool,
        "0000320193",
        "Assets",
        None,
        "2024-12-31",
        "0000320193-26-000001",
        "2026-02-01",
        "12345678901234567890.123456789",
    )
    .await;
    insert_fact(
        &pool,
        "0000320193",
        "AssetsCurrent",
        None,
        "2024-12-31",
        "0000320193-25-000001",
        "2025-02-01",
        "20",
    )
    .await;
    insert_fact(
        &pool,
        "0000320193",
        "LiabilitiesCurrent",
        None,
        "2024-12-31",
        "0000320193-25-000001",
        "2025-02-01",
        "10",
    )
    .await;
    insert_fact(
        &pool,
        "0000320193",
        "NetIncomeLoss",
        Some("2024-01-01"),
        "2024-12-31",
        "0000320193-25-000001",
        "2025-02-01",
        "5",
    )
    .await;
    insert_fact(
        &pool,
        "0000320193",
        "UnmappedButReportedConcept",
        Some("2024-07-01"),
        "2024-12-31",
        "0000320193-25-000001",
        "2025-02-01",
        "7.125",
    )
    .await;
    insert_fact(
        &pool,
        "0000320193",
        "Revenues",
        Some("2024-01-01"),
        "2024-12-31",
        "0000320193-25-000001",
        "2025-02-01",
        "120",
    )
    .await;
    insert_fact(
        &pool,
        "0000320193",
        "Revenues",
        Some("2024-01-01"),
        "2024-09-30",
        "0000320193-24-000001",
        "2024-11-01",
        "90",
    )
    .await;
    // Peers' nearby dates must never substitute for the requested economic period.
    insert_fact(
        &pool,
        "0000789019",
        "AssetsCurrent",
        None,
        "2024-12-30",
        "0000789019-25-000001",
        "2025-02-01",
        "50",
    )
    .await;
    insert_fact(
        &pool,
        "0000789019",
        "LiabilitiesCurrent",
        None,
        "2024-12-30",
        "0000789019-25-000001",
        "2025-02-01",
        "5",
    )
    .await;
    let schema = schema(Some(pool.clone()), true);
    let companies = result(
        &schema,
        r#"{ companies(first:1) { nodes { cik } hasNextPage endCursor } }"#,
        json!({}),
    )
    .await;
    assert_eq!(companies["companies"]["nodes"][0]["cik"], "0000320193");
    assert_eq!(companies["companies"]["hasNextPage"], true);
    let second = result(
        &schema,
        r#"query($after:String){companies(first:1,after:$after){nodes{cik}hasNextPage}}"#,
        json!({"after":companies["companies"]["endCursor"]}),
    )
    .await;
    assert_eq!(second["companies"]["nodes"][0]["cik"], "0000789019");
    assert_eq!(second["companies"]["hasNextPage"], false);
    assert_eq!(
        result(
            &schema,
            r#"{ companies(search:"%") { nodes { cik } } company(cik:"999") { cik } }"#,
            json!({})
        )
        .await,
        json!({"companies":{"nodes":[]},"company":null})
    );
    let query = r#"query($revision:CompanyRevisionInput){companyFacts(cik:"320193",filter:{concept:"Assets",revision:$revision}){nodes{value accession periodStart periodEnd anchor}hasNextPage}}"#;
    for (revision, value, accession) in [
        (
            json!({"mode":"LATEST"}),
            "12345678901234567890.123456789",
            "0000320193-26-000001",
        ),
        (json!({"mode":"ORIGINAL"}), "100", "0000320193-25-000001"),
        (
            json!({"mode":"AS_OF","asOf":"2025-03-01"}),
            "100",
            "0000320193-25-000002",
        ),
        (
            json!({"mode":"ACCESSION","accession":"0000320193-25-000001"}),
            "100",
            "0000320193-25-000001",
        ),
    ] {
        let response = result(&schema, query, json!({"revision":revision})).await;
        let nodes = response["companyFacts"]["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0]["value"], value);
        assert_eq!(nodes[0]["accession"], accession);
        assert!(nodes[0]["periodStart"].is_null());
    }
    let history=result(&schema,r#"{companyFacts(cik:"320193",first:2,filter:{concept:"Assets",includeRevisions:true}){nodes{accession value}hasNextPage endCursor}}"#,json!({})).await;
    assert_eq!(
        history["companyFacts"]["nodes"].as_array().unwrap().len(),
        2
    );
    assert_eq!(history["companyFacts"]["hasNextPage"], true);
    let last=result(&schema,r#"query($after:String){companyFacts(cik:"320193",first:2,after:$after,filter:{concept:"Assets",includeRevisions:true}){nodes{accession value}hasNextPage}}"#,json!({"after":history["companyFacts"]["endCursor"]})).await;
    assert_eq!(last["companyFacts"]["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(last["companyFacts"]["hasNextPage"], false);
    let statement=result(&schema,r#"{financialStatement(cik:"320193",kind:BALANCE_SHEET){notice lines{key value restated anchor provenance{accession}} unclassifiedFacts{concept value anchor}periods{start end kind}}}"#,json!({})).await;
    let assets = statement["financialStatement"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["key"] == "assets")
        .unwrap();
    assert_eq!(assets["value"], "12345678901234567890.123456789");
    assert_eq!(assets["restated"], true);
    assert!(assets["anchor"]
        .as_str()
        .unwrap()
        .contains("0000320193-26-000001"));
    assert!(statement["financialStatement"]["unclassifiedFacts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l["concept"] == "UnmappedButReportedConcept" && l["value"] == "7.125"));
    let ratios=result(&schema,r#"{companyRatios(cik:"320193",periodEnd:"2024-12-31"){name value unavailableReason}companyPeers(cik:"320193",periodEnd:"2024-12-31"){company{cik}ratios{name value unavailableReason}}}"#,json!({})).await;
    let current = ratios["companyRatios"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "current_ratio")
        .unwrap();
    assert_eq!(current["value"], "2");
    let peer_current = ratios["companyPeers"][0]["ratios"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "current_ratio")
        .unwrap();
    assert!(peer_current["value"].is_null());
    assert!(peer_current["unavailableReason"].as_str().is_some());
    for (target, expected) in [
        (changed.reference().unwrap(), true),
        (original.reference().unwrap(), true),
        (
            SubjectRef::Filing {
                filing: FilingRef {
                    cik: "0000320193".into(),
                    accession: "0000320193-24-000099".into(),
                },
            },
            true,
        ),
        (
            SubjectRef::Statement {
                filing: FilingRef {
                    cik: "0000320193".into(),
                    accession: "0000320193-24-000099".into(),
                },
                statement: "balance_sheet".into(),
            },
            false,
        ),
    ] {
        let anchor = serde_json::to_string(&AnnotationAnchor::V1 {
            target,
            chart_id: None,
            observation_date: None,
            observation_value: None,
        })
        .unwrap();
        let checked = result(
            &schema,
            "query($anchor:String!){companyResearchTargetExists(anchor:$anchor)}",
            json!({"anchor":anchor}),
        )
        .await;
        assert_eq!(checked["companyResearchTargetExists"], expected);
    }
    let q4=result(&schema,r#"{financialStatement(cik:"320193",kind:INCOME_STATEMENT,periodStart:"2024-10-01",periodEnd:"2024-12-31"){lines{key value derivation subjectAnchor provenance{anchor filingAnchor}}}}"#,json!({})).await;
    let line = q4["financialStatement"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["key"] == "revenue")
        .unwrap();
    assert_eq!(line["value"], "30");
    assert_eq!(line["derivation"], "fourth_quarter");
    assert!(line["provenance"]
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["anchor"].as_str().is_some() && p["filingAnchor"].as_str().is_some()));
    let checked = result(
        &schema,
        "query($anchor:String!){companyResearchTargetExists(anchor:$anchor)}",
        json!({"anchor":line["subjectAnchor"]}),
    )
    .await;
    assert_eq!(checked["companyResearchTargetExists"], true);
    // Revoking the catalog grant conceals even known CIKs before issuing a database read.
    let denied_schema = self::schema(Some(pool.clone()), false);
    let denied = (&denied_schema)
        .execute(r#"{company(cik:"320193"){name}}"#)
        .await;
    assert_eq!(
        denied.errors[0].extensions.as_ref().unwrap().get("code"),
        Some(&async_graphql::Value::from("FORBIDDEN"))
    );
    drop(schema);
    drop(pool);
    drop(db);
}
