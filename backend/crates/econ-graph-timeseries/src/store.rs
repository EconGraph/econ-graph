use crate::{SqlCatalog, TablePointer};
use bigdecimal::Zero;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use econ_graph_core::{
    error::{AppError, AppResult},
    models::{
        Observation, ObservationPage, ObservationRequest, PageContext, RevisionMode,
        TimeSeriesStore,
    },
    schema::economic_series,
};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;
#[derive(Clone)]
pub struct IcebergStore {
    pub catalog: SqlCatalog,
    selection: Option<econ_graph_core::observation_storage::ObservationStorage>,
}
impl IcebergStore {
    pub fn new(catalog: SqlCatalog) -> Self {
        Self {
            catalog,
            selection: None,
        }
    }
    pub fn with_selection(
        catalog: SqlCatalog,
        selection: econ_graph_core::observation_storage::ObservationStorage,
    ) -> Self {
        Self {
            catalog,
            selection: Some(selection),
        }
    }
    pub async fn read_page_at(
        &self,
        pointer: &TablePointer,
        request: ObservationRequest,
    ) -> anyhow::Result<ObservationPage> {
        request.validate()?;
        // A fixed metadata pointer is the read snapshot even while another crawler publishes.
        // DataFusion performs physical scans; selection remains exact on decoded decimal DTOs.
        let rows = self
            .catalog
            .read_rows_filtered(pointer, Some(&request))
            .await?;
        Ok(select_page(rows, pointer.dataset_id, &request))
    }
}
impl TimeSeriesStore for IcebergStore {
    async fn read_page(&self, request: ObservationRequest) -> AppResult<ObservationPage> {
        request.validate()?;
        let mut conn = self
            .catalog
            .pool
            .get()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let (dataset, pointer) = conn
            .build_transaction()
            .repeatable_read()
            .read_only()
            .run::<_, AppError, _>(async |conn| {
                if let Some(selection) = &self.selection {
                    selection.assert_read_selection_conn(conn).await?;
                }
                let dataset = economic_series::table
                    .find(request.series_id)
                    .select(economic_series::dataset_id)
                    .first::<Uuid>(conn)
                    .await
                    .optional()?;
                let pointer = if let Some(dataset_id) = dataset {
                    SqlCatalog::pointer_conn(conn, dataset_id)
                        .await
                        .map_err(|e| AppError::DatabaseError(e.to_string()))?
                } else {
                    None
                };
                Ok((dataset, pointer))
            })
            .await?;
        drop(conn);
        let Some(dataset_id) = dataset else {
            return Ok(ObservationPage {
                series_id: request.series_id,
                dataset_id: None,
                points: vec![],
                context: vec![],
                total_count: 0,
                offset: request.offset.unwrap_or(0).max(0),
            });
        };
        let pointer = pointer.ok_or_else(|| {
            AppError::DatabaseError(format!(
                "Iceberg dataset {dataset_id} not yet migrated; refusing silent storage fallback"
            ))
        })?;
        self.read_page_at(&pointer, request)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))
    }
}
/// Reference selection currently bounded to a dataset snapshot. Production scale approval and
/// query pushdown benchmarks remain release gates; no floating-point/Decimal128 casts occur.
pub fn select_page(
    rows: Vec<Observation>,
    dataset_id: Uuid,
    req: &ObservationRequest,
) -> ObservationPage {
    let rows: Vec<_> = rows
        .into_iter()
        .filter(|r| {
            r.series_id == req.series_id
                && req.start_date.is_none_or(|d| r.date >= d)
                && req.end_date.is_none_or(|d| r.date <= d)
        })
        .collect();
    let original = matches!(
        req.revisions,
        RevisionMode::Original | RevisionMode::OriginalLatest | RevisionMode::OriginalAsOf(_)
    );
    let as_of = match req.revisions {
        RevisionMode::AsOf(d) | RevisionMode::OriginalAsOf(d) => Some(d),
        _ => None,
    };
    let one = matches!(
        req.revisions,
        RevisionMode::Latest
            | RevisionMode::AsOf(_)
            | RevisionMode::OriginalLatest
            | RevisionMode::OriginalAsOf(_)
    );
    let real_original_dates: HashSet<_> = rows
        .iter()
        .filter(|p| p.is_original_release && p.revision_date != p.date)
        .map(|p| p.date)
        .collect();
    let mut candidates: Vec<_> = rows
        .iter()
        .filter(|p| {
            (!original || p.is_original_release)
                && as_of.is_none_or(|d| p.revision_date <= d)
                && (req.revisions == RevisionMode::All
                    || !(p.is_original_release
                        && p.revision_date == p.date
                        && real_original_dates.contains(&p.date)))
        })
        .cloned()
        .collect();
    if one {
        let mut latest = BTreeMap::new();
        for p in candidates {
            let key = (p.revision_date, !p.is_original_release, p.id);
            let prev = latest.entry(p.date).or_insert_with(|| p.clone());
            if key > (prev.revision_date, !prev.is_original_release, prev.id) {
                *prev = p;
            }
        }
        candidates = latest.into_values().collect();
    }
    candidates.sort_by_key(|p| (p.date, p.revision_date, p.id));
    let total_count = candidates.len() as i64;
    let offset = req.offset.unwrap_or(0).max(0);
    let points: Vec<_> = candidates
        .iter()
        .skip(offset as usize)
        .take(req.limit.unwrap_or(10000).clamp(0, 10000) as usize)
        .cloned()
        .collect();
    let context = if let (Some(context), Some(first)) = (req.context, points.as_slice().first()) {
        let earlier = candidates.iter().take(offset as usize);
        match context {
            PageContext::PreviousPoint => earlier.last().cloned().into_iter().collect(),
            PageContext::FirstPoint => earlier
                .filter(|r| r.value.as_ref().is_some_and(|v| !v.is_zero()))
                .take(1)
                .cloned()
                .collect(),
            PageContext::Days(n) => {
                let from = chrono::Duration::try_days(n)
                    .and_then(|d| first.date.checked_sub_signed(d))
                    .unwrap_or(chrono::NaiveDate::MIN);
                earlier.filter(|p| p.date >= from).cloned().collect()
            }
        }
    } else {
        vec![]
    };
    ObservationPage {
        series_id: req.series_id,
        dataset_id: Some(dataset_id),
        points,
        context,
        total_count,
        offset,
    }
}
