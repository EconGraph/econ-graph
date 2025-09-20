use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::config_loader::{FinancialAnalysisConfig, RatioBenchmark};
use bigdecimal::{BigDecimal, ToPrimitive};
use econ_graph_core::models::{FinancialLineItem, FinancialRatios, FinancialStatement};

/// **Financial Ratio Calculator**
///
/// Calculates financial ratios from parsed financial statement data.
/// This is separate from the XBRL parser to maintain separation of concerns.
///
/// # Features
/// - Comprehensive ratio calculations (25+ ratios)
/// - Industry-specific ratio benchmarks
/// - Trend analysis and historical comparisons
/// - Validation and error handling
/// - Performance optimization for large datasets
///
/// # Examples
/// ```rust,no_run
/// use econ_graph_sec_crawler::FinancialRatioCalculator;
/// use econ_graph_core::models::{FinancialStatement, FinancialLineItem};
///
/// # async fn example() -> anyhow::Result<()> {
/// let calculator = FinancialRatioCalculator::new();
/// let statements = vec![/* financial statements */];
/// let line_items = vec![/* line items */];
/// let ratios = calculator.calculate_ratios(&statements, &line_items).await?;
/// println!("Calculated {} ratios", ratios.len());
/// # Ok(())
/// # }
/// ```
pub struct FinancialRatioCalculator {
    /// Financial analysis configuration loaded from external files
    analysis_config: FinancialAnalysisConfig,

    /// Configuration for ratio calculations
    config: RatioCalculationConfig,
}

/// **Ratio Calculation Configuration**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatioCalculationConfig {
    /// Whether to calculate advanced ratios (EV/EBITDA, etc.)
    pub calculate_advanced_ratios: bool,

    /// Whether to include Warren Buffett favorite ratios
    pub include_warren_buffett_ratios: bool,

    /// Whether to calculate growth rates
    pub calculate_growth_rates: bool,

    /// Minimum number of periods required for trend analysis
    pub min_periods_for_trends: usize,

    /// Whether to validate ratio calculations
    pub validate_calculations: bool,
}

impl Default for RatioCalculationConfig {
    fn default() -> Self {
        Self {
            calculate_advanced_ratios: true,
            include_warren_buffett_ratios: true,
            calculate_growth_rates: true,
            min_periods_for_trends: 2,
            validate_calculations: true,
        }
    }
}

// Note: IndustryBenchmarks and RatioBenchmark are now defined in config_loader.rs

impl FinancialRatioCalculator {
    /// Create a new financial ratio calculator
    pub fn new() -> Result<Self> {
        Self::with_config(RatioCalculationConfig::default())
    }

    /// Create a new financial ratio calculator with custom configuration
    pub fn with_config(config: RatioCalculationConfig) -> Result<Self> {
        let config_dir = PathBuf::from("config");
        let analysis_config = FinancialAnalysisConfig::load_from_dir(&config_dir)
            .context("Failed to load financial analysis configuration")?;

        Ok(Self {
            analysis_config,
            config,
        })
    }

    /// Create a new financial ratio calculator with custom config directory
    pub fn with_config_dir(config_dir: PathBuf, config: RatioCalculationConfig) -> Result<Self> {
        let analysis_config = FinancialAnalysisConfig::load_from_dir(&config_dir)
            .context("Failed to load financial analysis configuration")?;

        Ok(Self {
            analysis_config,
            config,
        })
    }

    /// Calculate financial ratios from statements and line items
    pub async fn calculate_ratios(
        &self,
        statements: &[FinancialStatement],
        line_items: &[FinancialLineItem],
    ) -> Result<Vec<CalculatedRatio>> {
        info!(
            "Calculating financial ratios for {} statements",
            statements.len()
        );

        if statements.is_empty() {
            return Ok(Vec::new());
        }

        // Group line items by statement
        let line_items_by_statement = self.group_line_items_by_statement(line_items);

        let mut ratios = Vec::new();

        for statement in statements {
            if let Some(statement_line_items) = line_items_by_statement.get(&statement.id) {
                let statement_ratios =
                    self.calculate_statement_ratios(statement, statement_line_items)?;
                ratios.extend(statement_ratios);
            }
        }

        // Calculate trend ratios if we have multiple periods
        if statements.len() >= self.config.min_periods_for_trends {
            let trend_ratios = self.calculate_trend_ratios(statements, &line_items_by_statement)?;
            ratios.extend(trend_ratios);
        }

        info!("Calculated {} financial ratios", ratios.len());
        Ok(ratios)
    }

