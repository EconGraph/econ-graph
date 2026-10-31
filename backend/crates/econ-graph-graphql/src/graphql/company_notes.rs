//! Owner-private notes reuse financial annotations and the common versioned anchor contract.
use super::context::{app_pool, current_user, require_role};
use async_graphql::{ComplexObject, Context, Result, SimpleObject, ID};
use econ_graph_auth::Role;
use econ_graph_core::{
    database::DataPool,
    models::{AnnotationAnchor, FinancialAnnotation},
};
use econ_graph_services::services::{company_notes as service, series_check::DataPlaneClient};
use std::sync::Arc;

#[derive(SimpleObject)]
#[graphql(complex)]
pub struct CompanyNote {
    pub id: ID,
    /// Original versioned target; never silently reattached to a later filing.
    pub anchor: String,
    pub cik: String,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
#[ComplexObject]
impl CompanyNote {
    /// The note survives missing/inaccessible source data. This checks only target existence;
    /// the original anchor and user-authored content are never rewritten by resolution.
    async fn target_status(&self, ctx: &Context<'_>) -> &'static str {
        let auth = ctx.data_opt::<Arc<super::context::GraphQLContext>>();
        if !auth.is_some_and(|a| a.can_read_catalog(Role::CatalogSecRead)) {
            return "not_found";
        }
        let Ok(anchor) = parse_anchor(&self.anchor) else {
            return "unavailable";
        };
        match target_exists(ctx, &anchor).await {
            Ok(true) => "found",
            Ok(false) => "not_found",
            Err(_) => "unavailable",
        }
    }
}
async fn target_exists(ctx: &Context<'_>, anchor: &AnnotationAnchor) -> Result<bool> {
    if ctx.data_opt::<DataPool>().is_some() {
        super::company_research::target_exists(ctx, anchor).await
    } else if let Some(client) = ctx.data_opt::<Arc<DataPlaneClient>>() {
        Ok(client.company_research_target_exists(anchor).await?)
    } else {
        Err(econ_graph_core::error::AppError::DataPlaneUnavailable(
            "Company research target check unavailable".into(),
        )
        .into())
    }
}
impl TryFrom<FinancialAnnotation> for CompanyNote {
    type Error = async_graphql::Error;
    fn try_from(note: FinancialAnnotation) -> Result<Self> {
        let anchor = AnnotationAnchor::try_from(&note)?;
        let (cik, _) = service::company_anchor(&anchor)?;
        Ok(Self {
            id: note.id.into(),
            anchor: serde_json::to_string(&anchor)?,
            cik,
            content: note.content,
            created_at: note.created_at,
            updated_at: note.updated_at,
        })
    }
}
fn parse_anchor(input: &str) -> Result<AnnotationAnchor> {
    if input.len() > 16_384 {
        return Err(super::errors::bad_user_input(
            "Company note anchor is too large",
        ));
    }
    let anchor = AnnotationAnchor::from_json(input)
        .map_err(|_| super::errors::bad_user_input("Invalid company note anchor"))?;
    service::company_anchor(&anchor)?;
    Ok(anchor)
}

pub async fn company_notes(
    ctx: &Context<'_>,
    cik: String,
    anchor: Option<String>,
    offset: Option<i32>,
) -> Result<Vec<CompanyNote>> {
    let owner = current_user(ctx)?.id;
    let anchor = anchor.as_deref().map(parse_anchor).transpose()?;
    service::list(
        &app_pool(ctx)?.0,
        owner,
        &cik,
        anchor.as_ref(),
        i64::from(offset.unwrap_or(0)),
    )
    .await?
    .into_iter()
    .map(CompanyNote::try_from)
    .collect()
}

pub async fn create(ctx: &Context<'_>, anchor: String, content: String) -> Result<CompanyNote> {
    let owner = require_role(ctx, Role::AnnotationCreate)?.user_id;
    // Local grant required even when the remote plane has a broader shared-plane grant.
    require_role(ctx, Role::CatalogSecRead)?;
    let anchor = parse_anchor(&anchor)?;
    let exists = target_exists(ctx, &anchor).await?;
    if !exists {
        return Err(async_graphql::Error::new(
            "Company research target not found",
        ));
    }
    service::create(&app_pool(ctx)?.0, owner, anchor, content)
        .await?
        .try_into()
}

pub async fn update(ctx: &Context<'_>, id: ID, content: String) -> Result<CompanyNote> {
    let owner = current_user(ctx)?.id;
    service::update(
        &app_pool(ctx)?.0,
        owner,
        uuid::Uuid::parse_str(id.as_str())?,
        content,
    )
    .await?
    .ok_or_else(|| async_graphql::Error::new("Company note not found"))?
    .try_into()
}

pub async fn delete(ctx: &Context<'_>, id: ID) -> Result<bool> {
    let owner = current_user(ctx)?.id;
    service::delete(
        &app_pool(ctx)?.0,
        owner,
        uuid::Uuid::parse_str(id.as_str())?,
    )
    .await
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_graphql::{Request, Variables};
    use econ_graph_core::{
        config::DataGraphqlConfig,
        database::AppPool,
        models::{SubjectRef, User},
        test_utils::ScratchDatabase,
    };
    use std::time::Duration;
    use wiremock::{
        matchers::{body_partial_json, method},
        Mock, MockServer, ResponseTemplate,
    };

    #[tokio::test]
    async fn company_notes_enforce_owner_and_roles_and_preserve_targets_during_source_outages() {
        let database = ScratchDatabase::create("gql_company_notes");
        econ_graph_core::run_migrations(&database.url)
            .await
            .unwrap();
        let pool = econ_graph_core::create_pool(&database.url).await.unwrap();
        let alice = User::get_or_create_for_subject(
            &pool,
            uuid::Uuid::new_v4(),
            None,
            false,
            Some("Alice"),
        )
        .await
        .unwrap();
        let bob =
            User::get_or_create_for_subject(&pool, uuid::Uuid::new_v4(), None, false, Some("Bob"))
                .await
                .unwrap();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(
                serde_json::json!({"operationName":"CompanyResearchTargetExists"}),
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"data":{"companyResearchTargetExists":true}}),
                ),
            )
            .mount(&server)
            .await;
        let remote = Arc::new(
            DataPlaneClient::new(&DataGraphqlConfig {
                url: server.uri(),
                host: None,
                timeout: Duration::from_secs(2),
                forward_authorization: false,
                allow_insecure_authorization: false,
            })
            .unwrap(),
        );
        let schema = super::super::schema::create_schema_with_data_plane(
            AppPool(pool),
            None,
            Some(remote),
            (),
        );
        let alice_auth = Arc::new(super::super::context::GraphQLContext::signed_in(
            alice.clone(),
            [Role::AnnotationCreate, Role::CatalogSecRead],
        ));
        let bob_auth = Arc::new(super::super::context::GraphQLContext::signed_in(
            bob,
            [Role::AnnotationCreate, Role::CatalogSecRead],
        ));
        let anchor = serde_json::to_string(&AnnotationAnchor::V1 {
            target: SubjectRef::Company {
                cik: "0000320193".into(),
            },
            chart_id: None,
            observation_date: None,
            observation_value: None,
        })
        .unwrap();
        let create = || {
            Request::new("mutation($anchor:String!){createCompanyNote(anchor:$anchor,content:\"Private evidence\"){id anchor content targetStatus}}")
            .variables(Variables::from_json(serde_json::json!({"anchor":anchor})))
        };
        let anonymous = schema
            .execute(create().data(Arc::new(super::super::context::GraphQLContext::anonymous())))
            .await;
        assert!(!anonymous.errors.is_empty());
        for roles in [vec![Role::AnnotationCreate], vec![Role::CatalogSecRead]] {
            let denied =
                schema
                    .execute(create().data(Arc::new(
                        super::super::context::GraphQLContext::signed_in(alice.clone(), roles),
                    )))
                    .await;
            assert!(!denied.errors.is_empty());
        }
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "Role denials must precede remote lookup"
        );
        let result = schema.execute(create().data(alice_auth.clone())).await;
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let data = result.data.into_json().unwrap();
        let note = &data["createCompanyNote"];
        let id = note["id"].as_str().unwrap();
        assert_eq!(note["anchor"], anchor);
        assert_eq!(note["targetStatus"], "found");
        let list = "{companyNotes(cik:\"0000320193\"){id anchor content targetStatus}}";
        let bob_list = schema
            .execute(Request::new(list).data(bob_auth.clone()))
            .await;
        assert!(bob_list.errors.is_empty());
        assert_eq!(
            bob_list.data.into_json().unwrap()["companyNotes"],
            serde_json::json!([])
        );
        let update = format!("mutation{{updateCompanyNote(id:\"{id}\",content:\"Edited evidence\"){{id anchor content}}}}");
        assert!(!schema
            .execute(Request::new(&update).data(bob_auth.clone()))
            .await
            .errors
            .is_empty());
        let delete = format!("mutation{{deleteCompanyNote(id:\"{id}\")}}");
        let denied_delete = schema.execute(Request::new(&delete).data(bob_auth)).await;
        assert!(denied_delete.errors.is_empty());
        assert_eq!(
            denied_delete.data.into_json().unwrap()["deleteCompanyNote"],
            false
        );
        // Existing app-authored content remains accessible and editable with the source offline.
        server.reset().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let outage = schema
            .execute(Request::new(list).data(alice_auth.clone()))
            .await;
        assert!(outage.errors.is_empty(), "{:?}", outage.errors);
        let rows = outage.data.into_json().unwrap();
        assert_eq!(rows["companyNotes"][0]["content"], "Private evidence");
        assert_eq!(rows["companyNotes"][0]["anchor"], anchor);
        assert_eq!(rows["companyNotes"][0]["targetStatus"], "unavailable");
        assert!(
            !schema
                .execute(create().data(alice_auth.clone()))
                .await
                .errors
                .is_empty(),
            "New targets fail closed during outages"
        );
        let edited = schema
            .execute(Request::new(update).data(alice_auth.clone()))
            .await;
        assert!(edited.errors.is_empty(), "{:?}", edited.errors);
        assert_eq!(
            edited.data.into_json().unwrap()["updateCompanyNote"]["anchor"],
            anchor
        );
        // Losing catalog access hides the target's existence, but not one's own authored content.
        let revoked = Arc::new(super::super::context::GraphQLContext::signed_in(alice, []));
        let retained = schema
            .execute(Request::new(list).data(revoked.clone()))
            .await;
        assert!(retained.errors.is_empty());
        assert_eq!(
            retained.data.into_json().unwrap()["companyNotes"][0]["targetStatus"],
            "not_found"
        );
        let deleted = schema.execute(Request::new(delete).data(revoked)).await;
        assert!(deleted.errors.is_empty());
        assert_eq!(deleted.data.into_json().unwrap()["deleteCompanyNote"], true);
    }
}
