//! SEC company research boundary. All source reads enforce the SEC catalog grant and
//! remain disabled in release builds until the complete company workflow is accepted.
use super::{
    context::{self, GraphQLContext},
    errors::bad_user_input,
};
use async_graphql::{Context, Enum, Error, ErrorExtensions, InputObject, Result, SimpleObject, ID};
use chrono::{DateTime, NaiveDate, Utc};
use econ_graph_auth::Role;
use econ_graph_core::models::company_financials::{
    self, FactEvidence, RevisionSelection, StatementKind,
};
use econ_graph_core::models::{
    AnnotationAnchor, Company, CompanyFact, CompanyFactSubject, FilingRef, ResearchPeriod,
    SubjectRef,
};
use econ_graph_services::services::company_research::{
    self as service, CompanyFactFilter, CompanyRevision,
};
use std::{collections::BTreeMap, sync::Arc};

pub fn require_access(ctx: &Context<'_>) -> Result<()> {
    if !cfg!(flag_financial_components) {
        return Err(Error::new("Company research is not enabled")
            .extend_with(|_, e| e.set("code", "FEATURE_DISABLED")));
    }
    let permitted = ctx
        .data_opt::<Arc<GraphQLContext>>()
        .is_some_and(|auth| auth.can_read_catalog(Role::CatalogSecRead));
    if !permitted {
        return Err(super::catalogs::forbidden());
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Enum)]
pub enum CompanyRevisionMode {
    #[default]
    Latest,
    Original,
    AsOf,
    Accession,
}
#[derive(Debug, Clone, InputObject)]
pub struct CompanyRevisionInput {
    #[graphql(default)]
    pub mode: CompanyRevisionMode,
    pub as_of: Option<NaiveDate>,
    pub accession: Option<String>,
}
impl CompanyRevisionInput {
    fn validated(self) -> Result<(CompanyRevision, RevisionSelection)> {
        match (self.mode, self.as_of, self.accession) {
            (CompanyRevisionMode::Latest, None, None) => {
                Ok((CompanyRevision::Latest, RevisionSelection::Latest))
            }
            (CompanyRevisionMode::Original, None, None) => {
                Ok((CompanyRevision::Original, RevisionSelection::Original))
            }
            (CompanyRevisionMode::AsOf, Some(date), None) => {
                Ok((CompanyRevision::AsOf(date), RevisionSelection::AsOf(date)))
            }
            (CompanyRevisionMode::Accession, None, Some(accession)) => {
                FilingRef {
                    cik: "0000000001".into(),
                    accession: accession.clone(),
                }
                .validate()
                .map_err(|e| bad_user_input(e.to_string()))?;
                Ok((
                    CompanyRevision::Accession(accession.clone()),
                    RevisionSelection::Accession(accession),
                ))
            }
            _ => Err(bad_user_input(
                "Revision mode requires exactly its matching asOf or accession argument",
            )),
        }
    }
}
fn revision(input: Option<CompanyRevisionInput>) -> Result<(CompanyRevision, RevisionSelection)> {
    input
        .unwrap_or(CompanyRevisionInput {
            mode: CompanyRevisionMode::Latest,
            as_of: None,
            accession: None,
        })
        .validated()
}
fn page(first: Option<i32>, after: Option<&str>, max: i32) -> Result<(i64, i64)> {
    let first = first.unwrap_or(25.min(max));
    if first < 1 || first > max {
        return Err(bad_user_input(format!("first must be between 1 and {max}")));
    }
    let offset = match after {
        None => 0,
        Some(raw) => raw
            .parse::<i64>()
            .ok()
            .filter(|v| (0..=1_000_000).contains(v))
            .ok_or_else(|| bad_user_input("Invalid company page cursor"))?,
    };
    Ok((first.into(), offset))
}
fn dates(period: &ResearchPeriod) -> (Option<NaiveDate>, NaiveDate) {
    match *period {
        ResearchPeriod::Instant { date } => (None, date),
        ResearchPeriod::Duration { start, end } => (Some(start), end),
    }
}
fn period(start: Option<NaiveDate>, end: NaiveDate) -> Result<ResearchPeriod> {
    if start.is_some_and(|date| date > end) {
        return Err(bad_user_input("Period starts after its end"));
    }
    Ok(match start {
        Some(start) => ResearchPeriod::Duration { start, end },
        None => ResearchPeriod::Instant { date: end },
    })
}
fn checked_cik(cik: &str) -> Result<String> {
    service::canonical_cik(cik).map_err(|e| bad_user_input(e.to_string()))
}

