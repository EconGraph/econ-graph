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
