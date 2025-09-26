/**
 * REQUIREMENT: Critical security tests to verify no UUIDv7 format is returned in GraphQL responses
 * PURPOSE: Prevent information leakage by ensuring UUIDv7 (which contains timestamp data) is never exposed
 *
 * SECURITY CRITICAL: UUIDv7 contains timestamp information that could leak sensitive data:
 * - When users were created
 * - When data was inserted into the system
 * - System timing information
 * - Potential for correlation attacks
 *
 * This test ensures all UUIDs returned via GraphQL are in standard UUID format (v4 or other non-v7 formats)
 * to prevent any information leakage about system internals or user creation patterns.
 */

#[cfg(test)]
mod tests {
    use crate::{
        database::{create_pool, DatabasePool},
        error::AppResult,
        graphql::create_schema_with_data,
        models::{NewUser, User, NewDataSource, DataSource, NewEconomicSeries, EconomicSeries},
        test_utils::TestContainer,
    };
    use async_graphql::{Request, Variables};
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use serde_json::json;
    use serial_test::serial;
    use uuid::Uuid;

    /// Test that user IDs in GraphQL responses are not UUIDv7 format
    #[tokio::test]
    #[serial]
    async fn test_user_id_not_uuidv7_in_graphql_response() -> AppResult<()> {
        // REQUIREMENT: SECURITY CRITICAL - Verify user IDs returned via GraphQL are not in UUIDv7 format
        // PURPOSE: Prevent information leakage about user creation timestamps and system internals

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Create GraphQL schema
        let schema = create_schema_with_data(pool.clone());

        // Create multiple test users to verify various user IDs
        let test_users = vec![
            ("user1@example.com", "User One"),
            ("user2@example.com", "User Two"),
            ("user3@example.com", "User Three"),
            ("user4@example.com", "User Four"),
            ("user5@example.com", "User Five"),
        ];

        for (email, name) in test_users {
            let test_user = create_test_user(&pool, email, name).await?;

            // Test user query via GraphQL
            let query = r#"
                query GetUser($id: ID!) {
                    user(id: $id) {
                        id
                        email
                        name
                    }
                }
            "#;

            let variables = json!({
                "id": test_user.id.to_string()
            });

            let request = Request::new(query).variables(Variables::from_json(variables));
            let response = schema.execute(request).await;

            // Verify the query succeeded
            assert!(
                response.errors.is_empty(),
                "GraphQL user query should succeed: {:?}",
                response.errors
            );

            let data = response
                .data
                .into_json()
                .expect("Response should be valid JSON");

            let user_data = data
                .get("user")
                .expect("Should have user field");

            let user_id = user_data
                .get("id")
                .expect("Should have id field")
                .as_str()
                .expect("ID should be a string");

            // Verify the user ID is not UUIDv7 format
            assert!(
                !is_uuidv7(user_id),
                "User ID '{}' should not be in UUIDv7 format",
                user_id
            );

            // Verify it's still a valid UUID
            assert!(
                Uuid::parse_str(user_id).is_ok(),
                "User ID '{}' should be a valid UUID",
                user_id
            );

            println!("✅ User ID '{}' is valid UUID but not UUIDv7 format", user_id);
        }

        println!("✅ All user IDs verified to not be UUIDv7 format in GraphQL responses");
        Ok(())
    }

