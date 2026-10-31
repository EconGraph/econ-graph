//! Conservative publication state, separate from whether a history fetch succeeded.
//!
//! Only complete catalogs under unchanged selection rules count as absence evidence.
//! Three successful absences are required. A snapshot that drops more than 20% of the
//! series listed by the last accepted catalog of the same scope is rejected and kept for
//! review; an operator can accept it with `crawler accept-guarded-catalog`. Rows,
//! observations and old catalog labels are never deleted.
//!
//! Locking: catalog reconciliation and successor confirmation take an exclusive
//! per-source transaction advisory lock. Per-series writers (fetch success, NotFound,
//! restriction) take that lock shared, then an exclusive per-series advisory lock, so
//! they run concurrently across series and serialize only on the same series. The worker
//! takes the shared source lock at the start of every fetch job's commit, for every source,
//! before any row write: a discovery holds the exclusive lock while it updates series rows, so
//! a fetch that wrote rows before locking could deadlock with it. Every writer takes the
//! source lock first and at most one series lock per transaction.
//!
//! Known limits, accepted by design:
//! - The worker reads a catalog's facts (`catalog_scope`, [`CatalogLifecycle`]) from the
//!   adapter after the walk returns, not with the walk's result. The queue's active-item index
//!   allows one active discovery job per source, so normally no other walk in the process
//!   replaces them in between. The exception is a job whose lease expired mid-walk: its own
//!   commit is rejected as a lost lease, but a walk it finishes late can replace the facts a
//!   newer job of the same source then reads. Both walks saw a complete upstream catalog.
//!   A [chunked discovery](crate::discovery_run) doesn't have this limit: its run stores the
//!   facts its enumeration and chunks returned, and its finalize reads them from the run.
//! - A normal fetch and a forced refetch of the same series may both answer NotFound and so
//!   record two consecutive misses for one upstream state. That can retire a series one
//!   attempt early; a later successful fetch reactivates it.
//! - `observed_at` is the worker's clock as the walk starts (for a chunked discovery, as its
//!   run starts). A worker whose clock runs behind can have its snapshot ignored as older than
//!   one already applied.
//! - A chunked discovery's run can take hours, and its snapshot is reconciled at finalize from
//!   chunks discovered over that whole time. A series listed by an early chunk counts as
//!   listed even if fetches later in the run retired it on consecutive NotFound results, so
//!   finalize can reactivate it wrongly. The reverse can't happen: a chunk only adds positive
//!   evidence, so no series is wrongly retired.

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
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{persist, DiscoveredSeries, SourceId};

/// Verified upstream facts, published only after all pages of a catalog succeeded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogLifecycle {
    /// Existing retained ids positively matched by the current selection rules. These
    /// seed the first scope baseline even if already missing upstream. Unmatched legacy
    /// rows are never inferred absent merely because the adapter adopted new rules.
    pub scope_ids: Vec<String>,
    /// Entries that the upstream forbids us from exposing publicly.
    pub restricted_ids: Vec<String>,
    /// Entries explicitly described as discontinued, whose history stays public.
    pub discontinued_ids: Vec<String>,
    /// Explicitly configured entries whose upstream lookup confirmed NotFound.
    pub confirmed_missing_ids: Vec<String>,
    /// Ids the complete catalog walk listed upstream that the selection rules did not select,
    /// for example a FRED series retitled out of a `title_contains` rule. They are still
    /// published, so they never count as absent and keep being fetched.
    #[serde(default)]
    pub unselected_ids: Vec<String>,
    /// External-id prefixes of listed upstream tables the rules did not select (BEA
    /// `bea_nipa/T10101.`). Same meaning as `unselected_ids`, for sources whose catalog lists
    /// tables rather than series.
    #[serde(default)]
    pub unselected_prefixes: Vec<String>,
}

impl CatalogLifecycle {
    /// Ids, among `ids`, that upstream still lists outside the selection rules.
    fn unselected<'a>(
        &self,
        listed: &BTreeSet<&str>,
        ids: impl Iterator<Item = &'a str>,
    ) -> BTreeSet<&'a str> {
        let exact: BTreeSet<&str> = self.unselected_ids.iter().map(String::as_str).collect();
        ids.filter(|id| {
            !listed.contains(id)
                && (exact.contains(id)
                    || self
                        .unselected_prefixes
                        .iter()
                        .any(|p| id.starts_with(p.as_str())))
        })
        .collect()
    }
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
    /// Tracked series listed upstream but no longer selected by the rules, when this result
    /// came from a catalog snapshot.
    pub(crate) unselected_tracked: Option<usize>,
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
    /// Listed by the last accepted catalog of the current scope: the guard's denominator.
    #[diesel(sql_type = diesel::sql_types::Bool)]
    in_accepted_catalog: bool,
}