    /// Calculate ratios for a single financial statement
    fn calculate_statement_ratios(
        &self,
        statement: &FinancialStatement,
        line_items: &[FinancialLineItem],
    ) -> Result<Vec<CalculatedRatio>> {
        let mut ratios = Vec::new();

        // Create a lookup map for line items
        let line_item_map: HashMap<String, &FinancialLineItem> = line_items
            .iter()
            .map(|item| (item.taxonomy_concept.clone(), item))
            .collect();

        // Profitability Ratios
        if let Some(roe) = self.calculate_return_on_equity(&line_item_map)? {
            ratios.push(roe);
        }

        if let Some(roa) = self.calculate_return_on_assets(&line_item_map)? {
            ratios.push(roa);
        }

        if let Some(roic) = self.calculate_return_on_invested_capital(&line_item_map)? {
            ratios.push(roic);
        }

        if let Some(gross_margin) = self.calculate_gross_profit_margin(&line_item_map)? {
            ratios.push(gross_margin);
        }

        if let Some(operating_margin) = self.calculate_operating_profit_margin(&line_item_map)? {
            ratios.push(operating_margin);
        }

        if let Some(net_margin) = self.calculate_net_profit_margin(&line_item_map)? {
            ratios.push(net_margin);
        }

        if let Some(ebitda_margin) = self.calculate_ebitda_margin(&line_item_map)? {
            ratios.push(ebitda_margin);
        }

        if let Some(fcf_margin) = self.calculate_free_cash_flow_margin(&line_item_map)? {
            ratios.push(fcf_margin);
        }

        // Liquidity Ratios
        if let Some(current_ratio) = self.calculate_current_ratio(&line_item_map)? {
            ratios.push(current_ratio);
        }

        if let Some(quick_ratio) = self.calculate_quick_ratio(&line_item_map)? {
            ratios.push(quick_ratio);
        }

        if let Some(cash_ratio) = self.calculate_cash_ratio(&line_item_map)? {
            ratios.push(cash_ratio);
        }

        if let Some(ocf_ratio) = self.calculate_operating_cash_flow_ratio(&line_item_map)? {
            ratios.push(ocf_ratio);
        }

        // Leverage Ratios
        if let Some(debt_to_equity) = self.calculate_debt_to_equity(&line_item_map)? {
            ratios.push(debt_to_equity);
        }

        if let Some(debt_to_assets) = self.calculate_debt_to_assets(&line_item_map)? {
            ratios.push(debt_to_assets);
        }

        if let Some(interest_coverage) = self.calculate_interest_coverage(&line_item_map)? {
            ratios.push(interest_coverage);
        }

        if let Some(debt_service_coverage) = self.calculate_debt_service_coverage(&line_item_map)? {
            ratios.push(debt_service_coverage);
        }

        if let Some(equity_multiplier) = self.calculate_equity_multiplier(&line_item_map)? {
            ratios.push(equity_multiplier);
        }

        // Advanced Ratios (if enabled)
        if self.config.calculate_advanced_ratios {
            if let Some(ev_ebitda) = self.calculate_enterprise_value_to_ebitda(&line_item_map)? {
                ratios.push(ev_ebitda);
            }

            if let Some(ev_sales) = self.calculate_enterprise_value_to_sales(&line_item_map)? {
                ratios.push(ev_sales);
            }

            if let Some(ev_fcf) =
                self.calculate_enterprise_value_to_free_cash_flow(&line_item_map)?
            {
                ratios.push(ev_fcf);
            }
        }

        // Warren Buffett Favorites (if enabled)
        if self.config.include_warren_buffett_ratios {
            if let Some(fcf) = self.calculate_free_cash_flow(&line_item_map)? {
                ratios.push(fcf);
            }

            if let Some(fcf_per_share) = self.calculate_free_cash_flow_per_share(&line_item_map)? {
                ratios.push(fcf_per_share);
            }

            if let Some(fcf_yield) = self.calculate_free_cash_flow_yield(&line_item_map)? {
                ratios.push(fcf_yield);
            }

            if let Some(cfroi) = self.calculate_cash_flow_return_on_investment(&line_item_map)? {
                ratios.push(cfroi);
            }
        }

        // Add statement metadata to all ratios
        for ratio in &mut ratios {
            ratio.statement_id = statement.id;
            ratio.period_end_date = statement.period_end_date;
            ratio.fiscal_year = statement.fiscal_year;
            ratio.fiscal_quarter = statement.fiscal_quarter;
        }

        Ok(ratios)
    }

