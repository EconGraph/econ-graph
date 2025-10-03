/// Comprehensive tests for SEC Crawler GraphQL API
/// 
/// This module contains integration tests for all GraphQL endpoints,
/// including queries, mutations, and error handling scenarios.

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test;
    use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
    use serde_json::json;
    use std::sync::Arc;
    use uuid::Uuid;

    /// Test helper for creating test GraphQL requests
    async fn create_test_app() -> impl actix_web::dev::Service<actix_web::dev::ServiceRequest, Response = actix_web::dev::ServiceResponse, Error = actix_web::Error> {
        // Initialize test database
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:password@localhost/econ_graph_test".to_string());
        
        let pool = create_pool(&database_url).await.unwrap();
        let schema = create_schema(pool).await;
        
        // Create test app
        App::new()
            .app_data(web::Data::new(schema))
            .service(web::resource("/graphql").to(graphql_handler))
            .service(web::resource("/graphql/playground").to(playground_handler))
    }

    /// Test helper for executing GraphQL queries
    async fn execute_graphql_query(
        app: &impl actix_web::dev::Service<actix_web::dev::ServiceRequest, Response = actix_web::dev::ServiceResponse, Error = actix_web::Error>,
        query: &str,
        variables: Option<serde_json::Value>,
    ) -> actix_web::test::TestRequest {
        let mut request = test::TestRequest::post()
            .uri("/graphql")
            .set_json(&json!({
                "query": query,
                "variables": variables.unwrap_or(json!({}))
            }));
        
        request
    }

    #[actix_web::test]
    async fn test_search_companies_query() {
        let app = create_test_app().await;
        
        let query = r#"
            query SearchCompanies($input: CompanySearchInput!) {
                searchCompanies(input: $input) {
                    nodes {
                        id
                        cik
                        ticker
                        name
                        legalName
                        industry
                        sector
                        isActive
                    }
                    totalCount
                    hasNextPage
                    hasPreviousPage
                }
            }
        "#;
        
        let variables = json!({
            "input": {
                "query": "Apple",
                "limit": 10,
                "includeInactive": false
            }
        });
        
        let req = execute_graphql_query(&app, query, Some(variables)).await;
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
        
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["data"]["searchCompanies"].is_object());
        assert!(body["data"]["searchCompanies"]["nodes"].is_array());
        assert!(body["data"]["searchCompanies"]["totalCount"].is_number());
    }

    #[actix_web::test]
    async fn test_search_companies_fuzzy_matching() {
        let app = create_test_app().await;
        
        let query = r#"
            query SearchCompanies($input: CompanySearchInput!) {
                searchCompanies(input: $input) {
                    nodes {
                        id
                        name
                        ticker
                    }
                }
            }
        "#;
        
        // Test fuzzy search with common misspellings
        let fuzzy_queries = vec!["Appel", "Aple", "Apple Computer"];
        
        for query_text in fuzzy_queries {
            let variables = json!({
                "input": {
                    "query": query_text,
                    "limit": 10,
                    "includeInactive": false
                }
            });
            
            let req = execute_graphql_query(&app, query, Some(variables)).await;
            let resp = test::call_service(&app, req).await;
            
            assert!(resp.status().is_success());
            
            let body: serde_json::Value = test::read_body_json(resp).await;
            assert!(body["data"]["searchCompanies"]["nodes"].is_array());
        }
    }

    #[actix_web::test]
    async fn test_search_companies_by_ticker() {
        let app = create_test_app().await;
        
        let query = r#"
            query SearchCompanies($input: CompanySearchInput!) {
                searchCompanies(input: $input) {
                    nodes {
                        id
                        name
                        ticker
                    }
                }
            }
        "#;
        
        let variables = json!({
            "input": {
                "query": "AAPL",
                "limit": 10,
                "includeInactive": false
            }
        });
        
        let req = execute_graphql_query(&app, query, Some(variables)).await;
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
        
        let body: serde_json::Value = test::read_body_json(resp).await;
        let nodes = body["data"]["searchCompanies"]["nodes"].as_array().unwrap();
        
        // All returned companies should have AAPL ticker
        for node in nodes {
            if let Some(ticker) = node["ticker"].as_str() {
                assert!(ticker.to_lowercase().contains("aapl"));
            }
        }
    }

    #[actix_web::test]
    async fn test_search_companies_by_cik() {
        let app = create_test_app().await;
        
        let query = r#"
            query SearchCompanies($input: CompanySearchInput!) {
                searchCompanies(input: $input) {
                    nodes {
                        id
                        cik
                        name
                    }
                }
            }
        "#;
        
        let variables = json!({
            "input": {
                "query": "0000320193",
                "limit": 10,
                "includeInactive": false
            }
        });
        
        let req = execute_graphql_query(&app, query, Some(variables)).await;
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
        
        let body: serde_json::Value = test::read_body_json(resp).await;
        let nodes = body["data"]["searchCompanies"]["nodes"].as_array().unwrap();
        
        // All returned companies should have matching CIK
        for node in nodes {
            let cik = node["cik"].as_str().unwrap();
            assert!(cik.contains("0000320193"));
        }
    }

    #[actix_web::test]
    async fn test_search_companies_limit() {
        let app = create_test_app().await;
        
        let query = r#"
            query SearchCompanies($input: CompanySearchInput!) {
                searchCompanies(input: $input) {
                    nodes {
                        id
                        name
                    }
                }
            }
        "#;
        
        let variables = json!({
            "input": {
                "query": "Inc",
                "limit": 5,
                "includeInactive": false
            }
        });
        
        let req = execute_graphql_query(&app, query, Some(variables)).await;
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
        
        let body: serde_json::Value = test::read_body_json(resp).await;
        let nodes = body["data"]["searchCompanies"]["nodes"].as_array().unwrap();
        
        // Should respect limit parameter
        assert!(nodes.len() <= 5);
    }

    #[actix_web::test]
    async fn test_search_companies_include_inactive() {
        let app = create_test_app().await;
        
        let query = r#"
            query SearchCompanies($input: CompanySearchInput!) {
                searchCompanies(input: $input) {
                    nodes {
                        id
                        name
                        isActive
                    }
                }
            }
        "#;
        
        // Test excluding inactive companies
        let variables_active = json!({
            "input": {
                "query": "Apple",
                "limit": 10,
                "includeInactive": false
            }
        });
        
        let req_active = execute_graphql_query(&app, query, Some(variables_active)).await;
        let resp_active = test::call_service(&app, req_active).await;
        
        assert!(resp_active.status().is_success());
        
        let body_active: serde_json::Value = test::read_body_json(resp_active).await;
        let nodes_active = body_active["data"]["searchCompanies"]["nodes"].as_array().unwrap();
        
        // All companies should be active when includeInactive is false
        for node in nodes_active {
            assert!(node["isActive"].as_bool().unwrap());
        }
        
        // Test including inactive companies
        let variables_all = json!({
            "input": {
                "query": "Apple",
                "limit": 10,
                "includeInactive": true
            }
        });
        
        let req_all = execute_graphql_query(&app, query, Some(variables_all)).await;
        let resp_all = test::call_service(&app, req_all).await;
        
        assert!(resp_all.status().is_success());
        
        let body_all: serde_json::Value = test::read_body_json(resp_all).await;
        assert!(body_all["data"]["searchCompanies"]["nodes"].is_array());
    }

    #[actix_web::test]
    async fn test_company_query() {
        let app = create_test_app().await;
        
        let query = r#"
            query GetCompany($id: ID!) {
                company(id: $id) {
                    id
                    cik
                    ticker
                    name
                    legalName
                    industry
                    sector
                    isActive
                }
            }
        "#;
        
        let test_id = Uuid::new_v4().to_string();
        let variables = json!({
            "id": test_id
        });
        
        let req = execute_graphql_query(&app, query, Some(variables)).await;
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
        
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["data"]["company"].is_null()); // Should return null for non-existent company
    }

    #[actix_web::test]
    async fn test_company_financial_statements_query() {
        let app = create_test_app().await;
        
        let query = r#"
            query GetCompanyFinancialStatements($companyId: ID!, $limit: Int, $offset: Int) {
                companyFinancialStatements(companyId: $companyId, limit: $limit, offset: $offset) {
                    nodes {
                        id
                        statementType
                        period
                        fiscalYear
                        fiscalQuarter
                        totalRevenue
                        netIncome
                        totalAssets
                        totalLiabilities
                        shareholdersEquity
                    }
                    totalCount
                    hasNextPage
                    hasPreviousPage
                }
            }
        "#;
        
        let test_company_id = Uuid::new_v4().to_string();
        let variables = json!({
            "companyId": test_company_id,
            "limit": 10,
            "offset": 0
        });
        
        let req = execute_graphql_query(&app, query, Some(variables)).await;
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
        
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["data"]["companyFinancialStatements"]["nodes"].is_array());
        assert!(body["data"]["companyFinancialStatements"]["totalCount"].is_number());
        assert!(body["data"]["companyFinancialStatements"]["hasNextPage"].is_boolean());
        assert!(body["data"]["companyFinancialStatements"]["hasPreviousPage"].is_boolean());
    }

    #[actix_web::test]
    async fn test_trigger_sec_crawl_mutation() {
        let app = create_test_app().await;
        
        let mutation = r#"
            mutation TriggerSecCrawl($input: SecCrawlInput!) {
                triggerSecCrawl(input: $input) {
                    success
                    message
                    documentsProcessed
                    documentsSkipped
                    documentsFailed
                    totalSizeBytes
                    processingTimeMs
                    errors
                    warnings
                }
            }
        "#;
        
        let variables = json!({
            "input": {
                "companyId": Uuid::new_v4().to_string(),
                "startDate": "2023-01-01",
                "endDate": "2023-12-31",
                "formTypes": ["10-K", "10-Q", "8-K"],
                "maxDocuments": 100,
                "includeAmendments": true,
                "includeExhibits": false,
                "rateLimit": 10,
                "retryAttempts": 3,
                "timeout": 300
            }
        });
        
        let req = execute_graphql_query(&app, mutation, Some(variables)).await;
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
        
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["data"]["triggerSecCrawl"]["success"].is_boolean());
        assert!(body["data"]["triggerSecCrawl"]["message"].is_string());
        assert!(body["data"]["triggerSecCrawl"]["documentsProcessed"].is_number());
        assert!(body["data"]["triggerSecCrawl"]["documentsSkipped"].is_number());
        assert!(body["data"]["triggerSecCrawl"]["documentsFailed"].is_number());
        assert!(body["data"]["triggerSecCrawl"]["totalSizeBytes"].is_number());
        assert!(body["data"]["triggerSecCrawl"]["processingTimeMs"].is_number());
        assert!(body["data"]["triggerSecCrawl"]["errors"].is_array());
        assert!(body["data"]["triggerSecCrawl"]["warnings"].is_array());
    }

    #[actix_web::test]
    async fn test_import_sec_rss_mutation() {
        let app = create_test_app().await;
        
        let mutation = r#"
            mutation ImportSecRss($input: SecRssImportInput!) {
                importSecRss(input: $input) {
                    success
                    message
                    itemsProcessed
                    itemsSkipped
                    itemsFailed
                    newCompanies
                    updatedCompanies
                    processingTimeMs
                    errors
                    warnings
                }
            }
        "#;
        
        let variables = json!({
            "input": {
                "feedUrl": "https://www.sec.gov/Archives/edgar/daily-index/xbrl/companyfacts/",
                "maxItems": 1000,
                "includeAmendments": true,
                "includeExhibits": false,
                "rateLimit": 10,
                "retryAttempts": 3,
                "timeout": 300
            }
        });
        
        let req = execute_graphql_query(&app, mutation, Some(variables)).await;
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
        
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["data"]["importSecRss"]["success"].is_boolean());
        assert!(body["data"]["importSecRss"]["message"].is_string());
        assert!(body["data"]["importSecRss"]["itemsProcessed"].is_number());
        assert!(body["data"]["importSecRss"]["itemsSkipped"].is_number());
        assert!(body["data"]["importSecRss"]["itemsFailed"].is_number());
        assert!(body["data"]["importSecRss"]["newCompanies"].is_number());
        assert!(body["data"]["importSecRss"]["updatedCompanies"].is_number());
        assert!(body["data"]["importSecRss"]["processingTimeMs"].is_number());
        assert!(body["data"]["importSecRss"]["errors"].is_array());
        assert!(body["data"]["importSecRss"]["warnings"].is_array());
    }

    #[actix_web::test]
    async fn test_graphql_playground() {
        let app = create_test_app().await;
        
        let req = test::TestRequest::get()
            .uri("/graphql/playground")
            .to_request();
        
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
        
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        
        // Should contain GraphQL playground HTML
        assert!(body_str.contains("GraphQL Playground"));
        assert!(body_str.contains("graphql"));
    }

    #[actix_web::test]
    async fn test_invalid_query() {
        let app = create_test_app().await;
        
        let query = r#"
            query InvalidQuery {
                nonExistentField {
                    id
                }
            }
        "#;
        
        let req = execute_graphql_query(&app, query, None).await;
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
        
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["errors"].is_array());
        assert!(!body["errors"].as_array().unwrap().is_empty());
    }

    #[actix_web::test]
    async fn test_malformed_query() {
        let app = create_test_app().await;
        
        let query = r#"
            query MalformedQuery {
                searchCompanies(input: {
                    query: "Apple"
                    # Missing closing brace
            }
        "#;
        
        let req = execute_graphql_query(&app, query, None).await;
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
        
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["errors"].is_array());
        assert!(!body["errors"].as_array().unwrap().is_empty());
    }

    #[actix_web::test]
    async fn test_empty_query() {
        let app = create_test_app().await;
        
        let query = r#"
            query EmptyQuery {
                searchCompanies(input: { query: "" }) {
                    nodes {
                        id
                        name
                    }
                }
            }
        "#;
        
        let req = execute_graphql_query(&app, query, None).await;
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
        
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["data"]["searchCompanies"]["nodes"].is_array());
    }

    #[actix_web::test]
    async fn test_special_characters_in_query() {
        let app = create_test_app().await;
        
        let query = r#"
            query SearchWithSpecialChars($input: CompanySearchInput!) {
                searchCompanies(input: $input) {
                    nodes {
                        id
                        name
                    }
                }
            }
        "#;
        
        let special_queries = vec!["Apple & Co.", "Microsoft-Corp", "AT&T", "3M Company"];
        
        for query_text in special_queries {
            let variables = json!({
                "input": {
                    "query": query_text,
                    "limit": 10,
                    "includeInactive": false
                }
            });
            
            let req = execute_graphql_query(&app, query, Some(variables)).await;
            let resp = test::call_service(&app, req).await;
            
            assert!(resp.status().is_success());
            
            let body: serde_json::Value = test::read_body_json(resp).await;
            assert!(body["data"]["searchCompanies"]["nodes"].is_array());
        }
    }

    #[actix_web::test]
    async fn test_large_result_set() {
        let app = create_test_app().await;
        
        let query = r#"
            query SearchLargeResultSet($input: CompanySearchInput!) {
                searchCompanies(input: $input) {
                    nodes {
                        id
                        name
                        ticker
                    }
                    totalCount
                }
            }
        "#;
        
        let variables = json!({
            "input": {
                "query": "Inc",
                "limit": 100,
                "includeInactive": false
            }
        });
        
        let req = execute_graphql_query(&app, query, Some(variables)).await;
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
        
        let body: serde_json::Value = test::read_body_json(resp).await;
        let nodes = body["data"]["searchCompanies"]["nodes"].as_array().unwrap();
        
        // Should handle large result sets efficiently
        assert!(nodes.len() <= 100);
    }

    #[actix_web::test]
    async fn test_concurrent_requests() {
        let app = create_test_app().await;
        
        let query = r#"
            query SearchCompanies($input: CompanySearchInput!) {
                searchCompanies(input: $input) {
                    nodes {
                        id
                        name
                    }
                }
            }
        "#;
        
        let variables = json!({
            "input": {
                "query": "Apple",
                "limit": 10,
                "includeInactive": false
            }
        });
        
        // Test concurrent requests
        let mut handles = vec![];
        
        for _ in 0..10 {
            let app_clone = app.clone();
            let query_clone = query.to_string();
            let variables_clone = variables.clone();
            
            let handle = tokio::spawn(async move {
                let req = execute_graphql_query(&app_clone, &query_clone, Some(variables_clone)).await;
                let resp = test::call_service(&app_clone, req).await;
                resp.status().is_success()
            });
            
            handles.push(handle);
        }
        
        // Wait for all requests to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result);
        }
    }

    #[actix_web::test]
    async fn test_rate_limiting() {
        let app = create_test_app().await;
        
        let query = r#"
            query SearchCompanies($input: CompanySearchInput!) {
                searchCompanies(input: $input) {
                    nodes {
                        id
                        name
                    }
                }
            }
        "#;
        
        let variables = json!({
            "input": {
                "query": "Apple",
                "limit": 10,
                "includeInactive": false
            }
        });
        
        // Make many requests quickly to test rate limiting
        let mut requests = vec![];
        
        for _ in 0..100 {
            let req = execute_graphql_query(&app, query, Some(variables.clone())).await;
            requests.push(req);
        }
        
        // Process requests
        let mut success_count = 0;
        let mut rate_limited_count = 0;
        
        for req in requests {
            let resp = test::call_service(&app, req).await;
            
            if resp.status().is_success() {
                success_count += 1;
            } else if resp.status() == 429 {
                rate_limited_count += 1;
            }
        }
        
        // Should have some successful requests and some rate limited
        assert!(success_count > 0);
        // Rate limiting should kick in for excessive requests
        assert!(rate_limited_count > 0);
    }
}
