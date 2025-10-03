use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Comprehensive financial ratios for investment analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialRatios {
    // Profitability Ratios
    pub return_on_equity: Option<f64>,
    pub return_on_assets: Option<f64>,
    pub return_on_invested_capital: Option<f64>,
    pub gross_profit_margin: Option<f64>,
    pub operating_profit_margin: Option<f64>,
    pub net_profit_margin: Option<f64>,
    pub ebitda_margin: Option<f64>,
    pub free_cash_flow_margin: Option<f64>,

    // Liquidity Ratios
    pub current_ratio: Option<f64>,
    pub quick_ratio: Option<f64>,
    pub cash_ratio: Option<f64>,
    pub operating_cash_flow_ratio: Option<f64>,

    // Leverage Ratios
    pub debt_to_equity: Option<f64>,
    pub debt_to_assets: Option<f64>,
    pub interest_coverage: Option<f64>,
    pub debt_service_coverage: Option<f64>,
    pub equity_multiplier: Option<f64>,

    // Valuation Ratios (Traditional)
    pub price_to_earnings: Option<f64>,
    pub price_to_sales: Option<f64>,
    pub price_to_book: Option<f64>,
    pub peg_ratio: Option<f64>,

    // Enterprise Value Ratios (Modern/Analyst Preferred)
    pub enterprise_value_to_ebitda: Option<f64>,
    pub enterprise_value_to_sales: Option<f64>,
    pub enterprise_value_to_free_cash_flow: Option<f64>,

    // Cash Flow Ratios (Warren Buffett Favorites)
    pub free_cash_flow: Option<f64>,
    pub free_cash_flow_per_share: Option<f64>,
    pub free_cash_flow_yield: Option<f64>,
    pub cash_flow_return_on_investment: Option<f64>,
    pub cash_conversion_cycle: Option<f64>,

    // Growth Ratios
    pub revenue_growth_rate: Option<f64>,
    pub earnings_growth_rate: Option<f64>,
    pub free_cash_flow_growth_rate: Option<f64>,
    pub book_value_growth_rate: Option<f64>,

    // Metadata
    pub calculation_date: DateTime<Utc>,
    pub period_end_date: DateTime<Utc>,
    pub company_id: uuid::Uuid,
    pub statement_id: uuid::Uuid,
}

/// Market data required for valuation ratios
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub market_capitalization: f64,
    pub enterprise_value: f64,
    pub stock_price: f64,
    pub shares_outstanding: f64,
    pub total_debt: f64,
    pub cash_and_equivalents: f64,
    pub minority_interests: Option<f64>,
    pub preferred_stock: Option<f64>,
}

/// Financial statement data for ratio calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialStatementData {
    // Income Statement
    pub revenue: f64,
    pub cost_of_goods_sold: f64,
    pub gross_profit: f64,
    pub operating_income: f64,
    pub ebitda: f64,
    pub interest_expense: f64,
    pub net_income: f64,
    pub depreciation_amortization: f64,

    // Balance Sheet
    pub total_assets: f64,
    pub current_assets: f64,
    pub cash_and_equivalents: f64,
    pub inventory: f64,
    pub current_liabilities: f64,
    pub total_debt: f64,
    pub shareholders_equity: f64,
    pub accounts_receivable: f64,
    pub accounts_payable: f64,

    // Cash Flow Statement
    pub operating_cash_flow: f64,
    pub capital_expenditures: f64,
    pub free_cash_flow: f64,

    // Additional metrics
    pub shares_outstanding: f64,
    pub book_value_per_share: f64,
    pub earnings_per_share: f64,
}

/// Ratio calculation engine with educational context
pub struct FinancialRatioCalculator;

