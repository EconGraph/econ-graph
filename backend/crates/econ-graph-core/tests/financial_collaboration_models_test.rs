use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use econ_graph_core::enums::{AnnotationType, AssignmentType};
use econ_graph_core::models::{
    AnnotationAssignment, AnnotationReply, AnnotationTemplate, FinancialAnnotation,
    NewAnnotationAssignment, NewAnnotationReply, NewAnnotationTemplate, NewFinancialAnnotation,
    User,
};
use econ_graph_core::schema::{
    annotation_assignments, annotation_replies, annotation_templates, financial_annotations,
};
use econ_graph_core::test_utils::ScratchDatabase;
use uuid::Uuid;

/// Persist each financial collaboration model against the actual app migrations.
#[tokio::test]
async fn financial_collaboration_models_match_the_app_schema() {
    let database = ScratchDatabase::create("financial_collaboration_models");
    econ_graph_core::run_migrations(&database.url)
        .await
        .unwrap();
    let pool = econ_graph_core::create_pool(&database.url).await.unwrap();
    let user =
        User::get_or_create_for_subject(&pool, Uuid::new_v4(), None, false, Some("Reviewer"))
            .await
            .unwrap();
    let mut connection = pool.get().await.unwrap();
    let annotation: FinancialAnnotation = diesel::insert_into(financial_annotations::table)
        .values(
            NewFinancialAnnotation::new(
                Uuid::new_v4(),
                user.id,
                "Review revenue growth".into(),
                AnnotationType::RevenueGrowth,
            )
            .with_tags(vec!["revenue".into()]),
        )
        .returning(FinancialAnnotation::as_returning())
        .get_result(&mut connection)
        .await
        .unwrap();
    assert_eq!(annotation.title, "Review revenue growth");
    assert_eq!(annotation.created_by, user.id);
    assert_eq!(annotation.tags, Some(vec![Some("revenue".into())]));

    let reply: AnnotationReply = diesel::insert_into(annotation_replies::table)
        .values(NewAnnotationReply::new(
            annotation.id,
            user.id,
            "Checked".into(),
        ))
        .returning(AnnotationReply::as_returning())
        .get_result(&mut connection)
        .await
        .unwrap();
    assert_eq!(reply.annotation_id, annotation.id);
    assert_eq!(reply.created_by, user.id);
    assert!(reply.parent_reply_id.is_none());

    let assignment: AnnotationAssignment = diesel::insert_into(annotation_assignments::table)
        .values(
            NewAnnotationAssignment::new(annotation.id, user.id, user.id, AssignmentType::Review)
                .with_notes("Verify the source".into()),
        )
        .returning(AnnotationAssignment::as_returning())
        .get_result(&mut connection)
        .await
        .unwrap();
    assert_eq!(assignment.annotation_id, annotation.id);
    assert_eq!(
        assignment.instructions.as_deref(),
        Some("Verify the source")
    );
    assert!(assignment.completed_at.is_none());

    let template: AnnotationTemplate = diesel::insert_into(annotation_templates::table)
        .values(NewAnnotationTemplate::new(
            "Revenue review".into(),
            "Check revenue".into(),
            AnnotationType::RevenueGrowth,
            user.id,
        ))
        .returning(AnnotationTemplate::as_returning())
        .get_result(&mut connection)
        .await
        .unwrap();
    let stored: AnnotationTemplate = annotation_templates::table
        .find(template.id)
        .select(AnnotationTemplate::as_select())
        .first(&mut connection)
        .await
        .unwrap();
    assert_eq!(stored.template_content, "Check revenue");
    assert!(stored.is_active);
    assert_eq!(stored.usage_count, Some(0));
}
