//! Conservative publication state, separate from whether a history fetch succeeded.
//!
//! Only complete catalogs under unchanged selection rules count as absence evidence.
//! Three successful absences are required; losing more than 20% of a tracked scope
//! rejects the snapshot. Rows, observations and old catalog labels are never deleted.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::sql_types::{Jsonb, Nullable, Text, Timestamptz, Uuid as SqlUuid};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use econ_graph_core::{
    error::{AppError, AppResult},
    DatabasePool,
};
use econ_graph_metrics::crawler::CRAWLER_CATALOG_METRICS;
use serde::Serialize;
use uuid::Uuid;

use crate::{persist, DiscoveredSeries, SourceId};

/// Verified upstream facts, published only after all pages of a catalog succeeded.
#[derive(Debug, Clone, Default)]
pub struct CatalogLifecycle {
    /// Entries that the upstream forbids us from exposing publicly.
    pub restricted_ids: Vec<String>,
    /// Entries explicitly described as discontinued, whose history stays public.
    pub discontinued_ids: Vec<String>,
    /// Explicitly configured entries whose upstream lookup confirmed NotFound.
    pub confirmed_missing_ids: Vec<String>,
}

/// Durable evidence requirements, independent of worker replica/process lifetime.
#[derive(Debug, Clone, Copy)]
pub struct LifecyclePolicy {
    pub consecutive_absences: u32,
    pub max_missing_percent: u32,
}

impl Default for LifecyclePolicy {
    fn default() -> Self {
        Self {
            consecutive_absences: 3,
            max_missing_percent: 20,
        }
    }
}

/// Result of one complete catalog reconciliation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reconciled {
    pub guarded: bool,
    pub transitions: usize,
    pub(crate) transition_statuses: Vec<String>,
}

#[derive(Debug, Clone, QueryableByName, Serialize)]
struct State {
    #[diesel(sql_type = Text)]
    external_id: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    since: Option<DateTime<Utc>>,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    missing_refreshes: i32,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    last_seen_at: Option<DateTime<Utc>>,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    proposed_successor_id: Option<Uuid>,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    successor_id: Option<Uuid>,
}

#[derive(QueryableByName)]
struct Scope {
    #[diesel(sql_type = Text)]
    fingerprint: String,
    #[diesel(sql_type = Timestamptz)]
    last_refresh_at: DateTime<Utc>,
}

