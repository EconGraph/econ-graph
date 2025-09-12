/**
 * REQUIREMENT: Comprehensive integration tests for collaboration features
 * PURPOSE: Verify end-to-end functionality of chart annotations, comments, and sharing
 * This ensures professional collaboration features work correctly for institutional users
 */

#[cfg(test)]
mod tests {
    use crate::{
        error::AppResult,
        models::{NewUser, User},
        services::{collaboration_service::PermissionLevel, CollaborationService},
        test_utils::TestContainer,
    };
    use bigdecimal::BigDecimal;
    use chrono::{NaiveDate, Utc};
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use std::str::FromStr;
    use uuid::Uuid;

    /// Create a test user for collaboration tests
    async fn create_test_user(
        container: &TestContainer,
        email: &str,
        name: &str,
    ) -> AppResult<User> {
        use crate::schema::users;

        let mut conn = container.pool.get().await?;

        let new_user = NewUser {
            email: email.to_string(),
            name: name.to_string(),
            avatar_url: None,
            provider: "test".to_string(),
            provider_id: None,
            password_hash: None,
            role: "user".to_string(),
            organization: Some("Test Org".to_string()),
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
            .await?;

        Ok(user)
    }

    #[tokio::test]
    async fn test_create_and_retrieve_annotation() -> AppResult<()> {
        let container = TestContainer::new().await;
        let collaboration_service = CollaborationService::new(container.pool.clone());

        // Create test user
        let user = create_test_user(&container, "annotator@test.com", "Test Annotator").await?;

        // Create annotation
        let series_id = Uuid::new_v4();
        let annotation_date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let annotation_value = Some(BigDecimal::from_str("100.5").unwrap());

        let annotation = collaboration_service
            .create_annotation(
                user.id,
                series_id,
                annotation_date,
                annotation_value.clone(),
                "Test Annotation".to_string(),
                "This is a test annotation for GDP data".to_string(),
                "note".to_string(),
                Some("#ff0000".to_string()),
                true, // is_public
            )
            .await?;

        // Verify annotation was created correctly
        assert_eq!(annotation.user_id, user.id);
        assert_eq!(annotation.title, "Test Annotation");
        assert_eq!(annotation.annotation_date, annotation_date);
        assert_eq!(annotation.annotation_value, annotation_value);
        assert_eq!(annotation.is_visible, Some(true));

        // Retrieve annotations for series
        let annotations = collaboration_service
            .get_annotations_for_series(&series_id.to_string(), Some(user.id))
            .await?;

        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].id, annotation.id);