impl FinancialRatioCalculator {
    /// Calculate all financial ratios from statement and market data
    pub fn calculate_all_ratios(
        statement: &FinancialStatementData,
        market_data: &MarketData,
        company_id: uuid::Uuid,
        statement_id: uuid::Uuid,
        period_end_date: DateTime<Utc>,
    ) -> FinancialRatios {
        FinancialRatios {
            // Profitability Ratios
            return_on_equity: Self::calculate_roe(statement),
            return_on_assets: Self::calculate_roa(statement),
            return_on_invested_capital: Self::calculate_roic(statement),
            gross_profit_margin: Self::calculate_gross_profit_margin(statement),
            operating_profit_margin: Self::calculate_operating_profit_margin(statement),
            net_profit_margin: Self::calculate_net_profit_margin(statement),
            ebitda_margin: Self::calculate_ebitda_margin(statement),
            free_cash_flow_margin: Self::calculate_free_cash_flow_margin(statement),

            // Liquidity Ratios
            current_ratio: Self::calculate_current_ratio(statement),
            quick_ratio: Self::calculate_quick_ratio(statement),
            cash_ratio: Self::calculate_cash_ratio(statement),
            operating_cash_flow_ratio: Self::calculate_operating_cash_flow_ratio(statement),

            // Leverage Ratios
            debt_to_equity: Self::calculate_debt_to_equity(statement),
            debt_to_assets: Self::calculate_debt_to_assets(statement),
            interest_coverage: Self::calculate_interest_coverage(statement),
            debt_service_coverage: Self::calculate_debt_service_coverage(statement),
            equity_multiplier: Self::calculate_equity_multiplier(statement),

            // Valuation Ratios (Traditional)
            price_to_earnings: Self::calculate_pe_ratio(market_data, statement),
            price_to_sales: Self::calculate_ps_ratio(market_data, statement),
            price_to_book: Self::calculate_pb_ratio(market_data, statement),
            peg_ratio: None, // Requires growth rate calculation

            // Enterprise Value Ratios (Modern/Analyst Preferred)
            enterprise_value_to_ebitda: Self::calculate_ev_ebitda(market_data, statement),
            enterprise_value_to_sales: Self::calculate_ev_sales(market_data, statement),
            enterprise_value_to_free_cash_flow: Self::calculate_ev_fcf(market_data, statement),

            // Cash Flow Ratios (Warren Buffett Favorites)
            free_cash_flow: Some(statement.free_cash_flow),
            free_cash_flow_per_share: Self::calculate_fcf_per_share(statement),
            free_cash_flow_yield: Self::calculate_fcf_yield(market_data, statement),
            cash_flow_return_on_investment: Self::calculate_cfroi(statement),
            cash_conversion_cycle: None, // Requires additional data

            // Growth Ratios (require historical data)
            revenue_growth_rate: None,
            earnings_growth_rate: None,
            free_cash_flow_growth_rate: None,
            book_value_growth_rate: None,

            calculation_date: Utc::now(),
            period_end_date,
            company_id,
            statement_id,
        }
    }

    // Profitability Ratios
    fn calculate_roe(statement: &FinancialStatementData) -> Option<f64> {
        if statement.shareholders_equity > 0.0 {
            Some(statement.net_income / statement.shareholders_equity)
        } else {
            None
        }
    }

    fn calculate_roa(statement: &FinancialStatementData) -> Option<f64> {
        if statement.total_assets > 0.0 {
            Some(statement.net_income / statement.total_assets)
        } else {
            None
        }
    }

    fn calculate_roic(statement: &FinancialStatementData) -> Option<f64> {
        // NOPAT = Operating Income * (1 - Tax Rate)
        // For simplicity, using operating income as proxy for NOPAT
        let invested_capital = statement.total_debt + statement.shareholders_equity;
        if invested_capital > 0.0 {
            Some(statement.operating_income / invested_capital)
        } else {
            None
        }
    }

    fn calculate_gross_profit_margin(statement: &FinancialStatementData) -> Option<f64> {
        if statement.revenue > 0.0 {
            Some(statement.gross_profit / statement.revenue)
        } else {
            None
        }
    }

    fn calculate_operating_profit_margin(statement: &FinancialStatementData) -> Option<f64> {
        if statement.revenue > 0.0 {
            Some(statement.operating_income / statement.revenue)
        } else {
            None
        }
    }

    fn calculate_net_profit_margin(statement: &FinancialStatementData) -> Option<f64> {
        if statement.revenue > 0.0 {
            Some(statement.net_income / statement.revenue)
        } else {
            None
        }
    }

    fn calculate_ebitda_margin(statement: &FinancialStatementData) -> Option<f64> {
        if statement.revenue > 0.0 {
            Some(statement.ebitda / statement.revenue)
        } else {
            None
        }
    }

