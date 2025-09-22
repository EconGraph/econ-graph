use async_graphql::{Context, Object, Result, SimpleObject, InputObject, Enum};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use crate::models::{
    Company, FinancialStatement, FinancialLineItem, FinancialRatios,
    FinancialAnnotation, AnnotationReply, AnnotationAssignment, AnnotationTemplate,
    MarketData, FinancialStatementData, RatioExplanation, EducationalResource,
    FinancialRatioCalculator, EducationalContentLibrary,
};
use crate::services::FinancialDataService;
use crate::database::DatabasePool;

/// Input for company search
#[derive(InputObject)]
pub struct CompanySearchInput {
    pub query: String,
    pub limit: Option<i32>,
}

/// Input for financial statement filtering
#[derive(InputObject)]
pub struct FinancialStatementFilter {
    pub company_id: Option<Uuid>,
    pub filing_type: Option<String>,
    pub form_type: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub fiscal_year: Option<i32>,
    pub fiscal_quarter: Option<i32>,
}

/// Input for annotation filtering
#[derive(InputObject)]
pub struct AnnotationFilter {
    pub statement_id: Option<Uuid>,
    pub line_item_id: Option<Uuid>,
    pub author_id: Option<Uuid>,
    pub annotation_type: Option<String>,
    pub status: Option<String>,
    pub is_private: Option<bool>,
}

/// Input for creating annotations
#[derive(InputObject)]
pub struct CreateAnnotationInput {
    pub statement_id: Uuid,
    pub line_item_id: Option<Uuid>,
    pub content: String,
    pub annotation_type: String,
    pub tags: Option<Vec<String>>,
    pub highlights: Option<serde_json::Value>,
    pub mentions: Option<Vec<Uuid>>,
    pub parent_annotation_id: Option<Uuid>,
    pub is_private: Option<bool>,
}

/// Input for market data
#[derive(InputObject)]
pub struct MarketDataInput {
    pub market_capitalization: f64,
    pub enterprise_value: f64,
    pub stock_price: f64,
    pub shares_outstanding: f64,
    pub total_debt: f64,
    pub cash_and_equivalents: f64,
    pub minority_interests: Option<f64>,
    pub preferred_stock: Option<f64>,
}

/// Financial health score
#[derive(SimpleObject)]
pub struct FinancialHealthScore {
    pub overall: f64,
    pub profitability: f64,
    pub liquidity: f64,
    pub leverage: f64,
    pub efficiency: f64,
    pub growth: f64,
    pub last_updated: DateTime<Utc>,
}

/// Key performance indicators
#[derive(SimpleObject)]
pub struct KeyPerformanceIndicators {
    pub revenue_growth: Option<f64>,
    pub earnings_growth: Option<f64>,
    pub free_cash_flow_growth: Option<f64>,
    pub return_on_equity: Option<f64>,
    pub return_on_invested_capital: Option<f64>,
    pub debt_to_equity: Option<f64>,
    pub current_ratio: Option<f64>,
    pub enterprise_value_to_ebitda: Option<f64>,
    pub free_cash_flow_yield: Option<f64>,
}

/// Financial trends data
#[derive(SimpleObject)]
pub struct FinancialTrends {
    pub revenue_trend: Vec<TrendDataPoint>,
    pub earnings_trend: Vec<TrendDataPoint>,
    pub free_cash_flow_trend: Vec<TrendDataPoint>,
    pub roe_trend: Vec<TrendDataPoint>,
    pub debt_trend: Vec<TrendDataPoint>,
}

#[derive(SimpleObject)]
pub struct TrendDataPoint {
    pub date: DateTime<Utc>,
    pub value: f64,
    pub period: String,
}

/// Peer comparison data
#[derive(SimpleObject)]
pub struct PeerComparison {
    pub industry: String,
    pub percentile: i32,
    pub benchmark: f64,
    pub company_value: f64,
    pub metric: String,
    pub peer_count: i32,
}