    /// Test that data source IDs in GraphQL responses are not UUIDv7 format
    #[tokio::test]
    #[serial]
    async fn test_data_source_id_not_uuidv7_in_graphql_response() -> AppResult<()> {
        // REQUIREMENT: SECURITY CRITICAL - Verify data source IDs returned via GraphQL are not in UUIDv7 format
        // PURPOSE: Prevent information leakage about data source creation timestamps and system internals

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Create GraphQL schema
        let schema = create_schema_with_data(pool.clone());

        // Create multiple test data sources
        let test_sources = vec![
            ("Test Source 1", "https://api1.example.com"),
            ("Test Source 2", "https://api2.example.com"),
            ("Test Source 3", "https://api3.example.com"),
        ];

        for (name, base_url) in test_sources {
            let test_source = create_test_data_source(&pool, name, base_url).await?;

            // Test data source query via GraphQL
            let query = r#"
                query GetDataSource($id: ID!) {
                    dataSource(id: $id) {
                        id
                        name
                        baseUrl
                    }
                }
            "#;

            let variables = json!({
                "id": test_source.id.to_string()
            });

            let request = Request::new(query).variables(Variables::from_json(variables));
            let response = schema.execute(request).await;

            // Verify the query succeeded
            assert!(
                response.errors.is_empty(),
                "GraphQL data source query should succeed: {:?}",
                response.errors
            );

            let data = response
                .data
                .into_json()
                .expect("Response should be valid JSON");

            let source_data = data
                .get("dataSource")
                .expect("Should have dataSource field");

            let source_id = source_data
                .get("id")
                .expect("Should have id field")
                .as_str()
                .expect("ID should be a string");

            // Verify the data source ID is not UUIDv7 format
            assert!(
                !is_uuidv7(source_id),
                "Data source ID '{}' should not be in UUIDv7 format",
                source_id
            );

            // Verify it's still a valid UUID
            assert!(
                Uuid::parse_str(source_id).is_ok(),
                "Data source ID '{}' should be a valid UUID",
                source_id
            );

            println!("✅ Data source ID '{}' is valid UUID but not UUIDv7 format", source_id);
        }

        println!("✅ All data source IDs verified to not be UUIDv7 format in GraphQL responses");
        Ok(())
    }

    /// Test that economic series IDs in GraphQL responses are not UUIDv7 format
    #[tokio::test]
    #[serial]
    async fn test_economic_series_id_not_uuidv7_in_graphql_response() -> AppResult<()> {
        // REQUIREMENT: SECURITY CRITICAL - Verify economic series IDs returned via GraphQL are not in UUIDv7 format
        // PURPOSE: Prevent information leakage about series creation timestamps and system internals

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Create GraphQL schema
        let schema = create_schema_with_data(pool.clone());

        // Create a test data source first
        let test_source = create_test_data_source(&pool, "Test Source", "https://api.example.com").await?;

        // Create multiple test economic series
        let test_series = vec![
            ("GDP", "Gross Domestic Product", "Quarterly"),
            ("UNRATE", "Unemployment Rate", "Monthly"),
            ("CPIAUCSL", "Consumer Price Index", "Monthly"),
        ];

        for (external_id, title, frequency) in test_series {
            let test_series = create_test_economic_series(&pool, &test_source.id, external_id, title, frequency).await?;

            // Test economic series query via GraphQL
            let query = r#"
                query GetEconomicSeries($id: ID!) {
                    economicSeries(id: $id) {
                        id
                        title
                        frequency
                        externalId
                    }
                }
            "#;

            let variables = json!({
                "id": test_series.id.to_string()
            });

            let request = Request::new(query).variables(Variables::from_json(variables));
            let response = schema.execute(request).await;

            // Verify the query succeeded
            assert!(
                response.errors.is_empty(),
                "GraphQL economic series query should succeed: {:?}",
                response.errors
            );

            let data = response
                .data
                .into_json()
                .expect("Response should be valid JSON");

            let series_data = data
                .get("economicSeries")
                .expect("Should have economicSeries field");

            let series_id = series_data
                .get("id")
                .expect("Should have id field")
                .as_str()
                .expect("ID should be a string");

            // Verify the economic series ID is not UUIDv7 format
            assert!(
                !is_uuidv7(series_id),
                "Economic series ID '{}' should not be in UUIDv7 format",
                series_id
            );

            // Verify it's still a valid UUID
            assert!(
                Uuid::parse_str(series_id).is_ok(),
                "Economic series ID '{}' should be a valid UUID",
                series_id
            );

            println!("✅ Economic series ID '{}' is valid UUID but not UUIDv7 format", series_id);
        }

        println!("✅ All economic series IDs verified to not be UUIDv7 format in GraphQL responses");
        Ok(())
    }