#[derive(SimpleObject)]
pub struct CompanyResearchCompany {
    pub id: ID,
    pub cik: String,
    pub name: String,
    pub ticker: Option<String>,
    pub sic_code: Option<String>,
    pub sic_description: Option<String>,
    pub fiscal_year_end: Option<String>,
    pub updated_at: DateTime<Utc>,
}
impl From<Company> for CompanyResearchCompany {
    fn from(c: Company) -> Self {
        Self {
            id: c.id.into(),
            cik: c.cik,
            name: c.name,
            ticker: c.ticker,
            sic_code: c.sic_code,
            sic_description: c.sic_description,
            fiscal_year_end: c.fiscal_year_end,
            updated_at: c.updated_at,
        }
    }
}
#[derive(SimpleObject)]
pub struct CompanyResearchCompanyConnection {
    pub nodes: Vec<CompanyResearchCompany>,
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}
pub async fn companies(
    ctx: &Context<'_>,
    search: Option<String>,
    first: Option<i32>,
    after: Option<String>,
) -> Result<CompanyResearchCompanyConnection> {
    require_access(ctx)?;
    let pool = context::data_pool(ctx)?;
    let (first, offset) = page(first, after.as_deref(), 100)?;
    let mut rows = service::list_companies(pool, search.as_deref(), first + 1, offset).await?;
    let has_next_page = rows.len() as i64 > first;
    rows.truncate(first as usize);
    Ok(CompanyResearchCompanyConnection {
        end_cursor: has_next_page.then(|| (offset + rows.len() as i64).to_string()),
        nodes: rows.into_iter().map(Into::into).collect(),
        has_next_page,
    })
}
pub async fn company(ctx: &Context<'_>, cik: String) -> Result<Option<CompanyResearchCompany>> {
    require_access(ctx)?;
    let pool = context::data_pool(ctx)?;
    Ok(service::company(pool, &checked_cik(&cik)?)
        .await?
        .map(Into::into))
}