/// Apply a complete successful snapshot. The caller's transaction includes discovered
/// metadata and validators; a failed transaction records no evidence. The source row
/// serializes replica commits, and older snapshots cannot reverse a newer snapshot.
pub(crate) async fn reconcile_conn(
    conn: &mut AsyncPgConnection,
    source: SourceId,
    found: &[DiscoveredSeries],
    facts: &CatalogLifecycle,
    fingerprint: &str,
    observed_at: DateTime<Utc>,
) -> AppResult<Reconciled> {
    let source_id = persist::data_source_id_conn(conn, source).await?;
    diesel::sql_query("SELECT id FROM data_sources WHERE id = $1 FOR UPDATE")
        .bind::<SqlUuid, _>(source_id)
        .execute(&mut *conn)
        .await?;
    let scope: Option<Scope> = diesel::sql_query(
        "SELECT fingerprint, last_refresh_at FROM catalog_lifecycle_scopes WHERE source_id=$1",
    )
    .bind::<SqlUuid, _>(source_id)
    .get_result(&mut *conn)
    .await
    .optional()?;
    if scope
        .as_ref()
        .is_some_and(|s| s.last_refresh_at >= observed_at)
    {
        return Ok(Reconciled::default());
    }
    let same_scope = scope.as_ref().is_some_and(|s| s.fingerprint == fingerprint);
    let existing: Vec<State> = diesel::sql_query(
        "SELECT external_id,status,since,missing_refreshes,last_seen_at,proposed_successor_id,successor_id \
         FROM series_lifecycle WHERE source_id=$1 AND (scope_fingerprint=$2 OR external_id=ANY($3))")
        .bind::<SqlUuid,_>(source_id).bind::<Text,_>(fingerprint)
        .bind::<diesel::sql_types::Array<Text>,_>(found.iter().map(|d| d.external_id.clone()).collect::<Vec<_>>())
        .load(&mut *conn).await?;
    let policy = LifecyclePolicy::default();
    let (mut states, guarded) = plan(&existing, found, facts, same_scope, observed_at, policy);
    if guarded {
        // Explicit legal restrictions are positive evidence, independent of an incomplete-
        // looking disappearance snapshot. Hide those ids while changing no absence state.
        let (positive, _) = plan(&existing, found, facts, false, observed_at, policy);
        states = positive
            .into_iter()
            .filter(|s| facts.restricted_ids.contains(&s.external_id))
            .collect();
        tracing::warn!(%source, "catalog mass-disappearance guard: absence evidence unchanged");
    }
    // Suggest only a unique exact-title/frequency match. It is a proposal, never an
    // automatic successor link or a merge of values (which may use a different base).
    for state in &mut states {
        if state.status == "missing_from_catalog" || state.status == "retired" {
            let candidates: Vec<Uuid> = diesel::sql_query(
                "SELECT DISTINCT COALESCE(es.id,sm.id) AS id FROM series_metadata old \
                 JOIN series_metadata sm ON sm.source_id=old.source_id \
                    AND lower(trim(sm.title))=lower(trim(old.title)) \
                    AND sm.frequency IS NOT DISTINCT FROM old.frequency \
                 LEFT JOIN economic_series es ON es.source_id=sm.source_id AND es.external_id=sm.external_id \
                 WHERE old.source_id=$1 AND old.external_id=$2 AND sm.external_id<>$2 \
                   AND sm.external_id = ANY($3) ORDER BY id LIMIT 2")
                .bind::<SqlUuid,_>(source_id).bind::<Text,_>(&state.external_id)
                .bind::<diesel::sql_types::Array<Text>,_>(found.iter().map(|d| d.external_id.clone()).collect::<Vec<_>>())
                .load::<Id>( &mut *conn).await?.into_iter().map(|r| r.id).collect();
            state.proposed_successor_id = (candidates.len() == 1).then(|| candidates[0]);
        }
    }
    let transitions = states
        .iter()
        .filter(|s| {
            existing
                .iter()
                .find(|old| old.external_id == s.external_id)
                .map(|old| old.status.as_str())
                .unwrap_or("active")
                != s.status
        })
        .count();
    write_states(conn, source_id, fingerprint, &states).await?;
    if !guarded {
        diesel::sql_query(
            "INSERT INTO catalog_lifecycle_scopes(source_id,fingerprint,last_refresh_at) \
                      VALUES($1,$2,$3) ON CONFLICT(source_id) DO UPDATE SET \
                      fingerprint=EXCLUDED.fingerprint,last_refresh_at=EXCLUDED.last_refresh_at",
        )
        .bind::<SqlUuid, _>(source_id)
        .bind::<Text, _>(fingerprint)
        .bind::<Timestamptz, _>(observed_at)
        .execute(&mut *conn)
        .await?;
    }
    let mut transition_statuses = Vec::new();
    for s in &states {
        if existing
            .iter()
            .find(|old| old.external_id == s.external_id)
            .map(|old| old.status.as_str())
            .unwrap_or("active")
            != s.status
        {
            transition_statuses.push(s.status.clone());
        }
    }
    Ok(Reconciled {
        guarded,
        transitions,
        transition_statuses,
    })
}

#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
}