    fn calculate_free_cash_flow_margin(statement: &FinancialStatementData) -> Option<f64> {
        if statement.revenue > 0.0 {
            Some(statement.free_cash_flow / statement.revenue)
        } else {
            None
        }
    }

    // Liquidity Ratios
    fn calculate_current_ratio(statement: &FinancialStatementData) -> Option<f64> {
        if statement.current_liabilities > 0.0 {
            Some(statement.current_assets / statement.current_liabilities)
        } else {
            None
        }
    }

    fn calculate_quick_ratio(statement: &FinancialStatementData) -> Option<f64> {
        if statement.current_liabilities > 0.0 {
            let quick_assets = statement.current_assets - statement.inventory;
            Some(quick_assets / statement.current_liabilities)
        } else {
            None
        }
    }

    fn calculate_cash_ratio(statement: &FinancialStatementData) -> Option<f64> {
        if statement.current_liabilities > 0.0 {
            Some(statement.cash_and_equivalents / statement.current_liabilities)
        } else {
            None
        }
    }

    fn calculate_operating_cash_flow_ratio(statement: &FinancialStatementData) -> Option<f64> {
        if statement.current_liabilities > 0.0 {
            Some(statement.operating_cash_flow / statement.current_liabilities)
        } else {
            None
        }
    }

    // Leverage Ratios
    fn calculate_debt_to_equity(statement: &FinancialStatementData) -> Option<f64> {
        if statement.shareholders_equity > 0.0 {
            Some(statement.total_debt / statement.shareholders_equity)
        } else {
            None
        }
    }

    fn calculate_debt_to_assets(statement: &FinancialStatementData) -> Option<f64> {
        if statement.total_assets > 0.0 {
            Some(statement.total_debt / statement.total_assets)
        } else {
            None
        }
    }

    fn calculate_interest_coverage(statement: &FinancialStatementData) -> Option<f64> {
        if statement.interest_expense > 0.0 {
            Some(statement.operating_income / statement.interest_expense)
        } else {
            None
        }
    }

    fn calculate_debt_service_coverage(statement: &FinancialStatementData) -> Option<f64> {
        // Simplified: using operating cash flow as proxy for debt service capability
        if statement.total_debt > 0.0 {
            Some(statement.operating_cash_flow / statement.total_debt)
        } else {
            None
        }
    }

    fn calculate_equity_multiplier(statement: &FinancialStatementData) -> Option<f64> {
        if statement.shareholders_equity > 0.0 {
            Some(statement.total_assets / statement.shareholders_equity)
        } else {
            None
        }
    }

    // Valuation Ratios (Traditional)
    fn calculate_pe_ratio(
        market_data: &MarketData,
        statement: &FinancialStatementData,
    ) -> Option<f64> {
        if statement.earnings_per_share > 0.0 {
            Some(market_data.stock_price / statement.earnings_per_share)
        } else {
            None
        }
    }

    fn calculate_ps_ratio(
        market_data: &MarketData,
        statement: &FinancialStatementData,
    ) -> Option<f64> {
        if statement.revenue > 0.0 {
            Some(market_data.market_capitalization / statement.revenue)
        } else {
            None
        }
    }

    fn calculate_pb_ratio(
        market_data: &MarketData,
        statement: &FinancialStatementData,
    ) -> Option<f64> {
        if statement.book_value_per_share > 0.0 {
            Some(market_data.stock_price / statement.book_value_per_share)
        } else {
            None
        }
    }

    // Enterprise Value Ratios (Modern/Analyst Preferred)
    fn calculate_ev_ebitda(
        market_data: &MarketData,
        statement: &FinancialStatementData,
    ) -> Option<f64> {
        if statement.ebitda > 0.0 {
            Some(market_data.enterprise_value / statement.ebitda)
        } else {
            None
        }
    }

    fn calculate_ev_sales(
        market_data: &MarketData,
        statement: &FinancialStatementData,
    ) -> Option<f64> {
        if statement.revenue > 0.0 {
            Some(market_data.enterprise_value / statement.revenue)
        } else {
            None
        }
    }

    fn calculate_ev_fcf(
        market_data: &MarketData,
        statement: &FinancialStatementData,
    ) -> Option<f64> {
        if statement.free_cash_flow > 0.0 {
            Some(market_data.enterprise_value / statement.free_cash_flow)
        } else {
            None
        }
    }

