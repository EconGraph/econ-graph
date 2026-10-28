//! Catalog authorization shared by every data resolver. Unknown catalog names deny access.
use super::context::{data_pool, GraphQLContext};
use async_graphql::{Context, ErrorExtensions, Result};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use econ_graph_auth::Role;
use econ_graph_core::models::DataSource;
use econ_graph_core::schema::{data_sources, economic_series};
use std::sync::Arc;
use uuid::Uuid;

/// Source IDs the caller may read, cached only for this request.
pub async fn allowed_sources(ctx: &Context<'_>) -> Result<Vec<Uuid>> {
    let pool = data_pool(ctx)?;
    let auth = ctx.data_opt::<Arc<GraphQLContext>>();
    let load = || async {
        let mut conn = pool.get().await?;
        let rows = data_sources::table
            .select(DataSource::as_select())
            .load::<DataSource>(&mut conn)
            .await?;
        Ok::<_, async_graphql::Error>(
            rows.into_iter()
                .filter(|source| {
                    // Legacy fixture schemas have no HTTP auth context. Production always denies
                    // absent contexts; authorization tests supply an explicit context.
                    #[cfg(test)]
                    if auth.is_none() {
                        return true;
                    }
                    Role::for_catalog(&source.name)
                        .is_some_and(|role| auth.is_some_and(|auth| auth.can_read_catalog(role)))
                })
                .map(|source| source.id)
                .collect(),
        )
    };
    match auth {
        Some(auth) => Ok(auth.catalog_sources.get_or_try_init(load).await?.clone()),
        None => load().await,
    }
}

/// Deny reads for a source outside the caller's catalog grants.
pub async fn require_source(ctx: &Context<'_>, source: Uuid) -> Result<()> {
    if allowed_sources(ctx).await?.contains(&source) {
        Ok(())
    } else {
        Err(forbidden())
    }
}

/// `false` only for a series that exists in a catalog the caller can't read, which callers then
/// answer exactly as for an unknown series.
pub async fn series_readable_or_missing(ctx: &Context<'_>, series: Uuid) -> Result<bool> {
    let pool = data_pool(ctx)?;
    let mut conn = pool.get().await?;
    let source = economic_series::table
        .filter(economic_series::id.eq(series))
        .select(economic_series::source_id)
        .first::<Uuid>(&mut conn)
        .await
        .optional()?;
    drop(conn);
    Ok(match source {
        Some(source) => allowed_sources(ctx).await?.contains(&source),
        None => true,
    })
}

/// Whether the caller may read the catalog with this data source name.
fn can_read_catalog_named(auth: Option<&GraphQLContext>, name: &str) -> bool {
    // As in `allowed_sources`: only legacy unit-test schemas lack an auth context.
    #[cfg(test)]
    if auth.is_none() {
        return true;
    }
    Role::for_catalog(name).is_some_and(|role| auth.is_some_and(|auth| auth.can_read_catalog(role)))
}

/// Whether the series exists and the caller may read its catalog, checked in the data database
/// or, on a backend without one, through the data plane (whose own grants for this request also
/// apply). A series the caller can't read answers `false`, exactly like a missing one, so
/// annotation paths reveal nothing about other catalogs. `false` on a backend with neither a data
/// database nor a data plane, where no catalog can be checked (startup refuses that with sign-in on).
///
/// # Errors
/// `DATA_PLANE_UNAVAILABLE` when the data plane can't answer, or a database error.
pub async fn series_visible(ctx: &Context<'_>, series: Uuid) -> Result<bool> {
    let auth = ctx.data_opt::<Arc<GraphQLContext>>().map(Arc::as_ref);
    Ok(super::context::series_check(ctx)
        .readable_series_exists(series, |name| can_read_catalog_named(auth, name))
        .await?)
}

/// The catalog name of a series the caller may read, or `None` for a missing or unreadable one.
pub async fn readable_series_catalog(ctx: &Context<'_>, series: Uuid) -> Result<Option<String>> {
    let pool = data_pool(ctx)?;
    let auth = ctx.data_opt::<Arc<GraphQLContext>>().map(Arc::as_ref);
    Ok(
        econ_graph_services::services::series_service::series_catalog(pool, series)
            .await?
            .filter(|name| can_read_catalog_named(auth, name)),
    )
}

/// Refuse an annotation, or its comments, when it is on a series the caller can't read, with
/// the same `Annotation not found` error as for a missing annotation. Chart annotations (no
/// series) pass.
pub async fn require_annotation_readable(
    ctx: &Context<'_>,
    service: &econ_graph_services::services::collaboration_service::CollaborationService,
    annotation: Uuid,
) -> Result<()> {
    let Some(series) = service.annotation_series_id(annotation).await? else {
        return Ok(());
    };
    let visible = match Uuid::parse_str(&series) {
        Ok(series) => series_visible(ctx, series).await?,
        Err(_) => false,
    };
    if visible {
        Ok(())
    } else {
        Err(econ_graph_core::error::AppError::NotFound("Annotation not found".to_string()).into())
    }
}

/// Stable authorization error without internal identifiers.
pub fn forbidden() -> async_graphql::Error {
    async_graphql::Error::new("Insufficient catalog permissions")
        .extend_with(|_, ext| ext.set("code", "FORBIDDEN"))
}
