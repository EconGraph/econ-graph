// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! `crawler check-selection`: checks each source's curated list or catalog selection rules
//! against the source's live upstream catalog, without a database.
//!
//! For each source it downloads the catalog the worker selects from and reports:
//!
//! - **missing**: a curated entry the source no longer lists, or a rule that matches nothing;
//! - **discontinued**: a curated entry the source still lists but no longer updates;
//! - **restricted**: a curated FRED series whose notes carry a copyright restriction;
//! - **over-broad**: a rule that matches more than its `max_matches`;
//! - **over-cap**: a selection past the rules' volume limits (`max_series`, `max_tables`,
//!   `max_catalog_series`), which stops discovery;
//! - **over-quota**: a refresh whose estimated requests exceed the source's daily quota.
//!
//! Rule-selected FRED series that are discontinued or restricted, and FRED's known-retired
//! include probes (`expected_missing`), are notices: discovery keeps them on purpose.
//!
//! | Source     | Catalog checked                                     | Selection                       |
//! |------------|-----------------------------------------------------|---------------------------------|
//! | FRED       | release/category series pages, `/series` overrides  | `catalog_selection/fred.toml`   |
//! | BEA        | `GetParameterValues(TableName)`, then each table    | `catalog_selection/bea.toml`    |
//! | BLS        | the survey series files (`cu.series`, ...)          | `bls_series.csv`                |
//! | World Bank | each indicator's `/indicator/{id}`                  | `wdi_indicators.csv`            |
//! | Census     | BDS `variables.json` and `geography.json`           | the adapter's keyword rules     |
//! | FHFA       | the HPI master CSV                                  | the adapter's scope             |
//!
//! Every request goes through the given [`HttpFetcher`], which the CLI builds exactly as the
//! worker does, so each source's rate limit, concurrency and retry policy apply. The adapters'
//! own catalog code does the downloading and matching. The [`CrawlCtx`] gets a pool that cannot
//! connect ([`offline_pool`]): nothing here reads or writes a database, and anything that tried
//! would fail the check instead.
//!
//! A source whose policy needs an API key that isn't set is skipped, which is neither a pass
//! nor a failure ([`SelectionReport::exit_code`]).

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
use std::time::Duration;

use chrono::{Datelike, NaiveDate};
use diesel_async::pooled_connection::bb8::Pool;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::AsyncPgConnection;
use econ_graph_core::DatabasePool;
use serde::Serialize;

use crate::adapter::{ApiKeys, CrawlCtx, SourceAdapter};
use crate::catalog_selection::is_volume_guard;
use crate::error::CrawlError;
use crate::http::{HttpFetcher, IfChanged};
use crate::policy::SourcePolicy;
use crate::reference;
use crate::reference_file::download_seed_codes;
use crate::source::SourceId;
use crate::sources::bea::{BeaAdapter, BeaDataset};
use crate::sources::bls::{self, BlsAdapter};
use crate::sources::census::CensusAdapter;
use crate::sources::fhfa::FhfaAdapter;
use crate::sources::fred::{self, FredAdapter, FredCatalogRules};
use crate::sources::world_bank::{self, WorldBankAdapter, WDI_SOURCE_ID};

/// The sources `check-selection` checks, in report order: every live adapter with a curated
/// list, selection rules or a fixed scope.
pub const CHECKED_SOURCES: [SourceId; 6] = [
    SourceId::Fred,
    SourceId::Bea,
    SourceId::Bls,
    SourceId::WorldBank,
    SourceId::Census,
    SourceId::Fhfa,
];

/// Exit status: every checked source is clean.
pub const EXIT_CLEAN: u8 = 0;
/// Exit status: at least one source has a problem.
pub const EXIT_PROBLEMS: u8 = 1;
/// Exit status: at least one source's check could not finish (a download or parse failed).
pub const EXIT_FAILED: u8 = 2;
/// Exit status: no problems or failures, but at least one source was skipped for a missing key.
pub const EXIT_SKIPPED: u8 = 3;

/// A database pool that never connects: the host is in the reserved `.invalid` domain, so a
/// checkout fails at name resolution. [`CrawlCtx`] needs a pool; this one guarantees the check
/// writes nothing, because any database access fails it.
pub fn offline_pool() -> DatabasePool {
    let manager = AsyncDieselConnectionManager::<AsyncPgConnection>::new(
        "postgres://check-selection@no-database.invalid/none",
    );
    Pool::builder()
        .max_size(1)
        .min_idle(None)
        .connection_timeout(Duration::from_secs(1))
        .build_unchecked(manager)
}

