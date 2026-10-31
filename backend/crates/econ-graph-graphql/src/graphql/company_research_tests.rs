//! Real data-plane regression tests; every fixture owns an independently migrated DB.
use super::*;
#[cfg(flag_financial_components)]
use async_graphql::{Request, Variables};
#[cfg(flag_financial_components)]
use diesel::sql_types::{Date, Nullable, Numeric, Text, Uuid as SqlUuid};
#[cfg(flag_financial_components)]
use diesel_async::RunQueryDsl;
#[cfg(flag_financial_components)]
use econ_graph_core::database::{create_pool, run_migration_set, MigrationSet};
use econ_graph_core::database::{AppPool, DataPool, DatabasePool};
#[cfg(flag_financial_components)]
use econ_graph_core::test_utils::ScratchDatabase;
#[cfg(flag_financial_components)]
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
#[cfg(flag_financial_components)]
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
        diesel::sql_query("INSERT INTO companies(cik,name,ticker,sic_code) VALUES ('0000320193','Apple research fixture','AAPL','3571'),('0000789019','Compatible peer fixture','PEER','3571'),('0000000002','Metadata-only issuer','META','9999'),('0000000003','Zed XBRL issuer','ZED','9999')").execute(&mut conn).await.unwrap();
        diesel::sql_query("INSERT INTO company_fact_filings(id,cik,accession,filed,form,is_xbrl) VALUES('00000000-0000-0000-0000-000000000111','0000320193','0000320193-24-000099','2024-04-01','10-K',true),('00000000-0000-0000-0000-000000000112','0000000003','0000000003-24-000001','2024-04-01','10-K',true)").execute(&mut conn).await.unwrap();
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
    // More unknown subjects than the hard response cap; the raw evidence stays
    // fully accessible through bounded pages without serializing every anchor.
    for index in 0..251 {
        insert_fact(
            &pool,
            "0000320193",
            &format!("ZUnmapped{index:03}"),
            Some("2024-07-01"),
            "2024-12-31",
            "0000320193-25-000001",
            "2025-02-01",
            "1",
        )
        .await;
    }
    insert_fact(
        &pool,
        "0000320193",
        "ZOtherPeriod",
        Some("2024-01-01"),
        "2024-06-30",
        "0000320193-25-000001",
        "2025-02-01",
        "2",
    )
    .await;
    insert_fact(
        &pool,
        "0000320193",
        "ZOtherInstant",
        None,
        "2023-12-31",
        "0000320193-25-000001",
        "2025-02-01",
        "3",
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
    assert_eq!(second["companies"]["hasNextPage"], true);
    assert_eq!(
        result(
            &schema,
            r#"{companies(search:"Zed"){nodes{cik}}}"#,
            json!({})
        )
        .await["companies"]["nodes"][0]["cik"],
        "0000000003"
    );
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
    assert_eq!(
        statement["financialStatement"]["unclassifiedFacts"]
            .as_array()
            .unwrap()
            .len(),
        100
    );
    let metadata=result(&schema,r#"{financialStatement(cik:"320193",kind:INCOME_STATEMENT,unclassifiedFirst:0){unclassifiedFacts{key}periods{start end}}}"#,json!({})).await;
    assert!(metadata["financialStatement"]["unclassifiedFacts"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(metadata["financialStatement"]["periods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["end"] == "2024-06-30"));
    let query = r#"query($after:String){financialStatement(cik:"320193",kind:INCOME_STATEMENT,periodStart:"2024-07-01",periodEnd:"2024-12-31",unclassifiedFirst:100,unclassifiedAfter:$after){unclassifiedFacts{key periodStart periodEnd}unclassifiedHasNextPage unclassifiedEndCursor}}"#;
    let mut after = Value::Null;
    let mut seen = std::collections::BTreeSet::new();
    loop {
        let page = result(&schema, query, json!({"after":after})).await;
        let data = &page["financialStatement"];
        let rows = data["unclassifiedFacts"].as_array().unwrap();
        assert!(rows.len() <= 100);
        for row in rows {
            assert!(seen.insert(row["key"].as_str().unwrap().to_owned()));
            assert_eq!(row["periodStart"], "2024-07-01");
            assert_eq!(row["periodEnd"], "2024-12-31");
        }
        if data["unclassifiedHasNextPage"] == false {
            break;
        }
        after = data["unclassifiedEndCursor"].clone();
        assert!(after.is_string());
    }
    assert_eq!(seen.len(), 252);
    let instants=result(&schema,r#"{financialStatement(cik:"320193",kind:INCOME_STATEMENT,instantOnly:true,periodEnd:"2023-12-31",unclassifiedFirst:250){unclassifiedFacts{concept periodStart}}}"#,json!({})).await;
    assert_eq!(
        instants["financialStatement"]["unclassifiedFacts"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let too_large=(&schema).execute(r#"{financialStatement(cik:"320193",kind:INCOME_STATEMENT,unclassifiedFirst:251){unclassifiedFacts{key}}}"#).await;
    assert_eq!(
        too_large.errors[0].extensions.as_ref().unwrap().get("code"),
        Some(&async_graphql::Value::from("BAD_USER_INPUT"))
    );
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

#[cfg(flag_financial_components)]
async fn persist_ratio_fixture(pool: &DatabasePool, facts: &[CompanyFact]) {
    let rows=facts.iter().map(|fact| {
        let(start,end)=dates(&fact.subject.period);
        json!({"id":fact.id,"cik":fact.subject.cik,"taxonomy":fact.subject.taxonomy,"concept":fact.subject.concept,"unit":fact.subject.unit,"start":start,"end":end,"accession":fact.accession,"filed":fact.filed,"value":fact.value.to_string()})
    }).collect::<Vec<_>>();
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query("INSERT INTO company_facts(id,cik,taxonomy,concept,period_start,period_end,unit,accession,filed,value,form)
        SELECT (r->>'id')::uuid,r->>'cik',r->>'taxonomy',r->>'concept',(r->>'start')::date,(r->>'end')::date,r->>'unit',r->>'accession',(r->>'filed')::date,(r->>'value')::numeric,'10-K'
        FROM jsonb_array_elements($1) r").bind::<diesel::sql_types::Jsonb,_>(Value::Array(rows)).execute(&mut conn).await.unwrap();
}

#[cfg(flag_financial_components)]
#[allow(clippy::too_many_arguments)]
fn ratio_fixture(
    cik: &str,
    taxonomy: &str,
    concept: &str,
    unit: &str,
    start: Option<&str>,
    end: &str,
    accession: &str,
    filed: &str,
    value: i64,
) -> CompanyFact {
    CompanyFact::new(
        CompanyFactSubject {
            cik: cik.into(),
            taxonomy: taxonomy.into(),
            concept: concept.into(),
            unit: unit.into(),
            period: match start {
                Some(start) => ResearchPeriod::Duration {
                    start: start.parse().unwrap(),
                    end: end.parse().unwrap(),
                },
                None => ResearchPeriod::Instant {
                    date: end.parse().unwrap(),
                },
            },
        },
        accession.into(),
        filed.parse().unwrap(),
        value.into(),
        "10-K".into(),
    )
    .unwrap()
}

#[cfg(flag_financial_components)]
#[tokio::test]
async fn company_research_ratio_sql_preserves_all_revision_and_derivation_semantics() {
    let db = tokio::task::spawn_blocking(|| ScratchDatabase::create("company_ratio_operands"))
        .await
        .unwrap();
    run_migration_set(&db.url, MigrationSet::Data)
        .await
        .unwrap();
    let pool = create_pool(&db.url).await.unwrap();
    let mut facts = Vec::new();
    // July-year-end issuer: off-calendar Q4 and a TTM comprising three derived quarters.
    let periods = [
        (
            "2023-07-01",
            "2023-09-30",
            "0000000101-23-000001",
            "2023-11-01",
            100,
        ),
        (
            "2023-07-01",
            "2023-12-31",
            "0000000101-24-000001",
            "2024-02-01",
            220,
        ),
        (
            "2023-07-01",
            "2024-03-31",
            "0000000101-24-000002",
            "2024-05-01",
            350,
        ),
        (
            "2023-07-01",
            "2024-06-30",
            "0000000101-24-000003",
            "2024-08-01",
            490,
        ),
        (
            "2024-07-01",
            "2024-09-30",
            "0000000101-24-000004",
            "2024-11-01",
            150,
        ),
    ];
    for (concept, factor) in [
        ("Revenues", 10),
        ("GrossProfit", 5),
        ("OperatingIncomeLoss", 3),
        ("NetIncomeLoss", 2),
    ] {
        for (start, end, accession, filed, value) in periods {
            facts.push(ratio_fixture(
                "0000000101",
                "us-gaap",
                concept,
                "USD",
                Some(start),
                end,
                accession,
                filed,
                value * factor,
            ));
            facts.push(ratio_fixture(
                "0000000101",
                "us-gaap",
                concept,
                "USD",
                Some(start),
                end,
                "0000000101-25-000001",
                "2025-02-01",
                value * factor + factor,
            ));
        }
    }
    for (concept, value) in [
        ("AssetsCurrent", 100),
        ("LiabilitiesCurrent", 50),
        ("Liabilities", 200),
        ("StockholdersEquity", 100),
    ] {
        facts.push(ratio_fixture(
            "0000000101",
            "us-gaap",
            concept,
            "USD",
            None,
            "2024-06-30",
            "0000000101-24-000003",
            "2024-08-01",
            value,
        ));
    }
    // IFRS aliases; the first TTM quarter is Q4 and needs a prior nine-month cumulative.
    for (concept, factor) in [
        ("Revenue", 10),
        ("GrossProfit", 5),
        ("ProfitLossFromOperatingActivities", 3),
        ("ProfitLoss", 2),
    ] {
        for (start, end, accession, filed, value) in [
            (
                "2023-01-01",
                "2023-09-30",
                "0000000102-23-000001",
                "2023-11-01",
                33,
            ),
            (
                "2023-01-01",
                "2023-12-31",
                "0000000102-24-000001",
                "2024-02-01",
                46,
            ),
            (
                "2024-01-01",
                "2024-03-31",
                "0000000102-24-000002",
                "2024-05-01",
                10,
            ),
            (
                "2024-01-01",
                "2024-06-30",
                "0000000102-24-000003",
                "2024-08-01",
                21,
            ),
            (
                "2024-01-01",
                "2024-09-30",
                "0000000102-24-000004",
                "2024-11-01",
                33,
            ),
        ] {
            facts.push(ratio_fixture(
                "0000000102",
                "ifrs-full",
                concept,
                "USD",
                Some(start),
                end,
                accession,
                filed,
                value * factor,
            ));
        }
    }
    // These rows increase the full-company load and maximum filed date, but cannot
    // be ratio operands. The query must not load them or depend on their cutoff.
    for cik in ["0000000101", "0000000102"] {
        for i in 0..100 {
            facts.push(ratio_fixture(
                cik,
                "us-gaap",
                &format!("UnknownConcept{i}"),
                "USD",
                Some("2024-01-01"),
                "2024-09-30",
                "0000000101-25-000002",
                "2025-06-01",
                1,
            ));
        }
        facts.push(ratio_fixture(
            cik,
            "us-gaap",
            "Revenues",
            "USD",
            Some("2010-01-01"),
            "2010-12-31",
            "0000000101-11-000001",
            "2011-02-01",
            1,
        ));
        facts.push(ratio_fixture(
            cik,
            "us-gaap",
            "Revenues",
            "USD",
            Some("2026-01-01"),
            "2026-12-31",
            "0000000101-27-000001",
            "2027-02-01",
            1,
        ));
    }
    persist_ratio_fixture(&pool, &facts).await;
    let q4 = period(
        Some("2024-04-01".parse().unwrap()),
        "2024-06-30".parse().unwrap(),
    )
    .unwrap();
    let ttm = period(
        Some("2023-10-01".parse().unwrap()),
        "2024-09-30".parse().unwrap(),
    )
    .unwrap();
    let instant = period(None, "2024-06-30".parse().unwrap()).unwrap();
    let selections = [
        RevisionSelection::Latest,
        RevisionSelection::Original,
        RevisionSelection::AsOf("2024-12-01".parse().unwrap()),
        RevisionSelection::AsOf("2024-09-01".parse().unwrap()),
        RevisionSelection::Accession("0000000101-25-000001".into()),
    ];
    for cik in ["0000000101", "0000000102"] {
        let full = service::calculation_facts(&pool, cik).await.unwrap();
        for requested in [&q4, &ttm, &instant] {
            let filtered = service::ratio_facts(&pool, cik, requested).await.unwrap();
            assert!(filtered.len() < full.len() / 2);
            assert!(filtered
                .iter()
                .all(|f| !f.subject.concept.starts_with("Unknown")));
            for selection in &selections {
                assert_eq!(
                    company_financials::calculate_ratios(&full, requested, selection).unwrap(),
                    company_financials::calculate_ratios(&filtered, requested, selection).unwrap(),
                    "{cik} {requested:?} {selection:?}"
                );
            }
        }
    }
    let base = service::ratio_facts(&pool, "0000000101", &q4)
        .await
        .unwrap();
    assert!(
        company_financials::calculate_ratios(&base, &q4, &RevisionSelection::Original)
            .unwrap()
            .iter()
            .all(|r| r.value.is_some())
    );
    let ifrs = service::ratio_facts(&pool, "0000000102", &ttm)
        .await
        .unwrap();
    assert!(
        company_financials::calculate_ratios(&ifrs, &ttm, &RevisionSelection::Latest)
            .unwrap()
            .iter()
            .all(|r| r.value.is_some())
    );
    // Keep additional currency/taxonomy alternatives, and an almost-nine-month
    // cumulative that makes Q4 ambiguous. Filtering must not fabricate certainty.
    let ambiguous = [
        ratio_fixture(
            "0000000101",
            "us-gaap",
            "Revenues",
            "EUR",
            Some("2023-07-01"),
            "2024-06-30",
            "0000000101-24-000003",
            "2024-08-01",
            4900,
        ),
        ratio_fixture(
            "0000000101",
            "us-gaap",
            "Revenues",
            "EUR",
            Some("2023-07-01"),
            "2024-03-31",
            "0000000101-24-000002",
            "2024-05-01",
            3500,
        ),
        ratio_fixture(
            "0000000101",
            "ifrs-full",
            "Revenue",
            "USD",
            Some("2023-07-01"),
            "2024-06-30",
            "0000000101-24-000003",
            "2024-08-01",
            4900,
        ),
        ratio_fixture(
            "0000000101",
            "ifrs-full",
            "Revenue",
            "USD",
            Some("2023-07-01"),
            "2024-03-31",
            "0000000101-24-000002",
            "2024-05-01",
            3500,
        ),
        ratio_fixture(
            "0000000101",
            "us-gaap",
            "GrossProfit",
            "USD",
            Some("2023-07-01"),
            "2024-03-20",
            "0000000101-24-000002",
            "2024-05-01",
            1700,
        ),
    ];
    persist_ratio_fixture(&pool, &ambiguous).await;
    let full = service::calculation_facts(&pool, "0000000101")
        .await
        .unwrap();
    for requested in [&q4, &ttm] {
        let filtered = service::ratio_facts(&pool, "0000000101", requested)
            .await
            .unwrap();
        assert!(filtered.iter().any(|f| f.subject.unit == "EUR"));
        assert!(filtered.iter().any(|f| f.subject.taxonomy == "ifrs-full"));
        for selection in &selections {
            assert_eq!(
                company_financials::calculate_ratios(&full, requested, selection).unwrap(),
                company_financials::calculate_ratios(&filtered, requested, selection).unwrap(),
                "ambiguous {requested:?} {selection:?}"
            );
        }
    }
    let filtered = service::ratio_facts(&pool, "0000000101", &q4)
        .await
        .unwrap();
    assert!(
        company_financials::calculate_ratios(&filtered, &q4, &RevisionSelection::Original)
            .unwrap()
            .iter()
            .all(|r| r.value.is_none() && r.unavailable_reason.is_some())
    );
    // An issuer whose relevant revisions exceed the per-request budget fails
    // explicitly; it never returns a ratio computed from a partial source page.
    let oversized = (0..=service::MAX_RATIO_FACTS)
        .map(|i| {
            ratio_fixture(
                "0000000103",
                "us-gaap",
                "AssetsCurrent",
                "USD",
                None,
                "2024-06-30",
                &format!("{:010}-25-000001", i + 1),
                "2025-02-01",
                1,
            )
        })
        .collect::<Vec<_>>();
    persist_ratio_fixture(&pool, &oversized).await;
    let err = service::ratio_facts(&pool, "0000000103", &instant)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("ratio operand limit"));
}