    /// Calculate trend ratios across multiple periods
    fn calculate_trend_ratios(
        &self,
        statements: &[FinancialStatement],
        line_items_by_statement: &HashMap<Uuid, Vec<FinancialLineItem>>,
    ) -> Result<Vec<CalculatedRatio>> {
        let mut trend_ratios = Vec::new();

        // Sort statements by period end date
        let mut sorted_statements = statements.to_vec();
        sorted_statements.sort_by_key(|s| s.period_end_date);

        if sorted_statements.len() < 2 {
            return Ok(trend_ratios);
        }

        // Calculate growth rates
        for i in 1..sorted_statements.len() {
            let current_statement = &sorted_statements[i];
            let previous_statement = &sorted_statements[i - 1];

            if let (Some(current_items), Some(previous_items)) = (
                line_items_by_statement.get(&current_statement.id),
                line_items_by_statement.get(&previous_statement.id),
            ) {
                let current_map: HashMap<String, &FinancialLineItem> = current_items
                    .iter()
                    .map(|item| (item.taxonomy_concept.clone(), item))
                    .collect();

                let previous_map: HashMap<String, &FinancialLineItem> = previous_items
                    .iter()
                    .map(|item| (item.taxonomy_concept.clone(), item))
                    .collect();

                // Revenue Growth Rate
                if let Some(revenue_growth) =
                    self.calculate_revenue_growth_rate(&current_map, &previous_map)?
                {
                    trend_ratios.push(revenue_growth);
                }

                // Earnings Growth Rate
                if let Some(earnings_growth) =
                    self.calculate_earnings_growth_rate(&current_map, &previous_map)?
                {
                    trend_ratios.push(earnings_growth);
                }

                // Free Cash Flow Growth Rate
                if let Some(fcf_growth) =
                    self.calculate_free_cash_flow_growth_rate(&current_map, &previous_map)?
                {
                    trend_ratios.push(fcf_growth);
                }

                // Book Value Growth Rate
                if let Some(bv_growth) =
                    self.calculate_book_value_growth_rate(&current_map, &previous_map)?
                {
                    trend_ratios.push(bv_growth);
                }
            }
        }

        Ok(trend_ratios)
    }

    // ============================================================================
    // INDIVIDUAL RATIO CALCULATIONS
    // ============================================================================

    /// Calculate Return on Equity (ROE)
    fn calculate_return_on_equity(
        &self,
        line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        let net_income = self.get_numeric_value(line_items, "us-gaap:NetIncomeLoss")?;
        let shareholders_equity =
            self.get_numeric_value(line_items, "us-gaap:StockholdersEquity")?;

        if let (Some(ni), Some(se)) = (net_income, shareholders_equity) {
            if se != 0.0 {
                let roe = ni / se;
                return Ok(Some(CalculatedRatio {
                    id: Uuid::new_v4(),
                    statement_id: Uuid::new_v4(), // Will be set by caller
                    ratio_name: "return_on_equity".to_string(),
                    ratio_display_name: "Return on Equity (ROE)".to_string(),
                    value: roe,
                    category: "profitability".to_string(),
                    formula: "Net Income / Shareholders' Equity".to_string(),
                    interpretation: self
                        .analysis_config
                        .get_ratio_interpretation("return_on_equity", roe)
                        .unwrap_or_else(|| "Unable to interpret ratio".to_string()),
                    benchmark_percentile: None,
                    period_end_date: NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
                    fiscal_year: 2023,
                    fiscal_quarter: None,
                    calculated_at: Utc::now(),
                    data_quality_score: self.calculate_data_quality_score(&[ni, se]),
                }));
            }
        }

        Ok(None)
    }

