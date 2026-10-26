//! Public lifecycle metadata. These loaders batch a page of series without caching across
//! requests, so a confirmed successor or a catalog refresh is visible immediately.

use std::collections::HashMap;

use async_graphql::{Context, Result, SimpleObject, ID};
use chrono::{DateTime, Utc};
use dataloader::BatchFn;
use diesel::{OptionalExtension, QueryableByName};
use diesel_async::RunQueryDsl;
use econ_graph_core::database::DatabasePool;
use uuid::Uuid;

/// Publication state is independent of the last crawl's success or failure.
#[derive(SimpleObject, Clone, Debug, Default)]
#[graphql(name = "SeriesLifecycle")]
pub struct SeriesLifecycleType {
    /// active, discontinued, missing_from_catalog, retired or superseded.
    pub status: String,
    /// When this publication state was confirmed, not the last observation date.
    pub since: Option<DateTime<Utc>>,
    /// A human-confirmed successor. Histories remain separate.
    pub successor_id: Option<ID>,
    /// A possible successor that has not been confirmed; never presented as a replacement.
    pub proposed_successor_id: Option<ID>,
}

#[derive(QueryableByName)]
struct LifecycleRow {
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    id: Uuid,
    #[diesel(sql_type = diesel::sql_types::Text)]
    status: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Timestamptz>)]
    since: Option<DateTime<Utc>>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Uuid>)]
    successor_id: Option<Uuid>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Uuid>)]
    proposed_successor_id: Option<Uuid>,
}

pub struct SeriesLifecycleBatcher {
    pub pool: DatabasePool,
}

impl BatchFn<Uuid, Option<SeriesLifecycleType>> for SeriesLifecycleBatcher {
    fn load(
        &mut self,
        keys: &[Uuid],
    ) -> impl std::future::Future<Output = HashMap<Uuid, Option<SeriesLifecycleType>>> {
        let pool = self.pool.clone();
        let keys = keys.to_vec();
        async move {
            let query = async {
                let mut conn = pool.get().await?;
                let rows = diesel::sql_query(
                    "SELECT es.id, COALESCE(l.status, 'active') AS status, l.since, \
                       CASE WHEN successor_public.id IS NOT NULL THEN l.successor_id END AS successor_id, \
                       CASE WHEN proposed_public.id IS NOT NULL THEN l.proposed_successor_id END AS proposed_successor_id \
                     FROM economic_series es \
                     LEFT JOIN series_lifecycle l ON l.source_id=es.source_id AND l.external_id=es.external_id \
                     LEFT JOIN economic_series successor_public ON successor_public.id=l.successor_id AND successor_public.end_date IS NOT NULL \
                       AND NOT EXISTS (SELECT 1 FROM series_lifecycle sl WHERE sl.source_id=successor_public.source_id \
                         AND sl.external_id=successor_public.external_id AND sl.status='restricted') \
                     LEFT JOIN economic_series proposed_public ON proposed_public.id=l.proposed_successor_id AND proposed_public.end_date IS NOT NULL \
                       AND NOT EXISTS (SELECT 1 FROM series_lifecycle pl WHERE pl.source_id=proposed_public.source_id \
                         AND pl.external_id=proposed_public.external_id AND pl.status='restricted') \
                     WHERE es.id=ANY($1) AND COALESCE(l.status, 'active') <> 'restricted'",
                )
                .bind::<diesel::sql_types::Array<diesel::sql_types::Uuid>, _>(&keys)
                .load::<LifecycleRow>(&mut conn)
                .await?;
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(rows)
            }
            .await;
            match query {
                Ok(rows) => {
                    let mut result: HashMap<_, _> = keys.into_iter().map(|id| (id, None)).collect();
                    for row in rows {
                        result.insert(
                            row.id,
                            Some(SeriesLifecycleType {
                                status: row.status,
                                since: row.since,
                                successor_id: row.successor_id.map(|id| ID(id.to_string())),
                                proposed_successor_id: row
                                    .proposed_successor_id
                                    .map(|id| ID(id.to_string())),
                            }),
                        );
                    }
                    result
                }
                Err(error) => {
                    tracing::error!(%error, "Unable to load series lifecycle");
                    HashMap::new()
                }
            }
        }
    }
}

pub async fn load_series_lifecycle(
    ctx: &Context<'_>,
    id: &ID,
) -> Result<Option<SeriesLifecycleType>> {
    let loaders = crate::graphql::context::data_loaders(ctx)?;
    loaders
        .series_lifecycle_loader
        .try_load(Uuid::parse_str(id)?)
        .await
        .map_err(|_| async_graphql::Error::new("failed to load series lifecycle"))
}

/// Current retained-series counts. Restricted rows are excluded from this public summary.
#[derive(SimpleObject, QueryableByName, Clone, Debug)]
#[graphql(name = "SourceLifecycleCounts")]
pub struct SourceLifecycleCountsType {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub discontinued: i32,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub missing_from_catalog: i32,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub retired: i32,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub superseded: i32,
}

