//! Private company research notes in the existing app-owned financial annotation model.
//! The data service validates creation targets; later source loss never deletes authored content.
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use econ_graph_core::{
    database::DatabasePool,
    enums::{AnnotationStatus, AnnotationType},
    error::{AppError, AppResult},
    models::{AnnotationAnchor, FinancialAnnotation, NewFinancialAnnotation, SubjectRef},
    schema::financial_annotations as notes,
};
use uuid::Uuid;

/// Extract the canonical issuer only for company-research targets. Chart notes retain the
/// selected economic or filed fact anchor; saved chart objects belong to the workspace.
pub fn company_anchor(anchor: &AnnotationAnchor) -> AppResult<(String, String)> {
    anchor
        .validate()
        .map_err(|error| AppError::ValidationError(error.to_string()))?;
    let AnnotationAnchor::V1 {
        target, chart_id, ..
    } = anchor;
    if chart_id.is_some() {
        return Err(AppError::ValidationError(
            "Company notes cannot name a workspace chart".into(),
        ));
    }
    let cik = match target {
        SubjectRef::Company { cik } => cik,
        SubjectRef::Filing { filing } | SubjectRef::Statement { filing, .. } => &filing.cik,
        SubjectRef::Fact { subject } | SubjectRef::FiledFact { subject, .. } => &subject.cik,
        _ => {
            return Err(AppError::ValidationError(
                "Unsupported company note target".into(),
            ))
        }
    };
    Ok((
        cik.clone(),
        target
            .canonical_key()
            .map_err(|e| AppError::ValidationError(e.to_string()))?,
    ))
}

fn validate_content(content: &str) -> AppResult<()> {
    if content.trim().is_empty() || content.chars().count() > 5_000 {
        return Err(AppError::ValidationError(
            "A note requires 1–5000 characters".into(),
        ));
    }
    Ok(())
}

/// Creation assumes the caller has already authorized and resolved the target on the data plane.
pub async fn create(
    pool: &DatabasePool,
    owner: Uuid,
    anchor: AnnotationAnchor,
    content: String,
) -> AppResult<FinancialAnnotation> {
    validate_content(&content)?;
    let (cik, key) = company_anchor(&anchor)?;
    let mut note =
        NewFinancialAnnotation::new(Uuid::nil(), owner, content, AnnotationType::Comment);
    note.statement_id = None;
    note.research_anchor =
        Some(serde_json::to_value(anchor).map_err(|e| AppError::ValidationError(e.to_string()))?);
    note.research_anchor_key = Some(key);
    note.research_cik = Some(cik);
    let mut conn = pool.get().await?;
    Ok(diesel::insert_into(notes::table)
        .values(note)
        .returning(FinancialAnnotation::as_returning())
        .get_result(&mut conn)
        .await?)
}

/// Lists only the caller's notes, including those whose original source target no longer exists.
pub async fn list(
    pool: &DatabasePool,
    owner: Uuid,
    cik: &str,
    anchor: Option<&AnnotationAnchor>,
    offset: i64,
) -> AppResult<Vec<FinancialAnnotation>> {
    if offset < 0 {
        return Err(AppError::ValidationError("Invalid notes offset".into()));
    }
    let company = AnnotationAnchor::V1 {
        target: SubjectRef::Company { cik: cik.into() },
        chart_id: None,
        observation_date: None,
        observation_value: None,
    };
    company_anchor(&company)?;
    let mut query = notes::table
        .filter(notes::created_by.eq(owner))
        .filter(notes::research_cik.eq(cik))
        .filter(notes::research_anchor.is_not_null())
        .into_boxed();
    if let Some(anchor) = anchor {
        let (issuer, key) = company_anchor(anchor)?;
        if issuer != cik {
            return Err(AppError::ValidationError(
                "Company and note target differ".into(),
            ));
        }
        query = query.filter(notes::research_anchor_key.eq(key));
    }
    let mut conn = pool.get().await?;
    Ok(query
        .order((notes::created_at.desc(), notes::id.desc()))
        .offset(offset)
        .limit(100)
        .select(FinancialAnnotation::as_select())
        .load(&mut conn)
        .await?)
}

/// Owner and note kind are constrained in the mutation itself, preventing guessed-ID updates.
pub async fn update(
    pool: &DatabasePool,
    owner: Uuid,
    id: Uuid,
    content: String,
) -> AppResult<Option<FinancialAnnotation>> {
    validate_content(&content)?;
    let title: String = content
        .lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect();
    let mut conn = pool.get().await?;
    Ok(diesel::update(
        notes::table
            .filter(notes::id.eq(id))
            .filter(notes::created_by.eq(owner))
            .filter(notes::research_anchor.is_not_null()),
    )
    .set((
        notes::content.eq(content),
        notes::title.eq(title),
        notes::status.eq(AnnotationStatus::Active),
    ))
    .returning(FinancialAnnotation::as_returning())
    .get_result(&mut conn)
    .await
    .optional()?)
}