/// Team member presence
#[derive(SimpleObject)]
pub struct TeamMember {
    pub user_id: Uuid,
    pub name: String,
    pub is_online: bool,
    pub current_statement_id: Option<Uuid>,
    pub last_activity: DateTime<Utc>,
}

/// Highlight range for annotations
#[derive(SimpleObject)]
pub struct HighlightRange {
    pub start: i32,
    pub end: i32,
    pub color: String,
    pub label: Option<String>,
}

/// Assignment status enum
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum AssignmentStatus {
    Pending,
    InProgress,
    Completed,
    Overdue,
    Cancelled,
}

/// Assignment type enum
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum AssignmentType {
    Review,
    Analyze,
    Verify,
    Comment,
    Approve,
    Investigate,
    Compare,
    Benchmark,
    RiskAssessment,
    TrendAnalysis,
    RatioAnalysis,
    PeerComparison,
}

/// Annotation type enum
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum AnnotationType {
    Comment,
    Question,
    Concern,
    Insight,
    Risk,
    Opportunity,
    OneTimeItem,
    IndustryContext,
    RevenueGrowth,
    CostConcern,
    CashFlow,
    BalanceSheet,
    RatioAnalysis,
    PeerComparison,
    TrendAnalysis,
}

/// Extended Company type with financial data
#[Object]
impl Company {
    async fn id(&self) -> Uuid {
        self.id
    }

    async fn cik(&self) -> &str {
        &self.cik
    }

    async fn ticker(&self) -> Option<&str> {
        self.ticker.as_deref()
    }

    async fn name(&self) -> &str {
        &self.name
    }

    async fn industry(&self) -> Option<&str> {
        self.industry.as_deref()
    }

    async fn sector(&self) -> Option<&str> {
        self.sector.as_deref()
    }

    /// Get financial statements for this company
    async fn financial_statements(
        &self,
        ctx: &Context<'_>,
        filter: Option<FinancialStatementFilter>,
        limit: Option<i32>,
    ) -> Result<Vec<FinancialStatement>> {
        let pool = ctx.data::<DatabasePool>()?;
        let service = FinancialDataService::new(&pool.get().await?);

        let limit = limit.unwrap_or(10) as i64;
        service.get_financial_statements_by_company(self.id, limit)
            .map_err(|e| async_graphql::Error::new(e.to_string()))
    }

    /// Get financial health score
    async fn financial_health(&self, ctx: &Context<'_>) -> Result<FinancialHealthScore> {
        // TODO: Implement financial health calculation
        Ok(FinancialHealthScore {
            overall: 75.0,
            profitability: 80.0,
            liquidity: 70.0,
            leverage: 75.0,
            efficiency: 80.0,
            growth: 70.0,
            last_updated: Utc::now(),
        })
    }

    /// Get key performance indicators
    async fn key_metrics(&self, ctx: &Context<'_>) -> Result<KeyPerformanceIndicators> {
        // TODO: Implement KPI calculation
        Ok(KeyPerformanceIndicators {
            revenue_growth: Some(0.15),
            earnings_growth: Some(0.12),
            free_cash_flow_growth: Some(0.18),
            return_on_equity: Some(0.20),
            return_on_invested_capital: Some(0.15),
            debt_to_equity: Some(0.3),
            current_ratio: Some(2.5),
            enterprise_value_to_ebitda: Some(15.0),
            free_cash_flow_yield: Some(0.05),
        })
    }

    /// Get financial trends
    async fn trends(&self, ctx: &Context<'_>, periods: Option<i32>) -> Result<FinancialTrends> {
        // TODO: Implement trend analysis
        Ok(FinancialTrends {
            revenue_trend: vec![],
            earnings_trend: vec![],
            free_cash_flow_trend: vec![],
            roe_trend: vec![],
            debt_trend: vec![],
        })
    }

    /// Get peer comparison data
    async fn peer_comparison(&self, ctx: &Context<'_>) -> Result<Vec<PeerComparison>> {
        // TODO: Implement peer comparison
        Ok(vec![])
    }
}

