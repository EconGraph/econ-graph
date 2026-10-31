//! Companyfacts constraints keep economic periods and filed revisions distinguishable.

use std::str::FromStr;

use bigdecimal::BigDecimal;
use diesel::{connection::SimpleConnection, prelude::*, sql_types::Numeric};
use econ_graph_core::test_utils::POSTGRES_IMAGE_TAG;
use testcontainers::core::WaitFor;
use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};

const UP: &str = include_str!("../../../migrations/data/2026-10-31-120000_company_facts/up.sql");
const DOWN: &str =
    include_str!("../../../migrations/data/2026-10-31-120000_company_facts/down.sql");

#[derive(QueryableByName)]
struct Value {
    #[diesel(sql_type = Numeric)]
    value: BigDecimal,
}

fn insert_fact(
    conn: &mut PgConnection,
    id: i32,
    start: &str,
    accession: &str,
) -> QueryResult<usize> {
    diesel::sql_query(format!(
        "INSERT INTO company_facts
          (id,cik,taxonomy,concept,period_start,period_end,unit,accession,filed,value,form)
         VALUES ('00000000-0000-0000-0000-{id:012}','0000320193','us-gaap','Revenue',
                 {start},'2023-12-31','USD','{accession}','2024-02-01',
                 123456789012345678901234567890.12345678901234567890,'10-K')"
    ))
    .execute(conn)
}

#[tokio::test]
async fn companyfacts_migration_preserves_precision_and_revision_boundaries() {
    let container = GenericImage::new("postgres", POSTGRES_IMAGE_TAG)
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .start()
        .await
        .expect("start isolated PostgreSQL");
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let mut conn = PgConnection::establish(&format!(
        "postgres://postgres:postgres@127.0.0.1:{port}/postgres"
    ))
    .unwrap();
    conn.batch_execute(UP).unwrap();

    insert_fact(&mut conn, 1, "NULL", "0000320193-24-000001").unwrap();
    // SQL NULLs must compare equal for instant identity: a second revision row
    // with another UUID cannot bypass the subject/accession uniqueness rule.
    assert!(insert_fact(&mut conn, 2, "NULL", "0000320193-24-000001").is_err());
    insert_fact(&mut conn, 3, "'2023-01-01'", "0000320193-24-000001").unwrap();
    insert_fact(&mut conn, 4, "'2023-10-01'", "0000320193-24-000001").unwrap();
    insert_fact(&mut conn, 5, "NULL", "0000320193-24-000002").unwrap();
    assert!(insert_fact(&mut conn, 6, "'2024-01-01'", "0000320193-24-000001").is_err());

    let value = diesel::sql_query(
        "SELECT value FROM company_facts WHERE id = '00000000-0000-0000-0000-000000000001'",
    )
    .get_result::<Value>(&mut conn)
    .unwrap();
    assert_eq!(
        value.value,
        BigDecimal::from_str("123456789012345678901234567890.12345678901234567890").unwrap()
    );

    assert!(diesel::sql_query(
        "INSERT INTO company_fact_load_runs (id,status) VALUES (gen_random_uuid(),'succeeded')"
    )
    .execute(&mut conn)
    .is_err());
    diesel::sql_query(
        "INSERT INTO company_fact_load_runs (id,status) VALUES (gen_random_uuid(),'running')",
    )
    .execute(&mut conn)
    .unwrap();
    diesel::sql_query("UPDATE company_fact_load_runs SET status='succeeded',completed_at=now()")
        .execute(&mut conn)
        .unwrap();

    conn.batch_execute(DOWN).unwrap();
    conn.batch_execute(UP).unwrap();
}