#[derive(Debug, Clone, InputObject)]
pub struct CompanyFactFilterInput {
    pub taxonomy: Option<String>,
    pub concept: Option<String>,
    pub unit: Option<String>,
    pub period_start: Option<NaiveDate>,
    pub period_end: Option<NaiveDate>,
    #[graphql(default)]
    pub instant_only: bool,
    #[graphql(default)]
    pub include_revisions: bool,
    pub revision: Option<CompanyRevisionInput>,
}
#[derive(SimpleObject)]
#[graphql(name = "CompanyFact")]
pub struct CompanyFactType {
    pub id: ID,
    pub cik: String,
    pub taxonomy: String,
    pub concept: String,
    pub unit: String,
    pub period_start: Option<NaiveDate>,
    pub period_end: NaiveDate,
    pub accession: String,
    pub filed: NaiveDate,
    pub value: String,
    pub form: String,
    pub fiscal_year: Option<i32>,
    pub fiscal_period: Option<String>,
    pub frame: Option<String>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub anchor: String,
}
impl From<CompanyFact> for CompanyFactType {
    fn from(f: CompanyFact) -> Self {
        let (start, end) = dates(&f.subject.period);
        let anchor = f
            .reference()
            .ok()
            .map(|target| AnnotationAnchor::V1 {
                target,
                chart_id: None,
                observation_date: None,
                observation_value: None,
            })
            .and_then(|a| serde_json::to_string(&a).ok())
            .unwrap_or_default();
        Self {
            id: f.id.into(),
            cik: f.subject.cik,
            taxonomy: f.subject.taxonomy,
            concept: f.subject.concept,
            unit: f.subject.unit,
            period_start: start,
            period_end: end,
            accession: f.accession,
            filed: f.filed,
            value: f.value.to_string(),
            form: f.form,
            fiscal_year: f.fiscal_year,
            fiscal_period: f.fiscal_period,
            frame: f.frame,
            label: f.label,
            description: f.description,
            anchor,
        }
    }
}
#[derive(SimpleObject)]
pub struct CompanyFactConnection {
    pub nodes: Vec<CompanyFactType>,
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}
pub async fn company_facts(
    ctx: &Context<'_>,
    cik: String,
    filter: Option<CompanyFactFilterInput>,
    first: Option<i32>,
    after: Option<String>,
) -> Result<CompanyFactConnection> {
    require_access(ctx)?;
    let pool = context::data_pool(ctx)?;
    let cik = checked_cik(&cik)?;
    let (first, offset) = page(first, after.as_deref(), 1000)?;
    let f = filter.unwrap_or(CompanyFactFilterInput {
        taxonomy: None,
        concept: None,
        unit: None,
        period_start: None,
        period_end: None,
        instant_only: false,
        include_revisions: false,
        revision: None,
    });
    for token in [&f.taxonomy, &f.concept, &f.unit].into_iter().flatten() {
        if token.is_empty() || token.len() > 1000 {
            return Err(bad_user_input("Invalid company fact filter"));
        }
    }
    if f.instant_only && f.period_start.is_some() {
        return Err(bad_user_input(
            "An instant filter cannot have a period start",
        ));
    }
    if let (Some(start), Some(end)) = (f.period_start, f.period_end) {
        period(Some(start), end)?;
    }
    let filter = CompanyFactFilter {
        taxonomy: f.taxonomy,
        concept: f.concept,
        unit: f.unit,
        period_start: f.period_start,
        period_end: f.period_end,
        instant_only: f.instant_only,
        include_revisions: f.include_revisions,
        revision: revision(f.revision)?.0,
    };
    let mut rows = service::company_facts(pool, &cik, &filter, first + 1, offset).await?;
    let has_next_page = rows.len() as i64 > first;
    rows.truncate(first as usize);
    Ok(CompanyFactConnection {
        end_cursor: has_next_page.then(|| (offset + rows.len() as i64).to_string()),
        nodes: rows.into_iter().map(Into::into).collect(),
        has_next_page,
    })
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Enum)]
pub enum CompanyStatementKind {
    IncomeStatement,
    BalanceSheet,
    CashFlow,
}
impl From<CompanyStatementKind> for StatementKind {
    fn from(k: CompanyStatementKind) -> Self {
        match k {
            CompanyStatementKind::IncomeStatement => Self::IncomeStatement,
            CompanyStatementKind::BalanceSheet => Self::BalanceSheet,
            CompanyStatementKind::CashFlow => Self::CashFlow,
        }
    }
}
#[derive(SimpleObject)]
pub struct CompanyPeriod {
    pub start: Option<NaiveDate>,
    pub end: NaiveDate,
    pub kind: String,
}
fn serialized_anchor(target: SubjectRef) -> Option<String> {
    target.validate().ok()?;
    serde_json::to_string(&AnnotationAnchor::V1 {
        target,
        chart_id: None,
        observation_date: None,
        observation_value: None,
    })
    .ok()
}
#[derive(SimpleObject)]
pub struct CompanyFactEvidence {
    pub anchor: Option<String>,
    pub filing_anchor: Option<String>,
    pub fact_id: ID,
    pub cik: String,
    pub taxonomy: String,
    pub concept: String,
    pub unit: String,
    pub period_start: Option<NaiveDate>,
    pub period_end: NaiveDate,
    pub accession: String,
    pub filed: NaiveDate,
    pub value: String,
}
impl From<FactEvidence> for CompanyFactEvidence {
    fn from(f: FactEvidence) -> Self {
        let (start, end) = dates(&f.period);
        let filing = FilingRef {
            cik: f.cik.clone(),
            accession: f.accession.clone(),
        };
        let filing_anchor = serialized_anchor(SubjectRef::Filing {
            filing: filing.clone(),
        });
        let anchor = CompanyFactSubject {
            cik: f.cik.clone(),
            taxonomy: f.taxonomy.clone(),
            concept: f.concept.clone(),
            period: f.period.clone(),
            unit: f.unit.clone(),
        }
        .reference()
        .ok()
        .and_then(|subject| serialized_anchor(SubjectRef::FiledFact { subject, filing }));
        Self {
            anchor,
            filing_anchor,
            fact_id: f.fact_id.into(),
            cik: f.cik,
            taxonomy: f.taxonomy,
            concept: f.concept,
            unit: f.unit,
            period_start: start,
            period_end: end,
            accession: f.accession,
            filed: f.filed,
            value: f.value.to_string(),
        }
    }
}
#[derive(SimpleObject)]
pub struct CompanyStatementLine {
    pub key: String,
    pub label: String,
    pub taxonomy: String,
    pub concept: String,
    pub period_start: Option<NaiveDate>,
    pub period_end: NaiveDate,
    pub unit: String,
    pub value: String,
    pub provenance: Vec<CompanyFactEvidence>,
    pub derivation: Option<String>,
    pub restated: bool,
    pub anchor: Option<String>,
    pub subject_anchor: Option<String>,
}
impl From<company_financials::FinancialStatementLine> for CompanyStatementLine {
    fn from(line: company_financials::FinancialStatementLine) -> Self {
        let (start, end) = dates(&line.period);
        let subject_anchor = line
            .provenance
            .first()
            .and_then(|f| {
                CompanyFactSubject {
                    cik: f.cik.clone(),
                    taxonomy: line.taxonomy.clone(),
                    concept: line.concept.clone(),
                    period: line.period.clone(),
                    unit: line.unit.clone(),
                }
                .reference()
                .ok()
            })
            .and_then(|subject| serialized_anchor(SubjectRef::Fact { subject }));
        let anchor = if line.derivation.is_none() && line.provenance.len() == 1 {
            let f = &line.provenance[0];
            let subject = CompanyFactSubject {
                cik: f.cik.clone(),
                taxonomy: f.taxonomy.clone(),
                concept: f.concept.clone(),
                period: f.period.clone(),
                unit: f.unit.clone(),
            };
            subject.reference().ok().and_then(|subject| {
                serde_json::to_string(&AnnotationAnchor::V1 {
                    target: SubjectRef::FiledFact {
                        subject,
                        filing: FilingRef {
                            cik: f.cik.clone(),
                            accession: f.accession.clone(),
                        },
                    },
                    chart_id: None,
                    observation_date: None,
                    observation_value: None,
                })
                .ok()
            })
        } else {
            None
        };
        Self {
            subject_anchor,
            key: line.key,
            label: line.label,
            taxonomy: line.taxonomy,
            concept: line.concept,
            period_start: start,
            period_end: end,
            unit: line.unit,
            value: line.value.to_string(),
            provenance: line.provenance.into_iter().map(Into::into).collect(),
            derivation: line
                .derivation
                .and_then(|d| serde_json::to_value(d).ok())
                .and_then(|v| v.as_str().map(str::to_owned)),
            restated: line.restated,
            anchor,
        }
    }
}
#[derive(SimpleObject)]
pub struct CompanyDerivationUnavailable {
    pub taxonomy: String,
    pub concept: String,
    pub unit: String,
    pub period_start: Option<NaiveDate>,
    pub period_end: NaiveDate,
    pub derivation: String,
    pub reason: String,
}
impl From<company_financials::DerivationUnavailable> for CompanyDerivationUnavailable {
    fn from(d: company_financials::DerivationUnavailable) -> Self {
        let (start, end) = dates(&d.period);
        Self {
            taxonomy: d.taxonomy,
            concept: d.concept,
            unit: d.unit,
            period_start: start,
            period_end: end,
            derivation: serde_json::to_value(d.derivation)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            reason: d.reason,
        }
    }
}
#[derive(SimpleObject)]
pub struct CompanyResearchStatement {
    pub unclassified_has_next_page: bool,
    pub unclassified_end_cursor: Option<String>,
    pub notice: String,
    pub unclassified_facts: Vec<CompanyStatementLine>,
    pub unavailable_derivations: Vec<CompanyDerivationUnavailable>,
    pub kind: CompanyStatementKind,
    pub lines: Vec<CompanyStatementLine>,
    pub periods: Vec<CompanyPeriod>,
}
/// Index evidence once by economic subject. Revision selection emits at most one
/// unclassified row per subject, so checking each selected subject's own bucket
/// visits at most the input evidence count overall, rather than rescanning every
/// issuer fact for every output row.
fn fact_history(facts: &[CompanyFact]) -> Result<BTreeMap<String, Vec<&CompanyFact>>> {
    let mut history: BTreeMap<String, Vec<&CompanyFact>> = BTreeMap::new();
    for fact in facts {
        let key = fact
            .subject
            .canonical_key()
            .map_err(|e| bad_user_input(e.to_string()))?;
        history.entry(key).or_default().push(fact);
    }
    Ok(history)
}