/// The outcome of one source's check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    /// Checked, nothing to fix.
    Ok,
    /// Checked; see `problems`.
    Problems,
    /// The check could not finish; see `error`.
    Failed,
    /// Not checked: the source needs an API key that isn't set.
    Skipped,
}

/// What a finding is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    /// A curated entry the source no longer lists, or a rule that matches nothing.
    Missing,
    /// Listed upstream but no longer updated.
    Discontinued,
    /// Copyright-restricted (FRED notes).
    Restricted,
    /// A rule matching more than its `max_matches`.
    OverBroad,
    /// A selection past a volume limit.
    OverCap,
    /// A refresh needing more requests than the source's daily quota.
    OverQuota,
    /// A known-retired include probe, absent as expected.
    ExpectedMissing,
}

/// One problem or notice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub kind: FindingKind,
    /// The entry, rule or limit it is about.
    pub entry: String,
    pub detail: String,
}

/// How many current catalog entries one selection rule matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleCheck {
    pub rule: String,
    pub matched: usize,
    pub max_matches: Option<usize>,
}

/// The [`Bound`] on selected series.
const SERIES: &str = "selected series";

/// A count against a selection limit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Bound {
    pub what: String,
    pub count: usize,
    pub limit: usize,
}

/// The requests one refresh of every selected series takes, at the source's policy rate.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RefreshEstimate {
    /// Requests for one refresh of every selected series: batched as the worker batches them,
    /// counting every request a full fetch makes (FRED: `/series`, `/series/vintagedates`,
    /// `/series/observations`; a BEA Regional batch: `GetData` and its line codes).
    pub requests: usize,
    /// The policy's sustained rate.
    pub requests_per_minute: f64,
    /// `requests` at that rate.
    pub minutes: f64,
    /// The policy's daily quota, if the source has one.
    pub daily_quota: Option<u32>,
}

impl RefreshEstimate {
    fn new(requests: usize, policy: &SourcePolicy) -> Self {
        let per_minute = policy.requests_per_second * 60.0;
        Self {
            requests,
            requests_per_minute: per_minute,
            minutes: requests as f64 / per_minute,
            daily_quota: policy.daily_quota,
        }
    }
}

/// One source's check.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceCheck {
    pub source: SourceId,
    pub status: CheckStatus,
    /// Why the source was skipped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    /// Why the check could not finish.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Curated entries checked (BLS series, World Bank indicators).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub curated: Option<usize>,
    /// Series the selection yields now.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<usize>,
    pub rules: Vec<RuleCheck>,
    pub bounds: Vec<Bound>,
    pub problems: Vec<Finding>,
    pub notices: Vec<Finding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh: Option<RefreshEstimate>,
}

impl SourceCheck {
    fn new(source: SourceId) -> Self {
        Self {
            source,
            status: CheckStatus::Ok,
            skipped: None,
            error: None,
            curated: None,
            selected: None,
            rules: Vec::new(),
            bounds: Vec::new(),
            problems: Vec::new(),
            notices: Vec::new(),
            refresh: None,
        }
    }

    fn problem(&mut self, kind: FindingKind, entry: impl Into<String>, detail: impl Into<String>) {
        self.problems.push(Finding {
            kind,
            entry: entry.into(),
            detail: detail.into(),
        });
    }

    fn notice(&mut self, kind: FindingKind, entry: impl Into<String>, detail: impl Into<String>) {
        self.notices.push(Finding {
            kind,
            entry: entry.into(),
            detail: detail.into(),
        });
    }

    /// Records a count against a limit, and an over-cap problem past it.
    fn bound(&mut self, what: &str, count: usize, limit: usize) {
        if count > limit {
            self.problem(
                FindingKind::OverCap,
                what,
                format!("{count} exceeds the limit of {limit}"),
            );
        }
        self.bounds.push(Bound {
            what: what.to_string(),
            count,
            limit,
        });
    }

    /// Records a rule's match count, with a problem when it matches nothing or more than its
    /// `max_matches`.
    fn rule(&mut self, rule: String, matched: usize, max_matches: Option<usize>, what: &str) {
        if matched == 0 {
            self.problem(
                FindingKind::Missing,
                &rule,
                format!("matches no {what} in the current catalog"),
            );
        }
        if let Some(max) = max_matches.filter(|max| matched > *max) {
            self.problem(
                FindingKind::OverBroad,
                &rule,
                format!("matches {matched} {what}, more than its max_matches of {max}"),
            );
        }
        self.rules.push(RuleCheck {
            rule,
            matched,
            max_matches,
        });
    }