/// One complete catalog as reconciliation sees it. A guarded snapshot is stored in this
/// form so an operator can accept exactly what was reviewed.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snapshot {
    listed: Vec<String>,
    facts: CatalogLifecycle,
    fingerprint: String,
    observed_at: DateTime<Utc>,
}

/// Namespaces for the two-key form of the transaction advisory locks.
const SOURCE_LOCK_CLASS: i32 = 0x4C43_5352;
const SERIES_LOCK_CLASS: i32 = 0x4C43_5345;

/// Exclusive per-source lock for writers that read and rewrite a whole scope.
pub(crate) async fn lock_source_exclusive(
    conn: &mut AsyncPgConnection,
    source_id: Uuid,
) -> AppResult<()> {
    diesel::sql_query("SELECT pg_advisory_xact_lock($1, hashtext($2::text))")
        .bind::<diesel::sql_types::Integer, _>(SOURCE_LOCK_CLASS)
        .bind::<SqlUuid, _>(source_id)
        .execute(conn)
        .await?;
    Ok(())
}

/// Shared source lock: excludes only a reconciliation of the same source.
pub(crate) async fn lock_source_shared(
    conn: &mut AsyncPgConnection,
    source_id: Uuid,
) -> AppResult<()> {
    diesel::sql_query("SELECT pg_advisory_xact_lock_shared($1, hashtext($2::text))")
        .bind::<diesel::sql_types::Integer, _>(SOURCE_LOCK_CLASS)
        .bind::<SqlUuid, _>(source_id)
        .execute(conn)
        .await?;
    Ok(())
}

/// Shared source lock plus an exclusive lock on one series. Held until commit.
pub(crate) async fn lock_series(
    conn: &mut AsyncPgConnection,
    source_id: Uuid,
    external_id: &str,
) -> AppResult<()> {
    diesel::sql_query(
        "SELECT pg_advisory_xact_lock_shared($1, hashtext($3::text)), \
                pg_advisory_xact_lock($2, hashtext($3::text || '/' || $4))",
    )
    .bind::<diesel::sql_types::Integer, _>(SOURCE_LOCK_CLASS)
    .bind::<diesel::sql_types::Integer, _>(SERIES_LOCK_CLASS)
    .bind::<SqlUuid, _>(source_id)
    .bind::<Text, _>(external_id)
    .execute(conn)
    .await?;
    Ok(())
}

#[derive(QueryableByName)]
struct Scope {
    #[diesel(sql_type = Text)]
    fingerprint: String,
    #[diesel(sql_type = Timestamptz)]
    last_refresh_at: DateTime<Utc>,
}

/// Apply a complete successful snapshot. The caller's transaction includes discovered
/// metadata and validators; a failed transaction records no evidence. The source lock
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
    lock_source_exclusive(conn, source_id).await?;
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
    let snapshot = Snapshot {
        listed: found.iter().map(|d| d.external_id.clone()).collect(),
        facts: facts.clone(),
        fingerprint: fingerprint.to_string(),
        observed_at,
    };
    apply_snapshot(
        conn,
        source,
        source_id,
        &snapshot,
        same_scope,
        LifecyclePolicy::default(),
    )
    .await
}

async fn load_existing(
    conn: &mut AsyncPgConnection,
    source_id: Uuid,
    snapshot: &Snapshot,
) -> AppResult<Vec<State>> {
    let facts = &snapshot.facts;
    Ok(diesel::sql_query(
        "SELECT external_id,status,since,missing_refreshes,last_seen_at,proposed_successor_id,successor_id, \
            (in_accepted_catalog AND scope_fingerprint=$2) AS in_accepted_catalog \
         FROM series_lifecycle WHERE source_id=$1 AND (scope_fingerprint=$2 OR external_id=ANY($3))")
        .bind::<SqlUuid,_>(source_id).bind::<Text,_>(&snapshot.fingerprint)
        .bind::<diesel::sql_types::Array<Text>,_>(snapshot.listed.iter().cloned()
            .chain(facts.scope_ids.iter().cloned())
            .chain(facts.restricted_ids.iter().cloned())
            .chain(facts.discontinued_ids.iter().cloned())
            .chain(facts.confirmed_missing_ids.iter().cloned()).collect::<Vec<_>>())
        .load(&mut *conn).await?)
}