fn earlier_changed_value(
    fact: &CompanyFact,
    history: &BTreeMap<String, Vec<&CompanyFact>>,
) -> Result<bool> {
    let key = fact
        .subject
        .canonical_key()
        .map_err(|e| bad_user_input(e.to_string()))?;
    Ok(history.get(&key).is_some_and(|revisions| {
        revisions.iter().any(|prior| {
            (&prior.filed, &prior.accession) < (&fact.filed, &fact.accession)
                && prior.value != fact.value
        })
    }))
}

#[allow(clippy::too_many_arguments)]
pub async fn financial_statement(
    ctx: &Context<'_>,
    cik: String,
    kind: CompanyStatementKind,
    revision_input: Option<CompanyRevisionInput>,
    period_start: Option<NaiveDate>,
    period_end: Option<NaiveDate>,
    instant_only: bool,
    unclassified_first: Option<i32>,
    unclassified_after: Option<String>,
) -> Result<CompanyResearchStatement> {
    require_access(ctx)?;
    let pool = context::data_pool(ctx)?;
    let cik = checked_cik(&cik)?;
    let selection = revision(revision_input)?.1;
    if instant_only && period_start.is_some() {
        return Err(bad_user_input(
            "An instant filter cannot have a period start",
        ));
    }
    let (first, offset) = if unclassified_first == Some(0) {
        (0, 0)
    } else {
        page(
            unclassified_first.or(Some(100)),
            unclassified_after.as_deref(),
            250,
        )?
    };
    if let (Some(start), Some(end)) = (period_start, period_end) {
        period(Some(start), end)?;
    }
    let facts = service::calculation_facts(pool, &cik).await?;
    let mut lines = company_financials::normalized_statement(&facts, kind.into(), &selection)
        .map_err(|e| bad_user_input(e.to_string()))?;
    lines.retain(|l| {
        let (start, end) = dates(&l.period);
        period_start.is_none_or(|s| start == Some(s))
            && period_end.is_none_or(|e| end == e)
            && (!instant_only || start.is_none())
    });
    let mut periods = BTreeMap::new();
    for line in &lines {
        let (start, end) = dates(&line.period);
        periods.entry((end, start)).or_insert(CompanyPeriod {
            start,
            end,
            kind: if start.is_some() {
                "duration"
            } else {
                "instant"
            }
            .into(),
        });
    }
    let history = fact_history(&facts)?;
    let selected_unclassified =
        company_financials::unclassified_statement_facts(&facts, &selection)
            .map_err(|e| bad_user_input(e.to_string()))?
            .into_iter()
            .filter(|f| {
                let (start, end) = dates(&f.subject.period);
                period_start.is_none_or(|s| start == Some(s))
                    && period_end.is_none_or(|e| end == e)
                    && (!instant_only || start.is_none())
            })
            .collect::<Vec<_>>();
    // Period discovery is complete even when the caller omits or pages Other facts.
    for fact in &selected_unclassified {
        let (start, end) = dates(&fact.subject.period);
        periods.entry((end, start)).or_insert(CompanyPeriod {
            start,
            end,
            kind: if start.is_some() {
                "duration"
            } else {
                "instant"
            }
            .into(),
        });
    }
    let total = selected_unclassified.len() as i64;
    let unclassified_has_next_page = first > 0 && offset + first < total;
    let unclassified_end_cursor = unclassified_has_next_page.then(|| (offset + first).to_string());
    let unclassified_facts = selected_unclassified
        .into_iter()
        .skip(offset as usize)
        .take(first as usize)
        .map(|f| {
            let evidence = FactEvidence::from(&f);
            let restated = earlier_changed_value(&f, &history)?;
            Ok(company_financials::FinancialStatementLine {
                key: format!("{}:{}", f.subject.taxonomy, f.subject.concept),
                label: f.label.clone().unwrap_or_else(|| f.subject.concept.clone()),
                taxonomy: f.subject.taxonomy,
                concept: f.subject.concept,
                period: f.subject.period,
                unit: f.subject.unit,
                value: f.value,
                provenance: vec![evidence],
                derivation: None,
                restated,
            }
            .into())
        })
        .collect::<Result<Vec<CompanyStatementLine>>>()?;
    let unavailable_derivations =
        company_financials::statement_derivation_diagnostics(&facts, kind.into(), &selection)
            .map_err(|e| bad_user_input(e.to_string()))?
            .into_iter()
            .filter(|d| {
                let (start, end) = dates(&d.period);
                period_start.is_none_or(|s| start == Some(s))
                    && period_end.is_none_or(|e| end == e)
                    && (!instant_only || start.is_none())
            })
            .map(Into::into)
            .collect();
    Ok(CompanyResearchStatement{unclassified_has_next_page,unclassified_end_cursor,notice:"Normalized standard concepts from SEC companyfacts. Other reported facts remain available; original filing layouts, custom concepts and segments require as-filed research.".into(),unclassified_facts,unavailable_derivations,kind,lines:lines.into_iter().map(Into::into).collect(),periods:periods.into_values().rev().collect()})
}
#[derive(SimpleObject)]
pub struct CompanyFinancialRatio {
    pub name: String,
    pub value: Option<String>,
    pub unit: String,
    pub provenance: Vec<CompanyFactEvidence>,
    pub unavailable_reason: Option<String>,
}
impl From<company_financials::CompanyFinancialRatio> for CompanyFinancialRatio {
    fn from(r: company_financials::CompanyFinancialRatio) -> Self {
        Self {
            name: r.name,
            value: r.value.map(|v| v.to_string()),
            unit: r.unit,
            provenance: r.provenance.into_iter().map(Into::into).collect(),
            unavailable_reason: r.unavailable_reason,
        }
    }
}
pub async fn company_ratios(
    ctx: &Context<'_>,
    cik: String,
    period_start: Option<NaiveDate>,
    period_end: NaiveDate,
    revision_input: Option<CompanyRevisionInput>,
) -> Result<Vec<CompanyFinancialRatio>> {
    require_access(ctx)?;
    let pool = context::data_pool(ctx)?;
    let cik = checked_cik(&cik)?;
    let selection = revision(revision_input)?.1;
    let period = period(period_start, period_end)?;
    let facts = service::ratio_facts(pool, &cik, &period).await?;
    Ok(
        company_financials::calculate_ratios(&facts, &period, &selection)
            .map_err(|e| bad_user_input(e.to_string()))?
            .into_iter()
            .map(Into::into)
            .collect(),
    )
}
#[derive(SimpleObject)]
pub struct CompanyPeerComparison {
    pub company: CompanyResearchCompany,
    pub ratios: Vec<CompanyFinancialRatio>,
}
pub async fn company_peers(
    ctx: &Context<'_>,
    cik: String,
    period_start: Option<NaiveDate>,
    period_end: NaiveDate,
    revision_input: Option<CompanyRevisionInput>,
    first: Option<i32>,
) -> Result<Vec<CompanyPeerComparison>> {
    require_access(ctx)?;
    let pool = context::data_pool(ctx)?;
    let cik = checked_cik(&cik)?;
    let selection = revision(revision_input)?.1;
    let period = period(period_start, period_end)?;
    let (first, _) = page(first, None, 20)?;
    let peers = service::sic_peers(pool, &cik, first).await?;
    let mut results = Vec::new();
    for peer in peers {
        let facts = service::ratio_facts(pool, &peer.cik, &period).await?;
        let ratios = company_financials::calculate_ratios(&facts, &period, &selection)
            .map_err(|e| bad_user_input(e.to_string()))?;
        results.push(CompanyPeerComparison {
            company: peer.into(),
            ratios: ratios.into_iter().map(Into::into).collect(),
        });
    }
    Ok(results)
}