fn plan(
    existing: &[State],
    found: &[DiscoveredSeries],
    facts: &CatalogLifecycle,
    same_scope: bool,
    now: DateTime<Utc>,
    policy: LifecyclePolicy,
) -> (Vec<State>, bool) {
    let listed: BTreeSet<&str> = found.iter().map(|d| d.external_id.as_str()).collect();
    // Compare the new snapshot with the previous successful one, not all retained
    // history. Previously absent series still accumulate evidence, but do not make
    // gradual normal retirements look like one mass disappearance forever.
    let previously_listed: Vec<&State> = existing
        .iter()
        .filter(|s| s.missing_refreshes == 0)
        .collect();
    let newly_missing = previously_listed
        .iter()
        .filter(|s| !listed.contains(s.external_id.as_str()))
        .count();
    if same_scope
        && (listed.is_empty()
            || (!previously_listed.is_empty()
                && newly_missing as u128 * 100
                    > previously_listed.len() as u128 * u128::from(policy.max_missing_percent)))
    {
        return (Vec::new(), true);
    }
    let restricted: BTreeSet<&str> = facts.restricted_ids.iter().map(String::as_str).collect();
    let discontinued: BTreeSet<&str> = facts.discontinued_ids.iter().map(String::as_str).collect();
    let confirmed: BTreeSet<&str> = facts
        .confirmed_missing_ids
        .iter()
        .map(String::as_str)
        .collect();
    let mut states: BTreeMap<String, State> = existing
        .iter()
        .cloned()
        .map(|s| (s.external_id.clone(), s))
        .collect();
    for id in listed
        .iter()
        .copied()
        .chain(restricted.iter().copied())
        .chain(confirmed.iter().copied())
    {
        states.entry(id.to_string()).or_insert_with(|| State {
            external_id: id.to_string(),
            status: "active".into(),
            since: None,
            missing_refreshes: 0,
            last_seen_at: None,
            proposed_successor_id: None,
            successor_id: None,
        });
    }
    for state in states.values_mut() {
        let old_status = state.status.clone();
        if !same_scope {
            state.missing_refreshes = 0;
        }
        let present = listed.contains(state.external_id.as_str());
        if restricted.contains(state.external_id.as_str()) || state.status == "restricted" {
            state.status = "restricted".into();
        } else if state.successor_id.is_some() {
            state.status = "superseded".into();
        } else if present {
            state.status = if discontinued.contains(state.external_id.as_str()) {
                "discontinued"
            } else {
                "active"
            }
            .into();
        } else if same_scope || confirmed.contains(state.external_id.as_str()) {
            state.missing_refreshes = state.missing_refreshes.saturating_add(1);
            if state.missing_refreshes
                >= i32::try_from(policy.consecutive_absences).unwrap_or(i32::MAX)
            {
                state.status = if confirmed.contains(state.external_id.as_str())
                    || state.status == "retired"
                {
                    "retired"
                } else {
                    "missing_from_catalog"
                }
                .into();
            }
        }
        if present {
            state.missing_refreshes = 0;
            state.last_seen_at = Some(now);
            state.proposed_successor_id = None;
        }
        if state.status == "active" {
            state.since = None;
        } else if state.status != old_status || state.since.is_none() {
            state.since = Some(now);
        }
    }
    (states.into_values().collect(), false)
}

async fn write_states(
    conn: &mut AsyncPgConnection,
    source_id: Uuid,
    fingerprint: &str,
    states: &[State],
) -> AppResult<()> {
    let payload =
        serde_json::to_value(states).map_err(|e| AppError::InternalError(e.to_string()))?;
    diesel::sql_query(
        "INSERT INTO series_lifecycle(source_id,external_id,status,since,missing_refreshes,last_seen_at, \
            proposed_successor_id,successor_id,scope_fingerprint) \
         SELECT $1,x.external_id,x.status,x.since,x.missing_refreshes,x.last_seen_at, \
            x.proposed_successor_id,x.successor_id,$2 \
         FROM jsonb_to_recordset($3) AS x(external_id text,status text,since timestamptz, \
            missing_refreshes integer,last_seen_at timestamptz,proposed_successor_id uuid,successor_id uuid) \
         ON CONFLICT(source_id,external_id) DO UPDATE SET status=EXCLUDED.status,since=EXCLUDED.since, \
            missing_refreshes=EXCLUDED.missing_refreshes,last_seen_at=EXCLUDED.last_seen_at, \
            proposed_successor_id=EXCLUDED.proposed_successor_id,successor_id=EXCLUDED.successor_id, \
            scope_fingerprint=EXCLUDED.scope_fingerprint")
        .bind::<SqlUuid,_>(source_id).bind::<Text,_>(fingerprint).bind::<Jsonb,_>(payload).execute(&mut *conn).await?;
    // A lifecycle badge must not remove historical data from normal series browsing.
    // Only a verified legal restriction makes a row inactive; the API additionally
    // checks restricted status so includeInactive cannot bypass that restriction.
    diesel::sql_query("UPDATE economic_series es SET \
        crawl_status=CASE WHEN l.status='active' THEN \
            CASE WHEN es.crawl_status IN ('discontinued','missing_from_catalog','retired','restricted','superseded') \
                 THEN 'success' ELSE es.crawl_status END ELSE l.status END, \
        is_active=(l.status<>'restricted') \
        FROM series_lifecycle l WHERE l.source_id=es.source_id AND l.external_id=es.external_id \
            AND l.source_id=$1 AND l.scope_fingerprint=$2")
        .bind::<SqlUuid,_>(source_id).bind::<Text,_>(fingerprint).execute(&mut *conn).await?;
    diesel::sql_query(
        "UPDATE series_metadata sm SET is_active=(l.status<>'restricted') \
        FROM series_lifecycle l WHERE l.source_id=sm.source_id AND l.external_id=sm.external_id \
            AND l.source_id=$1 AND l.scope_fingerprint=$2",
    )
    .bind::<SqlUuid, _>(source_id)
    .bind::<Text, _>(fingerprint)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn update_gauges(
    conn: &mut AsyncPgConnection,
    source: SourceId,
    source_id: Uuid,
) -> AppResult<()> {
    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type=Text)]
        status: String,
        #[diesel(sql_type=diesel::sql_types::BigInt)]
        n: i64,
    }
    let counts: Vec<Count> = diesel::sql_query(
        "SELECT status,count(*) AS n FROM series_lifecycle WHERE source_id=$1 GROUP BY status",
    )
    .bind::<SqlUuid, _>(source_id)
    .load(conn)
    .await?;
    for status in [
        "active",
        "discontinued",
        "missing_from_catalog",
        "retired",
        "restricted",
        "superseded",
    ] {
        CRAWLER_CATALOG_METRICS
            .series
            .with_label_values(&[source.as_str(), status])
            .set(
                counts
                    .iter()
                    .find(|c| c.status == status)
                    .map_or(0, |c| c.n),
            );
    }
    Ok(())
}