async fn apply_snapshot(
    conn: &mut AsyncPgConnection,
    source: SourceId,
    source_id: Uuid,
    snapshot: &Snapshot,
    same_scope: bool,
    policy: LifecyclePolicy,
) -> AppResult<Reconciled> {
    let facts = &snapshot.facts;
    let fingerprint = snapshot.fingerprint.as_str();
    let observed_at = snapshot.observed_at;
    let existing = load_existing(conn, source_id, snapshot).await?;
    let fetch_retired = fetch_retired_ids(conn, source_id, &existing).await?;
    let (mut states, guarded) = plan_listed(
        &existing,
        &snapshot.listed,
        facts,
        same_scope,
        observed_at,
        policy,
        &fetch_retired,
    );
    if guarded {
        // Explicit legal restrictions are positive evidence, independent of an incomplete-
        // looking disappearance snapshot. Hide those ids while changing no absence state.
        let (positive, _) = plan_listed(
            &existing,
            &snapshot.listed,
            facts,
            false,
            observed_at,
            policy,
            &fetch_retired,
        );
        states = positive
            .into_iter()
            .filter(|s| facts.restricted_ids.contains(&s.external_id))
            .collect();
        tracing::warn!(%source, "catalog mass-disappearance guard: absence evidence unchanged; \
            review and run `crawler accept-guarded-catalog` to accept this snapshot");
    }
    // Suggest only a unique exact-title/frequency match. It is a proposal, never an
    // automatic successor link or a merge of values (which may use a different base).
    let candidates_listed: Vec<String> = snapshot
        .listed
        .iter()
        .filter(|id| !facts.restricted_ids.contains(id))
        .cloned()
        .collect();
    let ended: Vec<String> = states
        .iter()
        .filter(|s| s.status == "missing_from_catalog" || s.status == "retired")
        .map(|s| s.external_id.clone())
        .collect();
    let proposals = successor_proposals(conn, source_id, &ended, &candidates_listed).await?;
    for state in &mut states {
        if state.status == "missing_from_catalog" || state.status == "retired" {
            state.proposed_successor_id = proposals.get(&state.external_id).copied();
        }
    }
    let transition_statuses: Vec<String> = states
        .iter()
        .filter(|s| {
            existing
                .iter()
                .find(|old| old.external_id == s.external_id)
                .map(|old| old.status.as_str())
                .unwrap_or("active")
                != s.status
        })
        .map(|s| s.status.clone())
        .collect();
    write_states(
        conn,
        source_id,
        fingerprint,
        &states,
        &snapshot.listed,
        !guarded,
    )
    .await?;
    let listed: BTreeSet<&str> = snapshot.listed.iter().map(String::as_str).collect();
    let unselected_tracked = facts
        .unselected(&listed, existing.iter().map(|s| s.external_id.as_str()))
        .len();
    if unselected_tracked > 0 {
        tracing::warn!(
            %source,
            series = unselected_tracked,
            "tracked series are still listed upstream but no longer match the selection rules; \
             they stay published and scheduled; review the catalog_selection rules"
        );
    }
    // A guarded snapshot is kept for review; any accepted one discards it. The refresh time
    // advances either way so an older snapshot cannot be applied after a newer one.
    let pending = if guarded {
        Some(serde_json::to_value(snapshot).map_err(|e| AppError::InternalError(e.to_string()))?)
    } else {
        None
    };
    diesel::sql_query(
        "INSERT INTO catalog_lifecycle_scopes(source_id,fingerprint,last_refresh_at,guarded_snapshot,guarded_at) \
         VALUES($1,$2,$3,$4,CASE WHEN $4 IS NULL THEN NULL ELSE $3 END) ON CONFLICT(source_id) DO UPDATE SET \
         fingerprint=EXCLUDED.fingerprint, \
         last_refresh_at=GREATEST(catalog_lifecycle_scopes.last_refresh_at,EXCLUDED.last_refresh_at), \
         guarded_snapshot=EXCLUDED.guarded_snapshot,guarded_at=EXCLUDED.guarded_at",
    )
    .bind::<SqlUuid, _>(source_id)
    .bind::<Text, _>(fingerprint)
    .bind::<Timestamptz, _>(observed_at)
    .bind::<Nullable<Jsonb>, _>(pending)
    .execute(&mut *conn)
    .await?;
    Ok(Reconciled {
        guarded,
        transitions: transition_statuses.len(),
        transition_statuses,
        unselected_tracked: Some(unselected_tracked),
    })
}

/// What accepting the stored guarded snapshot would change, for operator review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardedReview {
    pub guarded_at: DateTime<Utc>,
    pub listed: usize,
    pub previously_listed: usize,
    /// Listed by the last accepted catalog but absent from the guarded one.
    pub newly_missing: Vec<String>,
    /// Status changes applied; zero for a review without `accept`.
    pub transitions: usize,
}