/// Extended FinancialStatement type with annotations and ratios
#[Object]
impl FinancialStatement {
    async fn id(&self) -> Uuid {
        self.id
    }

    async fn company_id(&self) -> Uuid {
        self.company_id
    }

    async fn filing_type(&self) -> &str {
        &self.filing_type
    }

    async fn form_type(&self) -> &str {
        &self.form_type
    }

    async fn accession_number(&self) -> &str {
        &self.accession_number
    }

    async fn filing_date(&self) -> DateTime<Utc> {
        self.filing_date.and_hms_opt(0, 0, 0).unwrap().and_utc()
    }

    async fn period_end_date(&self) -> DateTime<Utc> {
        self.period_end_date.and_hms_opt(0, 0, 0).unwrap().and_utc()
    }

    async fn fiscal_year(&self) -> i32 {
        self.fiscal_year
    }

    async fn fiscal_quarter(&self) -> Option<i32> {
        self.fiscal_quarter
    }

    /// Get financial line items for this statement
    async fn line_items(&self, ctx: &Context<'_>) -> Result<Vec<FinancialLineItem>> {
        let pool = ctx.data::<DatabasePool>()?;
        let service = FinancialDataService::new(&pool.get().await?);

        service.get_financial_line_items_by_statement(self.id)
            .map_err(|e| async_graphql::Error::new(e.to_string()))
    }

    /// Get financial ratios for this statement
    async fn ratios(
        &self,
        ctx: &Context<'_>,
        market_data: Option<MarketDataInput>,
    ) -> Result<Option<FinancialRatios>> {
        if let Some(market_data) = market_data {
            // TODO: Get financial statement data and calculate ratios
            // This would require converting the statement to FinancialStatementData
            // and MarketData, then using FinancialRatioCalculator
            Ok(None)
        } else {
            Ok(None)
        }
    }

    /// Get annotations for this statement
    async fn annotations(
        &self,
        ctx: &Context<'_>,
        filter: Option<AnnotationFilter>,
    ) -> Result<Vec<FinancialAnnotation>> {
        let pool = ctx.data::<DatabasePool>()?;
        let service = FinancialDataService::new(&pool.get().await?);

        let filter = filter.unwrap_or_default();
        service.get_annotations_by_filter(filter)
            .map_err(|e| async_graphql::Error::new(e.to_string()))
    }

    /// Get annotation count
    async fn annotation_count(&self, ctx: &Context<'_>) -> Result<i32> {
        let pool = ctx.data::<DatabasePool>()?;
        let service = FinancialDataService::new(&pool.get().await?);

        let filter = crate::models::AnnotationFilter {
            statement_id: Some(self.id),
            ..Default::default()
        };

        let annotations = service.get_annotations_by_filter(filter)
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(annotations.len() as i32)
    }

    /// Get team presence (who's currently viewing/editing)
    async fn team_presence(&self, ctx: &Context<'_>) -> Result<Vec<TeamMember>> {
        // TODO: Implement real-time team presence tracking
        Ok(vec![])
    }

    /// Get assignments for this statement
    async fn assignments(&self, ctx: &Context<'_>) -> Result<Vec<AnnotationAssignment>> {
        let pool = ctx.data::<DatabasePool>()?;
        let service = FinancialDataService::new(&pool.get().await?);

        let filter = crate::models::AnnotationAssignmentFilter {
            statement_id: Some(self.id),
            ..Default::default()
        };

        service.get_assignments_by_filter(filter)
            .map_err(|e| async_graphql::Error::new(e.to_string()))
    }
}

/// Extended FinancialAnnotation type
#[Object]
impl FinancialAnnotation {
    async fn id(&self) -> Uuid {
        self.id
    }

    async fn statement_id(&self) -> Uuid {
        self.statement_id
    }

    async fn line_item_id(&self) -> Option<Uuid> {
        self.line_item_id
    }