        Ok(())
    }

    #[tokio::test]
    async fn test_annotation_visibility_permissions() -> AppResult<()> {
        let container = TestContainer::new().await;
        let collaboration_service = CollaborationService::new(container.pool.clone());

        // Create test users
        let user1 = create_test_user(&container, "user1@test.com", "User One").await?;
        let user2 = create_test_user(&container, "user2@test.com", "User Two").await?;

        let series_id = Uuid::new_v4();
        let annotation_date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

        // Create private annotation by user1
        let private_annotation = collaboration_service
            .create_annotation(
                user1.id,
                series_id,
                annotation_date,
                None,
                "Private Note".to_string(),
                "This is private".to_string(),
                "note".to_string(),
                None,
                false, // is_public = false
            )
            .await?;

        // Create public annotation by user1
        let public_annotation = collaboration_service
            .create_annotation(
                user1.id,
                series_id,
                annotation_date,
                None,
                "Public Note".to_string(),
                "This is public".to_string(),
                "highlight".to_string(),
                Some("#00ff00".to_string()),
                true, // is_public = true
            )
            .await?;

        // User1 should see both annotations
        let user1_annotations = collaboration_service
            .get_annotations_for_series(&series_id.to_string(), Some(user1.id))
            .await?;
        assert_eq!(user1_annotations.len(), 2);

        // User2 should only see the public annotation
        let user2_annotations = collaboration_service
            .get_annotations_for_series(&series_id.to_string(), Some(user2.id))
            .await?;
        assert_eq!(user2_annotations.len(), 1);
        assert_eq!(user2_annotations[0].id, public_annotation.id);

        // Anonymous user should only see the public annotation
        let anonymous_annotations = collaboration_service
            .get_annotations_for_series(&series_id.to_string(), None)
            .await?;
        assert_eq!(anonymous_annotations.len(), 1);
        assert_eq!(anonymous_annotations[0].id, public_annotation.id);

        Ok(())
    }

    #[tokio::test]
    async fn test_annotation_comments_workflow() -> AppResult<()> {
        let container = TestContainer::new().await;
        let collaboration_service = CollaborationService::new(container.pool.clone());

        // Create test users
        let user1 = create_test_user(&container, "commenter1@test.com", "Commenter One").await?;
        let user2 = create_test_user(&container, "commenter2@test.com", "Commenter Two").await?;

        let series_id = Uuid::new_v4();
        let annotation_date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

        // Create annotation
        let annotation = collaboration_service
            .create_annotation(
                user1.id,
                series_id,
                annotation_date,
                None,
                "Discussion Point".to_string(),
                "What do you think about this trend?".to_string(),
                "question".to_string(),
                Some("#ffff00".to_string()),
                true,
            )
            .await?;

        // Add comment by user2
        let comment1 = collaboration_service
            .add_comment(
                user2.id,
                annotation.id,
                "I think this shows strong growth momentum.".to_string(),
            )
            .await?;

        // Add reply by user1
        let comment2 = collaboration_service
            .add_comment(
                user1.id,
                annotation.id,
                "Good point! The underlying fundamentals support this view.".to_string(),
            )
            .await?;

        // Retrieve all comments for the annotation
        let comments = collaboration_service
            .get_comments_for_annotation(annotation.id)
            .await?;

        assert_eq!(comments.len(), 2);

        // Verify comments are ordered by creation time
        assert_eq!(comments[0].id, comment1.id);
        assert_eq!(comments[0].user_id, user2.id);
        assert_eq!(
            comments[0].content,
            "I think this shows strong growth momentum."
        );

        assert_eq!(comments[1].id, comment2.id);
        assert_eq!(comments[1].user_id, user1.id);

        Ok(())
    }

    #[tokio::test]
    async fn test_chart_sharing_and_permissions() -> AppResult<()> {
        let container = TestContainer::new().await;
        let collaboration_service = CollaborationService::new(container.pool.clone());

        // Create test users
        let owner = create_test_user(&container, "owner@test.com", "Chart Owner").await?;
        let viewer = create_test_user(&container, "viewer@test.com", "Chart Viewer").await?;
        let editor = create_test_user(&container, "editor@test.com", "Chart Editor").await?;

        let chart_id = Uuid::new_v4();

        // Share chart with viewer (view permission)
        let viewer_collab = collaboration_service
            .share_chart(chart_id, owner.id, viewer.id, PermissionLevel::View)
            .await?;

        assert_eq!(viewer_collab.chart_id, chart_id);
        assert_eq!(viewer_collab.user_id, viewer.id);
        assert_eq!(viewer_collab.invited_by, Some(owner.id));
        assert_eq!(viewer_collab.role, Some("view".to_string()));

        // Share chart with editor (edit permission)
        let editor_collab = collaboration_service
            .share_chart(chart_id, owner.id, editor.id, PermissionLevel::Edit)
            .await?;

        assert_eq!(editor_collab.role, Some("edit".to_string()));

        // Get all collaborators for the chart
        let collaborators = collaboration_service.get_collaborators(chart_id).await?;

        assert_eq!(collaborators.len(), 2);

        // Verify both collaborators are present
        let collaborator_ids: Vec<Uuid> = collaborators.iter().map(|(c, _)| c.user_id).collect();
        assert!(collaborator_ids.contains(&viewer.id));
        assert!(collaborator_ids.contains(&editor.id));

        Ok(())
    }

    #[tokio::test]
    async fn test_annotation_deletion_permissions() -> AppResult<()> {
        let container = TestContainer::new().await;
        let collaboration_service = CollaborationService::new(container.pool.clone());

        // Create test users
        let owner = create_test_user(&container, "owner@test.com", "Annotation Owner").await?;
        let other_user = create_test_user(&container, "other@test.com", "Other User").await?;

        let series_id = Uuid::new_v4();
        let annotation_date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

        // Create annotation by owner
        let annotation = collaboration_service
            .create_annotation(
                owner.id,
                series_id,
                annotation_date,
                None,
                "To Be Deleted".to_string(),
                "This annotation will be deleted".to_string(),
                "note".to_string(),
                None,
                true,
            )
            .await?;

        // Other user should not be able to delete the annotation
        let delete_result = collaboration_service
            .delete_annotation(annotation.id, other_user.id)
            .await;
        assert!(delete_result.is_err());

        // Owner should be able to delete the annotation
        let delete_result = collaboration_service
            .delete_annotation(annotation.id, owner.id)
            .await;
        assert!(delete_result.is_ok());

        // Verify annotation is deleted
        let annotations = collaboration_service
            .get_annotations_for_series(&series_id.to_string(), Some(owner.id))
            .await?;
        assert_eq!(annotations.len(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_collaboration_workflow_end_to_end() -> AppResult<()> {
        let container = TestContainer::new().await;
        let collaboration_service = CollaborationService::new(container.pool.clone());

        // Create test users representing different roles
        let analyst = create_test_user(&container, "analyst@bank.com", "Senior Analyst").await?;
        let manager = create_test_user(&container, "manager@bank.com", "Portfolio Manager").await?;
        let researcher =
            create_test_user(&container, "researcher@bank.com", "Research Associate").await?;

        let series_id = Uuid::new_v4(); // GDP series
        let chart_id = Uuid::new_v4();
        let annotation_date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

        // 1. Analyst creates initial annotation on GDP data
        let initial_annotation = collaboration_service.create_annotation(
            analyst.id,
            series_id,
            annotation_date,
            Some(BigDecimal::from_str("2.1").unwrap()),
            "Q4 GDP Growth Analysis".to_string(),
            "GDP growth of 2.1% indicates moderate economic expansion. Key drivers include consumer spending and business investment.".to_string(),
            "analysis".to_string(),
            Some("#0066cc".to_string()),
            true,
        ).await?;

        // 2. Share the chart with team members
        let manager_collab = collaboration_service
            .share_chart(chart_id, analyst.id, manager.id, PermissionLevel::Edit)
            .await?;

        let researcher_collab = collaboration_service
            .share_chart(
                chart_id,
                analyst.id,
                researcher.id,
                PermissionLevel::Comment,
            )
            .await?;

        // 3. Manager adds strategic commentary
        let manager_comment = collaboration_service.add_comment(
            manager.id,
            initial_annotation.id,
            "This aligns with our Q1 investment thesis. Consider increasing allocation to growth sectors.".to_string(),
        ).await?;

        // 4. Researcher adds supporting data
        let supporting_annotation = collaboration_service.create_annotation(
            researcher.id,
            series_id,
            NaiveDate::from_ymd_opt(2024, 1, 20).unwrap(),
            Some(BigDecimal::from_str("1.8").unwrap()),
            "Leading Indicators Support".to_string(),
            "Employment data and consumer confidence metrics support continued growth trajectory.".to_string(),
            "supporting_data".to_string(),
            Some("#00cc66".to_string()),
            true,
        ).await?;

        // 5. Analyst responds to manager's comment
        let analyst_response = collaboration_service.add_comment(
            analyst.id,
            initial_annotation.id,
            "Agreed. I'll prepare sector allocation recommendations for the next portfolio review.".to_string(),
        ).await?;

        // 6. Verify the complete collaboration workflow

        // Check all annotations are visible to team
        let all_annotations = collaboration_service
            .get_annotations_for_series(&series_id.to_string(), Some(manager.id))
            .await?;
        assert_eq!(all_annotations.len(), 2);

        // Check comment thread
        let comments = collaboration_service
            .get_comments_for_annotation(initial_annotation.id)
            .await?;
        assert_eq!(comments.len(), 2);

        // Verify comment authors
        let comment_authors: Vec<Uuid> = comments.iter().map(|c| c.user_id).collect();
        assert!(comment_authors.contains(&manager.id));
        assert!(comment_authors.contains(&analyst.id));

        // Check chart collaborators
        let collaborators = collaboration_service.get_collaborators(chart_id).await?;
        assert_eq!(collaborators.len(), 2);

        // Verify permission levels
        let manager_permission = collaborators
            .iter()
            .find(|(c, _)| c.user_id == manager.id)
            .and_then(|(c, _)| c.role.as_ref());
        assert_eq!(manager_permission, Some(&"edit".to_string()));

        let researcher_permission = collaborators
            .iter()
            .find(|(c, _)| c.user_id == researcher.id)
            .and_then(|(c, _)| c.role.as_ref());
        assert_eq!(researcher_permission, Some(&"comment".to_string()));

        println!("✅ End-to-end collaboration workflow test completed successfully");
        println!("   - {} annotations created", all_annotations.len());
        println!("   - {} comments in discussion", comments.len());
        println!(
            "   - {} team members collaborating",
            collaborators.len() + 1
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_collaboration_performance_and_scale() -> AppResult<()> {
        let container = TestContainer::new().await;
        let collaboration_service = CollaborationService::new(container.pool.clone());

        // Create multiple users
        let mut users = Vec::new();
        for i in 0..10 {
            let email = format!("user{}@test.com", i);
            let name = format!("User {}", i);
            let user = create_test_user(&container, &email, &name).await?;
            users.push(user);
        }

        let series_id = Uuid::new_v4();
        let chart_id = Uuid::new_v4();

        // Performance test: Create multiple annotations rapidly
        let start_time = std::time::Instant::now();

        let annotations =
            futures::future::try_join_all(users.iter().enumerate().map(|(i, user)| {
                let service = &collaboration_service;
                async move {
                    service
                        .create_annotation(
                            user.id,
                            series_id,
                            NaiveDate::from_ymd_opt(2024, 1, ((i % 28) + 1) as u32).unwrap(),
                            Some(BigDecimal::from_str(&format!("{}.{}", 100 + i, i % 10)).unwrap()),
                            format!("Annotation {}", i),
                            format!("Analysis point {} for performance testing", i),
                            "performance_test".to_string(),
                            Some(format!("#{:06x}", i * 111111 % 0xFFFFFF)),
                            true,
                        )
                        .await
                }
            }))
            .await?;

        let annotation_creation_time = start_time.elapsed();

        // Performance test: Share chart with all users
        let share_start = std::time::Instant::now();

        futures::future::try_join_all(users.iter().skip(1).map(|user| {
            collaboration_service.share_chart(
                chart_id,
                users[0].id,
                user.id,
                PermissionLevel::Comment,
            )
        }))
        .await?;

        let sharing_time = share_start.elapsed();

        // Performance test: Add comments to first annotation
        let comment_start = std::time::Instant::now();

        futures::future::try_join_all(users.iter().map(|user| {
            collaboration_service.add_comment(
                user.id,
                annotations[0].id,
                format!("Performance test comment from user {}", user.name),
            )
        }))
        .await?;

        let commenting_time = comment_start.elapsed();

        // Verify all operations completed successfully
        let final_annotations = collaboration_service
            .get_annotations_for_series(&series_id.to_string(), Some(users[0].id))
            .await?;
        assert_eq!(final_annotations.len(), 10);

        let final_comments = collaboration_service
            .get_comments_for_annotation(annotations[0].id)
            .await?;
        assert_eq!(final_comments.len(), 10);

        let final_collaborators = collaboration_service.get_collaborators(chart_id).await?;
        assert_eq!(final_collaborators.len(), 9); // All users except the owner

        println!("✅ Performance test results:");
        println!(
            "   - Annotation creation: {:?} for {} annotations",
            annotation_creation_time,
            annotations.len()
        );
        println!(
            "   - Chart sharing: {:?} for {} collaborators",
            sharing_time,
            final_collaborators.len()
        );
        println!(
            "   - Comment creation: {:?} for {} comments",
            commenting_time,
            final_comments.len()
        );
        println!(
            "   - Average annotation creation: {:?}",
            annotation_creation_time / annotations.len() as u32
        );

        // Performance assertions
        assert!(
            annotation_creation_time.as_millis() < 5000,
            "Annotation creation should complete within 5 seconds"
        );
        assert!(
            sharing_time.as_millis() < 3000,
            "Chart sharing should complete within 3 seconds"
        );
        assert!(
            commenting_time.as_millis() < 3000,
            "Comment creation should complete within 3 seconds"
        );

        Ok(())
    }
}