/// Review, and with `accept` apply, the newest snapshot the mass-disappearance guard
/// rejected for `source`. `accept` is the guarded-at time the operator reviewed; a newer
/// guarded snapshot stored since then is refused rather than applied unreviewed. Accepting records one absence for each newly missing series,
/// exactly as an unguarded catalog would; three accepted absences are still required
/// before a series is marked missing. No rows or observations are deleted.
pub async fn review_guarded_catalog(
    pool: &DatabasePool,
    source: SourceId,
    accept: Option<DateTime<Utc>>,
) -> AppResult<GuardedReview> {
    #[derive(QueryableByName)]
    struct Pending {
        #[diesel(sql_type = Text)]
        fingerprint: String,
        #[diesel(sql_type = Nullable<Jsonb>)]
        guarded_snapshot: Option<serde_json::Value>,
        #[diesel(sql_type = Nullable<Timestamptz>)]
        guarded_at: Option<DateTime<Utc>>,
    }
    let mut conn = pool
        .get()
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    let (review, metrics) = conn
        .transaction::<_, AppError, _>(async move |conn| {
            let source_id = persist::data_source_id_conn(conn, source).await?;
            lock_source_exclusive(conn, source_id).await?;
            let pending: Option<Pending> = diesel::sql_query(
                "SELECT fingerprint,guarded_snapshot,guarded_at FROM catalog_lifecycle_scopes \
                 WHERE source_id=$1",
            )
            .bind::<SqlUuid, _>(source_id)
            .get_result(&mut *conn)
            .await
            .optional()?;
            let Some(Pending {
                fingerprint,
                guarded_snapshot: Some(raw),
                guarded_at: Some(guarded_at),
            }) = pending
            else {
                return Err(AppError::ValidationError(format!(
                    "{source} has no guarded catalog snapshot waiting for review"
                )));
            };
            let snapshot: Snapshot = serde_json::from_value(raw)
                .map_err(|e| AppError::InternalError(format!("stored guarded snapshot: {e}")))?;
            if snapshot.fingerprint != fingerprint {
                return Err(AppError::ValidationError(
                    "the selection rules changed after this snapshot was guarded; the next \
                     catalog sets a new baseline instead"
                        .into(),
                ));
            }
            if snapshot.listed.is_empty() {
                return Err(AppError::ValidationError(
                    "an empty catalog cannot be accepted".into(),
                ));
            }
            let existing = load_existing(conn, source_id, &snapshot).await?;
            let listed: BTreeSet<&str> = snapshot.listed.iter().map(String::as_str).collect();
            let previously: Vec<&State> =
                existing.iter().filter(|s| s.in_accepted_catalog).collect();
            let unselected = snapshot
                .facts
                .unselected(&listed, previously.iter().map(|s| s.external_id.as_str()));
            let mut review = GuardedReview {
                guarded_at,
                listed: listed.len(),
                previously_listed: previously.len(),
                newly_missing: previously
                    .iter()
                    .filter(|s| {
                        !listed.contains(s.external_id.as_str())
                            && !unselected.contains(s.external_id.as_str())
                    })
                    .map(|s| s.external_id.clone())
                    .collect(),
                transitions: 0,
            };
            let Some(reviewed_at) = accept else {
                return Ok((review, None));
            };
            if reviewed_at != guarded_at {
                return Err(AppError::ValidationError(format!(
                    "the waiting snapshot was guarded at {}, not {}; review it again",
                    guarded_at.to_rfc3339(),
                    reviewed_at.to_rfc3339()
                )));
            }
            let metrics = apply_snapshot(
                conn,
                source,
                source_id,
                &snapshot,
                true,
                LifecyclePolicy {
                    max_missing_percent: 100,
                    ..LifecyclePolicy::default()
                },
            )
            .await?;
            review.transitions = metrics.transitions;
            Ok((review, Some(metrics)))
        })
        .await?;
    drop(conn);
    if let Some(metrics) = metrics {
        metrics.publish(source);
        if let Err(error) = refresh_metrics(pool, &[source]).await {
            tracing::warn!(%source, %error, "publication metrics refresh failed after accepting a guarded catalog");
        }
    }
    Ok(review)
}

/// For each ended series, the one listed, public series with the same normalized title and
/// frequency, if exactly one exists. One set-based query: reconciliation holds the
/// exclusive source lock, so its cost must not grow with one query per ended series.
async fn successor_proposals(
    conn: &mut AsyncPgConnection,
    source_id: Uuid,
    ended: &[String],
    listed: &[String],
) -> AppResult<BTreeMap<String, Uuid>> {
    #[derive(QueryableByName)]
    struct Proposal {
        #[diesel(sql_type = Text)]
        external_id: String,
        #[diesel(sql_type = SqlUuid)]
        id: Uuid,
    }
    if ended.is_empty() || listed.is_empty() {
        return Ok(BTreeMap::new());
    }
    let rows: Vec<Proposal> = diesel::sql_query(
        "WITH old AS ( \
             SELECT external_id, lower(trim(title)) AS title, frequency FROM series_metadata \
             WHERE source_id=$1 AND external_id = ANY($2)), \
         listed AS ( \
             SELECT COALESCE(es.id, sm.id) AS id, sm.external_id, lower(trim(sm.title)) AS title, \
                 sm.frequency \
             FROM series_metadata sm \
             LEFT JOIN economic_series es ON es.source_id=sm.source_id AND es.external_id=sm.external_id \
             WHERE sm.source_id=$1 AND sm.external_id = ANY($3) \
               AND NOT EXISTS (SELECT 1 FROM series_lifecycle candidate \
                   WHERE candidate.source_id=sm.source_id AND candidate.external_id=sm.external_id \
                     AND candidate.status IN ('restricted','superseded'))) \
         SELECT old.external_id, (array_agg(DISTINCT listed.id))[1] AS id \
         FROM old JOIN listed ON listed.title=old.title \
             AND listed.frequency IS NOT DISTINCT FROM old.frequency \
             AND listed.external_id<>old.external_id \
         GROUP BY old.external_id HAVING count(DISTINCT listed.id)=1",
    )
    .bind::<SqlUuid, _>(source_id)
    .bind::<diesel::sql_types::Array<Text>, _>(ended)
    .bind::<diesel::sql_types::Array<Text>, _>(listed)
    .load(conn)
    .await?;
    Ok(rows.into_iter().map(|r| (r.external_id, r.id)).collect())
}