/// Refresh publication-state gauges after commit and periodically after worker restart.
pub async fn refresh_metrics(pool: &DatabasePool, sources: &[SourceId]) -> AppResult<()> {
    let mut conn = pool
        .get()
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    for &source in sources {
        let id: Option<Id> = diesel::sql_query("SELECT id FROM data_sources WHERE name=$1")
            .bind::<Text, _>(&persist::data_source_template(source).name)
            .get_result(&mut conn)
            .await
            .optional()?;
        if let Some(id) = id {
            update_gauges(&mut conn, source, id.id).await?;
        }
    }
    Ok(())
}

impl Reconciled {
    /// Call only after the containing result transaction committed.
    pub(crate) fn publish(&self, source: SourceId) {
        if self.guarded {
            CRAWLER_CATALOG_METRICS
                .guard_rejections_total
                .with_label_values(&[source.as_str(), "mass_disappearance"])
                .inc();
        }
        for status in &self.transition_statuses {
            CRAWLER_CATALOG_METRICS
                .transitions_total
                .with_label_values(&[source.as_str(), status])
                .inc();
        }
    }
}

/// Record confirmed NotFound as one durable absence, even when no observations were
/// ever fetched. Failed HTTP requests and parse errors must never call this function.
pub(crate) async fn record_not_found_conn(
    conn: &mut AsyncPgConnection,
    source: SourceId,
    external_id: &str,
) -> AppResult<Option<String>> {
    let source_id = persist::data_source_id_conn(conn, source).await?;
    diesel::sql_query("SELECT id FROM data_sources WHERE id=$1 FOR UPDATE")
        .bind::<SqlUuid, _>(source_id)
        .execute(&mut *conn)
        .await?;
    // Fetch-only evidence has its own scope: it can badge retained history that
    // predates catalog rules without expanding the catalog disappearance denominator.
    diesel::sql_query("INSERT INTO series_lifecycle(source_id,external_id,scope_fingerprint)
        SELECT $1,$2,'fetch-only' WHERE EXISTS(SELECT 1 FROM economic_series WHERE source_id=$1 AND external_id=$2)
            OR EXISTS(SELECT 1 FROM series_metadata WHERE source_id=$1 AND external_id=$2)
        ON CONFLICT(source_id,external_id) DO NOTHING")
        .bind::<SqlUuid,_>(source_id).bind::<Text,_>(external_id).execute(&mut *conn).await?;
    let change:Option<StatusChange> = diesel::sql_query("UPDATE series_lifecycle SET missing_refreshes=LEAST(missing_refreshes::bigint+1,2147483647)::integer, \
        status=CASE WHEN missing_refreshes>=2 AND status NOT IN ('restricted','superseded') THEN 'retired' ELSE status END, \
        since=CASE WHEN missing_refreshes>=2 AND status NOT IN ('retired','restricted','superseded') THEN NOW() ELSE since END \
        WHERE source_id=$1 AND external_id=$2 RETURNING CASE WHEN old.status<>status THEN status ELSE NULL END AS status")
        .bind::<SqlUuid,_>(source_id).bind::<Text,_>(external_id).get_result(&mut *conn).await.optional()?;
    diesel::sql_query(
        "UPDATE economic_series es SET crawl_status=l.status FROM series_lifecycle l \
        WHERE l.source_id=es.source_id AND l.external_id=es.external_id \
            AND l.source_id=$1 AND l.external_id=$2 AND l.status<>'active'",
    )
    .bind::<SqlUuid, _>(source_id)
    .bind::<Text, _>(external_id)
    .execute(&mut *conn)
    .await?;
    Ok(change.and_then(|c| c.status))
}

#[derive(QueryableByName)]
struct StatusChange {
    #[diesel(sql_type=Nullable<Text>)]
    status: Option<String>,
}

/// Hide an explicitly restricted fetch result under the worker's current lease.
/// Unlike an absence inference this positive legal fact does not need repeated catalogs.
pub(crate) async fn record_restricted_conn(
    conn: &mut AsyncPgConnection,
    source: SourceId,
    external_id: &str,
) -> AppResult<()> {
    let source_id = persist::data_source_id_conn(conn, source).await?;
    diesel::sql_query("SELECT id FROM data_sources WHERE id=$1 FOR UPDATE")
        .bind::<SqlUuid, _>(source_id)
        .execute(&mut *conn)
        .await?;
    diesel::sql_query("INSERT INTO series_lifecycle(source_id,external_id,status,since,scope_fingerprint)
        VALUES($1,$2,'restricted',NOW(),'fetch-only') ON CONFLICT(source_id,external_id) DO UPDATE SET
        status='restricted',since=CASE WHEN series_lifecycle.status='restricted' THEN series_lifecycle.since ELSE NOW() END")
        .bind::<SqlUuid,_>(source_id).bind::<Text,_>(external_id).execute(&mut *conn).await?;
    diesel::sql_query("UPDATE economic_series SET crawl_status='restricted',is_active=false WHERE source_id=$1 AND external_id=$2")
        .bind::<SqlUuid,_>(source_id).bind::<Text,_>(external_id).execute(&mut *conn).await?;
    diesel::sql_query(
        "UPDATE series_metadata SET is_active=false WHERE source_id=$1 AND external_id=$2",
    )
    .bind::<SqlUuid, _>(source_id)
    .bind::<Text, _>(external_id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Human confirmation of a proposed successor. Both ids must exist under the same
/// source, the old series must be missing/retired/discontinued, and the candidate must
/// have been proposed by a complete catalog. No observations or user references move.
pub async fn confirm_successor(
    pool: &DatabasePool,
    source: SourceId,
    external_id: &str,
    successor_external_id: &str,
) -> AppResult<()> {
    let mut conn = pool
        .get()
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    conn.transaction::<(),AppError,_>(async move |conn| {
        let source_id=persist::data_source_id_conn(conn,source).await?;
        diesel::sql_query("SELECT id FROM data_sources WHERE id=$1 FOR UPDATE").bind::<SqlUuid,_>(source_id).execute(&mut *conn).await?;
        let successor=persist::find_series_id_conn(conn,source,successor_external_id).await?
            .ok_or_else(||AppError::ValidationError("successor must have stored observations".into()))?;
        let target_blocked: Option<Id> = diesel::sql_query("SELECT $3::uuid AS id FROM series_lifecycle
            WHERE source_id=$1 AND external_id=$2 AND status IN ('restricted','superseded')")
            .bind::<SqlUuid,_>(source_id).bind::<Text,_>(successor_external_id).bind::<SqlUuid,_>(successor).get_result(&mut *conn).await.optional()?;
        if target_blocked.is_some() { return Err(AppError::ValidationError("successor must be public and cannot itself have a successor".into())); }
        let changed=diesel::sql_query("UPDATE series_lifecycle SET successor_id=$3,status='superseded',since=NOW() \
            WHERE source_id=$1 AND external_id=$2 AND proposed_successor_id=$3 \
                AND status IN ('missing_from_catalog','retired','discontinued') AND external_id<>$4")
            .bind::<SqlUuid,_>(source_id).bind::<Text,_>(external_id).bind::<SqlUuid,_>(successor)
            .bind::<Text,_>(successor_external_id).execute(&mut *conn).await?;
        if changed!=1 { return Err(AppError::ValidationError("no matching unconfirmed successor proposal for this missing series".into())); }
        diesel::sql_query("UPDATE economic_series SET crawl_status='superseded' WHERE source_id=$1 AND external_id=$2")
            .bind::<SqlUuid,_>(source_id).bind::<Text,_>(external_id).execute(&mut *conn).await?;
        Ok(())
    }).await?;
    CRAWLER_CATALOG_METRICS
        .transitions_total
        .with_label_values(&[source.as_str(), "superseded"])
        .inc();
    refresh_metrics(pool, &[source]).await
}

#[cfg(test)]
mod tests;