    /// Records the refresh estimate, with an over-quota problem past the daily quota.
    fn refresh(&mut self, estimate: RefreshEstimate) {
        if let Some(quota) = estimate
            .daily_quota
            .filter(|q| estimate.requests > *q as usize)
        {
            self.problem(
                FindingKind::OverQuota,
                "refresh",
                format!(
                    "about {} requests per refresh exceed the daily quota of {quota}",
                    estimate.requests
                ),
            );
        }
        self.refresh = Some(estimate);
    }

    /// Settles `status` from what was recorded.
    fn finish(mut self, result: Result<(), CrawlError>) -> Self {
        if let Err(e) = result {
            self.error = Some(e.to_string());
        }
        self.status = if self.skipped.is_some() {
            CheckStatus::Skipped
        } else if self.error.is_some() {
            CheckStatus::Failed
        } else if !self.problems.is_empty() {
            CheckStatus::Problems
        } else {
            CheckStatus::Ok
        };
        self
    }
}

/// Every checked source's result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SelectionReport {
    pub sources: Vec<SourceCheck>,
}

impl SelectionReport {
    /// [`EXIT_FAILED`] if any check failed, else [`EXIT_PROBLEMS`] if any source has a problem,
    /// else [`EXIT_SKIPPED`] if any source was skipped, else [`EXIT_CLEAN`].
    pub fn exit_code(&self) -> u8 {
        let any = |status| self.sources.iter().any(|s| s.status == status);
        if any(CheckStatus::Failed) {
            EXIT_FAILED
        } else if any(CheckStatus::Problems) {
            EXIT_PROBLEMS
        } else if any(CheckStatus::Skipped) {
            EXIT_SKIPPED
        } else {
            EXIT_CLEAN
        }
    }

    /// One line per source, then its rules, limits, refresh estimate and findings.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for s in &self.sources {
            let status = match s.status {
                CheckStatus::Ok => "ok".to_string(),
                CheckStatus::Problems => format!("{} problem(s)", s.problems.len()),
                CheckStatus::Failed => format!("failed: {}", s.error.as_deref().unwrap_or("-")),
                CheckStatus::Skipped => {
                    format!("skipped: {}", s.skipped.as_deref().unwrap_or("-"))
                }
            };
            let _ = writeln!(out, "{}: {status}", s.source);
            if let Some(n) = s.curated {
                let _ = writeln!(out, "  curated entries: {n}");
            }
            // A source with a series limit shows the count with its limit below.
            if let Some(n) = s
                .selected
                .filter(|_| !s.bounds.iter().any(|b| b.what == SERIES))
            {
                let _ = writeln!(out, "  selected series: {n}");
            }
            for r in &s.rules {
                let max = r
                    .max_matches
                    .map_or_else(String::new, |m| format!(" (max_matches {m})"));
                let _ = writeln!(out, "  rule {}: {} matched{max}", r.rule, r.matched);
            }
            for b in &s.bounds {
                let _ = writeln!(out, "  {}: {} of {}", b.what, b.count, b.limit);
            }
            if let Some(e) = &s.refresh {
                let quota = e.daily_quota.map_or_else(
                    || "no daily quota".to_string(),
                    |q| format!("daily quota {q}"),
                );
                let _ = writeln!(
                    out,
                    "  refresh: about {} requests, {:.1} min at {:.0}/min, {quota}",
                    e.requests, e.minutes, e.requests_per_minute
                );
            }
            for f in &s.problems {
                let _ = writeln!(
                    out,
                    "  problem {}: {}: {}",
                    kind_name(f.kind),
                    f.entry,
                    f.detail
                );
            }
            for f in &s.notices {
                let _ = writeln!(
                    out,
                    "  notice {}: {}: {}",
                    kind_name(f.kind),
                    f.entry,
                    f.detail
                );
            }
        }
        out
    }
}