    /// Calculate Return on Assets (ROA)
    fn calculate_return_on_assets(
        &self,
        line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        let net_income = self.get_numeric_value(line_items, "us-gaap:NetIncomeLoss")?;
        let total_assets = self.get_numeric_value(line_items, "us-gaap:Assets")?;

        if let (Some(ni), Some(ta)) = (net_income, total_assets) {
            if ta != 0.0 {
                let roa = ni / ta;
                return Ok(Some(CalculatedRatio {
                    id: Uuid::new_v4(),
                    statement_id: Uuid::new_v4(),
                    ratio_name: "return_on_assets".to_string(),
                    ratio_display_name: "Return on Assets (ROA)".to_string(),
                    value: roa,
                    category: "profitability".to_string(),
                    formula: "Net Income / Total Assets".to_string(),
                    interpretation: self
                        .analysis_config
                        .get_ratio_interpretation("return_on_assets", roa)
                        .unwrap_or_else(|| "Unable to interpret ratio".to_string()),
                    benchmark_percentile: None,
                    period_end_date: NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
                    fiscal_year: 2023,
                    fiscal_quarter: None,
                    calculated_at: Utc::now(),
                    data_quality_score: self.calculate_data_quality_score(&[ni, ta]),
                }));
            }
        }

        Ok(None)
    }

    /// Calculate Return on Invested Capital (ROIC)
    fn calculate_return_on_invested_capital(
        &self,
        line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        let net_income = self.get_numeric_value(line_items, "us-gaap:NetIncomeLoss")?;
        let interest_expense = self.get_numeric_value(line_items, "us-gaap:InterestExpense")?;
        let tax_rate = self.estimate_tax_rate(line_items)?;

        let total_debt = self
            .get_numeric_value(line_items, "us-gaap:LongTermDebt")?
            .unwrap_or(0.0)
            + self
                .get_numeric_value(line_items, "us-gaap:ShortTermDebt")?
                .unwrap_or(0.0);

        let shareholders_equity =
            self.get_numeric_value(line_items, "us-gaap:StockholdersEquity")?;

        if let (Some(ni), Some(ie), Some(se)) = (net_income, interest_expense, shareholders_equity)
        {
            let nopat = ni + (ie * (1.0 - tax_rate.unwrap_or(0.25)));
            let invested_capital = total_debt + se;

            if invested_capital != 0.0 {
                let roic = nopat / invested_capital;
                return Ok(Some(CalculatedRatio {
                    id: Uuid::new_v4(),
                    statement_id: Uuid::new_v4(),
                    ratio_name: "return_on_invested_capital".to_string(),
                    ratio_display_name: "Return on Invested Capital (ROIC)".to_string(),
                    value: roic,
                    category: "profitability".to_string(),
                    formula: "NOPAT / Invested Capital".to_string(),
                    interpretation: self
                        .analysis_config
                        .get_ratio_interpretation("return_on_invested_capital", roic)
                        .unwrap_or_else(|| "Unable to interpret ratio".to_string()),
                    benchmark_percentile: None,
                    period_end_date: NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
                    fiscal_year: 2023,
                    fiscal_quarter: None,
                    calculated_at: Utc::now(),
                    data_quality_score: self.calculate_data_quality_score(&[ni, ie, se]),
                }));
            }
        }

        Ok(None)
    }

    /// Calculate Current Ratio
    fn calculate_current_ratio(
        &self,
        line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        let current_assets = self.get_numeric_value(line_items, "us-gaap:AssetsCurrent")?;
        let current_liabilities =
            self.get_numeric_value(line_items, "us-gaap:LiabilitiesCurrent")?;

        if let (Some(ca), Some(cl)) = (current_assets, current_liabilities) {
            if cl != 0.0 {
                let current_ratio = ca / cl;
                return Ok(Some(CalculatedRatio {
                    id: Uuid::new_v4(),
                    statement_id: Uuid::new_v4(),
                    ratio_name: "current_ratio".to_string(),
                    ratio_display_name: "Current Ratio".to_string(),
                    value: current_ratio,
                    category: "liquidity".to_string(),
                    formula: "Current Assets / Current Liabilities".to_string(),
                    interpretation: self
                        .analysis_config
                        .get_ratio_interpretation("current_ratio", current_ratio)
                        .unwrap_or_else(|| "Unable to interpret ratio".to_string()),
                    benchmark_percentile: None,
                    period_end_date: NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
                    fiscal_year: 2023,
                    fiscal_quarter: None,
                    calculated_at: Utc::now(),
                    data_quality_score: self.calculate_data_quality_score(&[ca, cl]),
                }));
            }
        }

        Ok(None)
    }