/// Authoritative target validation for notes. Access decisions happen before existence reads.
/// A note can later survive removal of its source; this check is only for new attachments.
pub async fn target_exists(ctx: &Context<'_>, anchor: &AnnotationAnchor) -> Result<bool> {
    require_access(ctx)?;
    let pool = context::data_pool(ctx)?;
    anchor
        .validate()
        .map_err(|e| bad_user_input(e.to_string()))?;
    let AnnotationAnchor::V1 { target, .. } = anchor;
    match target {
        SubjectRef::Company { cik } => Ok(service::company(pool, cik).await?.is_some()),
        SubjectRef::Filing { filing } => {
            Ok(service::filing_exists(pool, &filing.cik, &filing.accession).await?)
        }
        SubjectRef::Statement { filing, statement } => {
            let kind = match statement.as_str() {
                "income_statement" => StatementKind::IncomeStatement,
                "balance_sheet" => StatementKind::BalanceSheet,
                "cash_flow" => StatementKind::CashFlow,
                _ => return Ok(false),
            };
            let facts = service::calculation_facts(pool, &filing.cik).await?;
            Ok(!company_financials::normalized_statement(
                &facts,
                kind,
                &RevisionSelection::Accession(filing.accession.clone()),
            )
            .map_err(|e| bad_user_input(e.to_string()))?
            .is_empty())
        }
        SubjectRef::Fact { subject } => {
            let Ok(subject) = CompanyFactSubject::from_reference(subject) else {
                return Ok(false);
            };
            if service::fact_exists(pool, &subject, None).await? {
                return Ok(true);
            }
            // A derived line has a stable economic subject but no fabricated raw row.
            // Validate its current derivation against the retained filed operands.
            let facts = service::calculation_facts(pool, &subject.cik).await?;
            for kind in [
                StatementKind::IncomeStatement,
                StatementKind::BalanceSheet,
                StatementKind::CashFlow,
            ] {
                let lines = company_financials::normalized_statement(
                    &facts,
                    kind,
                    &RevisionSelection::Latest,
                )
                .map_err(|e| bad_user_input(e.to_string()))?;
                if lines.iter().any(|line| {
                    line.taxonomy == subject.taxonomy
                        && line.concept == subject.concept
                        && line.unit == subject.unit
                        && line.period == subject.period
                }) {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        SubjectRef::FiledFact { subject, filing } => {
            let Ok(subject) = CompanyFactSubject::from_reference(subject) else {
                return Ok(false);
            };
            Ok(service::fact_exists(pool, &subject, Some(&filing.accession)).await?)
        }
        _ => Ok(false),
    }
}
pub async fn company_research_target_exists(ctx: &Context<'_>, anchor: String) -> Result<bool> {
    require_access(ctx)?;
    if anchor.len() > 16_384 {
        return Err(bad_user_input("Research anchor is too long"));
    }
    let anchor = serde_json::from_str::<AnnotationAnchor>(&anchor)
        .map_err(|_| bad_user_input("Invalid research anchor"))?;
    target_exists(ctx, &anchor).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn revision_arguments_are_unambiguous() {
        assert!(revision(None).is_ok());
        assert!(revision(Some(CompanyRevisionInput {
            mode: CompanyRevisionMode::AsOf,
            as_of: None,
            accession: None
        }))
        .is_err());
        assert!(revision(Some(CompanyRevisionInput {
            mode: CompanyRevisionMode::Latest,
            as_of: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
            accession: None
        }))
        .is_err());
    }
    fn history_fact(concept: &str, accession: &str, filed: &str, value: &str) -> CompanyFact {
        CompanyFact::new(
            CompanyFactSubject {
                cik: "0000320193".into(),
                taxonomy: "us-gaap".into(),
                concept: concept.into(),
                period: ResearchPeriod::Instant {
                    date: "2024-12-31".parse().unwrap(),
                },
                unit: "USD".into(),
            },
            accession.into(),
            filed.parse().unwrap(),
            value.parse().unwrap(),
            "10-K".into(),
        )
        .unwrap()
    }

    #[test]
    fn indexed_restatement_history_preserves_chronology_and_exact_values() {
        let original = history_fact(
            "UnmappedConcept",
            "0000320193-25-000001",
            "2025-01-01",
            "10.00",
        );
        let unchanged = history_fact(
            "UnmappedConcept",
            "0000320193-25-000002",
            "2025-01-01",
            "10",
        );
        let changed = history_fact(
            "UnmappedConcept",
            "0000320193-25-000003",
            "2025-01-01",
            "11",
        );
        let reverted = history_fact(
            "UnmappedConcept",
            "0000320193-25-000004",
            "2025-01-02",
            "10",
        );
        let other = history_fact("OtherConcept", "0000320193-24-000001", "2024-01-01", "99");
        let mut unit = other.clone();
        unit.subject.concept = "UnmappedConcept".into();
        unit.subject.unit = "EUR".into();
        let mut duration = other.clone();
        duration.subject.concept = "UnmappedConcept".into();
        duration.subject.period = ResearchPeriod::Duration {
            start: "2024-01-01".parse().unwrap(),
            end: "2024-12-31".parse().unwrap(),
        };
        let mut issuer = other.clone();
        issuer.subject.concept = "UnmappedConcept".into();
        issuer.subject.cik = "0000789019".into();
        // Intentionally shuffled. Neither input order nor future revisions can
        // turn an original/unchanged filing into a restatement.
        let facts = vec![
            reverted.clone(),
            unit,
            duration,
            changed.clone(),
            issuer,
            unchanged.clone(),
            other,
            original.clone(),
        ];
        let index = fact_history(&facts).unwrap();
        assert_eq!(index.len(), 5);
        assert!(!earlier_changed_value(&original, &index).unwrap());
        assert!(!earlier_changed_value(&unchanged, &index).unwrap());
        assert!(earlier_changed_value(&changed, &index).unwrap());
        assert!(earlier_changed_value(&reverted, &index).unwrap());
    }

    #[test]
    fn many_unclassified_subjects_keep_disjoint_history_buckets() {
        let mut facts = Vec::new();
        for i in 0..4_000 {
            let concept = format!("UnmappedConcept{i}");
            facts.push(history_fact(
                &concept,
                "0000320193-25-000001",
                "2025-01-01",
                "1",
            ));
            facts.push(history_fact(
                &concept,
                "0000320193-25-000002",
                "2025-02-01",
                "1.0",
            ));
            facts.push(history_fact(
                &concept,
                "0000320193-25-000003",
                "2025-03-01",
                "2",
            ));
        }
        let index = fact_history(&facts).unwrap();
        assert_eq!(index.len(), 4_000);
        assert_eq!(index.values().map(Vec::len).sum::<usize>(), facts.len());
        assert!(index.values().all(|bucket| bucket.len() == 3));
        for facts in facts.chunks_exact(3) {
            assert!(!earlier_changed_value(&facts[0], &index).unwrap());
            assert!(!earlier_changed_value(&facts[1], &index).unwrap());
            assert!(earlier_changed_value(&facts[2], &index).unwrap());
        }
    }

    #[test]
    fn pagination_and_period_reject_invalid_arguments() {
        assert_eq!(page(None, None, 20).unwrap(), (20, 0));
        assert!(page(Some(0), None, 100).is_err());
        assert!(page(Some(101), None, 100).is_err());
        assert!(page(None, Some("-1"), 100).is_err());
        assert!(period(
            Some(NaiveDate::from_ymd_opt(2024, 2, 1).unwrap()),
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()
        )
        .is_err());
    }
}

#[cfg(test)]
#[path = "company_research_tests.rs"]
mod api_tests;
