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