#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
}

#[cfg(test)]
fn plan(
    existing: &[State],
    found: &[DiscoveredSeries],
    facts: &CatalogLifecycle,
    same_scope: bool,
    now: DateTime<Utc>,
    policy: LifecyclePolicy,
) -> (Vec<State>, bool) {
    let listed: Vec<String> = found.iter().map(|d| d.external_id.clone()).collect();
    plan_listed(
        existing,
        &listed,
        facts,
        same_scope,
        now,
        policy,
        &BTreeSet::new(),
    )
}

/// Ids among `existing` currently retired by fetch evidence alone.
async fn fetch_retired_ids(
    conn: &mut AsyncPgConnection,
    source_id: Uuid,
    existing: &[State],
) -> AppResult<BTreeSet<String>> {
    #[derive(QueryableByName)]
    struct Row {
        #[diesel(sql_type = Text)]
        external_id: String,
    }
    let ids: Vec<String> = existing.iter().map(|s| s.external_id.clone()).collect();
    let rows: Vec<Row> = diesel::sql_query(
        "SELECT external_id FROM series_lifecycle \
         WHERE source_id=$1 AND fetch_retired AND external_id = ANY($2)",
    )
    .bind::<SqlUuid, _>(source_id)
    .bind::<diesel::sql_types::Array<Text>, _>(ids)
    .load(conn)
    .await?;
    Ok(rows.into_iter().map(|r| r.external_id).collect())
}