    async fn author_id(&self) -> Uuid {
        self.author_id
    }

    async fn content(&self) -> &str {
        &self.content
    }

    async fn annotation_type(&self) -> &str {
        &self.annotation_type
    }

    async fn tags(&self) -> &Vec<String> {
        &self.tags
    }

    async fn highlights(&self) -> &serde_json::Value {
        &self.highlights
    }

    async fn mentions(&self) -> &Vec<Uuid> {
        &self.mentions
    }

    async fn parent_annotation_id(&self) -> Option<Uuid> {
        self.parent_annotation_id
    }

    async fn status(&self) -> &str {
        &self.status
    }

    async fn is_private(&self) -> bool {
        self.is_private
    }

    async fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    async fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    /// Get replies to this annotation
    async fn replies(&self, ctx: &Context<'_>) -> Result<Vec<AnnotationReply>> {
        let pool = ctx.data::<DatabasePool>()?;
        let service = FinancialDataService::new(&pool.get().await?);

        service.get_annotation_replies(self.id)
            .map_err(|e| async_graphql::Error::new(e.to_string()))
    }
}

/// Extended FinancialRatios type with educational context
#[Object]
impl FinancialRatios {
    async fn return_on_equity(&self) -> Option<f64> {
        self.return_on_equity
    }

    async fn return_on_assets(&self) -> Option<f64> {
        self.return_on_assets
    }

    async fn return_on_invested_capital(&self) -> Option<f64> {
        self.return_on_invested_capital
    }

    async fn gross_profit_margin(&self) -> Option<f64> {
        self.gross_profit_margin
    }

    async fn operating_profit_margin(&self) -> Option<f64> {
        self.operating_profit_margin
    }

    async fn net_profit_margin(&self) -> Option<f64> {
        self.net_profit_margin
    }

    async fn ebitda_margin(&self) -> Option<f64> {
        self.ebitda_margin
    }

    async fn free_cash_flow_margin(&self) -> Option<f64> {
        self.free_cash_flow_margin
    }

    async fn current_ratio(&self) -> Option<f64> {
        self.current_ratio
    }

    async fn quick_ratio(&self) -> Option<f64> {
        self.quick_ratio
    }

    async fn cash_ratio(&self) -> Option<f64> {
        self.cash_ratio
    }

    async fn operating_cash_flow_ratio(&self) -> Option<f64> {
        self.operating_cash_flow_ratio
    }

    async fn debt_to_equity(&self) -> Option<f64> {
        self.debt_to_equity
    }

    async fn debt_to_assets(&self) -> Option<f64> {
        self.debt_to_assets
    }

    async fn interest_coverage(&self) -> Option<f64> {
        self.interest_coverage
    }

    async fn debt_service_coverage(&self) -> Option<f64> {
        self.debt_service_coverage
    }

    async fn equity_multiplier(&self) -> Option<f64> {
        self.equity_multiplier
    }

    async fn price_to_earnings(&self) -> Option<f64> {
        self.price_to_earnings
    }

    async fn price_to_sales(&self) -> Option<f64> {
        self.price_to_sales
    }

    async fn price_to_book(&self) -> Option<f64> {
        self.price_to_book
    }

    async fn peg_ratio(&self) -> Option<f64> {
        self.peg_ratio
    }

    /// Enterprise Value to EBITDA - The gold standard for valuation
    async fn enterprise_value_to_ebitda(&self) -> Option<f64> {
        self.enterprise_value_to_ebitda
    }

    async fn enterprise_value_to_sales(&self) -> Option<f64> {
        self.enterprise_value_to_sales
    }

    async fn enterprise_value_to_free_cash_flow(&self) -> Option<f64> {
        self.enterprise_value_to_free_cash_flow
    }

    /// Free Cash Flow - Warren Buffett's favorite metric
    async fn free_cash_flow(&self) -> Option<f64> {
        self.free_cash_flow
    }

    async fn free_cash_flow_per_share(&self) -> Option<f64> {
        self.free_cash_flow_per_share
    }

