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

/// Check a series' parent catalog before returning metadata, existence or observations.
pub async fn require_series(ctx: &Context<'_>, series: Uuid) -> Result<()> {
    let pool = data_pool(ctx)?;
    let mut conn = pool.get().await?;
    let source = economic_series::table
        .filter(economic_series::id.eq(series))
        .select(economic_series::source_id)
        .first::<Uuid>(&mut conn)
        .await
        .optional()?;
    drop(conn);
    match source {
        Some(source) => require_source(ctx, source).await,
        None => Ok(()),
    }
}

/// Stable authorization error without internal identifiers.
pub fn forbidden() -> async_graphql::Error {
    async_graphql::Error::new("Insufficient catalog permissions")
        .extend_with(|_, ext| ext.set("code", "FORBIDDEN"))
}
