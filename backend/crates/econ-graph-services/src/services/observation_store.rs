//! Current-Postgres implementation of the storage-independent observation contract.
//! The read unit is one consistent page/count/context snapshot. Writes intentionally remain
//! in persist_series_conn: it borrows the worker transaction which owns its lease and queue
//! completion; a pool-based write method here would break that all-or-nothing boundary.

pub use super::series_service::MAX_SERIES_DATA_PAGE;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
pub use econ_graph_core::models::{
    Observation, ObservationPage, ObservationRequest, PageContext, RevisionMode, TimeSeriesStore,
};
use econ_graph_core::{
    database::DatabasePool,
    error::{AppError, AppResult},
    models::{DataPoint, DataQueryParams},
    schema::{data_points, economic_series},
};

pub struct PostgresStore<'a> {
    pool: &'a DatabasePool,
}

impl<'a> PostgresStore<'a> {
    pub fn new(pool: &'a DatabasePool) -> Self {
        Self { pool }
    }
}

impl TimeSeriesStore for PostgresStore<'_> {
    async fn read_page(&self, request: ObservationRequest) -> AppResult<ObservationPage> {
        request.validate()?;
        let params = request.legacy_params();
        let context = request.context;
        let limit = params
            .limit
            .unwrap_or(MAX_SERIES_DATA_PAGE)
            .clamp(0, MAX_SERIES_DATA_PAGE);
        let offset = params.offset.unwrap_or(0).max(0);

        let mut conn = self.pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        // One snapshot, so a crawl writing in between can't make the count, the page and the
        // context disagree.
        conn.build_transaction()
            .repeatable_read()
            .read_only()
            .run(async |conn| {
                let total_count = filtered_data_points(&params)
                    .count()
                    .get_result::<i64>(conn)
                    .await?;

                let points = in_page_order(filtered_data_points(&params))
                    .limit(limit)
                    .offset(offset)
                    .load::<DataPoint>(conn)
                    .await?;

                let context = match (context, points.as_slice().first()) {
                    (Some(context), Some(first)) if offset > 0 => {
                        load_context(conn, &params, first, context).await?
                    }
                    _ => Vec::new(),
                };

                let dataset_id = economic_series::table
                    .find(params.series_id)
                    .select(economic_series::dataset_id)
                    .first::<uuid::Uuid>(conn)
                    .await
                    .optional()?;
                Ok(ObservationPage {
                    series_id: params.series_id,
                    dataset_id,
                    points: points.into_iter().map(Observation::from).collect(),
                    context: context.into_iter().map(Observation::from).collect(),
                    total_count,
                    offset,
                })
            })
            .await
    }
}

/// Points before `first` in page order, selected by `context`, in page order.
async fn load_context(
    conn: &mut diesel_async::AsyncPgConnection,
    params: &DataQueryParams,
    first: &DataPoint,
    context: PageContext,
) -> Result<Vec<DataPoint>, diesel::result::Error> {
    use data_points::dsl::{date, id, revision_date, value};

    // (date, revision_date, id) < first's, spelled out for Diesel.
    let before_first = date.lt(first.date).or(date.eq(first.date).and(
        revision_date
            .lt(first.revision_date)
            .or(revision_date.eq(first.revision_date).and(id.lt(first.id))),
    ));
    let earlier = filtered_data_points(params).filter(before_first);

    match context {
        PageContext::Days(days) => {
            let from = chrono::Duration::try_days(days)
                .and_then(|duration| first.date.checked_sub_signed(duration))
                .unwrap_or(chrono::NaiveDate::MIN);
            in_page_order(earlier.filter(date.ge(from)))
                .load::<DataPoint>(conn)
                .await
        }
        PageContext::PreviousPoint => {
            earlier
                .order_by((date.desc(), revision_date.desc(), id.desc()))
                .limit(1)
                .load::<DataPoint>(conn)
                .await
        }
        PageContext::FirstPoint => {
            // The percent-change base is the series' earliest *usable* value: a null or zero
            // reading at the very first date (or several) is skipped. `.ne` on a nullable column
            // already excludes NULLs under SQL's three-valued logic, so this one filter covers
            // both.
            in_page_order(earlier.filter(value.ne(bigdecimal::BigDecimal::from(0))))
                .limit(1)
                .load::<DataPoint>(conn)
                .await
        }
    }
}

/// Data points of `params.series_id` matching its date and revision filters, unordered and
/// unpaged. The count, the page and the context all start here so they agree.
fn filtered_data_points(
    params: &DataQueryParams,
) -> data_points::BoxedQuery<'static, diesel::pg::Pg> {
    let mut query = data_points::table
        .filter(data_points::series_id.eq(params.series_id))
        .into_boxed();

    if let Some(start_date) = params.start_date {
        query = query.filter(data_points::date.ge(start_date));
    }
    if let Some(end_date) = params.end_date {
        query = query.filter(data_points::date.le(end_date));
    }

    let original_only = params.original_only.unwrap_or(false);
    let as_of_or_latest = params.as_of.is_some() || params.latest_revision_only.unwrap_or(false);
    if original_only || as_of_or_latest {
        query = query.filter(econ_graph_core::models::exclude_synthetic_legacy_rows());
    }
    if original_only {
        query = query.filter(data_points::is_original_release.eq(true));
    }
    if as_of_or_latest {
        query = query.filter(econ_graph_core::models::revision_filter(
            params.as_of,
            original_only,
        ));
    }

    query
}

/// Page order: date, then revision date, then id, so pages are stable when a date has several
/// revisions.
fn in_page_order(
    query: data_points::BoxedQuery<'static, diesel::pg::Pg>,
) -> data_points::BoxedQuery<'static, diesel::pg::Pg> {
    query.order_by((
        data_points::date.asc(),
        data_points::revision_date.asc(),
        data_points::id.asc(),
    ))
}
