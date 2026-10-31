//! Normalized SEC companyfacts views and conservative, reproducible calculations.
//!
//! Fiscal labels describe the filing, not necessarily the economic period. Calculations
//! therefore use actual period boundaries and units. Every result carries its selected
//! filed operands; derived values are never inserted into the source fact table.

use std::collections::{BTreeMap, BTreeSet};

use bigdecimal::BigDecimal;
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{company_fact::CompanyFact, ResearchPeriod};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", content = "value", rename_all = "snake_case")]
pub enum RevisionSelection {
    Latest,
    Original,
    AsOf(NaiveDate),
    Accession(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("company financial calculation: {0}")]
pub struct FinancialCalculationError(pub String);

/// Reject contradictory duplicates instead of choosing whichever row happened to load last.
pub fn select_fact_revisions(
    facts: &[CompanyFact],
    selection: &RevisionSelection,
) -> Result<Vec<CompanyFact>, FinancialCalculationError> {
    let mut revisions: BTreeMap<(String, String), &CompanyFact> = BTreeMap::new();
    for fact in facts {
        fact.validate()
            .map_err(|e| FinancialCalculationError(e.to_string()))?;
        let key = (
            fact.subject
                .canonical_key()
                .map_err(|e| FinancialCalculationError(e.to_string()))?,
            fact.accession.clone(),
        );
        if revisions.insert(key, fact).is_some_and(|existing| {
            existing.value != fact.value
                || existing.filed != fact.filed
                || existing.form != fact.form
        }) {
            return Err(FinancialCalculationError(
                "contradictory values or filing metadata for one subject/accession".into(),
            ));
        }
    }
    let mut selected: BTreeMap<String, &CompanyFact> = BTreeMap::new();
    for fact in revisions.into_values() {
        if matches!(selection, RevisionSelection::AsOf(cutoff) if fact.filed > *cutoff)
            || matches!(selection, RevisionSelection::Accession(accession) if fact.accession != *accession)
        {
            continue;
        }
        let key = fact
            .subject
            .canonical_key()
            .map_err(|e| FinancialCalculationError(e.to_string()))?;
        match selected.get(&key) {
            None => {
                selected.insert(key, fact);
            }
            Some(previous) => {
                let ordering =
                    (&fact.filed, &fact.accession).cmp(&(&previous.filed, &previous.accession));
                let replace = match selection {
                    RevisionSelection::Original => ordering.is_lt(),
                    _ => ordering.is_gt(),
                };
                if replace {
                    selected.insert(key, fact);
                }
            }
        }
    }
    Ok(selected.into_values().cloned().collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatementKind {
    IncomeStatement,
    BalanceSheet,
    CashFlow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandardConcept {
    pub key: &'static str,
    pub label: &'static str,
    /// Additive flow facts can be differenced or accumulated. EPS and weighted
    /// average shares deliberately do not satisfy this property.
    pub additive: bool,
}

/// One catalog emits both the statement matcher and SQL operand metadata.
/// Adding a ratio-relevant alias here automatically includes it in ratio reads.
macro_rules! standard_concept_catalog {
    ($( $kind:ident: [$($concept:literal),+] => ($key:literal, $label:literal, $additive:literal); )+) => {
        /// Normalized companyfacts concepts, not as-filed layouts. Unknown facts
        /// remain available through the separately paginated Other facts view.
        pub fn standard_concept(taxonomy:&str,concept:&str,kind:StatementKind)->Option<StandardConcept> {
            if !STANDARD_TAXONOMIES.contains(&taxonomy) {return None;}
            match (kind,concept) {
                $((StatementKind::$kind,$($concept)|+) => Some(StandardConcept{key:$key,label:$label,additive:$additive}),)+
                _=>None,
            }
        }
        /// Every raw alias required by the ratio definitions, derived from the
        /// same catalog as statement classification rather than a second list.
        pub fn ratio_input_concepts(period:&ResearchPeriod)->Vec<&'static str> {
            let definitions=ratio_definitions(period);
            let mut concepts=Vec::new();
            $(if definitions.iter().any(|(_,top,bottom,kind)| *kind==StatementKind::$kind && (*top==$key || *bottom==$key)) {
                concepts.extend([$($concept),+]);
            })+
            concepts.sort_unstable();concepts.dedup();concepts
        }
    };
}

/// Taxonomy families whose standard concepts have an explicit normalized mapping.
pub const STANDARD_TAXONOMIES: &[&str] = &["us-gaap", "ifrs-full"];

standard_concept_catalog! {
    IncomeStatement: ["RevenueFromContractWithCustomerExcludingAssessedTax","RevenueFromContractWithCustomerIncludingAssessedTax","Revenues","SalesRevenueNet","Revenue"] => ("revenue", "Revenue", true);
    IncomeStatement: ["CostOfRevenue","CostOfGoodsAndServicesSold","CostOfSales"] => ("cost_of_revenue", "Cost of revenue", true);
    IncomeStatement: ["GrossProfit"] => ("gross_profit", "Gross profit", true);
    IncomeStatement: ["OperatingExpenses"] => ("operating_expenses", "Operating expenses", true);
    IncomeStatement: ["OperatingCostsAndExpenses"] => ("operating_costs_and_expenses", "Operating costs and expenses", true);
    IncomeStatement: ["CostsAndExpenses"] => ("total_costs_and_expenses", "Total costs and expenses", true);
    IncomeStatement: ["NonoperatingIncomeExpense"] => ("nonoperating_income", "Nonoperating income and expense", true);
    IncomeStatement: ["IncomeLossFromEquityMethodInvestments"] => ("equity_method_income", "Income from equity-method investments", true);
    IncomeStatement: ["IncomeLossFromContinuingOperationsBeforeIncomeTaxesExtraordinaryItemsNoncontrollingInterest","ProfitLossBeforeTax"] => ("pretax_income", "Income before tax", true);
    IncomeStatement: ["ResearchAndDevelopmentExpense"] => ("research_and_development", "Research and development", true);
    IncomeStatement: ["SellingGeneralAndAdministrativeExpense"] => ("selling_general_administrative", "Selling, general and administrative", true);
    IncomeStatement: ["OperatingIncomeLoss","ProfitLossFromOperatingActivities"] => ("operating_income", "Operating income", true);
    IncomeStatement: ["InterestExpense"] => ("interest_expense", "Interest expense", true);
    IncomeStatement: ["IncomeTaxExpenseBenefit","IncomeTaxExpenseContinuingOperations"] => ("income_tax", "Income tax", true);
    IncomeStatement: ["NetIncomeLoss","ProfitLoss"] => ("net_income", "Net income", true);
    IncomeStatement: ["NetIncomeLossAvailableToCommonStockholdersBasic"] => ("net_income_common", "Net income available to common shareholders", true);
    IncomeStatement: ["EarningsPerShareBasic","BasicEarningsLossPerShare"] => ("eps_basic", "Basic earnings per share", false);
    IncomeStatement: ["EarningsPerShareDiluted","DilutedEarningsLossPerShare"] => ("eps_diluted", "Diluted earnings per share", false);
    IncomeStatement: ["WeightedAverageNumberOfSharesOutstandingBasic","WeightedAverageNumberOfSharesOutstanding"] => ("shares_basic", "Weighted average basic shares", false);
    IncomeStatement: ["WeightedAverageNumberOfDilutedSharesOutstanding","AdjustedWeightedAverageShares"] => ("shares_diluted", "Weighted average diluted shares", false);
    BalanceSheet: ["CashAndCashEquivalentsAtCarryingValue","CashAndCashEquivalents"] => ("cash", "Cash and cash equivalents", false);
    BalanceSheet: ["AccountsReceivableNetCurrent","TradeAndOtherCurrentReceivables"] => ("receivables", "Current receivables", false);
    BalanceSheet: ["InventoryNet","Inventories"] => ("inventory", "Inventory", false);
    BalanceSheet: ["AssetsCurrent","CurrentAssets"] => ("current_assets", "Current assets", false);
    BalanceSheet: ["AssetsNoncurrent","NoncurrentAssets"] => ("noncurrent_assets", "Noncurrent assets", false);
    BalanceSheet: ["LiabilitiesNoncurrent","NoncurrentLiabilities"] => ("noncurrent_liabilities", "Noncurrent liabilities", false);
    BalanceSheet: ["CommonStocksIncludingAdditionalPaidInCapital"] => ("common_stock_and_paid_in_capital", "Common stock and additional paid-in capital", false);
    BalanceSheet: ["RetainedEarningsAccumulatedDeficit","RetainedEarnings"] => ("retained_earnings", "Retained earnings or accumulated deficit", false);
    BalanceSheet: ["PropertyPlantAndEquipmentNet","PropertyPlantAndEquipment"] => ("property_plant_equipment", "Property, plant and equipment", false);
    BalanceSheet: ["Assets"] => ("assets", "Total assets", false);
    BalanceSheet: ["AccountsPayableCurrent","TradeAndOtherCurrentPayables"] => ("payables", "Current payables", false);
    BalanceSheet: ["LiabilitiesCurrent","CurrentLiabilities"] => ("current_liabilities", "Current liabilities", false);
    BalanceSheet: ["LongTermDebtCurrent"] => ("current_debt", "Current portion of long-term debt", false);
    BalanceSheet: ["LongTermDebtNoncurrent","LongTermDebt"] => ("long_term_debt", "Long-term debt", false);
    BalanceSheet: ["Liabilities"] => ("liabilities", "Total liabilities", false);
    BalanceSheet: ["StockholdersEquity","EquityAttributableToOwnersOfParent"] => ("equity", "Shareholders' equity", false);
    BalanceSheet: ["StockholdersEquityIncludingPortionAttributableToNoncontrollingInterest","Equity"] => ("total_equity", "Equity including noncontrolling interests", false);
    BalanceSheet: ["LiabilitiesAndStockholdersEquity","EquityAndLiabilities"] => ("liabilities_and_equity", "Total liabilities and equity", false);
    CashFlow: ["NetIncomeLoss","ProfitLoss"] => ("net_income", "Net income", true);
    CashFlow: ["DepreciationDepletionAndAmortization","DepreciationDepletionAndAmortizationPropertyPlantAndEquipment","AdjustmentsForDepreciationAndAmortisationExpense"] => ("depreciation_amortization", "Depreciation and amortization", true);
    CashFlow: ["PaymentsToAcquireAvailableForSaleSecuritiesDebt"] => ("debt_security_purchases", "Purchases of available-for-sale debt securities", true);
    CashFlow: ["ProceedsFromSaleOfAvailableForSaleSecuritiesDebt"] => ("debt_security_sale_proceeds", "Proceeds from sales of available-for-sale debt securities", true);
    CashFlow: ["IncomeTaxesPaidNet"] => ("income_taxes_paid_net", "Income taxes paid, net", true);
    CashFlow: ["IncomeTaxesPaid"] => ("income_taxes_paid", "Income taxes paid", true);
    CashFlow: ["InterestPaidNet"] => ("interest_paid_net", "Interest paid, net", true);
    CashFlow: ["ShareBasedCompensation"] => ("share_based_compensation", "Share-based compensation", true);
    CashFlow: ["NetCashProvidedByUsedInOperatingActivities","CashFlowsFromUsedInOperatingActivities"] => ("operating_cash_flow", "Net cash from operating activities", true);
    CashFlow: ["NetCashProvidedByUsedInInvestingActivities","CashFlowsFromUsedInInvestingActivities"] => ("investing_cash_flow", "Net cash from investing activities", true);
    CashFlow: ["NetCashProvidedByUsedInFinancingActivities","CashFlowsFromUsedInFinancingActivities"] => ("financing_cash_flow", "Net cash from financing activities", true);
    CashFlow: ["PaymentsToAcquirePropertyPlantAndEquipment","PurchaseOfPropertyPlantAndEquipmentClassifiedAsInvestingActivities"] => ("capital_expenditure", "Purchases of property, plant and equipment", true);
    CashFlow: ["PaymentsOfDividends","DividendsPaidClassifiedAsFinancingActivities"] => ("dividends_paid", "Dividends paid", true);
    CashFlow: ["PaymentsForRepurchaseOfCommonStock"] => ("share_repurchases", "Share repurchases", true);
    CashFlow: ["CashCashEquivalentsRestrictedCashAndRestrictedCashEquivalentsPeriodIncreaseDecreaseIncludingExchangeRateEffect"] => ("change_in_cash_including_restricted", "Change in cash including restricted cash", true);
    CashFlow: ["CashAndCashEquivalentsPeriodIncreaseDecreaseIncludingExchangeRateEffect","IncreaseDecreaseInCashAndCashEquivalents"] => ("change_in_cash", "Change in cash", true);
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactEvidence {
    pub fact_id: Uuid,
    pub accession: String,
    pub filed: NaiveDate,
    pub cik: String,
    pub taxonomy: String,
    pub concept: String,
    pub period: ResearchPeriod,
    pub unit: String,
    pub value: BigDecimal,
}

impl From<&CompanyFact> for FactEvidence {
    fn from(fact: &CompanyFact) -> Self {
        Self {
            fact_id: fact.id,
            accession: fact.accession.clone(),
            filed: fact.filed,
            cik: fact.subject.cik.clone(),
            taxonomy: fact.subject.taxonomy.clone(),
            concept: fact.subject.concept.clone(),
            period: fact.subject.period.clone(),
            unit: fact.subject.unit.clone(),
            value: fact.value.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DerivationKind {
    QuarterDifference,
    FourthQuarter,
    TrailingTwelveMonths,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinancialStatementLine {
    pub key: String,
    pub label: String,
    pub taxonomy: String,
    pub concept: String,
    pub period: ResearchPeriod,
    pub unit: String,
    pub value: BigDecimal,
    pub provenance: Vec<FactEvidence>,
    pub derivation: Option<DerivationKind>,
    pub restated: bool,
}

fn fact_line(
    fact: &CompanyFact,
    mapping: StandardConcept,
    restated: bool,
) -> FinancialStatementLine {
    FinancialStatementLine {
        key: mapping.key.into(),
        label: fact.label.clone().unwrap_or_else(|| mapping.label.into()),
        taxonomy: fact.subject.taxonomy.clone(),
        concept: fact.subject.concept.clone(),
        period: fact.subject.period.clone(),
        unit: fact.subject.unit.clone(),
        value: fact.value.clone(),
        provenance: vec![fact.into()],
        derivation: None,
        restated,
    }
}

fn duration(period: &ResearchPeriod) -> Option<(NaiveDate, NaiveDate)> {
    match period {
        ResearchPeriod::Duration { start, end } => Some((*start, *end)),
        _ => None,
    }
}

fn additive(fact: &CompanyFact) -> bool {
    [StatementKind::IncomeStatement, StatementKind::CashFlow]
        .iter()
        .any(|kind| {
            standard_concept(&fact.subject.taxonomy, &fact.subject.concept, *kind)
                .is_some_and(|m| m.additive)
        })
}

fn compatible_subject(left: &CompanyFact, right: &CompanyFact) -> bool {
    left.subject.cik == right.subject.cik
        && left.subject.taxonomy == right.subject.taxonomy
        && left.subject.concept == right.subject.concept
        && left.subject.unit == right.subject.unit
}

/// Difference cumulative periods selected under a common revision policy. Cross-filing
/// operands are explicit: an annual filing need not report its first nine months.
/// A later first-nine-months restatement cannot be subtracted from an older annual report.
pub fn derive_quarter(
    cumulative: &CompanyFact,
    previous: &CompanyFact,
    cutoff: NaiveDate,
) -> Result<FinancialStatementLine, FinancialCalculationError> {
    let fail = |reason: &str| FinancialCalculationError(reason.into());
    cumulative.validate().map_err(|e| fail(&e.to_string()))?;
    previous.validate().map_err(|e| fail(&e.to_string()))?;
    if !compatible_subject(cumulative, previous) || !additive(cumulative) {
        return Err(fail(
            "quarter derivation requires the same additive concept, issuer, taxonomy and unit",
        ));
    }
    if cumulative.filed > cutoff || previous.filed > cutoff || previous.filed > cumulative.filed {
        return Err(fail(
            "quarter operands must precede the common cutoff and cumulative filing",
        ));
    }
    let (start, end) = duration(&cumulative.subject.period)
        .ok_or_else(|| fail("quarter requires duration facts"))?;
    let (previous_start, previous_end) = duration(&previous.subject.period)
        .ok_or_else(|| fail("quarter requires duration facts"))?;
    let quarter_start = previous_end
        .checked_add_signed(Duration::days(1))
        .ok_or_else(|| fail("period boundary overflow"))?;
    let days = (end - quarter_start).num_days() + 1;
    let previous_days = (previous_end - start).num_days() + 1;
    let cumulative_days = (end - start).num_days() + 1;
    if start != previous_start
        || !(65..=110).contains(&days)
        || !(65..=290).contains(&previous_days)
        || !(150..=380).contains(&cumulative_days)
    {
        return Err(fail(
            "cumulative periods must share a start and differ by one fiscal quarter",
        ));
    }
    let fourth = (350..=380).contains(&cumulative_days) && (250..=290).contains(&previous_days);
    let mapping = standard_concept(
        &cumulative.subject.taxonomy,
        &cumulative.subject.concept,
        StatementKind::IncomeStatement,
    )
    .or_else(|| {
        standard_concept(
            &cumulative.subject.taxonomy,
            &cumulative.subject.concept,
            StatementKind::CashFlow,
        )
    })
    .unwrap();
    let mut line = fact_line(cumulative, mapping, false);
    line.period = ResearchPeriod::Duration {
        start: quarter_start,
        end,
    };
    line.value = &cumulative.value - &previous.value;
    line.provenance = vec![cumulative.into(), previous.into()];
    line.derivation = Some(if fourth {
        DerivationKind::FourthQuarter
    } else {
        DerivationKind::QuarterDifference
    });
    Ok(line)
}

/// Four actual, contiguous fiscal quarters; never sum EPS or weighted-average shares.
pub fn derive_ttm(
    quarters: &[FinancialStatementLine],
    cutoff: NaiveDate,
) -> Result<FinancialStatementLine, FinancialCalculationError> {
    let fail = |reason: &str| FinancialCalculationError(reason.into());
    if quarters.len() != 4 {
        return Err(fail("TTM needs four quarters"));
    }
    let mut sorted: Vec<_> = quarters.iter().collect();
    sorted.sort_by_key(|q| duration(&q.period).map(|(start, _)| start));
    let first = sorted[0];
    if ![StatementKind::IncomeStatement, StatementKind::CashFlow]
        .iter()
        .any(|kind| {
            standard_concept(&first.taxonomy, &first.concept, *kind).is_some_and(|m| m.additive)
        })
    {
        return Err(fail(
            "TTM requires an additive flow concept; EPS and weighted shares are unavailable",
        ));
    }
    let mut last_end: Option<NaiveDate> = None;
    let mut evidence = Vec::new();
    let mut value = BigDecimal::from(0);
    let issuer = first.provenance.first().map(|p| &p.cik);
    for quarter in &sorted {
        let (start, end) =
            duration(&quarter.period).ok_or_else(|| fail("TTM requires duration facts"))?;
        if !(65..=110).contains(&((end - start).num_days() + 1))
            || last_end.is_some_and(|previous| previous.succ_opt() != Some(start))
        {
            return Err(fail(
                "TTM quarters must be contiguous without gaps or overlap",
            ));
        }
        if quarter.taxonomy != first.taxonomy
            || quarter.concept != first.concept
            || quarter.unit != first.unit
            || quarter.provenance.is_empty()
            || quarter
                .provenance
                .iter()
                .any(|p| Some(&p.cik) != issuer || p.filed > cutoff)
            || quarter.derivation == Some(DerivationKind::TrailingTwelveMonths)
        {
            return Err(fail(
                "TTM operands must share issuer, concept, taxonomy, unit and revision cutoff",
            ));
        }
        last_end = Some(end);
        value += &quarter.value;
        evidence.extend(quarter.provenance.clone());
    }
    let start = duration(&first.period).unwrap().0;
    let end = last_end.unwrap();
    if !(350..=380).contains(&((end - start).num_days() + 1)) {
        return Err(fail("TTM must cover a fiscal year"));
    }
    evidence.sort_by_key(|p| p.fact_id);
    evidence.dedup_by_key(|p| p.fact_id);
    let mut result = first.clone();
    result.period = ResearchPeriod::Duration { start, end };
    result.value = value;
    result.provenance = evidence;
    result.derivation = Some(DerivationKind::TrailingTwelveMonths);
    result.restated = sorted.iter().any(|q| q.restated);
    Ok(result)
}

/// Source lines plus safe derived quarters and TTM. The caller must show derivation
/// labels and source provenance, and must not present this as the as-filed layout.
pub fn normalized_statement(
    facts: &[CompanyFact],
    kind: StatementKind,
    selection: &RevisionSelection,
) -> Result<Vec<FinancialStatementLine>, FinancialCalculationError> {
    let selected = select_fact_revisions(facts, selection)?;
    let issuers: BTreeSet<_> = selected.iter().map(|f| &f.subject.cik).collect();
    if issuers.len() > 1 {
        return Err(FinancialCalculationError(
            "statement requires one company".into(),
        ));
    }
    let mut historical: BTreeMap<String, Vec<&CompanyFact>> = BTreeMap::new();
    for fact in facts {
        historical
            .entry(
                fact.subject
                    .canonical_key()
                    .map_err(|e| FinancialCalculationError(e.to_string()))?,
            )
            .or_default()
            .push(fact);
    }
    let mut result = Vec::new();
    for fact in &selected {
        let Some(mapping) = standard_concept(&fact.subject.taxonomy, &fact.subject.concept, kind)
        else {
            continue;
        };
        if (kind == StatementKind::BalanceSheet)
            != matches!(fact.subject.period, ResearchPeriod::Instant { .. })
        {
            continue;
        }
        let restated = historical
            .get(
                &fact
                    .subject
                    .canonical_key()
                    .map_err(|e| FinancialCalculationError(e.to_string()))?,
            )
            .is_some_and(|history| {
                history.iter().any(|other| {
                    other.value != fact.value
                        && (other.filed, &other.accession) < (fact.filed, &fact.accession)
                })
            });
        result.push(fact_line(fact, mapping, restated));
    }
    if kind != StatementKind::BalanceSheet && !matches!(selection, RevisionSelection::Accession(_))
    {
        let cutoff = match selection {
            RevisionSelection::AsOf(date) => *date,
            _ => selected
                .iter()
                .map(|f| f.filed)
                .max()
                .unwrap_or(NaiveDate::MIN),
        };
        let mut cumulative_groups: BTreeMap<(&str, &str, &str, NaiveDate), Vec<&CompanyFact>> =
            BTreeMap::new();
        for fact in &selected {
            if let Some((start, _)) = duration(&fact.subject.period) {
                cumulative_groups
                    .entry((
                        &fact.subject.taxonomy,
                        &fact.subject.concept,
                        &fact.subject.unit,
                        start,
                    ))
                    .or_default()
                    .push(fact);
            }
        }
        for cumulative in &selected {
            if standard_concept(
                &cumulative.subject.taxonomy,
                &cumulative.subject.concept,
                kind,
            )
            .is_none()
            {
                continue;
            }
            let Some((start, _)) = duration(&cumulative.subject.period) else {
                continue;
            };
            let Some(previous_facts) = cumulative_groups.get(&(
                &*cumulative.subject.taxonomy,
                &*cumulative.subject.concept,
                &*cumulative.subject.unit,
                start,
            )) else {
                continue;
            };
            let candidates: Vec<_> = previous_facts
                .iter()
                .filter_map(|previous| {
                    derive_quarter(cumulative, previous, cutoff)
                        .ok()
                        .map(|line| (*previous, line))
                })
                .collect();
            // Two different cumulative boundaries can both look quarter-like. Neither
            // wins by input order; keep both source facts and leave derivation unavailable.
            if candidates.len() != 1 {
                continue;
            }
            let (previous, mut line) = candidates.into_iter().next().unwrap();
            line.restated = result.iter().any(|source| {
                source
                    .provenance
                    .iter()
                    .any(|p| p.fact_id == cumulative.id || p.fact_id == previous.id)
                    && source.restated
            });
            if !result.iter().any(|existing| {
                existing.concept == line.concept
                    && existing.taxonomy == line.taxonomy
                    && existing.unit == line.unit
                    && existing.period == line.period
            }) {
                result.push(line);
            }
        }

        let quarter_lines: Vec<_> = result
            .iter()
            .filter(|line| {
                duration(&line.period).is_some_and(|(start, end)| {
                    (65..=110).contains(&((end - start).num_days() + 1))
                })
            })
            .cloned()
            .collect();
        let mut quarter_groups: BTreeMap<(&str, &str, &str), Vec<FinancialStatementLine>> =
            BTreeMap::new();
        for line in &quarter_lines {
            quarter_groups
                .entry((&line.taxonomy, &line.concept, &line.unit))
                .or_default()
                .push(line.clone());
        }
        for mut candidates in quarter_groups.into_values() {
            candidates.sort_by_key(|line| duration(&line.period).unwrap().1);
            for window in candidates.windows(4) {
                if let Ok(ttm) = derive_ttm(window, cutoff) {
                    result.push(ttm);
                }
            }
        }
    }
    result.sort_by(|a, b| {
        (
            &a.key,
            &a.concept,
            &a.unit,
            serde_json::to_string(&a.period).unwrap(),
            a.derivation.map(|d| format!("{d:?}")),
        )
            .cmp(&(
                &b.key,
                &b.concept,
                &b.unit,
                serde_json::to_string(&b.period).unwrap(),
                b.derivation.map(|d| format!("{d:?}")),
            ))
    });
    Ok(result)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivationUnavailable {
    pub taxonomy: String,
    pub concept: String,
    pub unit: String,
    pub period: ResearchPeriod,
    pub derivation: DerivationKind,
    pub reason: String,
}

/// Facts that cannot be reliably assigned to a normalized statement. Present these
/// as an explicit "Other reported facts" view with the same chart/export affordances;
/// companyfacts does not contain the presentation linkbases needed to classify every
/// source concept. Returning these separately avoids pretending unknown concepts
/// belong to all three statements or silently dropping them.
pub fn unclassified_statement_facts(
    facts: &[CompanyFact],
    selection: &RevisionSelection,
) -> Result<Vec<CompanyFact>, FinancialCalculationError> {
    Ok(select_fact_revisions(facts, selection)?
        .into_iter()
        .filter(|fact| {
            ![
                StatementKind::IncomeStatement,
                StatementKind::BalanceSheet,
                StatementKind::CashFlow,
            ]
            .iter()
            .any(|kind| {
                standard_concept(&fact.subject.taxonomy, &fact.subject.concept, *kind).is_some()
                    && (*kind == StatementKind::BalanceSheet)
                        == matches!(fact.subject.period, ResearchPeriod::Instant { .. })
            })
        })
        .collect())
}

/// Explain unavailable annual-to-Q4 calculations rather than inventing zero inputs
/// or subtracting EPS/weighted shares. Reported values are unaffected.
pub fn statement_derivation_diagnostics(
    facts: &[CompanyFact],
    kind: StatementKind,
    selection: &RevisionSelection,
) -> Result<Vec<DerivationUnavailable>, FinancialCalculationError> {
    if kind == StatementKind::BalanceSheet {
        return Ok(Vec::new());
    }
    let selected = select_fact_revisions(facts, selection)?;
    let cutoff = match selection {
        RevisionSelection::AsOf(date) => *date,
        _ => selected
            .iter()
            .map(|f| f.filed)
            .max()
            .unwrap_or(NaiveDate::MIN),
    };
    let mut result = Vec::new();
    for annual in &selected {
        let Some(mapping) =
            standard_concept(&annual.subject.taxonomy, &annual.subject.concept, kind)
        else {
            continue;
        };
        let Some((start, end)) = duration(&annual.subject.period) else {
            continue;
        };
        if !(350..=380).contains(&((end - start).num_days() + 1)) {
            continue;
        }
        let candidates: Vec<_> = selected
            .iter()
            .filter(|f| {
                compatible_subject(annual, f)
                    && duration(&f.subject.period).is_some_and(|(s, e)| {
                        s == start && (250..=290).contains(&((e - s).num_days() + 1))
                    })
            })
            .collect();
        let reason = if matches!(selection, RevisionSelection::Accession(_)) {
            Some("derived periods are unavailable for accession-pinned reads".into())
        } else if !mapping.additive {
            Some("EPS and weighted-average shares cannot be derived by subtracting cumulative values".into())
        } else if candidates.len() != 1 {
            Some("Q4 requires exactly one compatible first-nine-months fact; missing or ambiguous input".into())
        } else {
            derive_quarter(annual, candidates[0], cutoff)
                .err()
                .map(|e| e.0)
        };
        if let Some(reason) = reason {
            result.push(DerivationUnavailable {
                taxonomy: annual.subject.taxonomy.clone(),
                concept: annual.subject.concept.clone(),
                unit: annual.subject.unit.clone(),
                period: annual.subject.period.clone(),
                derivation: DerivationKind::FourthQuarter,
                reason,
            });
        }
    }
    Ok(result)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompanyFinancialRatio {
    pub name: String,
    /// A dimensionless fraction: 0.25 means 25%, not 25.
    pub value: Option<BigDecimal>,
    pub unit: String,
    pub provenance: Vec<FactEvidence>,
    pub unavailable_reason: Option<String>,
}

fn ratio_definitions(
    period: &ResearchPeriod,
) -> &'static [(&'static str, &'static str, &'static str, StatementKind)] {
    match period {
        ResearchPeriod::Instant { .. } => &[
            (
                "current_ratio",
                "current_assets",
                "current_liabilities",
                StatementKind::BalanceSheet,
            ),
            (
                "liabilities_to_equity",
                "liabilities",
                "equity",
                StatementKind::BalanceSheet,
            ),
        ],
        ResearchPeriod::Duration { .. } => &[
            (
                "gross_margin",
                "gross_profit",
                "revenue",
                StatementKind::IncomeStatement,
            ),
            (
                "operating_margin",
                "operating_income",
                "revenue",
                StatementKind::IncomeStatement,
            ),
            (
                "net_margin",
                "net_income",
                "revenue",
                StatementKind::IncomeStatement,
            ),
        ],
    }
}

/// Ratios use actual economic periods and one currency. No market-price ratios
/// are invented. Missing or ambiguous source concepts remain explicitly unavailable.
pub fn calculate_ratios(
    facts: &[CompanyFact],
    period: &ResearchPeriod,
    selection: &RevisionSelection,
) -> Result<Vec<CompanyFinancialRatio>, FinancialCalculationError> {
    let facts = select_fact_revisions(facts, selection)?;
    if facts
        .iter()
        .map(|f| &f.subject.cik)
        .collect::<BTreeSet<_>>()
        .len()
        > 1
    {
        return Err(FinancialCalculationError(
            "ratios require one company".into(),
        ));
    }
    let definitions = ratio_definitions(period);
    let statement_kind = match period {
        ResearchPeriod::Instant { .. } => StatementKind::BalanceSheet,
        _ => StatementKind::IncomeStatement,
    };
    let lines = normalized_statement(&facts, statement_kind, selection)?;
    let mut results = Vec::new();
    for (name, numerator, denominator, _kind) in definitions {
        let inputs = |key: &str| {
            let mut matches: Vec<_> = lines
                .iter()
                .filter(|line| line.period == *period && line.key == key)
                .collect();
            // Reported annual values and a TTM ending at fiscal year-end can
            // coexist; prefer the reported view of that identical interval.
            if matches.iter().any(|line| line.derivation.is_none()) {
                matches.retain(|line| line.derivation.is_none());
            }
            matches
        };
        let top = inputs(numerator);
        let bottom = inputs(denominator);
        let mut result = CompanyFinancialRatio {
            name: (*name).into(),
            value: None,
            unit: "ratio".into(),
            provenance: Vec::new(),
            unavailable_reason: None,
        };
        if top.len() != 1 || bottom.len() != 1 {
            result.unavailable_reason = Some(format!(
                "requires one unambiguous {numerator} and {denominator} for the requested period"
            ));
        } else {
            let (top, bottom) = (top[0], bottom[0]);
            result.provenance = top
                .provenance
                .iter()
                .chain(&bottom.provenance)
                .cloned()
                .collect();
            if top.unit != bottom.unit || top.taxonomy != bottom.taxonomy {
                result.unavailable_reason =
                    Some("inputs must use the same taxonomy and unit".into());
            } else if bottom.value == 0 {
                result.unavailable_reason = Some("denominator is zero".into());
            } else {
                result.value = Some(&top.value / &bottom.value);
            }
        }
        results.push(result);
    }
    Ok(results)
}

#[cfg(test)]
mod tests;