fn kind_name(kind: FindingKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// The adapters, curated lists and rules a check uses. [`Checker::live`] reads the lists and
/// rules from the crawler data directory, as the worker does; tests replace them.
pub struct Checker {
    pub ctx: CrawlCtx,
    /// Today, for the BLS discontinued test (latest period before last year).
    pub today: NaiveDate,
    pub fred: FredAdapter,
    pub bea: BeaAdapter,
    pub bls: BlsAdapter,
    pub world_bank: WorldBankAdapter,
    pub census: CensusAdapter,
    pub fhfa: FhfaAdapter,
    /// FRED rules instead of `catalog_selection/fred.toml`.
    pub(crate) fred_rules: Option<FredCatalogRules>,
    /// BLS series ids instead of `bls_series.csv`.
    pub bls_series: Option<Vec<String>>,
    /// World Bank indicator ids instead of `wdi_indicators.csv`.
    pub wdi_indicators: Option<Vec<String>>,
}

impl Checker {
    /// The production adapters, over `http` (built as the worker builds it) and `keys`.
    pub fn live(http: HttpFetcher, keys: ApiKeys, today: NaiveDate) -> Self {
        Self {
            ctx: CrawlCtx {
                http,
                pool: offline_pool(),
                keys,
                bypass_validators: true,
            },
            today,
            fred: FredAdapter::default(),
            bea: BeaAdapter::default(),
            bls: BlsAdapter::default(),
            world_bank: WorldBankAdapter::default(),
            census: CensusAdapter::default(),
            fhfa: FhfaAdapter::default(),
            fred_rules: None,
            bls_series: None,
            wdi_indicators: None,
        }
    }

    /// Checks each of `sources` in turn (one source's requests at a time, at its policy rate).
    pub async fn check(&self, sources: &[SourceId]) -> SelectionReport {
        let mut report = SelectionReport {
            sources: Vec::with_capacity(sources.len()),
        };
        for &source in sources {
            report.sources.push(self.check_source(source).await);
        }
        report
    }

    async fn check_source(&self, source: SourceId) -> SourceCheck {
        let mut check = SourceCheck::new(source);
        let policy = self.ctx.http.policy(source);
        if policy.needs_api_key && self.ctx.keys.get(source).is_none() {
            let var = crate::cli::api_key_env_var(source).unwrap_or("its API key");
            check.skipped = Some(format!("no API key ({var} is not set)"));
            return check.finish(Ok(()));
        }
        tracing::info!(%source, "checking selection");
        let result = match source {
            SourceId::Fred => self.check_fred(&mut check, &policy).await,
            SourceId::Bea => self.check_bea(&mut check, &policy).await,
            SourceId::Bls => self.check_bls(&mut check, &policy).await,
            SourceId::WorldBank => self.check_world_bank(&mut check, &policy).await,
            SourceId::Census => {
                self.check_discovered(&self.census, &mut check, &policy)
                    .await
            }
            SourceId::Fhfa => self.check_discovered(&self.fhfa, &mut check, &policy).await,
            other => Err(CrawlError::Permanent(format!(
                "{other} has no selection to check"
            ))),
        };
        check.finish(result)
    }

    async fn check_fred(
        &self,
        check: &mut SourceCheck,
        policy: &SourcePolicy,
    ) -> Result<(), CrawlError> {
        let rules = match &self.fred_rules {
            Some(rules) => rules.clone(),
            None => fred::load_catalog_rules()?,
        };
        let selection = match self.fred.select_catalog(&self.ctx, &rules).await {
            Ok(selection) => selection,
            Err(e) if fred::is_catalog_volume_guard(&e) => {
                check.problem(FindingKind::OverCap, "selection", e.to_string());
                return Ok(());
            }
            Err(e) => return Err(e),
        };
        for ((rule, max), matched) in rules
            .rule_summaries()
            .into_iter()
            .zip(selection.rule_matches)
        {
            check.rule(rule, matched, max, "series");
        }
        check.bound(
            "catalog rows inspected",
            selection.scanned,
            rules.max_catalog_series(),
        );
        check.bound(SERIES, selection.found.len(), rules.max_series());
        check.selected = Some(selection.found.len());
        let facts = self.fred.catalog_lifecycle();
        let included = |id: &String| rules.include().contains(id);
        for id in &facts.confirmed_missing_ids {
            if rules.expected_missing().contains(id) {
                check.notice(
                    FindingKind::ExpectedMissing,
                    id,
                    "known-retired include probe; FRED no longer has it",
                );
            } else {
                check.problem(FindingKind::Missing, id, "include entry FRED no longer has");
            }
        }
        for (ids, kind, what) in [
            (
                &facts.discontinued_ids,
                FindingKind::Discontinued,
                "title says (DISCONTINUED)",
            ),
            (
                &facts.restricted_ids,
                FindingKind::Restricted,
                "notes carry a copyright restriction",
            ),
        ] {
            for id in ids {
                if included(id) {
                    check.problem(kind, id, format!("include entry: {what}"));
                } else {
                    check.notice(kind, id, format!("selected by a rule: {what}"));
                }
            }
        }
        let ids: Vec<&str> = selection
            .found
            .iter()
            .map(|s| s.external_id.as_str())
            .collect();
        // No batching: /series, /series/vintagedates and /series/observations per series.
        check.refresh(estimate(&self.fred, policy, &ids, |_| 3));
        Ok(())
    }

    async fn check_bea(
        &self,
        check: &mut SourceCheck,
        policy: &SourcePolicy,
    ) -> Result<(), CrawlError> {
        let rules = self.bea.selection()?;
        // BEA's current TableName lists, downloaded the way `record-reference-seeds` does.
        let mut catalogs = HashMap::new();
        for list in self.bea.code_lists(&self.ctx.keys) {
            let dataset = BeaDataset::from_code(list.dataset).ok_or_else(|| {
                CrawlError::Permanent(format!(
                    "BEA code list for unknown dataset {}",
                    list.dataset
                ))
            })?;
            let (_, codes) = download_seed_codes(&self.ctx.http, SourceId::Bea, &list).await?;
            catalogs.insert(
                dataset,
                codes
                    .into_iter()
                    .map(|c| (c.code, c.label))
                    .collect::<HashMap<_, _>>(),
            );
        }
        for (rule, names) in rules.rule.iter().zip(rules.rule_matches(&catalogs)) {
            check.rule(rule.describe(), names.len(), rule.max_matches, "tables");
        }
        let tables = match rules.select(&catalogs) {
            Ok(tables) => tables,
            Err(e) if is_volume_guard(&e) => {
                check.problem(FindingKind::OverCap, "selected tables", e.to_string());
                return Ok(());
            }
            // Every rule matching nothing is already a problem per rule.
            Err(_) if !check.problems.is_empty() => return Ok(()),
            Err(e) => return Err(e),
        };
        check.bound("selected tables", tables.len(), rules.max_tables);
        let mut found = Vec::new();
        for table in &tables {
            let title = &catalogs[&table.dataset][&table.table_name];
            match self.bea.discover_table(&self.ctx, table, title).await {
                Ok(series) => found.extend(series),
                Err(e) if is_volume_guard(&e) => {
                    check.problem(FindingKind::OverCap, &table.table_name, e.to_string());
                }
                // Listed in the catalog, but BEA serves no rows or lines for it.
                Err(e @ (CrawlError::NotFound(_) | CrawlError::Permanent(_))) => {
                    check.problem(FindingKind::Discontinued, &table.table_name, e.to_string())
                }
                Err(e) => return Err(e),
            }
        }
        check.bound(SERIES, found.len(), rules.max_series);
        check.selected = Some(found.len());
        let ids: Vec<&str> = found.iter().map(|s| s.external_id.as_str()).collect();
        // A Regional batch also requests its line codes.
        check.refresh(estimate(&self.bea, policy, &ids, |key| {
            if key.is_some_and(|k| k.starts_with(crate::sources::bea::REGIONAL_DATASET)) {
                2
            } else {
                1
            }
        }));
        Ok(())
    }

    async fn check_bls(
        &self,
        check: &mut SourceCheck,
        policy: &SourcePolicy,
    ) -> Result<(), CrawlError> {
        let curated: Vec<String> = match &self.bls_series {
            Some(ids) => ids.clone(),
            None => reference::bls_series()?
                .iter()
                .map(|s| s.id.clone())
                .collect(),
        };
        check.curated = Some(curated.len());
        let files = self.bls.series_files();
        let mut listed = Vec::new();
        for (survey, url) in &files {
            let wanted: BTreeSet<String> = curated
                .iter()
                .filter(|id| id.starts_with(survey))
                .cloned()
                .collect();
            if wanted.is_empty() {
                continue;
            }
            let body = changed_body(&self.ctx.http, SourceId::Bls, url).await?;
            let rows = bls::parse_series_file(&body, &wanted)?;
            for id in &wanted {
                match rows.get(id) {
                    None => check.problem(
                        FindingKind::Missing,
                        id,
                        format!("not in BLS's series file {url}"),
                    ),
                    Some(row) if row.end_year().is_some_and(|y| y < self.today.year() - 1) => check
                        .problem(
                            FindingKind::Discontinued,
                            id,
                            format!("latest period is in {}", row.end_year().unwrap_or_default()),
                        ),
                    Some(_) => listed.push(id.clone()),
                }
            }
        }
        for id in curated
            .iter()
            .filter(|id| !files.iter().any(|(survey, _)| id.starts_with(survey)))
        {
            check.problem(
                FindingKind::Missing,
                id,
                "no BLS series file covers its survey",
            );
        }
        check.selected = Some(listed.len());
        let ids: Vec<&str> = listed.iter().map(String::as_str).collect();
        // A refresh's revision lookback fits one year window, keyed or not.
        check.refresh(estimate(&self.bls, policy, &ids, |_| 1));
        Ok(())
    }

    async fn check_world_bank(
        &self,
        check: &mut SourceCheck,
        policy: &SourcePolicy,
    ) -> Result<(), CrawlError> {
        let indicators: Vec<String> = match &self.wdi_indicators {
            Some(ids) => ids.clone(),
            None => reference::wdi_indicators()?
                .iter()
                .map(|i| i.id.clone())
                .collect(),
        };
        check.curated = Some(indicators.len());
        let mut listed = 0;
        for id in &indicators {
            let url = self.world_bank.indicator_url(id);
            let source = match changed_body(&self.ctx.http, SourceId::WorldBank, &url).await {
                Ok(body) => world_bank::indicator_source(id, &body),
                Err(e) => Err(e),
            };
            match source {
                Ok((source_id, _)) if source_id == WDI_SOURCE_ID => listed += 1,
                Ok((source_id, name)) => check.problem(
                    FindingKind::Discontinued,
                    id,
                    format!("moved out of WDI to source {source_id} ({name})"),
                ),
                Err(CrawlError::NotFound(_)) => check.problem(
                    FindingKind::Missing,
                    id,
                    "the World Bank doesn't know this indicator",
                ),
                Err(e) => return Err(e),
            }
        }
        check.selected = Some(listed);
        // One request per indicator serves all of its areas.
        check.refresh(RefreshEstimate::new(listed, policy));
        Ok(())
    }

    /// A source whose discovery reads no database (Census, FHFA): its discovery is the check.
    async fn check_discovered(
        &self,
        adapter: &dyn SourceAdapter,
        check: &mut SourceCheck,
        policy: &SourcePolicy,
    ) -> Result<(), CrawlError> {
        let found = adapter.discover(&self.ctx).await?;
        if found.is_empty() {
            check.problem(FindingKind::Missing, "selection", "matches no series");
        }
        if adapter.id() == SourceId::Census {
            let variables: BTreeSet<&str> = found
                .iter()
                .filter_map(|s| s.dataset.dimensions.0.get("variable").map(String::as_str))
                .collect();
            check.rule(
                "BDS variables matching the economic keyword rules".into(),
                variables.len(),
                None,
                "variables",
            );
        }
        check.selected = Some(found.len());
        let ids: Vec<&str> = found.iter().map(|s| s.external_id.as_str()).collect();
        check.refresh(estimate(adapter, policy, &ids, |_| 1));
        Ok(())
    }
}

/// The body of an unconditional GET of one of `source`'s reference files, through the shared
/// fetcher (its rate limit, timeout and redaction).
async fn changed_body(
    http: &HttpFetcher,
    source: SourceId,
    url: &str,
) -> Result<String, CrawlError> {
    match http
        .get_reference_file_if_changed(source, url, None)
        .await?
    {
        IfChanged::Changed { body, .. } => Ok(body),
        IfChanged::Unchanged { .. } => Err(CrawlError::Transient(format!(
            "{url}: unchanged to an unconditional request"
        ))),
    }
}

/// Requests to refresh `ids`: grouped by the adapter's batch key into chunks of the policy's
/// `max_batch` (unbatched ids one each), each costing `per_batch(key)` requests.
fn estimate(
    adapter: &dyn SourceAdapter,
    policy: &SourcePolicy,
    ids: &[&str],
    per_batch: impl Fn(Option<&str>) -> usize,
) -> RefreshEstimate {
    let mut groups: HashMap<String, usize> = HashMap::new();
    let mut requests = 0;
    for id in ids {
        match adapter.batch_key(id) {
            Some(key) => *groups.entry(key).or_default() += 1,
            None => requests += per_batch(None),
        }
    }
    let max_batch = policy.max_batch.max(1);
    for (key, n) in &groups {
        requests += n.div_ceil(max_batch) * per_batch(Some(key));
    }
    RefreshEstimate::new(requests, policy)
}

#[cfg(test)]
mod tests;
