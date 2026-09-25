// Regression tests for the enum-typed columns of the SEC/XBRL tables.
//
// The initial schema created these columns as Postgres ENUM types while schema.rs (and the Rust
// enums in crate::enums) bind them as Text, so every diesel insert failed with
// "column ... is of type compression_type but expression is of type text". Migration
// 2026-09-25-000003_enum_columns_to_varchar converts them to VARCHAR + CHECK. These tests insert
// through the diesel models and read the rows back.
//
// They need a reachable Postgres in DATABASE_URL; migrations are applied once per test binary.
// Each test uses a unique CIK / namespace so leftover rows from earlier runs can't interfere.

use crate::database::DatabasePool;
use crate::enums::{CompressionType, ProcessingStatus, TaxonomyFileType, TaxonomySourceType};
use crate::models::{FinancialStatement, NewFinancialStatement, XbrlTaxonomySchema};
use crate::schema::{companies, financial_statements, xbrl_taxonomy_schemas};
use chrono::{NaiveDate, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

static MIGRATED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

async fn test_pool() -> DatabasePool {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must point at a Postgres database for financial_statement tests");
    MIGRATED
        .get_or_init(|| async {
            crate::database::run_migrations(&url)
                .await
                .expect("migrations failed");
        })
        .await;
    crate::database::create_pool(&url)
        .await
        .expect("Failed to connect to test database")
}

/// Inserts a company with a unique 10-character CIK and returns its id.
async fn insert_company(pool: &DatabasePool) -> Uuid {
    let cik = format!("9{:09}", Uuid::new_v4().as_u128() % 1_000_000_000);
    let mut conn = pool.get().await.unwrap();
    diesel::insert_into(companies::table)
        .values((
            companies::cik.eq(&cik),
            companies::name.eq("Enum Regression Co"),
        ))
        .returning(companies::id)
        .get_result(&mut conn)
        .await
        .expect("insert company")
}

fn new_statement(company_id: Uuid, accession: &str) -> NewFinancialStatement {
    NewFinancialStatement {
        company_id,
        filing_type: "10-K".to_string(),
        form_type: "10-K".to_string(),
        accession_number: accession.to_string(),
        filing_date: NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(),
        period_end_date: NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
        fiscal_year: 2023,
        fiscal_quarter: None,
        document_type: "10-K".to_string(),
        document_url: "https://www.sec.gov/Archives/edgar/data/1/x.xml".to_string(),
        xbrl_file_oid: None,
        xbrl_file_content: Some(b"<xbrl/>".to_vec()),
        xbrl_file_size_bytes: Some(7),
        xbrl_file_compressed: true,
        xbrl_file_compression_type: CompressionType::Lz4,
        xbrl_file_hash: None,
        xbrl_processing_status: ProcessingStatus::Downloaded,
        xbrl_processing_error: None,
        xbrl_processing_started_at: None,
        xbrl_processing_completed_at: None,
        is_amended: false,
        amendment_type: None,
        original_filing_date: None,
        is_restated: false,
        restatement_reason: None,
    }
}

fn unique_accession() -> String {
    let n = Uuid::new_v4().as_u128();
    format!("{:010}-24-{:06}", n % 10_000_000_000, (n >> 64) % 1_000_000)
}

#[tokio::test]
async fn financial_statement_enum_columns_round_trip() {
    let pool = test_pool().await;
    let company_id = insert_company(&pool).await;
    let accession = unique_accession();
    let mut conn = pool.get().await.unwrap();

    let inserted: FinancialStatement = diesel::insert_into(financial_statements::table)
        .values(&new_statement(company_id, &accession))
        .returning(FinancialStatement::as_returning())
        .get_result(&mut conn)
        .await
        .expect("insert with enum-typed columns must succeed");
    assert_eq!(inserted.xbrl_file_compression_type, CompressionType::Lz4);
    assert_eq!(inserted.xbrl_processing_status, ProcessingStatus::Downloaded);

    // Update and filter on the columns through the typed values too.
    diesel::update(financial_statements::table.find(inserted.id))
        .set((
            financial_statements::xbrl_processing_status.eq(ProcessingStatus::Completed),
            financial_statements::xbrl_file_compression_type.eq(CompressionType::None),
        ))
        .execute(&mut conn)
        .await
        .expect("update enum-typed columns");
    let reloaded: FinancialStatement = financial_statements::table
        .filter(financial_statements::accession_number.eq(&accession))
        .filter(financial_statements::xbrl_processing_status.eq(ProcessingStatus::Completed))
        .select(FinancialStatement::as_select())
        .first(&mut conn)
        .await
        .expect("reload");
    assert_eq!(reloaded.xbrl_processing_status, ProcessingStatus::Completed);
    assert_eq!(reloaded.xbrl_file_compression_type, CompressionType::None);
}

#[tokio::test]
async fn financial_statement_columns_keep_defaults_and_checks() {
    let pool = test_pool().await;
    let company_id = insert_company(&pool).await;
    let accession = unique_accession();
    let mut conn = pool.get().await.unwrap();

    // Omitting the columns uses the defaults the enum columns had.
    let (compression, status): (CompressionType, ProcessingStatus) =
        diesel::insert_into(financial_statements::table)
            .values((
                financial_statements::company_id.eq(company_id),
                financial_statements::filing_type.eq("10-Q"),
                financial_statements::form_type.eq("10-Q"),
                financial_statements::accession_number.eq(&accession),
                financial_statements::filing_date.eq(NaiveDate::from_ymd_opt(2024, 5, 1).unwrap()),
                financial_statements::period_end_date
                    .eq(NaiveDate::from_ymd_opt(2024, 3, 31).unwrap()),
                financial_statements::fiscal_year.eq(2024),
                financial_statements::document_type.eq("10-Q"),
                financial_statements::document_url.eq("test"),
            ))
            .returning((
                financial_statements::xbrl_file_compression_type,
                financial_statements::xbrl_processing_status,
            ))
            .get_result(&mut conn)
            .await
            .expect("insert with defaults");
    assert_eq!(compression, CompressionType::Zstd);
    assert_eq!(status, ProcessingStatus::Pending);

    // The CHECK constraint still rejects values outside the former enum.
    let err = diesel::update(
        financial_statements::table.filter(financial_statements::accession_number.eq(&accession)),
    )
    .set(financial_statements::xbrl_file_compression_type.eq("brotli"))
    .execute(&mut conn)
    .await
    .expect_err("unknown compression type must be rejected");
    assert!(
        err.to_string().contains("chk_financial_statements_xbrl_file_compression_type"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn xbrl_taxonomy_schema_enum_columns_round_trip() {
    let pool = test_pool().await;
    let mut conn = pool.get().await.unwrap();
    let now = Utc::now();
    let row = XbrlTaxonomySchema {
        id: Uuid::new_v4(),
        schema_namespace: format!("http://example.com/{}", Uuid::new_v4()),
        schema_filename: "ex-20240101_lab.xml".to_string(),
        schema_version: None,
        schema_date: None,
        file_type: TaxonomyFileType::LabelLinkbase,
        source_type: TaxonomySourceType::UsGaap,
        file_content: Some(b"<linkbase/>".to_vec()),
        file_oid: None,
        file_size_bytes: 11,
        file_hash: "h".to_string(),
        is_compressed: false,
        compression_type: CompressionType::Gzip,
        source_url: None,
        download_url: None,
        original_filename: None,
        processing_status: ProcessingStatus::Processing,
        processing_error: None,
        processing_started_at: None,
        processing_completed_at: None,
        concepts_extracted: 0,
        relationships_extracted: 0,
        created_at: now,
        updated_at: now,
    };
    let back: XbrlTaxonomySchema = diesel::insert_into(xbrl_taxonomy_schemas::table)
        .values(&row)
        .returning(XbrlTaxonomySchema::as_returning())
        .get_result(&mut conn)
        .await
        .expect("insert taxonomy schema with enum-typed columns");
    assert_eq!(back.file_type, TaxonomyFileType::LabelLinkbase);
    assert_eq!(back.source_type, TaxonomySourceType::UsGaap);
    assert_eq!(back.compression_type, CompressionType::Gzip);
    assert_eq!(back.processing_status, ProcessingStatus::Processing);
}
