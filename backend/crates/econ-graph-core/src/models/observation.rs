//! Storage-independent observation read contracts. Database writes deliberately remain in
//! the crawler's caller-owned, lease-fenced transaction.

use std::collections::{BTreeMap, HashMap};

use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{DataPoint, DataQueryParams};

/// Explicit vintage selection. Results always use (date, revision_date, id) ascending order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", content = "date", rename_all = "snake_case")]
pub enum RevisionMode {
    /// Every stored revision, including historical synthetic vintages.
    All,
    /// Newest revision per date; ties prefer a non-original row, then the greatest UUID.
    Latest,
    /// Newest revision published on or before the given date, with the same tie-breaking.
    AsOf(NaiveDate),
    /// All rows marked original, excluding superseded synthetic legacy originals.
    Original,
    /// Compatibility for originalOnly combined with latestRevisionOnly.
    OriginalLatest,
    /// Compatibility for originalOnly combined with asOf.
    OriginalAsOf(NaiveDate),
}

/// Transformation context belongs to the same snapshot and revision selection as its page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageContext {
    /// Every earlier row within this many days; negative or overflowing durations are invalid.
    Days(i64),
    /// Immediate predecessor in the requested revision mode's page order.
    PreviousPoint,
    /// Earliest non-null, non-zero observation before the page.
    FirstPoint,
}

/// Read one series; date bounds are inclusive. Pagination is normalized by the store.
#[derive(Debug, Clone)]
pub struct ObservationRequest {
    pub series_id: Uuid,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub revisions: RevisionMode,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub context: Option<PageContext>,
}

impl ObservationRequest {
    /// Context windows are bounded durations, never negative or overflowing chrono values.
    pub fn validate(&self) -> crate::error::AppResult<()> {
        if let Some(PageContext::Days(days)) = self.context
            && (days < 0 || chrono::Duration::try_days(days).is_none())
        {
            return Err(crate::error::AppError::BadRequest(
                "observation context days must be non-negative and fit a duration".into(),
            ));
        }
        Ok(())
    }

    /// Preserve legacy combinations and precedence; unspecified revisions mean All.
    pub fn from_legacy(params: DataQueryParams, context: Option<PageContext>) -> Self {
        let original = params.original_only.unwrap_or(false);
        let latest = params.latest_revision_only.unwrap_or(false);
        let revisions = match (original, params.as_of, latest) {
            (true, Some(day), _) => RevisionMode::OriginalAsOf(day),
            (false, Some(day), _) => RevisionMode::AsOf(day),
            (true, None, true) => RevisionMode::OriginalLatest,
            (true, None, false) => RevisionMode::Original,
            (false, None, true) => RevisionMode::Latest,
            (false, None, false) => RevisionMode::All,
        };
        Self {
            series_id: params.series_id,
            start_date: params.start_date,
            end_date: params.end_date,
            revisions,
            limit: params.limit,
            offset: params.offset,
            context,
        }
    }

    /// Compatibility for existing Postgres predicates; no value or identity conversion.
    pub fn legacy_params(&self) -> DataQueryParams {
        let (original, latest, as_of) = match self.revisions {
            RevisionMode::All => (false, false, None),
            RevisionMode::Latest => (false, true, None),
            RevisionMode::AsOf(day) => (false, false, Some(day)),
            RevisionMode::Original => (true, false, None),
            RevisionMode::OriginalLatest => (true, true, None),
            RevisionMode::OriginalAsOf(day) => (true, false, Some(day)),
        };
        DataQueryParams {
            series_id: self.series_id,
            start_date: self.start_date,
            end_date: self.end_date,
            original_only: Some(original),
            latest_revision_only: Some(latest),
            as_of,
            limit: self.limit,
            offset: self.offset,
        }
    }
}

/// Lossless observation DTO, independent of an ORM or physical decimal representation.
/// JSON values are decimal strings (or null), never binary floating point. Attributes are
/// additive; current Postgres has none and returns an empty map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub id: Uuid,
    pub series_id: Uuid,
    pub date: NaiveDate,
    pub value: Option<BigDecimal>,
    pub revision_date: NaiveDate,
    pub is_original_release: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub attributes: BTreeMap<String, serde_json::Value>,
}

impl From<DataPoint> for Observation {
    fn from(point: DataPoint) -> Self {
        Self {
            id: point.id,
            series_id: point.series_id,
            date: point.date,
            value: point.value,
            revision_date: point.revision_date,
            is_original_release: point.is_original_release,
            created_at: point.created_at,
            updated_at: point.updated_at,
            attributes: BTreeMap::new(),
        }
    }
}

impl From<Observation> for DataPoint {
    fn from(point: Observation) -> Self {
        Self {
            id: point.id,
            series_id: point.series_id,
            date: point.date,
            value: point.value,
            revision_date: point.revision_date,
            is_original_release: point.is_original_release,
            created_at: point.created_at,
            updated_at: point.updated_at,
        }
    }
}

/// Page, count, dataset identity and transformation context read in one consistent snapshot.
/// This is a data-storage contract; callers must apply request-scoped authorization first.
#[derive(Debug, Clone)]
pub struct ObservationPage {
    pub series_id: Uuid,
    /// None for a missing or legally restricted series. Request-scoped catalog authorization
    /// remains the API's responsibility.
    pub dataset_id: Option<Uuid>,
    pub points: Vec<Observation>,
    pub context: Vec<Observation>,
    /// Whether each returned point differs numerically from its earliest real original
    /// release, keyed by observation UUID. The original itself is false; missing or
    /// later-published originals leave the point absent. Read in the same snapshot.
    pub revised: HashMap<Uuid, bool>,
    pub total_count: i64,
    pub offset: i64,
}

impl ObservationPage {
    pub fn has_next_page(&self) -> bool {
        self.offset.saturating_add(self.points.len() as i64) < self.total_count
    }
}
