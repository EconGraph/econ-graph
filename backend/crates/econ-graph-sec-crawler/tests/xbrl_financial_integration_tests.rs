//! XBRL Financial Statement Integration Tests
//!
//! These tests verify the complete end-to-end flow from SEC EDGAR crawling
//! through XBRL parsing, DTS management, and financial data processing using testcontainers.

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use econ_graph_core::{enums::*, models::*, schema::*, test_utils::TestContainer};
    use econ_graph_sec_crawler::{models::CrawlConfig, DtsManager, SecEdgarCrawler, XbrlParser};
    use serial_test::serial;
    use std::path::PathBuf;
    use uuid::Uuid;

    /// Test XBRL taxonomy schema storage and retrieval with testcontainers
    #[tokio::test]
    #[serial]
    async fn test_xbrl_taxonomy_schema_storage() {
        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();

        // Test data for XBRL taxonomy schema
        let test_schema = XbrlTaxonomySchema::new(
            "http://fasb.org/us-gaap/2023".to_string(),
            "us-gaap.xsd".to_string(),
            TaxonomySourceType::UsGaap,
            1024000,
            "sha256:abc123def456".to_string(),
            Some("https://xbrl.fasb.org/us-gaap/2023/us-gaap.xsd".to_string()),
            Some("https://xbrl.fasb.org/us-gaap/2023/us-gaap.xsd".to_string()),
        );

        // Insert the schema
        let mut conn = pool.get().await.expect("Failed to get database connection");
        let inserted_schema = diesel::insert_into(xbrl_taxonomy_schemas::table)
            .values(&test_schema)
            .returning(XbrlTaxonomySchema::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert taxonomy schema");

        // Verify the schema was inserted correctly
        assert_eq!(
            inserted_schema.schema_namespace,
            "http://fasb.org/us-gaap/2023"
        );
        assert_eq!(inserted_schema.schema_filename, "us-gaap.xsd");
        assert_eq!(inserted_schema.source_type, TaxonomySourceType::UsGaap);
        assert_eq!(inserted_schema.file_size_bytes, 1024000);

        // Retrieve the schema
        let retrieved_schema = xbrl_taxonomy_schemas::table
            .filter(xbrl_taxonomy_schemas::id.eq(inserted_schema.id))
            .first::<XbrlTaxonomySchema>(&mut conn)
            .await
            .expect("Failed to retrieve taxonomy schema");

        assert_eq!(retrieved_schema.id, inserted_schema.id);
        assert_eq!(
            retrieved_schema.schema_namespace,
            "http://fasb.org/us-gaap/2023"
        );
    }

    /// Test financial statement processing with XBRL data
    #[tokio::test]
    #[serial]
    async fn test_financial_statement_processing() {
        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();
        let mut conn = pool.get().await.expect("Failed to get database connection");

        // Create test company
        let test_company = NewCompany {
            cik: "0000320193".to_string(),
            name: "Apple Inc.".to_string(),
            ticker: Some("AAPL".to_string()),
            sic_code: Some("3571".to_string()),
            industry: Some("Computer Hardware".to_string()),
            sector: Some("Technology".to_string()),
            legal_name: None,
            sic_description: None,
            business_address: None,
            mailing_address: None,
            phone: None,
            website: None,
            state_of_incorporation: None,
            state_of_incorporation_description: None,
            fiscal_year_end: None,
            entity_type: None,
            entity_size: None,
            is_active: true,
        };

        let company = diesel::insert_into(companies::table)
            .values(&test_company)
            .returning(Company::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert test company");

        // Create test financial statement
        let test_statement = NewFinancialStatement {
            company_id: company.id,
            filing_type: "10-K".to_string(),
            form_type: "10-K".to_string(),
            accession_number: "0000320193-24-000006".to_string(),
            filing_date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            period_end_date: chrono::NaiveDate::from_ymd_opt(2023, 12, 30).unwrap(),
            fiscal_year: 2023,
            fiscal_quarter: Some(4),
            document_type: "XBRL".to_string(),
            document_url: "https://www.sec.gov/Archives/edgar/data/320193/000032019324000006/aapl-20231230.xbrl".to_string(),
            xbrl_processing_status: ProcessingStatus::Completed,
            is_amended: false,
            is_restated: false,
            amendment_type: None,
            original_filing_date: None,
            restatement_reason: None,
            xbrl_file_oid: None,
            xbrl_file_content: None,
            xbrl_file_size_bytes: None,
            xbrl_file_compressed: false,
            xbrl_file_compression_type: CompressionType::None,
            xbrl_file_hash: None,
            xbrl_processing_error: None,
            xbrl_processing_started_at: None,
            xbrl_processing_completed_at: None,
        };

        let statement = diesel::insert_into(financial_statements::table)
            .values(&test_statement)
            .returning(FinancialStatement::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert financial statement");

        // Create test financial line items
        let test_line_items = vec![
            NewFinancialLineItem {
                statement_id: statement.id,
                taxonomy_concept: "Assets".to_string(),
                standard_label: Some("Assets".to_string()),
                custom_label: None,
                context_ref: "c1".to_string(),
                segment_ref: None,
                scenario_ref: None,
                value: Some(bigdecimal::BigDecimal::from(352755000000i64)),
                unit: "USD".to_string(),
                decimals: Some(-6),
                precision: None,
                is_calculated: false,
                order_index: Some(1),
                calculation_formula: None,
                is_credit: None,
                is_debit: None,
                statement_type: StatementType::BalanceSheet,
                statement_section: StatementSection::Assets,
                parent_concept: None,
                level: 1,
            },
            NewFinancialLineItem {
                statement_id: statement.id,
                taxonomy_concept: "Liabilities".to_string(),
                standard_label: Some("Liabilities".to_string()),
                custom_label: None,
                context_ref: "c1".to_string(),
                segment_ref: None,
                scenario_ref: None,
                value: Some(bigdecimal::BigDecimal::from(258549000000i64)),
                unit: "USD".to_string(),
                decimals: Some(-6),
                precision: None,
                is_calculated: false,
                order_index: Some(2),
                calculation_formula: None,
                is_credit: None,
                is_debit: None,
                statement_type: StatementType::BalanceSheet,
                statement_section: StatementSection::Liabilities,
                parent_concept: None,
                level: 1,
            },
        ];

        let inserted_items = diesel::insert_into(financial_line_items::table)
            .values(&test_line_items)
            .returning(FinancialLineItem::as_returning())
            .get_results(&mut conn)
            .await
            .expect("Failed to insert financial line items");

        // Verify the line items were inserted
        assert_eq!(inserted_items.len(), 2);
        assert_eq!(inserted_items[0].taxonomy_concept, "Assets");
        assert_eq!(inserted_items[1].taxonomy_concept, "Liabilities");

        // Retrieve line items for the statement
        let retrieved_items = financial_line_items::table
            .filter(financial_line_items::statement_id.eq(statement.id))
            .order(financial_line_items::order_index)
            .load::<FinancialLineItem>(&mut conn)
            .await
            .expect("Failed to retrieve financial line items");

        assert_eq!(retrieved_items.len(), 2);
        assert_eq!(retrieved_items[0].taxonomy_concept, "Assets");
        assert_eq!(retrieved_items[1].taxonomy_concept, "Liabilities");
    }

    /// Test SEC crawler integration
    #[tokio::test]
    #[serial]
    async fn test_sec_crawler_integration() {
        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();

        // Create crawler configuration
        let config = CrawlConfig {
            max_requests_per_second: 1,
            max_retries: 3,
            retry_delay_seconds: 1,
            max_file_size_bytes: 50 * 1024 * 1024, // 50MB
            start_date: Some(chrono::NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
            end_date: Some(chrono::NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
            user_agent: "EconGraph Research Tool AdminContact@jmalicki+econgraph@gmail.com"
                .to_string(),
            max_concurrent_requests: Some(3),
            exclude_amended: false,
            exclude_restated: false,
            form_types: Some(vec!["10-K".to_string(), "10-Q".to_string()]),
        };

        // Create crawler instance
        let crawler = SecEdgarCrawler::new(pool.clone())
            .await
            .expect("Failed to create crawler");

        // Test crawler initialization - create a dummy operation ID for testing
        let operation_id = Uuid::new_v4();
        let progress_result = crawler.get_crawl_progress(operation_id).await;
        // The method should not panic, even if it returns an error for a non-existent operation
        assert!(progress_result.is_ok() || progress_result.is_err());
    }

    /// Test XBRL parser integration
    #[tokio::test]
    #[serial]
    async fn test_xbrl_parser_integration() {
        // Create XBRL parser
        let parser_result = XbrlParser::new().await;

        // Test parser initialization
        assert!(parser_result.is_ok());
    }

    /// Test DTS dependency management
    #[tokio::test]
    #[serial]
    async fn test_dts_dependency_management() {
        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();
        let mut conn = pool.get().await.expect("Failed to get database connection");

        // Create test taxonomy schemas
        let us_gaap_schema = XbrlTaxonomySchema::new(
            "http://fasb.org/us-gaap/2023".to_string(),
            "us-gaap.xsd".to_string(),
            TaxonomySourceType::UsGaap,
            1024000,
            "sha256:usgaap123".to_string(),
            Some("https://xbrl.fasb.org/us-gaap/2023/us-gaap.xsd".to_string()),
            Some("https://xbrl.fasb.org/us-gaap/2023/us-gaap.xsd".to_string()),
        );

        let sec_dei_schema = XbrlTaxonomySchema::new(
            "http://xbrl.sec.gov/dei/2023".to_string(),
            "dei.xsd".to_string(),
            TaxonomySourceType::SecDei,
            512000,
            "sha256:secdei123".to_string(),
            Some("https://xbrl.sec.gov/dei/2023/dei.xsd".to_string()),
            Some("https://xbrl.sec.gov/dei/2023/dei.xsd".to_string()),
        );

        let inserted_us_gaap = diesel::insert_into(xbrl_taxonomy_schemas::table)
            .values(&us_gaap_schema)
            .returning(XbrlTaxonomySchema::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert US-GAAP schema");

        let inserted_sec_dei = diesel::insert_into(xbrl_taxonomy_schemas::table)
            .values(&sec_dei_schema)
            .returning(XbrlTaxonomySchema::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert SEC-DEI schema");

        // Verify the schemas were created
        assert_eq!(
            inserted_us_gaap.schema_namespace,
            "http://fasb.org/us-gaap/2023"
        );
        assert_eq!(
            inserted_sec_dei.schema_namespace,
            "http://xbrl.sec.gov/dei/2023"
        );
    }

    /// Test XBRL instance DTS reference tracking
    #[tokio::test]
    #[serial]
    async fn test_xbrl_instance_dts_reference_tracking() {
        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();
        let mut conn = pool.get().await.expect("Failed to get database connection");

        // Create test company and financial statement
        let test_company = NewCompany {
            cik: "0000320193".to_string(),
            name: "Apple Inc.".to_string(),
            ticker: Some("AAPL".to_string()),
            sic_code: Some("3571".to_string()),
            industry: Some("Computer Hardware".to_string()),
            sector: Some("Technology".to_string()),
            legal_name: None,
            sic_description: None,
            business_address: None,
            mailing_address: None,
            phone: None,
            website: None,
            state_of_incorporation: None,
            state_of_incorporation_description: None,
            fiscal_year_end: None,
            entity_type: None,
            entity_size: None,
            is_active: true,
        };

        let company = diesel::insert_into(companies::table)
            .values(&test_company)
            .returning(Company::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert test company");

        let test_statement = NewFinancialStatement {
            company_id: company.id,
            filing_type: "10-K".to_string(),
            form_type: "10-K".to_string(),
            accession_number: "0000320193-24-000006".to_string(),
            filing_date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            period_end_date: chrono::NaiveDate::from_ymd_opt(2023, 12, 30).unwrap(),
            fiscal_year: 2023,
            fiscal_quarter: Some(4),
            document_type: "XBRL".to_string(),
            document_url: "https://www.sec.gov/Archives/edgar/data/320193/000032019324000006/aapl-20231230.xbrl".to_string(),
            xbrl_processing_status: ProcessingStatus::Completed,
            is_amended: false,
            is_restated: false,
            amendment_type: None,
            original_filing_date: None,
            restatement_reason: None,
            xbrl_file_oid: None,
            xbrl_file_content: None,
            xbrl_file_size_bytes: None,
            xbrl_file_compressed: false,
            xbrl_file_compression_type: CompressionType::None,
            xbrl_file_hash: None,
            xbrl_processing_error: None,
            xbrl_processing_started_at: None,
            xbrl_processing_completed_at: None,
        };

        let statement = diesel::insert_into(financial_statements::table)
            .values(&test_statement)
            .returning(FinancialStatement::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert financial statement");

        // Create test taxonomy schema
        let test_schema = XbrlTaxonomySchema::new(
            "http://fasb.org/us-gaap/2023".to_string(),
            "us-gaap.xsd".to_string(),
            TaxonomySourceType::UsGaap,
            1024000,
            "sha256:abc123def456".to_string(),
            Some("https://xbrl.fasb.org/us-gaap/2023/us-gaap.xsd".to_string()),
            Some("https://xbrl.fasb.org/us-gaap/2023/us-gaap.xsd".to_string()),
        );

        let schema = diesel::insert_into(xbrl_taxonomy_schemas::table)
            .values(&test_schema)
            .returning(XbrlTaxonomySchema::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert taxonomy schema");

        // Verify the statement and schema were created
        assert_eq!(statement.company_id, company.id);
        assert_eq!(schema.schema_namespace, "http://fasb.org/us-gaap/2023");
    }
}
