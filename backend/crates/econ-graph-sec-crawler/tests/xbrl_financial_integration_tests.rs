//! XBRL Financial Statement Integration Tests
//! 
//! These tests verify the complete end-to-end flow from SEC EDGAR crawling
//! through XBRL parsing, DTS management, and financial data processing using testcontainers.

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use econ_graph_core::{
        models::*,
        schema::*,
        enums::*,
        test_utils::TestContainer,
    };
    use econ_graph_sec_crawler::{
        SecEdgarCrawler,
        XbrlParser,
        DtsManager,
        models::{CrawlConfig, DtsReference},
        storage::XbrlStorage,
    };
    use serial_test::serial;
    use std::sync::Arc;

    /// Test XBRL taxonomy schema storage and retrieval with testcontainers
    #[tokio::test]
    #[serial]
    async fn test_xbrl_taxonomy_schema_storage() {
        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();

        // Test data for XBRL taxonomy schema
        let test_schema = NewXbrlTaxonomySchema {
            namespace: "http://fasb.org/us-gaap/2023".to_string(),
            prefix: "us-gaap".to_string(),
            version: "2023".to_string(),
            file_type: TaxonomyFileType::Schema,
            source_type: TaxonomySourceType::Standard,
            file_path: "/taxonomies/us-gaap/2023/us-gaap.xsd".to_string(),
            file_size: 1024000,
            content_hash: "sha256:abc123def456".to_string(),
            download_url: Some("https://xbrl.fasb.org/us-gaap/2023/us-gaap.xsd".to_string()),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        // Insert the schema
        let mut conn = pool.get().await.expect("Failed to get database connection");
        let inserted_schema = diesel::insert_into(xbrl_taxonomy_schemas::table)
            .values(&test_schema)
            .returning(XbrlTaxonomySchema::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert taxonomy schema");

        assert_eq!(inserted_schema.namespace, test_schema.namespace);
        assert_eq!(inserted_schema.prefix, test_schema.prefix);
        assert_eq!(inserted_schema.version, test_schema.version);

        // Retrieve and verify
        let retrieved_schema = xbrl_taxonomy_schemas::table
            .filter(xbrl_taxonomy_schemas::id.eq(inserted_schema.id))
            .first::<XbrlTaxonomySchema>(&mut conn)
            .await
            .expect("Failed to retrieve taxonomy schema");

        assert_eq!(retrieved_schema.namespace, test_schema.namespace);
        assert_eq!(retrieved_schema.prefix, test_schema.prefix);

        println!("✅ XBRL taxonomy schema storage test completed successfully!");
    }

    /// Test XBRL instance document processing end-to-end with testcontainers
    #[tokio::test]
    #[serial]
    async fn test_xbrl_instance_processing() {
        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();
        let mut conn = pool.get().await.expect("Failed to get database connection");

        // Create test company
        let test_company = NewCompany {
            cik: "0000320193".to_string(),
            name: "Apple Inc.".to_string(),
            ticker: "AAPL".to_string(),
            sic_code: Some("3571".to_string()),
            industry: Some("Computer Hardware".to_string()),
            sector: Some("Technology".to_string()),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let company = diesel::insert_into(companies::table)
            .values(&test_company)
            .returning(Company::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert test company");

        // Create test XBRL instance
        let test_instance = NewXbrlInstance {
            company_id: company.id,
            accession_number: "0000320193-24-000006".to_string(),
            filing_date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            period_end: chrono::NaiveDate::from_ymd_opt(2023, 12, 30).unwrap(),
            document_type: "10-K".to_string(),
            file_path: "/xbrl/instances/aapl_10k_2023.xml".to_string(),
            file_size: 2048000,
            content_hash: "sha256:def456ghi789".to_string(),
            processing_status: ProcessingStatus::Downloaded,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let instance = diesel::insert_into(xbrl_instances::table)
            .values(&test_instance)
            .returning(XbrlInstance::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert XBRL instance");

        // Create test XBRL facts
        let test_facts = vec![
            NewXbrlFact {
                instance_id: instance.id,
                concept_name: "Assets".to_string(),
                value: "352755000000".to_string(),
                unit: Some("USD".to_string()),
                decimals: Some(-6),
                context_id: "c1".to_string(),
                period_start: None,
                period_end: Some(chrono::NaiveDate::from_ymd_opt(2023, 12, 30).unwrap()),
                is_instant: true,
                is_duration: false,
                created_at: chrono::Utc::now().naive_utc(),
            },
            NewXbrlFact {
                instance_id: instance.id,
                concept_name: "Revenues".to_string(),
                value: "383285000000".to_string(),
                unit: Some("USD".to_string()),
                decimals: Some(-6),
                context_id: "c2".to_string(),
                period_start: Some(chrono::NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
                period_end: Some(chrono::NaiveDate::from_ymd_opt(2023, 12, 30).unwrap()),
                is_instant: false,
                is_duration: true,
                created_at: chrono::Utc::now().naive_utc(),
            },
        ];

        let inserted_facts: Vec<XbrlFact> = diesel::insert_into(xbrl_facts::table)
            .values(&test_facts)
            .returning(XbrlFact::as_returning())
            .get_results(&mut conn)
            .await
            .expect("Failed to insert XBRL facts");

        assert_eq!(inserted_facts.len(), 2);
        assert_eq!(inserted_facts[0].concept_name, "Assets");
        assert_eq!(inserted_facts[1].concept_name, "Revenues");

        // Verify facts can be retrieved by company and period
        let retrieved_facts = xbrl_facts::table
            .inner_join(xbrl_instances::table.on(xbrl_facts::instance_id.eq(xbrl_instances::id)))
            .filter(xbrl_instances::company_id.eq(company.id))
            .filter(xbrl_facts::period_end.eq(chrono::NaiveDate::from_ymd_opt(2023, 12, 30).unwrap()))
            .load::<(XbrlFact, XbrlInstance)>(&mut conn)
            .await
            .expect("Failed to retrieve XBRL facts");

        assert_eq!(retrieved_facts.len(), 1);
        assert_eq!(retrieved_facts[0].0.concept_name, "Assets");

        println!("✅ XBRL instance processing test completed successfully!");
    }

    /// Test financial statement generation from XBRL data with testcontainers
    #[tokio::test]
    #[serial]
    async fn test_financial_statement_generation() {
        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();
        let mut conn = pool.get().await.expect("Failed to get database connection");

        // Create test company and XBRL instance
        let test_company = NewCompany {
            cik: "0000320193".to_string(),
            name: "Apple Inc.".to_string(),
            ticker: "AAPL".to_string(),
            sic_code: Some("3571".to_string()),
            industry: Some("Computer Hardware".to_string()),
            sector: Some("Technology".to_string()),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let company = diesel::insert_into(companies::table)
            .values(&test_company)
            .returning(Company::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert test company");

        let test_instance = NewXbrlInstance {
            company_id: company.id,
            accession_number: "0000320193-24-000006".to_string(),
            filing_date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            period_end: chrono::NaiveDate::from_ymd_opt(2023, 12, 30).unwrap(),
            document_type: "10-K".to_string(),
            file_path: "/xbrl/instances/aapl_10k_2023.xml".to_string(),
            file_size: 2048000,
            content_hash: "sha256:def456ghi789".to_string(),
            processing_status: ProcessingStatus::Processed,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let instance = diesel::insert_into(xbrl_instances::table)
            .values(&test_instance)
            .returning(XbrlInstance::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert XBRL instance");

        // Create comprehensive XBRL facts for balance sheet
        let balance_sheet_facts = vec![
            // Assets
            NewXbrlFact {
                instance_id: instance.id,
                concept_name: "Assets".to_string(),
                value: "352755000000".to_string(),
                unit: Some("USD".to_string()),
                decimals: Some(-6),
                context_id: "c1".to_string(),
                period_start: None,
                period_end: Some(chrono::NaiveDate::from_ymd_opt(2023, 12, 30).unwrap()),
                is_instant: true,
                is_duration: false,
                created_at: chrono::Utc::now().naive_utc(),
            },
            NewXbrlFact {
                instance_id: instance.id,
                concept_name: "AssetsCurrent".to_string(),
                value: "143566000000".to_string(),
                unit: Some("USD".to_string()),
                decimals: Some(-6),
                context_id: "c1".to_string(),
                period_start: None,
                period_end: Some(chrono::NaiveDate::from_ymd_opt(2023, 12, 30).unwrap()),
                is_instant: true,
                is_duration: false,
                created_at: chrono::Utc::now().naive_utc(),
            },
            // Liabilities
            NewXbrlFact {
                instance_id: instance.id,
                concept_name: "Liabilities".to_string(),
                value: "258549000000".to_string(),
                unit: Some("USD".to_string()),
                decimals: Some(-6),
                context_id: "c1".to_string(),
                period_start: None,
                period_end: Some(chrono::NaiveDate::from_ymd_opt(2023, 12, 30).unwrap()),
                is_instant: true,
                is_duration: false,
                created_at: chrono::Utc::now().naive_utc(),
            },
            NewXbrlFact {
                instance_id: instance.id,
                concept_name: "LiabilitiesCurrent".to_string(),
                value: "145308000000".to_string(),
                unit: Some("USD".to_string()),
                decimals: Some(-6),
                context_id: "c1".to_string(),
                period_start: None,
                period_end: Some(chrono::NaiveDate::from_ymd_opt(2023, 12, 30).unwrap()),
                is_instant: true,
                is_duration: false,
                created_at: chrono::Utc::now().naive_utc(),
            },
            // Equity
            NewXbrlFact {
                instance_id: instance.id,
                concept_name: "StockholdersEquity".to_string(),
                value: "94206000000".to_string(),
                unit: Some("USD".to_string()),
                decimals: Some(-6),
                context_id: "c1".to_string(),
                period_start: None,
                period_end: Some(chrono::NaiveDate::from_ymd_opt(2023, 12, 30).unwrap()),
                is_instant: true,
                is_duration: false,
                created_at: chrono::Utc::now().naive_utc(),
            },
        ];

        let _facts = diesel::insert_into(xbrl_facts::table)
            .values(&balance_sheet_facts)
            .returning(XbrlFact::as_returning())
            .get_results(&mut conn)
            .await
            .expect("Failed to insert balance sheet facts");

        // Create financial statement
        let test_statement = NewFinancialStatement {
            company_id: company.id,
            statement_type: FinancialStatementType::BalanceSheet,
            period_start: None,
            period_end: chrono::NaiveDate::from_ymd_opt(2023, 12, 30).unwrap(),
            filing_date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            source_instance_id: Some(instance.id),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let statement = diesel::insert_into(financial_statements::table)
            .values(&test_statement)
            .returning(FinancialStatement::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert financial statement");

        // Create financial line items
        let line_items = vec![
            NewFinancialLineItem {
                statement_id: statement.id,
                concept_name: "Assets".to_string(),
                display_name: "Total Assets".to_string(),
                value: 352755000000.0,
                unit: Some("USD".to_string()),
                decimals: Some(-6),
                is_instant: true,
                is_duration: false,
                parent_concept: None,
                level: 0,
                created_at: chrono::Utc::now().naive_utc(),
            },
            NewFinancialLineItem {
                statement_id: statement.id,
                concept_name: "AssetsCurrent".to_string(),
                display_name: "Current Assets".to_string(),
                value: 143566000000.0,
                unit: Some("USD".to_string()),
                decimals: Some(-6),
                is_instant: true,
                is_duration: false,
                parent_concept: Some("Assets".to_string()),
                level: 1,
                created_at: chrono::Utc::now().naive_utc(),
            },
            NewFinancialLineItem {
                statement_id: statement.id,
                concept_name: "Liabilities".to_string(),
                display_name: "Total Liabilities".to_string(),
                value: 258549000000.0,
                unit: Some("USD".to_string()),
                decimals: Some(-6),
                is_instant: true,
                is_duration: false,
                parent_concept: None,
                level: 0,
                created_at: chrono::Utc::now().naive_utc(),
            },
            NewFinancialLineItem {
                statement_id: statement.id,
                concept_name: "StockholdersEquity".to_string(),
                display_name: "Stockholders' Equity".to_string(),
                value: 94206000000.0,
                unit: Some("USD".to_string()),
                decimals: Some(-6),
                is_instant: true,
                is_duration: false,
                parent_concept: None,
                level: 0,
                created_at: chrono::Utc::now().naive_utc(),
            },
        ];

        let inserted_items = diesel::insert_into(financial_line_items::table)
            .values(&line_items)
            .returning(FinancialLineItem::as_returning())
            .get_results(&mut conn)
            .await
            .expect("Failed to insert financial line items");

        assert_eq!(inserted_items.len(), 4);

        // Verify balance sheet equation: Assets = Liabilities + Equity
        let total_assets = inserted_items.iter()
            .find(|item| item.concept_name == "Assets")
            .unwrap()
            .value;
        
        let total_liabilities = inserted_items.iter()
            .find(|item| item.concept_name == "Liabilities")
            .unwrap()
            .value;
        
        let total_equity = inserted_items.iter()
            .find(|item| item.concept_name == "StockholdersEquity")
            .unwrap()
            .value;

        // Allow for small rounding differences
        let difference = (total_assets - (total_liabilities + total_equity)).abs();
        assert!(difference < 1.0, "Balance sheet equation not balanced: Assets={}, Liabilities={}, Equity={}, Difference={}", 
               total_assets, total_liabilities, total_equity, difference);

        println!("✅ Financial statement generation test completed successfully!");
    }

    /// Test SEC crawler integration with XBRL processing using testcontainers
    #[tokio::test]
    #[serial]
    async fn test_sec_crawler_xbrl_integration() {
        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();
        
        // Create crawler configuration
        let config = CrawlConfig {
            user_agent: "EconGraph Research Tool AdminContact@jmalicki+econgraph@gmail.com".to_string(),
            max_concurrent_requests: Some(3),
            rate_limit_delay: std::time::Duration::from_millis(100),
            timeout: std::time::Duration::from_secs(30),
        };

        // Create crawler instance
        let crawler = SecEdgarCrawler::new(pool.clone(), config);
        
        // Test CIK for Apple Inc.
        let cik = "0000320193".to_string();
        
        // Test company discovery and creation
        let company_result = crawler.discover_company(&cik).await;
        assert!(company_result.is_ok());
        
        let company = company_result.unwrap();
        assert_eq!(company.cik, cik);
        assert_eq!(company.name, "Apple Inc.");
        assert_eq!(company.ticker, "AAPL");

        // Verify company was stored in database
        let mut conn = pool.get().await.expect("Failed to get database connection");
        let stored_company = companies::table
            .filter(companies::cik.eq(&cik))
            .first::<Company>(&mut conn)
            .await
            .expect("Failed to retrieve company from database");
        
        assert_eq!(stored_company.name, "Apple Inc.");

        println!("✅ SEC crawler XBRL integration test completed successfully!");
    }

    /// Test XBRL parser integration with DTS manager using testcontainers
    #[tokio::test]
    #[serial]
    async fn test_xbrl_parser_dts_integration() {
        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();
        
        // Create DTS manager
        let dts_manager = DtsManager::new(pool.clone());
        
        // Create XBRL parser
        let parser = XbrlParser::new(dts_manager);
        
        // Test parsing a simple XBRL instance
        let test_xbrl_content = r#"
        <?xml version="1.0" encoding="UTF-8"?>
        <xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                     xmlns:us-gaap="http://fasb.org/us-gaap/2023"
                     xmlns:dei="http://xbrl.sec.gov/dei/2023">
            <xbrli:context id="c1">
                <xbrli:entity>
                    <xbrli:identifier scheme="http://www.sec.gov/CIK">0000320193</xbrli:identifier>
                </xbrli:entity>
                <xbrli:period>
                    <xbrli:instant>2023-12-30</xbrli:instant>
                </xbrli:period>
            </xbrli:context>
            <us-gaap:Assets contextRef="c1" unitRef="u1" decimals="-6">352755000000</us-gaap:Assets>
            <xbrli:unit id="u1">
                <xbrli:measure>iso4217:USD</xbrli:measure>
            </xbrli:unit>
        </xbrli:xbrl>
        "#;

        // Parse the XBRL content
        let result = parser.parse_xbrl_content(test_xbrl_content.as_bytes()).await;
        
        // Verify parsing succeeded
        assert!(result.is_ok());
        
        let parsed_data = result.unwrap();
        assert_eq!(parsed_data.facts.len(), 1);
        assert_eq!(parsed_data.facts[0].concept_name, "Assets");
        assert_eq!(parsed_data.facts[0].value, "352755000000");

        println!("✅ XBRL parser DTS integration test completed successfully!");
    }

    /// Test financial ratio calculation from XBRL data using testcontainers
    #[tokio::test]
    #[serial]
    async fn test_financial_ratio_calculation() {
        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();
        let mut conn = pool.get().await.expect("Failed to get database connection");

        // Create test company
        let test_company = NewCompany {
            cik: "0000320193".to_string(),
            name: "Apple Inc.".to_string(),
            ticker: "AAPL".to_string(),
            sic_code: Some("3571".to_string()),
            industry: Some("Computer Hardware".to_string()),
            sector: Some("Technology".to_string()),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let company = diesel::insert_into(companies::table)
            .values(&test_company)
            .returning(Company::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert test company");

        // Create financial statement with line items for ratio calculation
        let test_statement = NewFinancialStatement {
            company_id: company.id,
            statement_type: FinancialStatementType::IncomeStatement,
            period_start: Some(chrono::NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
            period_end: chrono::NaiveDate::from_ymd_opt(2023, 12, 30).unwrap(),
            filing_date: chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            source_instance_id: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let statement = diesel::insert_into(financial_statements::table)
            .values(&test_statement)
            .returning(FinancialStatement::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert financial statement");

        // Create line items for profitability ratios
        let line_items = vec![
            NewFinancialLineItem {
                statement_id: statement.id,
                concept_name: "Revenues".to_string(),
                display_name: "Net Sales".to_string(),
                value: 383285000000.0,
                unit: Some("USD".to_string()),
                decimals: Some(-6),
                is_instant: false,
                is_duration: true,
                parent_concept: None,
                level: 0,
                created_at: chrono::Utc::now().naive_utc(),
            },
            NewFinancialLineItem {
                statement_id: statement.id,
                concept_name: "NetIncomeLoss".to_string(),
                display_name: "Net Income".to_string(),
                value: 96995000000.0,
                unit: Some("USD".to_string()),
                decimals: Some(-6),
                is_instant: false,
                is_duration: true,
                parent_concept: None,
                level: 0,
                created_at: chrono::Utc::now().naive_utc(),
            },
        ];

        let _items = diesel::insert_into(financial_line_items::table)
            .values(&line_items)
            .returning(FinancialLineItem::as_returning())
            .get_results(&mut conn)
            .await
            .expect("Failed to insert line items");

        // Create financial ratio
        let test_ratio = NewFinancialRatio {
            company_id: company.id,
            ratio_name: "netProfitMargin".to_string(),
            ratio_display_name: "Net Profit Margin".to_string(),
            value: 0.253, // 25.3% = Net Income / Revenue
            period_start: Some(chrono::NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
            period_end: chrono::NaiveDate::from_ymd_opt(2023, 12, 30).unwrap(),
            calculation_method: "Net Income / Revenue".to_string(),
            industry_benchmark: Some(0.12), // 12% industry average
            percentile_rank: Some(95), // Top 5% of industry
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let ratio = diesel::insert_into(financial_ratios::table)
            .values(&test_ratio)
            .returning(FinancialRatio::as_returning())
            .get_result(&mut conn)
            .await
            .expect("Failed to insert financial ratio");

        // Verify ratio calculation
        assert_eq!(ratio.ratio_name, "netProfitMargin");
        assert!((ratio.value - 0.253).abs() < 0.001);
        assert_eq!(ratio.percentile_rank, Some(95));

        // Verify ratio can be retrieved by company and period
        let retrieved_ratios = financial_ratios::table
            .filter(financial_ratios::company_id.eq(company.id))
            .filter(financial_ratios::period_end.eq(chrono::NaiveDate::from_ymd_opt(2023, 12, 30).unwrap()))
            .load::<FinancialRatio>(&mut conn)
            .await
            .expect("Failed to retrieve financial ratios");

        assert_eq!(retrieved_ratios.len(), 1);
        assert_eq!(retrieved_ratios[0].ratio_name, "netProfitMargin");

        println!("✅ Financial ratio calculation test completed successfully!");
    }

    /// Test complete SEC crawler pipeline with XBRL processing using testcontainers
    #[tokio::test]
    #[serial]
    async fn test_complete_sec_crawler_xbrl_pipeline() {
        let container = TestContainer::new().await;
        container.clean_database().await.unwrap();
        let pool = container.pool();
        let mut conn = pool.get().await.expect("Failed to get database connection");

        // Create crawler configuration
        let config = CrawlConfig {
            user_agent: "EconGraph Research Tool AdminContact@jmalicki+econgraph@gmail.com".to_string(),
            max_concurrent_requests: Some(3),
            rate_limit_delay: std::time::Duration::from_millis(100),
            timeout: std::time::Duration::from_secs(30),
        };

        // Create crawler instance
        let crawler = SecEdgarCrawler::new(pool.clone(), config);
        
        // Test CIK for Apple Inc.
        let cik = "0000320193".to_string();
        
        // Step 1: Test company discovery and creation
        let company_result = crawler.discover_company(&cik).await;
        assert!(company_result.is_ok());
        
        let company = company_result.unwrap();
        assert_eq!(company.cik, cik);
        assert_eq!(company.name, "Apple Inc.");
        assert_eq!(company.ticker, "AAPL");

        // Verify company was stored in database
        let stored_company = companies::table
            .filter(companies::cik.eq(&cik))
            .first::<Company>(&mut conn)
            .await
            .expect("Failed to retrieve company from database");
        
        assert_eq!(stored_company.name, "Apple Inc.");

        // Step 2: Test XBRL instance discovery (mock for integration test)
        let instances_result = crawler.discover_xbrl_instances(&cik, "10-K", Some(1)).await;
        assert!(instances_result.is_ok());
        
        let instances = instances_result.unwrap();
        assert!(!instances.is_empty());
        
        let instance = &instances[0];
        assert_eq!(instance.company_id, company.id);
        assert_eq!(instance.document_type, "10-K");

        // Step 3: Test XBRL parsing
        let parser = XbrlParser::new(DtsManager::new(pool.clone()));
        let parse_result = parser.parse_xbrl_file(&instance.file_path).await;
        assert!(parse_result.is_ok());
        
        let parsed_data = parse_result.unwrap();
        assert!(!parsed_data.facts.is_empty());

        // Verify facts were stored
        let stored_facts = xbrl_facts::table
            .filter(xbrl_facts::instance_id.eq(instance.id))
            .load::<XbrlFact>(&mut conn)
            .await
            .expect("Failed to retrieve XBRL facts");
        
        assert_eq!(stored_facts.len(), parsed_data.facts.len());

        // Step 4: Test financial statement generation
        let statement_result = crawler.generate_financial_statements(&instance.id).await;
        assert!(statement_result.is_ok());
        
        let statements = statement_result.unwrap();
        assert!(!statements.is_empty());

        // Verify financial statements were created
        let stored_statements = financial_statements::table
            .filter(financial_statements::source_instance_id.eq(instance.id))
            .load::<FinancialStatement>(&mut conn)
            .await
            .expect("Failed to retrieve financial statements");
        
        assert_eq!(stored_statements.len(), statements.len());

        // Step 5: Test financial ratio calculation
        let ratios_result = crawler.calculate_financial_ratios(&company.id, &instance.period_end).await;
        assert!(ratios_result.is_ok());
        
        let ratios = ratios_result.unwrap();
        assert!(!ratios.is_empty());

        // Verify ratios were calculated and stored
        let stored_ratios = financial_ratios::table
            .filter(financial_ratios::company_id.eq(company.id))
            .filter(financial_ratios::period_end.eq(instance.period_end))
            .load::<FinancialRatio>(&mut conn)
            .await
            .expect("Failed to retrieve financial ratios");
        
        assert_eq!(stored_ratios.len(), ratios.len());

        println!("✅ Complete SEC crawler XBRL pipeline test completed successfully!");
        println!("   - Company discovery: ✅");
        println!("   - XBRL instance discovery: ✅");
        println!("   - XBRL parsing: ✅");
        println!("   - Financial statement generation: ✅");
        println!("   - Financial ratio calculation: ✅");
    }
}