    async fn free_cash_flow_yield(&self) -> Option<f64> {
        self.free_cash_flow_yield
    }

    async fn cash_flow_return_on_investment(&self) -> Option<f64> {
        self.cash_flow_return_on_investment
    }

    async fn cash_conversion_cycle(&self) -> Option<f64> {
        self.cash_conversion_cycle
    }

    async fn revenue_growth_rate(&self) -> Option<f64> {
        self.revenue_growth_rate
    }

    async fn earnings_growth_rate(&self) -> Option<f64> {
        self.earnings_growth_rate
    }

    async fn free_cash_flow_growth_rate(&self) -> Option<f64> {
        self.free_cash_flow_growth_rate
    }

    async fn book_value_growth_rate(&self) -> Option<f64> {
        self.book_value_growth_rate
    }

    async fn calculation_date(&self) -> DateTime<Utc> {
        self.calculation_date
    }

    async fn period_end_date(&self) -> DateTime<Utc> {
        self.period_end_date
    }

    async fn company_id(&self) -> Uuid {
        self.company_id
    }

    async fn statement_id(&self) -> Uuid {
        self.statement_id
    }
}

/// Root query for financial data
pub struct FinancialQuery;

#[Object]
impl FinancialQuery {
    /// Search for companies
    async fn search_companies(
        &self,
        ctx: &Context<'_>,
        input: CompanySearchInput,
    ) -> Result<Vec<Company>> {
        let pool = ctx.data::<DatabasePool>()?;
        let service = FinancialDataService::new(&pool.get().await?);

        let limit = input.limit.unwrap_or(10) as i64;
        service.search_companies(&input.query, limit)
            .map_err(|e| async_graphql::Error::new(e.to_string()))
    }

    /// Get company by CIK
    async fn company_by_cik(
        &self,
        ctx: &Context<'_>,
        cik: String,
    ) -> Result<Option<Company>> {
        let pool = ctx.data::<DatabasePool>()?;
        let service = FinancialDataService::new(&pool.get().await?);

        service.get_company_by_cik(&cik)
            .map_err(|e| async_graphql::Error::new(e.to_string()))
    }

    /// Get company by ticker
    async fn company_by_ticker(
        &self,
        ctx: &Context<'_>,
        ticker: String,
    ) -> Result<Option<Company>> {
        let pool = ctx.data::<DatabasePool>()?;
        let service = FinancialDataService::new(&pool.get().await?);

        service.get_company_by_ticker(&ticker)
            .map_err(|e| async_graphql::Error::new(e.to_string()))
    }

    /// Get financial statement by accession number
    async fn financial_statement_by_accession(
        &self,
        ctx: &Context<'_>,
        accession_number: String,
    ) -> Result<Option<FinancialStatement>> {
        let pool = ctx.data::<DatabasePool>()?;
        let service = FinancialDataService::new(&pool.get().await?);

        service.get_financial_statement_by_accession(&accession_number)
            .map_err(|e| async_graphql::Error::new(e.to_string()))
    }

    /// Get ratio explanation with educational context
    async fn ratio_explanation(
        &self,
        _ctx: &Context<'_>,
        ratio_name: String,
    ) -> Result<Option<RatioExplanation>> {
        Ok(EducationalContentLibrary::get_ratio_explanation(&ratio_name))
    }

    /// Get educational resources for a ratio
    async fn educational_resources(
        &self,
        _ctx: &Context<'_>,
        ratio_name: String,
    ) -> Result<Vec<EducationalResource>> {
        Ok(EducationalContentLibrary::get_educational_resources(&ratio_name))
    }

    /// Get benchmark data for a ratio
    async fn benchmark_data(
        &self,
        _ctx: &Context<'_>,
        ratio_name: String,
        industry: String,
    ) -> Result<Option<crate::models::BenchmarkData>> {
        Ok(EducationalContentLibrary::get_benchmark_data(&ratio_name, &industry))
    }
}
