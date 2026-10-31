//! Versioned current-catalog snapshots, separate from additive historical dimension labels.
//!
//! A conditional response identifies an immutable parsed snapshot by its body hash. Every
//! process loads that shared snapshot, so another worker updating the HTTP validator cannot
//! leave this worker using a stale in-memory list. Missing snapshots force a full download.

use diesel::sql_types::{Jsonb, Text};
use diesel::OptionalExtension;
use diesel_async::RunQueryDsl;
use econ_graph_core::models::Code;

use crate::adapter::CrawlCtx;
use crate::error::CrawlError;
use crate::http::IfChanged;
use crate::persist;
use crate::reference_file::CodeList;
use crate::source::SourceId;

fn database_error(e: impl std::fmt::Display) -> CrawlError {
    CrawlError::Transient(format!("reading/writing current catalog snapshot: {e}"))
}

async fn load(
    ctx: &CrawlCtx,
    source: SourceId,
    url: &str,
    hash: &str,
) -> Result<Option<Vec<Code>>, CrawlError> {
    #[derive(diesel::QueryableByName)]
    struct Row {
        #[diesel(sql_type = Jsonb)]
        codes: serde_json::Value,
    }
    let mut conn = ctx.pool.get().await.map_err(database_error)?;
    let row: Option<Row> = diesel::sql_query(
        "SELECT codes FROM catalog_reference_snapshots WHERE source = $1 AND url = $2 AND content_sha256 = $3"
    ).bind::<Text,_>(source.as_str()).bind::<Text,_>(url).bind::<Text,_>(hash)
    .get_result(&mut conn).await.optional().map_err(database_error)?;
    row.map(|r| serde_json::from_value(r.codes).map_err(database_error))
        .transpose()
}

async fn store(
    ctx: &CrawlCtx,
    source: SourceId,
    url: &str,
    hash: &str,
    codes: &[Code],
) -> Result<(), CrawlError> {
    let mut conn = ctx.pool.get().await.map_err(database_error)?;
    let payload = serde_json::to_value(codes).map_err(database_error)?;
    diesel::sql_query("INSERT INTO catalog_reference_snapshots (source,url,content_sha256,codes) VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING")
        .bind::<Text,_>(source.as_str()).bind::<Text,_>(url).bind::<Text,_>(hash).bind::<Jsonb,_>(payload)
        .execute(&mut conn).await.map_err(database_error)?;
    Ok(())
}

/// Loads precisely the catalog the source currently confirms. Historical labels remain
/// additive; removal from this snapshot never deletes an old label or series.
pub async fn current_codes(
    ctx: &CrawlCtx,
    source: SourceId,
    list: &CodeList,
) -> Result<Vec<Code>, CrawlError> {
    let known = persist::url_validators(&ctx.pool, source, &list.url)
        .await
        .map_err(database_error)?;
    let response = ctx
        .http
        .get_reference_file_if_changed(source, list.request_url(), known.as_ref())
        .await?;
    let (codes, validators) = match response {
        IfChanged::Changed { body, validators } => ((list.parse)(&body)?, validators),
        IfChanged::Unchanged { validators } => {
            let snapshot = match validators.content_sha256.as_deref() {
                Some(hash) => load(ctx, source, &list.url, hash).await?,
                None => None,
            };
            if let Some(codes) = snapshot {
                (codes, validators)
            } else {
                // Old reference-code seeds have validators but no exact membership snapshot.
                // Do not derive current membership from their additive historical labels.
                match ctx
                    .http
                    .get_reference_file_if_changed(source, list.request_url(), None)
                    .await?
                {
                    IfChanged::Changed { body, validators } => ((list.parse)(&body)?, validators),
                    IfChanged::Unchanged { .. } => {
                        return Err(CrawlError::Transient(
                            "catalog returned 304 without a stored membership snapshot".into(),
                        ))
                    }
                }
            }
        }
    };
    if codes.is_empty() {
        return Err(CrawlError::Parse(
            "current catalog contains no codes".into(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    if codes
        .iter()
        .any(|c| c.code.is_empty() || !seen.insert(c.code.clone()))
    {
        return Err(CrawlError::Parse(
            "current catalog contains empty or duplicate codes".into(),
        ));
    }
    let merged = persist::merge_dataset_dimension_code_entries(
        &ctx.pool,
        source,
        list.dataset,
        list.dimension,
        &codes,
    )
    .await
    .map_err(database_error)?;
    if !merged {
        return Err(CrawlError::Transient(format!(
            "catalog dataset {} has not been synchronized",
            list.dataset
        )));
    }
    let hash = validators
        .content_sha256
        .as_deref()
        .ok_or_else(|| CrawlError::Transient("catalog response has no content hash".into()))?;
    store(ctx, source, &list.url, hash, &codes).await?;
    persist::set_url_validators(&ctx.pool, source, &list.url, &validators)
        .await
        .map_err(database_error)?;
    Ok(codes)
}