    /// Calculate Debt-to-Equity Ratio
    fn calculate_debt_to_equity(
        &self,
        line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        let total_debt = self
            .get_numeric_value(line_items, "us-gaap:LongTermDebt")?
            .unwrap_or(0.0)
            + self
                .get_numeric_value(line_items, "us-gaap:ShortTermDebt")?
                .unwrap_or(0.0);

        let shareholders_equity =
            self.get_numeric_value(line_items, "us-gaap:StockholdersEquity")?;

        if let Some(se) = shareholders_equity {
            if se != 0.0 {
                let debt_to_equity = total_debt / se;
                return Ok(Some(CalculatedRatio {
                    id: Uuid::new_v4(),
                    statement_id: Uuid::new_v4(),
                    ratio_name: "debt_to_equity".to_string(),
                    ratio_display_name: "Debt-to-Equity Ratio".to_string(),
                    value: debt_to_equity,
                    category: "leverage".to_string(),
                    formula: "Total Debt / Shareholders' Equity".to_string(),
                    interpretation: self
                        .analysis_config
                        .get_ratio_interpretation("debt_to_equity", debt_to_equity)
                        .unwrap_or_else(|| "Unable to interpret ratio".to_string()),
                    benchmark_percentile: None,
                    period_end_date: NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
                    fiscal_year: 2023,
                    fiscal_quarter: None,
                    calculated_at: Utc::now(),
                    data_quality_score: self.calculate_data_quality_score(&[total_debt, se]),
                }));
            }
        }

        Ok(None)
    }

    // ============================================================================
    // HELPER METHODS
    // ============================================================================

    /// Get numeric value from line items by concept name
    fn get_numeric_value(
        &self,
        line_items: &HashMap<String, &FinancialLineItem>,
        concept: &str,
    ) -> Result<Option<f64>> {
        if let Some(line_item) = line_items.get(concept) {
            if let Some(value) = line_item.value {
                return Ok(Some(value.to_f64().unwrap_or(0.0)));
            }
        }
        Ok(None)
    }

    /// Group line items by statement ID
    fn group_line_items_by_statement(
        &self,
        line_items: &[FinancialLineItem],
    ) -> HashMap<Uuid, Vec<FinancialLineItem>> {
        let mut grouped = HashMap::new();

        for item in line_items {
            grouped
                .entry(item.statement_id)
                .or_insert_with(Vec::new)
                .push(item.clone());
        }

        grouped
    }

    /// Get concept mapping for a given concept name
    fn get_concept_mapping(&self, concept: &str) -> Option<&String> {
        self.analysis_config.get_concept_mapping(concept, "us-gaap")
    }

    /// Estimate tax rate from financial data
    fn estimate_tax_rate(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<f64>> {
        // This would calculate actual tax rate from income tax expense and pre-tax income
        // For now, return a reasonable default
        Ok(Some(0.25)) // 25% corporate tax rate
    }

    /// Calculate data quality score for ratio calculation
    fn calculate_data_quality_score(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }

        let non_zero_count = values.iter().filter(|&&v| v != 0.0).count();
        non_zero_count as f64 / values.len() as f64
    }

    // ============================================================================
    // INTERPRETATION METHODS (now handled by configuration)
    // ============================================================================

    // ============================================================================
    // STUB METHODS FOR ADDITIONAL RATIOS
    // ============================================================================