    /// Test that all UUIDs in a complex GraphQL query are not UUIDv7 format
    #[tokio::test]
    #[serial]
    async fn test_complex_query_no_uuidv7_in_response() -> AppResult<()> {
        // REQUIREMENT: SECURITY CRITICAL - Verify complex GraphQL queries don't return UUIDv7 format in any field
        // PURPOSE: Comprehensive security test to ensure no timestamp information leaks through any GraphQL response path

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Create GraphQL schema
        let schema = create_schema_with_data(pool.clone());

        // Create test data
        let test_user = create_test_user(&pool, "complex@example.com", "Complex Test User").await?;
        let test_source = create_test_data_source(&pool, "Complex Test Source", "https://complex.example.com").await?;
        let test_series = create_test_economic_series(&pool, &test_source.id, "COMPLEX", "Complex Test Series", "Monthly").await?;

        // Test a complex query that returns multiple entities with UUIDs
        let query = r#"
            query ComplexQuery($userId: ID!, $seriesId: ID!) {
                user(id: $userId) {
                    id
                    email
                    name
                }
                economicSeries(id: $seriesId) {
                    id
                    title
                    frequency
                    source {
                        id
                        name
                    }
                }
            }
        "#;

        let variables = json!({
            "userId": test_user.id.to_string(),
            "seriesId": test_series.id.to_string()
        });

        let request = Request::new(query).variables(Variables::from_json(variables));
        let response = schema.execute(request).await;

        // Verify the query succeeded
        assert!(
            response.errors.is_empty(),
            "Complex GraphQL query should succeed: {:?}",
            response.errors
        );

        let data = response
            .data
            .into_json()
            .expect("Response should be valid JSON");

        // Extract all UUIDs from the response
        let mut all_uuids = Vec::new();

        if let Some(user) = data.get("user") {
            if let Some(id) = user.get("id").and_then(|v| v.as_str()) {
                all_uuids.push(("user.id", id));
            }
        }

        if let Some(series) = data.get("economicSeries") {
            if let Some(id) = series.get("id").and_then(|v| v.as_str()) {
                all_uuids.push(("economicSeries.id", id));
            }
            if let Some(source) = series.get("source") {
                if let Some(id) = source.get("id").and_then(|v| v.as_str()) {
                    all_uuids.push(("economicSeries.source.id", id));
                }
            }
        }

        // Verify none of the UUIDs are in UUIDv7 format
        for (field_path, uuid_str) in all_uuids {
            assert!(
                !is_uuidv7(uuid_str),
                "Field '{}' with value '{}' should not be in UUIDv7 format",
                field_path,
                uuid_str
            );

            // Verify it's still a valid UUID
            assert!(
                Uuid::parse_str(uuid_str).is_ok(),
                "Field '{}' with value '{}' should be a valid UUID",
                field_path,
                uuid_str
            );

            println!("✅ Field '{}' with UUID '{}' is valid but not UUIDv7 format", field_path, uuid_str);
        }

        println!("✅ Complex GraphQL query verified - no UUIDv7 format in any response field");
        Ok(())
    }

    /// Test that GraphQL mutations don't return UUIDv7 format in response
    #[tokio::test]
    #[serial]
    async fn test_mutation_response_no_uuidv7() -> AppResult<()> {
        // REQUIREMENT: SECURITY CRITICAL - Verify GraphQL mutations don't return UUIDv7 format in response
        // PURPOSE: Prevent information leakage through mutation responses about creation timestamps

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Create GraphQL schema
        let schema = create_schema_with_data(pool.clone());

        // Create a test user
        let test_user = create_test_user(&pool, "mutation@example.com", "Mutation Test User").await?;

        // Test a mutation that returns a UUID
        let mutation = r#"
            mutation CreateAnnotation($input: CreateAnnotationInput!) {
                createAnnotation(input: $input) {
                    id
                    userId
                    title
                }
            }
        "#;

        let variables = json!({
            "input": {
                "userId": test_user.id.to_string(),
                "seriesId": Uuid::new_v4().to_string(),
                "annotationDate": "2024-01-15",
                "title": "Test Annotation",
                "content": "Test content",
                "annotationType": "note",
                "isPublic": true
            }
        });

        let request = Request::new(mutation).variables(Variables::from_json(variables));
        let response = schema.execute(request).await;

        // Verify the mutation succeeded
        assert!(
            response.errors.is_empty(),
            "GraphQL mutation should succeed: {:?}",
            response.errors
        );

        let data = response
            .data
            .into_json()
            .expect("Response should be valid JSON");

        let annotation = data
            .get("createAnnotation")
            .expect("Should have createAnnotation field");

        // Check annotation ID
        if let Some(id) = annotation.get("id").and_then(|v| v.as_str()) {
            assert!(
                !is_uuidv7(id),
                "Annotation ID '{}' should not be in UUIDv7 format",
                id
            );
            assert!(
                Uuid::parse_str(id).is_ok(),
                "Annotation ID '{}' should be a valid UUID",
                id
            );
            println!("✅ Annotation ID '{}' is valid UUID but not UUIDv7 format", id);
        }

        // Check user ID in response
        if let Some(user_id) = annotation.get("userId").and_then(|v| v.as_str()) {
            assert!(
                !is_uuidv7(user_id),
                "User ID '{}' in mutation response should not be in UUIDv7 format",
                user_id
            );
            assert!(
                Uuid::parse_str(user_id).is_ok(),
                "User ID '{}' in mutation response should be a valid UUID",
                user_id
            );
            println!("✅ User ID '{}' in mutation response is valid UUID but not UUIDv7 format", user_id);
        }

        println!("✅ GraphQL mutation response verified - no UUIDv7 format returned");
        Ok(())
    }

