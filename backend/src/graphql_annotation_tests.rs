/**
 * REQUIREMENT: GraphQL integration tests for chart annotation functionality
 * PURPOSE: Verify that GraphQL mutations work correctly with proper field name mapping
 * This ensures the frontend-backend field name alignment is working correctly
 */

#[cfg(test)]
mod tests {
    use crate::{
        database::{create_pool, DatabasePool},
        error::AppResult,
        graphql::create_schema_with_data,
        models::{NewUser, User},
        test_utils::TestContainer,
    };
    use async_graphql::{Request, Variables};
    use chrono::NaiveDate;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use serde_json::json;
    use serial_test::serial;
    use uuid::Uuid;

    /// Test that chart annotation creation works with correct field name mapping
    #[tokio::test]
    #[serial]
    async fn test_create_annotation_graphql_mutation() -> AppResult<()> {
        // REQUIREMENT: Test GraphQL createAnnotation mutation with proper field names
        // PURPOSE: Verify that the field name mapping between frontend and backend works correctly
        // This prevents the UUID parsing error that occurred due to field name mismatches

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Create GraphQL schema
        let schema = create_schema_with_data(pool.clone());

        // Create a test user
        let test_user = create_test_user(&pool, "test@example.com", "Test User").await?;
        let series_id = Uuid::new_v4();

        // Test the GraphQL mutation with correct field names (snake_case)
        let mutation = r#"
            mutation CreateAnnotation($input: CreateAnnotationInput!) {
                createAnnotation(input: $input) {
                    id
                    userId
                    seriesId
                    title
                    description
                    annotationType
                    color
                    isVisible
                    isPinned
                    createdAt
                }
            }
        "#;

        let variables = json!({
            "input": {
                "userId": test_user.id.to_string(),
                "seriesId": series_id.to_string(),
                "annotationDate": "2024-01-15",
                "annotationValue": 100.5,
                "title": "Test Annotation Title",
                "content": "This is a test annotation content",
                "annotationType": "note",
                "color": "#ff0000",
                "isPublic": true
            }
        });

        let request = Request::new(mutation).variables(Variables::from_json(variables));

        let response = async_graphql::Schema::execute(&schema, request).await;

        // Verify the mutation succeeded
        assert!(
            response.errors.is_empty(),
            "GraphQL mutation should succeed without errors: {:?}",
            response.errors
        );

        let data = response
            .data
            .into_json()
            .expect("Response should be valid JSON");
        let annotation = data
            .get("createAnnotation")
            .expect("Should have createAnnotation field");

        // Verify the annotation was created with correct data
        assert_eq!(annotation.get("title").unwrap(), "Test Annotation Title");
        assert_eq!(
            annotation.get("description").unwrap(),
            "This is a test annotation content"
        );
        assert_eq!(annotation.get("annotationType").unwrap(), "note");
        assert_eq!(annotation.get("color").unwrap(), "#ff0000");
        assert_eq!(annotation.get("isVisible").unwrap(), true);

        println!(
            "✅ GraphQL createAnnotation mutation test passed - field name mapping works correctly"
        );

        Ok(())
    }

    /// Test that the old camelCase field names would fail (regression test)
    #[tokio::test]
    #[serial]
    async fn test_create_annotation_with_incorrect_field_names() -> AppResult<()> {
        // REQUIREMENT: Test that incorrect field names (camelCase) fail appropriately
        // PURPOSE: Ensure we catch field name mismatches that would cause UUID parsing errors
        // This prevents regression of the field name mapping issue

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Create GraphQL schema
        let schema = create_schema_with_data(pool.clone());

        // Create a test user
        let test_user = create_test_user(&pool, "test2@example.com", "Test User 2").await?;
        let series_id = Uuid::new_v4();

        // Test the GraphQL mutation with incorrect field names (snake_case instead of camelCase)
        let mutation = r#"
            mutation CreateAnnotation($input: CreateAnnotationInput!) {
                createAnnotation(input: $input) {
                    id
                    title
                }
            }
        "#;

        let variables = json!({
            "input": {
                "user_id": test_user.id.to_string(),     // Wrong: should be userId (camelCase)
                "series_id": series_id.to_string(),      // Wrong: should be seriesId (camelCase)
                "annotation_date": "2024-01-15",         // Wrong: should be annotationDate (camelCase)
                "title": "Test Title",
                "content": "Test content",
                "annotation_type": "note"                // Wrong: should be annotationType (camelCase)
            }
        });

        let request = Request::new(mutation).variables(Variables::from_json(variables));

        let response = async_graphql::Schema::execute(&schema, request).await;

        // Verify the mutation failed due to incorrect field names
        assert!(
            !response.errors.is_empty(),
            "GraphQL mutation should fail with incorrect field names"
        );

        let error_message = response.errors[0].message.to_string();
        assert!(
            error_message.contains("field") && error_message.contains("required"),
            "Error should indicate missing required field: {}",
            error_message
        );

        println!("✅ GraphQL field name validation test passed - incorrect field names are properly rejected");

        Ok(())
    }

    /// Test that annotation title field accepts text (not UUID)
    #[tokio::test]
    #[serial]
    async fn test_annotation_title_accepts_text() -> AppResult<()> {
        // REQUIREMENT: Test that annotation title field accepts text input
        // PURPOSE: Verify that the title field is properly typed as String, not ID/UUID
        // This prevents the UUID parsing error that occurred when title was mis-mapped

        let container = TestContainer::new().await;
        let pool = container.pool();

        // Create GraphQL schema
        let schema = create_schema_with_data(pool.clone());

        // Create a test user
        let test_user = create_test_user(&pool, "test3@example.com", "Test User 3").await?;
        let series_id = Uuid::new_v4();

        // Test with various text titles that would fail if parsed as UUID
        let test_titles = vec![
            "Market Analysis",
            "GDP Growth Indicator",
            "Recession Warning",
            "Economic Recovery",
            "Inflation Alert",
            "Unemployment Trend",
            "Consumer Confidence",
            "Business Investment",
            "Trade Deficit",
            "Federal Reserve Policy",
        ];

        for title in test_titles {
            let mutation = r#"
                mutation CreateAnnotation($input: CreateAnnotationInput!) {
                    createAnnotation(input: $input) {
                        id
                        title
                    }
                }
            "#;

            let variables = json!({
                "input": {
                    "userId": test_user.id.to_string(),
                    "seriesId": series_id.to_string(),
                    "annotationDate": "2024-01-15",
                    "title": title,
                    "content": "Test content for title validation",
                    "annotationType": "note",
                    "isPublic": true
                }
            });

            let request = Request::new(mutation).variables(Variables::from_json(variables));

            let response = async_graphql::Schema::execute(&schema, request).await;

            // Verify the mutation succeeded with text title
            assert!(
                response.errors.is_empty(),
                "GraphQL mutation should succeed with text title '{}': {:?}",
                title,
                response.errors
            );

            let data = response
                .data
                .into_json()
                .expect("Response should be valid JSON");
            let annotation = data
                .get("createAnnotation")
                .expect("Should have createAnnotation field");
            assert_eq!(annotation.get("title").unwrap(), title);
        }

        println!("✅ GraphQL annotation title text validation test passed - titles accept text input correctly");

        Ok(())
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
}