    // Cash Flow Ratios (Warren Buffett Favorites)
    fn calculate_fcf_per_share(statement: &FinancialStatementData) -> Option<f64> {
        if statement.shares_outstanding > 0.0 {
            Some(statement.free_cash_flow / statement.shares_outstanding)
        } else {
            None
        }
    }

    fn calculate_fcf_yield(
        market_data: &MarketData,
        statement: &FinancialStatementData,
    ) -> Option<f64> {
        if market_data.market_capitalization > 0.0 {
            Some(statement.free_cash_flow / market_data.market_capitalization)
        } else {
            None
        }
    }

    fn calculate_cfroi(statement: &FinancialStatementData) -> Option<f64> {
        let invested_capital = statement.total_debt + statement.shareholders_equity;
        if invested_capital > 0.0 {
            Some(statement.free_cash_flow / invested_capital)
        } else {
            None
        }
    }
}

/// Ratio explanation and educational context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatioExplanation {
    pub ratio_name: String,
    pub formula: String,
    pub interpretation: String,
    pub investment_value: String,
    pub benchmark: BenchmarkData,
    pub educational_resources: Vec<EducationalResource>,
    pub related_ratios: Vec<String>,
    pub red_flags: Vec<String>,
    pub success_stories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkData {
    pub industry: String,
    pub average: f64,
    pub range: (f64, f64),
    pub percentile_25: f64,
    pub percentile_75: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EducationalResource {
    pub id: String,
    pub title: String,
    pub resource_type: String,
    pub difficulty: String,
    pub category: String,
    pub url: String,
    pub description: String,
    pub tags: Vec<String>,
    pub author: String,
    pub published_date: DateTime<Utc>,
    pub rating: f64,
    pub related_ratios: Vec<String>,
}

/// Educational content library for financial ratios
pub struct EducationalContentLibrary;

impl EducationalContentLibrary {
    /// Get comprehensive explanation for a financial ratio
    pub fn get_ratio_explanation(ratio_name: &str) -> Option<RatioExplanation> {
        match ratio_name {
            "return_on_equity" => Some(RatioExplanation {
                ratio_name: "Return on Equity (ROE)".to_string(),
                formula: "ROE = Net Income / Shareholders' Equity".to_string(),
                interpretation: "Measures how effectively a company uses shareholders' equity to generate profit".to_string(),
                investment_value: "Higher ROE indicates efficient capital utilization and strong business model. Warren Buffett looks for consistent ROE above 15%".to_string(),
                benchmark: BenchmarkData {
                    industry: "Technology".to_string(),
                    average: 0.15,
                    range: (0.05, 0.30),
                    percentile_25: 0.10,
                    percentile_75: 0.20,
                },
                educational_resources: vec![
                    EducationalResource {
                        id: "roe_cfi_guide".to_string(),
                        title: "ROE Analysis Guide".to_string(),
                        resource_type: "article".to_string(),
                        difficulty: "intermediate".to_string(),
                        category: "profitability".to_string(),
                        url: "https://corporatefinanceinstitute.com/resources/accounting/return-on-equity-roe/".to_string(),
                        description: "Comprehensive guide to understanding and calculating ROE".to_string(),
                        tags: vec!["roe".to_string(), "profitability".to_string(), "equity".to_string()],
                        author: "CFI".to_string(),
                        published_date: Utc::now(),
                        rating: 4.8,
                        related_ratios: vec!["return_on_assets".to_string(), "return_on_invested_capital".to_string()],
                    },
                ],
                related_ratios: vec!["return_on_assets".to_string(), "return_on_invested_capital".to_string()],
                red_flags: vec![
                    "ROE declining over time".to_string(),
                    "ROE significantly below industry average".to_string(),
                    "ROE driven by excessive debt rather than operational efficiency".to_string(),
                ],
                success_stories: vec![
                    "Apple's consistent ROE above 20% demonstrates efficient capital allocation".to_string(),
                    "Berkshire Hathaway's ROE of 15%+ over decades shows sustainable competitive advantages".to_string(),
                ],
            }),
            "enterprise_value_to_ebitda" => Some(RatioExplanation {
                ratio_name: "Enterprise Value to EBITDA (EV/EBITDA)".to_string(),
                formula: "EV/EBITDA = Enterprise Value / EBITDA".to_string(),
                interpretation: "The gold standard for valuation preferred by professional analysts".to_string(),
                investment_value: "Preferred over P/E because it eliminates capital structure differences, removes accounting differences in depreciation, and provides better cross-company comparability".to_string(),
                benchmark: BenchmarkData {
                    industry: "Technology".to_string(),
                    average: 20.0,
                    range: (10.0, 35.0),
                    percentile_25: 15.0,
                    percentile_75: 25.0,
                },
                educational_resources: vec![
                    EducationalResource {
                        id: "ev_ebitda_mckinsey".to_string(),
                        title: "McKinsey EV/EBITDA Analysis".to_string(),
                        resource_type: "article".to_string(),
                        difficulty: "advanced".to_string(),
                        category: "valuation".to_string(),
                        url: "https://www.mckinsey.com/capabilities/strategy-and-corporate-finance/our-insights/measuring-long-term-performance".to_string(),
                        description: "Professional-grade analysis of EV/EBITDA methodology".to_string(),
                        tags: vec!["ev_ebitda".to_string(), "valuation".to_string(), "enterprise_value".to_string()],
                        author: "McKinsey & Company".to_string(),
                        published_date: Utc::now(),
                        rating: 4.9,
                        related_ratios: vec!["enterprise_value_to_sales".to_string(), "enterprise_value_to_free_cash_flow".to_string()],
                    },
                ],
                related_ratios: vec!["enterprise_value_to_sales".to_string(), "enterprise_value_to_free_cash_flow".to_string()],
                red_flags: vec![
                    "EV/EBITDA significantly above industry average without justification".to_string(),
                    "EV/EBITDA declining rapidly over time".to_string(),
                    "EV/EBITDA based on projected rather than actual EBITDA".to_string(),
                ],
                success_stories: vec![
                    "Amazon's EV/EBITDA of 15-20x during growth phase was justified by massive reinvestment".to_string(),
                    "Microsoft's EV/EBITDA of 12-15x reflects strong recurring revenue and cash generation".to_string(),
                ],
            }),
            "free_cash_flow" => Some(RatioExplanation {
                ratio_name: "Free Cash Flow (FCF)".to_string(),
                formula: "FCF = Operating Cash Flow - Capital Expenditures".to_string(),
                interpretation: "The ultimate measure of business value according to Warren Buffett".to_string(),
                investment_value: "Shows the cash a company generates after accounting for cash outflows to support operations and maintain capital assets. This is the cash available for dividends, buybacks, and growth investments".to_string(),
                benchmark: BenchmarkData {
                    industry: "Technology".to_string(),
                    average: 0.15, // 15% of revenue
                    range: (0.05, 0.30),
                    percentile_25: 0.10,
                    percentile_75: 0.20,
                },
                educational_resources: vec![
                    EducationalResource {
                        id: "buffett_fcf_1986".to_string(),
                        title: "Warren Buffett on Free Cash Flow (1986 Letter)".to_string(),
                        resource_type: "article".to_string(),
                        difficulty: "intermediate".to_string(),
                        category: "cash_flow".to_string(),
                        url: "https://www.berkshirehathaway.com/letters/1986.html".to_string(),
                        description: "Buffett's seminal explanation of free cash flow and its importance".to_string(),
                        tags: vec!["free_cash_flow".to_string(), "warren_buffett".to_string(), "value_investing".to_string()],
                        author: "Warren Buffett".to_string(),
                        published_date: Utc::now(),
                        rating: 5.0,
                        related_ratios: vec!["free_cash_flow_yield".to_string(), "cash_flow_return_on_investment".to_string()],
                    },
                ],
                related_ratios: vec!["free_cash_flow_yield".to_string(), "cash_flow_return_on_investment".to_string()],
                red_flags: vec![
                    "Negative free cash flow for extended periods".to_string(),
                    "Free cash flow declining while revenue grows".to_string(),
                    "Free cash flow significantly below net income".to_string(),
                ],
                success_stories: vec![
                    "Apple generates $100B+ in free cash flow annually, enabling massive share buybacks".to_string(),
                    "Microsoft's consistent FCF growth supports dividend increases and strategic acquisitions".to_string(),
                ],
            }),
            _ => None,
        }
    }

    /// Get all available educational resources for a ratio
    pub fn get_educational_resources(ratio_name: &str) -> Vec<EducationalResource> {
        if let Some(explanation) = Self::get_ratio_explanation(ratio_name) {
            explanation.educational_resources
        } else {
            vec![]
        }
    }

    /// Get benchmark data for a ratio by industry
    pub fn get_benchmark_data(ratio_name: &str, _industry: &str) -> Option<BenchmarkData> {
        if let Some(explanation) = Self::get_ratio_explanation(ratio_name) {
            Some(explanation.benchmark)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_roe_calculation() {
        let statement = FinancialStatementData {
            net_income: 100.0,
            shareholders_equity: 1000.0,
            revenue: 1000.0,
            cost_of_goods_sold: 600.0,
            gross_profit: 400.0,
            operating_income: 200.0,
            ebitda: 250.0,
            interest_expense: 50.0,
            depreciation_amortization: 50.0,
            total_assets: 2000.0,
            current_assets: 500.0,
            cash_and_equivalents: 100.0,
            inventory: 200.0,
            current_liabilities: 300.0,
            total_debt: 500.0,
            accounts_receivable: 150.0,
            accounts_payable: 100.0,
            operating_cash_flow: 180.0,
            capital_expenditures: 50.0,
            free_cash_flow: 130.0,
            shares_outstanding: 100.0,
            book_value_per_share: 10.0,
            earnings_per_share: 1.0,
        };

        let roe = FinancialRatioCalculator::calculate_roe(&statement);
        assert_eq!(roe, Some(0.1)); // 10% ROE
    }

    #[test]
    fn test_ev_ebitda_calculation() {
        let statement = FinancialStatementData {
            ebitda: 250.0,
            revenue: 1000.0,
            cost_of_goods_sold: 600.0,
            gross_profit: 400.0,
            operating_income: 200.0,
            interest_expense: 50.0,
            net_income: 100.0,
            depreciation_amortization: 50.0,
            total_assets: 2000.0,
            current_assets: 500.0,
            cash_and_equivalents: 100.0,
            inventory: 200.0,
            current_liabilities: 300.0,
            total_debt: 500.0,
            shareholders_equity: 1000.0,
            accounts_receivable: 150.0,
            accounts_payable: 100.0,
            operating_cash_flow: 180.0,
            capital_expenditures: 50.0,
            free_cash_flow: 130.0,
            shares_outstanding: 100.0,
            book_value_per_share: 10.0,
            earnings_per_share: 1.0,
        };

        let market_data = MarketData {
            market_capitalization: 2000.0,
            enterprise_value: 2400.0, // Market Cap + Debt - Cash
            stock_price: 20.0,
            shares_outstanding: 100.0,
            total_debt: 500.0,
            cash_and_equivalents: 100.0,
            minority_interests: None,
            preferred_stock: None,
        };

        let ev_ebitda = FinancialRatioCalculator::calculate_ev_ebitda(&market_data, &statement);
        assert_eq!(ev_ebitda, Some(9.6)); // 2400 / 250 = 9.6x
    }

    #[test]
    fn test_free_cash_flow_calculation() {
        let statement = FinancialStatementData {
            operating_cash_flow: 180.0,
            capital_expenditures: 50.0,
            free_cash_flow: 130.0,
            revenue: 1000.0,
            cost_of_goods_sold: 600.0,
            gross_profit: 400.0,
            operating_income: 200.0,
            ebitda: 250.0,
            interest_expense: 50.0,
            net_income: 100.0,
            depreciation_amortization: 50.0,
            total_assets: 2000.0,
            current_assets: 500.0,
            cash_and_equivalents: 100.0,
            inventory: 200.0,
            current_liabilities: 300.0,
            total_debt: 500.0,
            shareholders_equity: 1000.0,
            accounts_receivable: 150.0,
            accounts_payable: 100.0,
            shares_outstanding: 100.0,
            book_value_per_share: 10.0,
            earnings_per_share: 1.0,
        };

        let fcf_per_share = FinancialRatioCalculator::calculate_fcf_per_share(&statement);
        assert_eq!(fcf_per_share, Some(1.3)); // 130 / 100 = 1.3 per share
    }
}