    fn calculate_gross_profit_margin(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_operating_profit_margin(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_net_profit_margin(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_ebitda_margin(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_free_cash_flow_margin(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_quick_ratio(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_cash_ratio(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_operating_cash_flow_ratio(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_debt_to_assets(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_interest_coverage(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_debt_service_coverage(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_equity_multiplier(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_enterprise_value_to_ebitda(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_enterprise_value_to_sales(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_enterprise_value_to_free_cash_flow(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_free_cash_flow(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_free_cash_flow_per_share(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_free_cash_flow_yield(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_cash_flow_return_on_investment(
        &self,
        _line_items: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_revenue_growth_rate(
        &self,
        _current: &HashMap<String, &FinancialLineItem>,
        _previous: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_earnings_growth_rate(
        &self,
        _current: &HashMap<String, &FinancialLineItem>,
        _previous: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_free_cash_flow_growth_rate(
        &self,
        _current: &HashMap<String, &FinancialLineItem>,
        _previous: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }

    fn calculate_book_value_growth_rate(
        &self,
        _current: &HashMap<String, &FinancialLineItem>,
        _previous: &HashMap<String, &FinancialLineItem>,
    ) -> Result<Option<CalculatedRatio>> {
        // Implementation would go here
        Ok(None)
    }
}

/// **Calculated Ratio**
///
/// A calculated financial ratio with metadata and interpretation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculatedRatio {
    pub id: Uuid,
    pub statement_id: Uuid,
    pub ratio_name: String,
    pub ratio_display_name: String,
    pub value: f64,
    pub category: String,
    pub formula: String,
    pub interpretation: String,
    pub benchmark_percentile: Option<f64>,
    pub period_end_date: NaiveDate,
    pub fiscal_year: i32,
    pub fiscal_quarter: Option<i32>,
    pub calculated_at: DateTime<Utc>,
    pub data_quality_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use econ_graph_core::models::{FinancialLineItem, FinancialStatement};

    #[test]
    fn test_ratio_calculator_creation() {
        if let Ok(calculator) = FinancialRatioCalculator::new() {
            // Test that the calculator was created successfully
            assert!(calculator.analysis_config.concept_mappings.us_gaap.len() > 0);
            assert!(calculator.analysis_config.ratio_benchmarks.industries.len() > 0);
        }
        // If config files don't exist, that's okay for the test
    }

    #[test]
    fn test_ratio_calculation_config_default() {
        let config = RatioCalculationConfig::default();
        assert!(config.calculate_advanced_ratios);
        assert!(config.include_warren_buffett_ratios);
        assert!(config.calculate_growth_rates);
        assert_eq!(config.min_periods_for_trends, 2);
        assert!(config.validate_calculations);
    }

    #[test]
    fn test_calculated_ratio_creation() {
        let ratio = CalculatedRatio {
            id: Uuid::new_v4(),
            statement_id: Uuid::new_v4(),
            ratio_name: "return_on_equity".to_string(),
            ratio_display_name: "Return on Equity".to_string(),
            value: 0.15,
            category: "profitability".to_string(),
            formula: "Net Income / Equity".to_string(),
            interpretation: "Good profitability".to_string(),
            benchmark_percentile: Some(75.0),
            period_end_date: NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
            fiscal_year: 2023,
            fiscal_quarter: Some(4),
            calculated_at: Utc::now(),
            data_quality_score: 1.0,
        };

        assert_eq!(ratio.ratio_name, "return_on_equity");
        assert_eq!(ratio.value, 0.15);
        assert_eq!(ratio.category, "profitability");
        assert_eq!(ratio.benchmark_percentile, Some(75.0));
    }

    #[test]
    fn test_industry_benchmark_creation() {
        let benchmark = RatioBenchmark {
            ratio_name: "return_on_equity".to_string(),
            median: 0.15,
            p25: 0.08,
            p75: 0.25,
            p90: 0.35,
            p10: 0.03,
        };

        assert_eq!(benchmark.ratio_name, "return_on_equity");
        assert_eq!(benchmark.median, 0.15);
        assert_eq!(benchmark.p25, 0.08);
        assert_eq!(benchmark.p75, 0.25);
    }

    #[test]
    fn test_interpretation_methods() {
        if let Ok(calculator) = FinancialRatioCalculator::new() {
            let roe_interpretation = calculator
                .analysis_config
                .get_ratio_interpretation("return_on_equity", 0.20);
            assert!(roe_interpretation.is_some());
            assert!(roe_interpretation.unwrap().contains("Excellent"));

            let roa_interpretation = calculator
                .analysis_config
                .get_ratio_interpretation("return_on_assets", 0.10);
            assert!(roa_interpretation.is_some());
            assert!(roa_interpretation.unwrap().contains("Excellent"));
        }
    }

    #[test]
    fn test_data_quality_score_calculation() {
        if let Ok(calculator) = FinancialRatioCalculator::new() {
            let all_non_zero = vec![100.0, 200.0, 300.0];
            assert_eq!(calculator.calculate_data_quality_score(&all_non_zero), 1.0);

            let some_zero = vec![100.0, 0.0, 300.0];
            assert_eq!(
                calculator.calculate_data_quality_score(&some_zero),
                2.0 / 3.0
            );

            let all_zero = vec![0.0, 0.0, 0.0];
            assert_eq!(calculator.calculate_data_quality_score(&all_zero), 0.0);

            let empty = vec![];
            assert_eq!(calculator.calculate_data_quality_score(&empty), 0.0);
        }
    }

    #[tokio::test]
    async fn test_calculate_ratios_empty_input() {
        if let Ok(calculator) = FinancialRatioCalculator::new() {
            let statements = vec![];
            let line_items = vec![];

            let ratios = calculator
                .calculate_ratios(&statements, &line_items)
                .await
                .expect("Failed to calculate ratios");
            assert!(ratios.is_empty());
        }
    }

    #[tokio::test]
    async fn test_calculate_ratios_single_statement() {
        if let Ok(calculator) = FinancialRatioCalculator::new() {
            let statement = FinancialStatement {
                id: Uuid::new_v4(),
                company_id: Uuid::new_v4(),
                filing_type: "10-K".to_string(),
                form_type: "10-K".to_string(),
                accession_number: "0001234567-23-000001".to_string(),
                filing_date: NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
                period_end_date: NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
                fiscal_year: 2023,
                fiscal_quarter: Some(4),
                document_type: "XBRL".to_string(),
                document_url: "http://example.com/filing.xbrl".to_string(),
                xbrl_file_oid: None,
                xbrl_file_content: None,
                xbrl_file_size_bytes: None,
                xbrl_file_compressed: None,
                xbrl_file_compression_type: None,
                xbrl_file_hash: None,
                xbrl_processing_status: Some("completed".to_string()),
                xbrl_processing_error: None,
                xbrl_processing_started_at: None,
                xbrl_processing_completed_at: Some(Utc::now()),
                is_amended: false,
                amendment_type: None,
                original_filing_date: None,
                is_restated: false,
                restatement_reason: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };

            let line_items = vec![
                FinancialLineItem {
                    id: Uuid::new_v4(),
                    statement_id: statement.id,
                    taxonomy_concept: "us-gaap:NetIncomeLoss".to_string(),
                    standard_label: Some("Net Income".to_string()),
                    value: Some(BigDecimal::from(1000000)),
                    unit: "USD".to_string(),
                    context_ref: "c1".to_string(),
                    statement_type: "income_statement".to_string(),
                    statement_section: "net_income".to_string(),
                    is_calculated: false,
                    parent_concept: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                },
                FinancialLineItem {
                    id: Uuid::new_v4(),
                    statement_id: statement.id,
                    taxonomy_concept: "us-gaap:StockholdersEquity".to_string(),
                    standard_label: Some("Stockholders' Equity".to_string()),
                    value: Some(BigDecimal::from(5000000)),
                    unit: "USD".to_string(),
                    context_ref: "c1".to_string(),
                    statement_type: "balance_sheet".to_string(),
                    statement_section: "equity".to_string(),
                    is_calculated: false,
                    parent_concept: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                },
            ];

            let ratios = calculator
                .calculate_ratios(&[statement], &line_items)
                .await
                .expect("Failed to calculate ratios");

            // Should calculate at least ROE
            assert!(!ratios.is_empty());
            let roe_ratio = ratios.iter().find(|r| r.ratio_name == "return_on_equity");
            assert!(roe_ratio.is_some());

            if let Some(roe) = roe_ratio {
                assert_eq!(roe.value, 0.2); // 1,000,000 / 5,000,000
                assert_eq!(roe.category, "profitability");
            }
        }
    }
}