pub async fn delete(pool: &DatabasePool, owner: Uuid, id: Uuid) -> AppResult<bool> {
    let mut conn = pool.get().await?;
    Ok(diesel::delete(
        notes::table
            .filter(notes::id.eq(id))
            .filter(notes::created_by.eq(owner))
            .filter(notes::research_anchor.is_not_null()),
    )
    .execute(&mut conn)
    .await?
        != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use econ_graph_core::models::{FactSubject, FilingRef, ResearchPeriod};
    use std::collections::BTreeMap;
    fn anchor(target: SubjectRef) -> AnnotationAnchor {
        AnnotationAnchor::V1 {
            target,
            chart_id: None,
            observation_date: None,
            observation_value: None,
        }
    }
    #[test]
    fn filed_fact_and_economic_subject_have_distinct_note_targets() {
        let subject = FactSubject {
            cik: "0000320193".into(),
            concept_namespace: "urn:econgraph:sec-companyfacts:taxonomy:us-gaap".into(),
            concept: "Assets".into(),
            period: ResearchPeriod::Instant {
                date: chrono::NaiveDate::from_ymd_opt(2025, 9, 30).unwrap(),
            },
            unit: "urn:econgraph:sec-companyfacts:unit:USD".into(),
            dimensions: BTreeMap::new(),
        };
        let fact = company_anchor(&anchor(SubjectRef::Fact {
            subject: subject.clone(),
        }))
        .unwrap();
        let filed = company_anchor(&anchor(SubjectRef::FiledFact {
            subject,
            filing: FilingRef {
                cik: "0000320193".into(),
                accession: "0000320193-25-000001".into(),
            },
        }))
        .unwrap();
        assert_eq!(fact.0, filed.0);
        assert_ne!(fact.1, filed.1);
    }
    #[test]
    fn refuses_non_company_targets_and_invalid_content() {
        assert!(company_anchor(&anchor(SubjectRef::Chart { id: Uuid::new_v4() })).is_err());
        assert!(company_anchor(&anchor(SubjectRef::Company {
            cik: "320193".into()
        }))
        .is_err());
        assert!(validate_content("   ").is_err());
        assert!(validate_content(&"é".repeat(5000)).is_ok());
        assert!(validate_content(&"é".repeat(5001)).is_err());
    }
}

#[cfg(test)]
mod database_tests {
    use super::*;
    use econ_graph_core::{models::User, test_utils::ScratchDatabase};
    #[tokio::test]
    async fn owners_cannot_read_edit_or_delete_each_others_notes_and_legacy_notes_survive() {
        let db = ScratchDatabase::create("company_notes");
        econ_graph_core::run_migrations(&db.url).await.unwrap();
        let pool = econ_graph_core::create_pool(&db.url).await.unwrap();
        let alice =
            User::get_or_create_for_subject(&pool, Uuid::new_v4(), None, false, Some("Alice"))
                .await
                .unwrap();
        let bob = User::get_or_create_for_subject(&pool, Uuid::new_v4(), None, false, Some("Bob"))
            .await
            .unwrap();
        let anchor = AnnotationAnchor::V1 {
            target: SubjectRef::Company {
                cik: "0000320193".into(),
            },
            chart_id: None,
            observation_date: None,
            observation_value: None,
        };
        let note = create(
            &pool,
            alice.id,
            anchor.clone(),
            "My original research".into(),
        )
        .await
        .unwrap();
        assert_eq!(AnnotationAnchor::try_from(&note).unwrap(), anchor);
        // No data-plane company/fact record exists. App content remains readable and editable.
        assert_eq!(
            list(&pool, alice.id, "0000320193", None, 0)
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(list(&pool, bob.id, "0000320193", None, 0)
            .await
            .unwrap()
            .is_empty());
        assert!(update(&pool, bob.id, note.id, "stolen".into())
            .await
            .unwrap()
            .is_none());
        assert!(!delete(&pool, bob.id, note.id).await.unwrap());
        let edited = update(
            &pool,
            alice.id,
            note.id,
            "Updated with missing source".into(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(edited.research_anchor, note.research_anchor);
        assert!(list(&pool, alice.id, "0000320193", None, 100)
            .await
            .unwrap()
            .is_empty());
        assert!(list(&pool, alice.id, "0000320193", None, -1).await.is_err());
        let legacy_id = Uuid::new_v4();
        let mut conn = pool.get().await.unwrap();
        let legacy: FinancialAnnotation = diesel::insert_into(notes::table)
            .values(NewFinancialAnnotation::new(
                legacy_id,
                alice.id,
                "Legacy note".into(),
                AnnotationType::Comment,
            ))
            .returning(FinancialAnnotation::as_returning())
            .get_result(&mut conn)
            .await
            .unwrap();
        drop(conn);
        let AnnotationAnchor::V1 { target, .. } = AnnotationAnchor::try_from(&legacy).unwrap();
        assert_eq!(
            target,
            SubjectRef::LegacyFinancial {
                statement_id: legacy_id,
                line_item_id: None
            }
        );
        assert!(update(&pool, alice.id, legacy.id, "wrong route".into())
            .await
            .unwrap()
            .is_none());
        assert!(!delete(&pool, alice.id, legacy.id).await.unwrap());
        assert!(delete(&pool, alice.id, note.id).await.unwrap());
        assert!(list(&pool, alice.id, "0000320193", None, 0)
            .await
            .unwrap()
            .is_empty());
    }
}