fn plan_listed(
    existing: &[State],
    listed: &[String],
    facts: &CatalogLifecycle,
    same_scope: bool,
    now: DateTime<Utc>,
    policy: LifecyclePolicy,
    fetch_retired: &BTreeSet<String>,
) -> (Vec<State>, bool) {
    let listed: BTreeSet<&str> = listed.iter().map(String::as_str).collect();
    // Compare the new snapshot with the last accepted one, not all retained history or
    // scope members that were already absent when the rules baseline was taken. Those
    // still accumulate evidence, but neither they nor gradual normal retirements can
    // make every later snapshot look like a mass disappearance.
    let previously_listed: Vec<&State> =
        existing.iter().filter(|s| s.in_accepted_catalog).collect();
    // Listed upstream but outside the rules: still published, so never absent.
    let unselected = facts.unselected(&listed, existing.iter().map(|s| s.external_id.as_str()));
    let newly_missing = previously_listed
        .iter()
        .filter(|s| {
            !listed.contains(s.external_id.as_str()) && !unselected.contains(s.external_id.as_str())
        })
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
    let previously_tracked: BTreeSet<&str> =
        existing.iter().map(|s| s.external_id.as_str()).collect();
    let mut states: BTreeMap<String, State> = existing
        .iter()
        .cloned()
        .map(|s| (s.external_id.clone(), s))
        .collect();
    for id in listed
        .iter()
        .copied()
        .chain(facts.scope_ids.iter().map(String::as_str))
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
            in_accepted_catalog: false,
        });
    }
    for state in states.values_mut() {
        let old_status = state.status.clone();
        if !same_scope {
            state.missing_refreshes = 0;
        }
        let present = listed.contains(state.external_id.as_str());
        let drifted = unselected.contains(state.external_id.as_str());
        state.in_accepted_catalog = present || drifted;
        if restricted.contains(state.external_id.as_str()) || state.status == "restricted" {
            state.status = "restricted".into();
        } else if state.successor_id.is_some() {
            state.status = "superseded".into();
        } else if drifted && !present && fetch_retired.contains(&state.external_id) {
            // Still published outside the rules, but retired by fetch evidence: only an
            // outright listing or a successful fetch may reactivate it.
        } else if present || drifted {
            state.status = if discontinued.contains(state.external_id.as_str())
                || (drifted && state.status == "discontinued")
            {
                "discontinued"
            } else {
                "active"
            }
            .into();
        } else if (same_scope && previously_tracked.contains(state.external_id.as_str()))
            || confirmed.contains(state.external_id.as_str())
        {
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
        if present || drifted {
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
    listed: &[String],
    accepted: bool,
) -> AppResult<()> {
    let payload =
        serde_json::to_value(states).map_err(|e| AppError::InternalError(e.to_string()))?;
    // Only an accepted snapshot moves the guard's baseline; a guarded one writes positive
    // legal evidence alone and leaves in_accepted_catalog as the last accepted catalog set it.
    // Only an outright listing or a successful fetch clears fetch evidence; so does any
    // return to active from another state, so stale counters cannot retire it at once.
    diesel::sql_query(format!(
        "INSERT INTO series_lifecycle(source_id,external_id,status,since,missing_refreshes,last_seen_at, \
            proposed_successor_id,successor_id,scope_fingerprint,in_accepted_catalog, \
            listed_in_accepted_catalog) \
         SELECT $1,x.external_id,x.status,x.since,x.missing_refreshes,x.last_seen_at, \
            x.proposed_successor_id,x.successor_id,$2,x.in_accepted_catalog AND $4, \
            x.external_id = ANY($5) AND $4 \
         FROM jsonb_to_recordset($3) AS x(external_id text,status text,since timestamptz, \
            missing_refreshes integer,last_seen_at timestamptz,proposed_successor_id uuid,successor_id uuid, \
            in_accepted_catalog boolean) \
         ON CONFLICT(source_id,external_id) DO UPDATE SET status=EXCLUDED.status,since=EXCLUDED.since, \
            in_accepted_catalog=CASE WHEN $4 THEN EXCLUDED.in_accepted_catalog \
                ELSE series_lifecycle.in_accepted_catalog END, \
            listed_in_accepted_catalog=CASE WHEN $4 THEN EXCLUDED.listed_in_accepted_catalog \
                ELSE series_lifecycle.listed_in_accepted_catalog END, \
            missing_refreshes=EXCLUDED.missing_refreshes,last_seen_at=EXCLUDED.last_seen_at, \
            proposed_successor_id=EXCLUDED.proposed_successor_id,successor_id=EXCLUDED.successor_id, \
            scope_fingerprint=EXCLUDED.scope_fingerprint, \
            fetch_retired=CASE WHEN EXCLUDED.status='retired' AND EXCLUDED.missing_refreshes<3 \
                THEN series_lifecycle.fetch_retired ELSE FALSE END, \
            fetch_not_found_count=CASE WHEN {reset} THEN 0 \
                ELSE series_lifecycle.fetch_not_found_count END, \
            fetch_absent_count=CASE WHEN {reset} THEN 0 \
                ELSE series_lifecycle.fetch_absent_count END",
        reset = "(EXCLUDED.external_id = ANY($5) AND $4) \
            OR (series_lifecycle.status<>'active' AND EXCLUDED.status='active')"
    ))
        .bind::<SqlUuid,_>(source_id).bind::<Text,_>(fingerprint).bind::<Jsonb,_>(payload)
        .bind::<diesel::sql_types::Bool,_>(accepted)
        .bind::<diesel::sql_types::Array<Text>,_>(listed).execute(&mut *conn).await?;
    // A lifecycle badge must not remove historical data from normal series browsing.
    // Only a verified legal restriction makes a row inactive; the API additionally
    // checks restricted status so includeInactive cannot bypass that restriction. A row
    // that was already inactive (for example deactivated by an earlier unlisted-series
    // sweep) stays inactive, and unscheduled, until a catalog lists it outright.
    diesel::sql_query("UPDATE economic_series es SET \
        crawl_status=CASE WHEN l.status='active' THEN \
            CASE WHEN es.crawl_status IN ('discontinued','missing_from_catalog','retired','restricted','superseded') \
                 THEN 'success' ELSE es.crawl_status END ELSE l.status END, \
        is_active=CASE WHEN l.status='restricted' THEN FALSE \
            WHEN l.listed_in_accepted_catalog THEN TRUE ELSE es.is_active END \
        FROM series_lifecycle l WHERE l.source_id=es.source_id AND l.external_id=es.external_id \
            AND l.source_id=$1 AND l.scope_fingerprint=$2")
        .bind::<SqlUuid,_>(source_id).bind::<Text,_>(fingerprint).execute(&mut *conn).await?;
    diesel::sql_query(
        "UPDATE series_metadata sm SET is_active=CASE WHEN l.status='restricted' THEN FALSE \
            WHEN l.listed_in_accepted_catalog THEN TRUE ELSE sm.is_active END \
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
    .load(&mut *conn)
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
    #[derive(QueryableByName)]
    struct Pending {
        #[diesel(sql_type = diesel::sql_types::Bool)]
        pending: bool,
    }
    let pending: Option<Pending> = diesel::sql_query(
        "SELECT guarded_at IS NOT NULL AS pending FROM catalog_lifecycle_scopes WHERE source_id=$1",
    )
    .bind::<SqlUuid, _>(source_id)
    .get_result(&mut *conn)
    .await
    .optional()?;
    CRAWLER_CATALOG_METRICS
        .guarded_snapshot_pending
        .with_label_values(&[source.as_str()])
        .set(i64::from(pending.is_some_and(|p| p.pending)));
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
        if let Some(n) = self.unselected_tracked {
            CRAWLER_CATALOG_METRICS
                .unselected_series
                .with_label_values(&[source.as_str()])
                .set(i64::try_from(n).unwrap_or(i64::MAX));
        }
    }
}

/// Record confirmed NotFound as one consecutive fetch miss, even when no observations
/// were ever fetched. Three in a row retire the series from fetching. This evidence never
/// changes the catalog absence counter. Failed HTTP requests and parse errors must never
/// call this function.
pub(crate) async fn record_not_found_conn(
    conn: &mut AsyncPgConnection,
    source: SourceId,
    external_id: &str,
) -> AppResult<Option<String>> {
    record_fetch_miss_conn(conn, source, external_id, FetchMiss::NotFound).await
}

/// Record that a successful, complete whole-table response omitted this series
/// ([`CrawlError::Absent`](crate::CrawlError::Absent)). Weaker than NotFound, so it has its
/// own counter and takes six in a row to retire the series from fetching. A series the
/// current accepted catalog lists is never counted: the catalog alone decides whether it was
/// removed, so a selected series cannot flip between fetch-retired and catalog-reactivated.
pub(crate) async fn record_absent_conn(
    conn: &mut AsyncPgConnection,
    source: SourceId,
    external_id: &str,
) -> AppResult<Option<String>> {
    record_fetch_miss_conn(conn, source, external_id, FetchMiss::Absent).await
}

/// Consecutive confirmed NotFound fetches that retire a series from fetching.
pub(crate) const NOT_FOUND_RETIRE_AFTER: i32 = 3;
/// Consecutive Absent fetches that retire a series the accepted catalog does not list.
pub(crate) const ABSENT_RETIRE_AFTER: i32 = 6;

#[derive(Clone, Copy)]
enum FetchMiss {
    NotFound,
    Absent,
}

impl FetchMiss {
    /// Counter column and the consecutive misses that retire a series.
    fn counter(self) -> (&'static str, i32) {
        match self {
            Self::NotFound => ("fetch_not_found_count", NOT_FOUND_RETIRE_AFTER),
            Self::Absent => ("fetch_absent_count", ABSENT_RETIRE_AFTER),
        }
    }

    /// Extra condition for counting this miss at all.
    /// The current accepted catalog is the only authority for series it lists outright:
    /// fetch misses for those are not counted (they still fail the fetch and raise the
    /// ordinary crawl-failure alerts), so a listed series cannot flip between fetch-retired
    /// and catalog-reactivated.
    const COUNTS_WHEN: &'static str =
        "NOT (listed_in_accepted_catalog AND scope_fingerprint IS NOT DISTINCT FROM \
         (SELECT fingerprint FROM catalog_lifecycle_scopes s WHERE s.source_id=$1))";
}

async fn record_fetch_miss_conn(
    conn: &mut AsyncPgConnection,
    source: SourceId,
    external_id: &str,
    miss: FetchMiss,
) -> AppResult<Option<String>> {
    let source_id = persist::data_source_id_conn(conn, source).await?;
    lock_series(conn, source_id, external_id).await?;
    // Fetch-only evidence has its own scope: it can badge retained history that
    // predates catalog rules without expanding the catalog disappearance denominator.
    diesel::sql_query("INSERT INTO series_lifecycle(source_id,external_id,scope_fingerprint)
        SELECT $1,$2,'fetch-only' WHERE EXISTS(SELECT 1 FROM economic_series WHERE source_id=$1 AND external_id=$2)
            OR EXISTS(SELECT 1 FROM series_metadata WHERE source_id=$1 AND external_id=$2)
        ON CONFLICT(source_id,external_id) DO NOTHING")
        .bind::<SqlUuid,_>(source_id).bind::<Text,_>(external_id).execute(&mut *conn).await?;
    let (counter, threshold) = miss.counter();
    let retire = format!("{counter}+1>=$3 AND status='active'");
    let change: Option<StatusChange> = diesel::sql_query(format!(
        "UPDATE series_lifecycle SET \
        {counter}=LEAST({counter}::bigint+1,2147483647)::integer, \
        status=CASE WHEN {retire} THEN 'retired' ELSE status END, \
        fetch_retired=CASE WHEN {retire} THEN TRUE ELSE fetch_retired END, \
        since=CASE WHEN {retire} THEN NOW() ELSE since END \
        WHERE source_id=$1 AND external_id=$2 AND {counts} \
        RETURNING CASE WHEN old.status<>status THEN status ELSE NULL END AS status",
        counts = FetchMiss::COUNTS_WHEN
    ))
    .bind::<SqlUuid, _>(source_id)
    .bind::<Text, _>(external_id)
    .bind::<diesel::sql_types::Integer, _>(threshold)
    .get_result(&mut *conn)
    .await
    .optional()?;
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

/// Reset only fetch evidence when history was successfully fetched. Run before the
/// economic-series upsert, with the same source-then-series lock order as the NotFound
/// and restriction writers. A normal fetch and a forced refetch can hold valid claims for
/// the same series at once, so the series lock must precede even an evidence-absent read.
/// The source lock is shared: successful fetches of different series persist
/// concurrently and wait only for a catalog reconciliation of their source.
///
/// A source that does not track lifecycle and has no lifecycle row has no evidence to
/// reset, so it takes no series lock here. The worker already holds the shared source lock
/// for every fetch job (see the module docs).
pub(crate) async fn record_fetch_success_conn(
    conn: &mut AsyncPgConnection,
    source_id: Uuid,
    external_id: &str,
    tracks_lifecycle: bool,
) -> AppResult<()> {
    if !tracks_lifecycle {
        #[derive(QueryableByName)]
        struct Exists {
            #[diesel(sql_type = diesel::sql_types::Bool)]
            present: bool,
        }
        let any: Exists = diesel::sql_query(
            "SELECT EXISTS(SELECT 1 FROM series_lifecycle WHERE source_id=$1) AS present",
        )
        .bind::<SqlUuid, _>(source_id)
        .get_result(&mut *conn)
        .await?;
        if !any.present {
            return Ok(());
        }
    }
    lock_series(conn, source_id, external_id).await?;
    diesel::sql_query("UPDATE series_lifecycle SET fetch_not_found_count=0,fetch_absent_count=0,fetch_retired=FALSE,
        status=CASE WHEN fetch_retired AND status='retired'
            THEN CASE WHEN missing_refreshes>=3 THEN 'missing_from_catalog' ELSE 'active' END ELSE status END,
        since=CASE WHEN fetch_retired AND status='retired' AND missing_refreshes<3 THEN NULL ELSE since END
        WHERE source_id=$1 AND external_id=$2
            AND (fetch_not_found_count>0 OR fetch_absent_count>0 OR fetch_retired)")
        .bind::<SqlUuid,_>(source_id).bind::<Text,_>(external_id).execute(&mut *conn).await?;
    Ok(())
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
) -> AppResult<Option<String>> {
    let source_id = persist::data_source_id_conn(conn, source).await?;
    lock_series(conn, source_id, external_id).await?;
    let change = diesel::sql_query("INSERT INTO series_lifecycle(source_id,external_id,status,since,scope_fingerprint)
        VALUES($1,$2,'restricted',NOW(),'fetch-only') ON CONFLICT(source_id,external_id) DO UPDATE SET
        status='restricted',since=CASE WHEN series_lifecycle.status='restricted' THEN series_lifecycle.since ELSE NOW() END
        RETURNING CASE WHEN old.status='restricted' THEN NULL ELSE status END AS status")
        .bind::<SqlUuid,_>(source_id).bind::<Text,_>(external_id).get_result::<StatusChange>(&mut *conn).await?;
    diesel::sql_query("UPDATE economic_series SET crawl_status='restricted',is_active=false WHERE source_id=$1 AND external_id=$2")
        .bind::<SqlUuid,_>(source_id).bind::<Text,_>(external_id).execute(&mut *conn).await?;
    diesel::sql_query(
        "UPDATE series_metadata SET is_active=false WHERE source_id=$1 AND external_id=$2",
    )
    .bind::<SqlUuid, _>(source_id)
    .bind::<Text, _>(external_id)
    .execute(&mut *conn)
    .await?;
    Ok(change.status)
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
        lock_source_exclusive(conn, source_id).await?;
        let successor:Id = diesel::sql_query("SELECT es.id FROM economic_series es
            WHERE es.source_id=$1 AND es.external_id=$2 AND es.is_active
                AND EXISTS(SELECT 1 FROM data_points dp WHERE dp.series_id=es.id)
                AND NOT EXISTS(SELECT 1 FROM series_lifecycle l WHERE l.source_id=es.source_id
                    AND l.external_id=es.external_id AND l.status<>'active')")
            .bind::<SqlUuid,_>(source_id).bind::<Text,_>(successor_external_id)
            .get_result(&mut *conn).await.optional()?
            .ok_or_else(||AppError::ValidationError("successor must be actively published, public and have stored observations".into()))?;
        let successor=successor.id;
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
    drop(conn);
    if let Err(error) = refresh_metrics(pool, &[source]).await {
        tracing::warn!(%source,%error,"publication metrics refresh failed after successor confirmation");
    }
    Ok(())
}

#[cfg(test)]
mod tests;