    /// Helper function to check if a UUID string is in UUIDv7 format
    /// SECURITY CRITICAL: UUIDv7 contains timestamp information that must never be exposed
    fn is_uuidv7(uuid_str: &str) -> bool {
        // UUIDv7 has version 7 in the version field (bits 12-15 of the time_high field)
        // The version field is at position 14 in the UUID string (0-indexed)
        // UUIDv7 contains timestamp data that could leak sensitive information about:
        // - When users were created
        // - When data was inserted
        // - System timing information
        if uuid_str.len() != 36 {
            return false;
        }

        // Parse the UUID to extract the version
        if let Ok(uuid) = Uuid::parse_str(uuid_str) {
            // Extract version from the UUID bytes
            let bytes = uuid.as_bytes();
            let version = (bytes[6] & 0xf0) >> 4;
            return version == 7;
        }

        false
    }

    /// Create a test user for GraphQL tests
    async fn create_test_user(pool: &DatabasePool, email: &str, name: &str) -> AppResult<User> {
        use crate::schema::users;

        let mut conn = pool.get().await?;

        let new_user = NewUser {
            email: email.to_string(),
            name: name.to_string(),
            avatar_url: None,
            provider: "test".to_string(),
            provider_id: Some(Uuid::new_v4().to_string()),
            password_hash: None,
            role: "user".to_string(),
            organization: None,
            theme: "light".to_string(),
            default_chart_type: "line".to_string(),
            notifications_enabled: true,
            collaboration_enabled: true,
            email_verified: true,
        };

        let user = diesel::insert_into(users::table)
            .values(&new_user)
            .returning(User::as_select())
            .get_result::<User>(&mut conn)
            .await
            .map_err(|e| crate::error::AppError::DatabaseError(e.to_string()))?;

        Ok(user)
    }

    /// Create a test data source for GraphQL tests
    async fn create_test_data_source(pool: &DatabasePool, name: &str, base_url: &str) -> AppResult<DataSource> {
        use crate::schema::data_sources;

        let mut conn = pool.get().await?;

        let new_source = NewDataSource {
            name: name.to_string(),
            description: Some("Test data source".to_string()),
            base_url: base_url.to_string(),
            api_key_required: false,
            rate_limit_per_minute: 1000,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some("https://docs.example.com".to_string()),
        };

        let source = diesel::insert_into(data_sources::table)
            .values(&new_source)
            .returning(DataSource::as_select())
            .get_result::<DataSource>(&mut conn)
            .await
            .map_err(|e| crate::error::AppError::DatabaseError(e.to_string()))?;

        Ok(source)
    }

    /// Create a test economic series for GraphQL tests
    async fn create_test_economic_series(
        pool: &DatabasePool,
        source_id: &Uuid,
        external_id: &str,
        title: &str,
        frequency: &str,
    ) -> AppResult<EconomicSeries> {
        use crate::schema::economic_series;

        let mut conn = pool.get().await?;

        let new_series = NewEconomicSeries {
            source_id: *source_id,
            external_id: external_id.to_string(),
            title: title.to_string(),
            description: Some("Test economic series".to_string()),
            units: Some("Test units".to_string()),
            frequency: frequency.to_string(),
            seasonal_adjustment: Some("Seasonally Adjusted".to_string()),
            is_active: true,
        };

        let series = diesel::insert_into(economic_series::table)
            .values(&new_series)
            .returning(EconomicSeries::as_select())
            .get_result::<EconomicSeries>(&mut conn)
            .await
            .map_err(|e| crate::error::AppError::DatabaseError(e.to_string()))?;

        Ok(series)
    }
}
