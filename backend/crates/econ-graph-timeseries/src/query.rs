//! Conservative development budgets. Production capacity/latency is a separate release gate;
//! exceeding a budget returns an error rather than silently truncating data or counts.
use anyhow::{bail, Context, Result};
use datafusion::{
    execution::runtime_env::RuntimeEnvBuilder,
    prelude::{SessionConfig, SessionContext},
};
#[derive(Debug, Clone, Copy)]
pub struct QueryLimits {
    pub max_rows: usize,
    pub memory_bytes: usize,
}
impl QueryLimits {
    pub fn from_env() -> Result<Self> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }
    pub fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> Result<Self> {
        let max_rows = lookup("ICEBERG_MAX_SCAN_ROWS")
            .map(|s| {
                s.parse::<usize>()
                    .context("ICEBERG_MAX_SCAN_ROWS must be a positive integer")
            })
            .transpose()?
            .unwrap_or(100_000);
        let memory_bytes = lookup("ICEBERG_QUERY_MEMORY_BYTES")
            .map(|s| {
                s.parse::<usize>()
                    .context("ICEBERG_QUERY_MEMORY_BYTES must be an integer")
            })
            .transpose()?
            .unwrap_or(256 * 1024 * 1024);
        if max_rows == 0 || max_rows.checked_add(1).is_none() || memory_bytes < 1024 * 1024 {
            bail!("Iceberg budgets require positive bounded rows and at least 1 MiB query memory")
        }
        Ok(Self {
            max_rows,
            memory_bytes,
        })
    }
}
pub fn context() -> Result<SessionContext> {
    let limits = QueryLimits::from_env()?;
    let runtime = RuntimeEnvBuilder::new()
        .with_memory_limit(limits.memory_bytes, 1.0)
        .build_arc()?;
    Ok(SessionContext::new_with_config_rt(
        SessionConfig::new()
            .with_batch_size(1024)
            .with_target_partitions(2),
        runtime,
    ))
}

/// A frozen-pointer summary scans history inside DataFusion, but bounds only the
/// aggregate/selected result before collection. History is still memory-budgeted.
async fn summary_context(
    config: &crate::StorageConfig,
    pointer: &crate::TablePointer,
    series: &[uuid::Uuid],
    max_results: usize,
) -> Result<(SessionContext, String, usize)> {
    use std::sync::Arc;
    if max_results == 0 {
        bail!("summary requires a positive result budget");
    }
    let budget = max_results.min(QueryLimits::from_env()?.max_rows);
    if series.len() > budget {
        bail!("summary series batch exceeds configured result budget");
    }
    let table = crate::SqlCatalog::load_at_config(config, pointer).await?;
    let ctx = context()?;
    let provider =
        iceberg_datafusion::IcebergStaticTableProvider::try_new_from_table(table).await?;
    ctx.register_table("observations", Arc::new(provider))?;
    let ids = series
        .iter()
        .map(|id| format!("'{id}'"))
        .collect::<Vec<_>>()
        .join(",");
    // Immutable series identity allows partition pruning before UUID upsert dedup.
    let logical = format!("logical AS (SELECT * FROM (SELECT *, row_number() OVER(PARTITION BY id ORDER BY _commit_generation DESC) AS commit_rank FROM observations WHERE series_id IN ({ids})) WHERE commit_rank=1)");
    Ok((ctx, logical, budget))
}

/// Count logical observation IDs per series at one immutable snapshot, including
/// every revision and NULL value. Missing series have no aggregate row.
pub async fn counts_at_config(
    config: &crate::StorageConfig,
    pointer: &crate::TablePointer,
    series: &[uuid::Uuid],
    max_results: usize,
) -> Result<Vec<(uuid::Uuid, i64)>> {
    use arrow_array::{Int64Array, StringArray};
    if series.is_empty() {
        return Ok(vec![]);
    }
    let (ctx, logical, budget) = summary_context(config, pointer, series, max_results).await?;
    let sql = format!("WITH {logical} SELECT series_id,COUNT(*) AS count FROM logical GROUP BY series_id LIMIT {}", budget+1);
    let batches = ctx.sql(&sql).await?.collect().await?;
    if batches.iter().map(|b| b.num_rows()).sum::<usize>() > budget {
        bail!("Iceberg count result exceeds configured result budget");
    }
    let mut result = Vec::new();
    for batch in batches {
        let ids = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .context("count series type")?;
        let counts = batch
            .column(1)
            .as_any()
            .downcast_ref::<Int64Array>()
            .context("count result type")?;
        for index in 0..batch.num_rows() {
            result.push((ids.value(index).parse()?, counts.value(index)));
        }
    }
    Ok(result)
}

/// Latest includes a current NULL unless `non_null_current` is requested. For a
/// cross-section, revision selection precedes NULL removal, so a NULL revision
/// never resurrects an older non-NULL revision of the same observation date.
pub async fn latest_at_config(
    config: &crate::StorageConfig,
    pointer: &crate::TablePointer,
    series: &[uuid::Uuid],
    on_date: Option<chrono::NaiveDate>,
    as_of: Option<chrono::NaiveDate>,
    non_null_current: bool,
    max_results: usize,
) -> Result<Vec<econ_graph_core::models::Observation>> {
    if series.is_empty() {
        return Ok(vec![]);
    }
    let (ctx, logical, budget) = summary_context(config, pointer, series, max_results).await?;
    // Compute synthetic-original evidence before the vintage cutoff. A window
    // avoids a correlated subquery over the commit-dedup window (unsupported by
    // DataFusion's common-subexpression optimizer).
    let synthetic_cte=", vintage_evidence AS (SELECT *, MAX(CASE WHEN is_original_release AND revision_date<>date THEN 1 ELSE 0 END) OVER(PARTITION BY series_id,date) AS has_real_original FROM logical)";
    let source = "vintage_evidence";
    let mut eligible = String::from(
        "NOT (p.is_original_release AND p.revision_date=p.date AND p.has_real_original=1)",
    );
    if let Some(day) = on_date {
        eligible.push_str(&format!(" AND p.date=DATE '{day}'"));
    }
    if let Some(day) = as_of {
        eligible.push_str(&format!(" AND p.revision_date<=DATE '{day}' AND NOT (p.is_original_release AND p.revision_date=p.date AND p.has_real_original=1)"));
    }
    let nonnull = if non_null_current {
        " AND value IS NOT NULL"
    } else {
        ""
    };
    let sql=format!("WITH {logical}{synthetic_cte}, current_revisions AS (SELECT p.*, row_number() OVER(PARTITION BY series_id,date ORDER BY revision_date DESC,is_original_release ASC,id DESC) AS revision_rank FROM {source} p WHERE {eligible}), selected AS (SELECT *,row_number() OVER(PARTITION BY series_id ORDER BY date DESC) AS latest_rank FROM current_revisions WHERE revision_rank=1{nonnull}) SELECT id,series_id,date,revision_date,value,is_original_release,created_at,updated_at,attributes FROM selected WHERE latest_rank=1 LIMIT {}",budget+1);
    let batches = ctx.sql(&sql).await?.collect().await?;
    if batches.iter().map(|b| b.num_rows()).sum::<usize>() > budget {
        bail!("Iceberg latest result exceeds configured result budget");
    }
    crate::encoding::from_batches(&batches)
}
