//! Runtime store selection. A stale process cannot serve a different backend after the
//! durable cutover; the PostgreSQL rollback shadow is never selected as an error fallback.

use econ_graph_core::models::{ObservationPage, ObservationRequest, TimeSeriesStore};
use econ_graph_core::observation_storage::{ObservationBackend, ObservationStorage};
use econ_graph_core::{AppError, AppResult, DatabasePool};

use super::observation_store::PostgresStore;

pub async fn read_observations(
    pool: &DatabasePool,
    request: ObservationRequest,
    selection: ObservationStorage,
) -> AppResult<ObservationPage> {
    request.validate()?;
    match selection.backend {
        ObservationBackend::Postgres => {
            PostgresStore::with_selection(pool, selection)
                .read_page(request)
                .await
        }
        ObservationBackend::Iceberg => {
            let config = econ_graph_timeseries::StorageConfig::from_env().map_err(|_| {
                AppError::ConfigError("invalid Iceberg storage configuration".into())
            })?;
            econ_graph_timeseries::IcebergStore::with_selection(
                econ_graph_timeseries::SqlCatalog::new(pool.clone(), config),
                selection,
            )
            .read_page(request)
            .await
        }
    }
}