pub async fn load_source_counts(
    ctx: &Context<'_>,
    source_id: &ID,
) -> Result<SourceLifecycleCountsType> {
    let pool = crate::graphql::context::data_pool(ctx)?;
    let mut conn = pool.get().await?;
    Ok(diesel::sql_query(
        "SELECT count(*) FILTER (WHERE l.status='discontinued')::integer AS discontinued, \
                count(*) FILTER (WHERE l.status='missing_from_catalog')::integer AS missing_from_catalog, \
                count(*) FILTER (WHERE l.status='retired')::integer AS retired, \
                count(*) FILTER (WHERE l.status='superseded')::integer AS superseded \
         FROM series_lifecycle l INNER JOIN economic_series es \
           ON es.source_id=l.source_id AND es.external_id=l.external_id \
         WHERE l.source_id=$1 AND es.end_date IS NOT NULL",
    )
    .bind::<diesel::sql_types::Uuid, _>(Uuid::parse_str(source_id)?)
    .get_result::<SourceLifecycleCountsType>(&mut conn)
    .await
    .optional()?
    .expect("an aggregate always returns one row"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::schema::create_schema;
    use econ_graph_core::database::{AppPool, DataPool};
    use econ_graph_core::models::{DataSource, EconomicSeries, NewDataSource, NewEconomicSeries};
    use serde_json::{json, Value};

    #[tokio::test]
    async fn lifecycle_is_live_and_never_exposes_restricted_successors() {
        let url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL names the isolated test server");
        crate::graphql::test_db::migrate_once(&url).await;
        let pool = econ_graph_core::database::create_pool(&url).await.unwrap();
        let source = DataSource::create(
            &pool,
            NewDataSource {
                name: format!("lifecycle-api-{}", Uuid::new_v4()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let dataset_id =
            econ_graph_core::test_utils::test_dataset_id(&mut pool.get().await.unwrap(), source.id)
                .await;
        let mut series = Vec::new();
        for external_id in ["OLD", "NEW", "RESTRICTED", "ACTIVE", "EMPTY"] {
            series.push(
                EconomicSeries::create(
                    &pool,
                    &NewEconomicSeries {
                        source_id: source.id,
                        dataset_id,
                        external_id: external_id.into(),
                        title: format!("Lifecycle {external_id}"),
                        end_date: (external_id != "EMPTY")
                            .then(|| chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap(),
            );
        }
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query("INSERT INTO series_lifecycle (source_id,external_id,status,since,proposed_successor_id,scope_fingerprint) VALUES ($1,'OLD','missing_from_catalog','2026-10-25T00:00:00Z',$2,'test'),($1,'RESTRICTED','restricted','2026-10-25T00:00:00Z',NULL,'test')")
            .bind::<diesel::sql_types::Uuid,_>(source.id)
            .bind::<diesel::sql_types::Uuid,_>(series[2].id)
            .execute(&mut conn).await.unwrap();
        drop(conn);
        let schema = create_schema(AppPool(pool.clone()), Some(DataPool(pool.clone())));
        let query = format!("{{ old: series(id: {}) {{ lifecycle {{ status since successorId proposedSuccessorId }} source {{ lifecycleCounts {{ discontinued missingFromCatalog retired superseded }} }} }} active: series(id: {}) {{ lifecycle {{ status since }} }} }}", json!(series[0].id), json!(series[3].id));
        let response = async_graphql::Schema::execute(&schema, query.clone()).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let result = response.data.into_json().unwrap();
        assert_eq!(result["old"]["lifecycle"]["status"], "missing_from_catalog");
        assert_eq!(
            result["old"]["lifecycle"]["since"],
            "2026-10-25T00:00:00+00:00"
        );
        assert_eq!(
            result["old"]["lifecycle"]["proposedSuccessorId"],
            Value::Null
        );
        assert_eq!(
            result["old"]["source"]["lifecycleCounts"],
            json!({"discontinued":0,"missingFromCatalog":1,"retired":0,"superseded":0})
        );
        assert_eq!(
            result["active"]["lifecycle"],
            json!({"status":"active","since":null})
        );

        // Metadata-only candidates cannot be opened through the public series query yet.
        // Do not offer a link that would resolve to "series not found".
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query("UPDATE series_lifecycle SET proposed_successor_id=$2 WHERE source_id=$1 AND external_id='OLD'")
            .bind::<diesel::sql_types::Uuid,_>(source.id)
            .bind::<diesel::sql_types::Uuid,_>(series[4].id)
            .execute(&mut conn).await.unwrap();
        drop(conn);
        let response = async_graphql::Schema::execute(&schema, query.clone()).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let result = response.data.into_json().unwrap();
        assert_eq!(
            result["old"]["lifecycle"]["proposedSuccessorId"],
            Value::Null
        );

        // A long-lived schema must not cache a candidate or a confirmation across requests.
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query("UPDATE series_lifecycle SET status='superseded', successor_id=$2, proposed_successor_id=$2 WHERE source_id=$1 AND external_id='OLD'")
            .bind::<diesel::sql_types::Uuid,_>(source.id)
            .bind::<diesel::sql_types::Uuid,_>(series[1].id)
            .execute(&mut conn).await.unwrap();
        drop(conn);
        let response = async_graphql::Schema::execute(&schema, query).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let result = response.data.into_json().unwrap();
        assert_eq!(result["old"]["lifecycle"]["status"], "superseded");
        assert_eq!(
            result["old"]["lifecycle"]["successorId"],
            series[1].id.to_string()
        );
        assert_eq!(result["old"]["source"]["lifecycleCounts"]["superseded"], 1);
        assert_eq!(
            result["old"]["source"]["lifecycleCounts"]["missingFromCatalog"],
            0
        );
    }
}
