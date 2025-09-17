// REQUIREMENT: Comprehensive catalog of economic time series for automated crawling
// PURPOSE: Define structured metadata for major economic indicators from FRED and BLS
// This enables systematic data collection across all key economic domains

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents the frequency at which economic data is published
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataFrequency {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    Annual,
}

/// Represents the seasonal adjustment status of economic data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SeasonalAdjustment {
    SeasonallyAdjusted,
    NotSeasonallyAdjusted,
    Both, // When both versions are available
}

/// Represents the data source provider
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DataSource {
    FRED,
    BLS,
    BEA,
    Census,
    Treasury,
}

/// Represents major economic categories for organization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EconomicCategory {
    GDP,
    Employment,
    Inflation,
    InterestRates,
    Money,
    Trade,
    Housing,
    Manufacturing,
    Consumer,
    Business,
    Government,
    International,
}

/// Comprehensive metadata for an economic time series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesDefinition {
    /// Unique identifier (e.g., FRED series ID or BLS series ID)
    pub id: String,

    /// Human-readable title
    pub title: String,

    /// Detailed description of what this series measures
    pub description: String,

    /// Data source provider
    pub source: DataSource,

    /// Publication frequency
    pub frequency: DataFrequency,

    /// Seasonal adjustment status
    pub seasonal_adjustment: SeasonalAdjustment,

    /// Units of measurement
    pub units: String,

    /// Primary economic category
    pub category: EconomicCategory,

    /// Additional tags for filtering and search
    pub tags: Vec<String>,

    /// Priority level for crawling (1=highest, 5=lowest)
    pub priority: u8,

    /// Whether this series is actively maintained
    pub is_active: bool,

    /// Expected data availability start date (YYYY-MM-DD)
    pub start_date: Option<String>,

    /// Any special notes about this series
    pub notes: Option<String>,
}

/// Complete catalog of economic time series to crawl
pub struct ComprehensiveSeriesCatalog {
    pub series: Vec<SeriesDefinition>,
}

impl ComprehensiveSeriesCatalog {
    /// Creates a new comprehensive catalog with all major economic indicators
    pub fn new() -> Self {
        let mut series = Vec::new();

        // GDP & Economic Growth Indicators
        series.extend(Self::gdp_indicators());

        // Employment & Labor Market Indicators
        series.extend(Self::employment_indicators());

        // Inflation & Price Indicators
        series.extend(Self::inflation_indicators());

        // Interest Rates & Monetary Policy
        series.extend(Self::interest_rate_indicators());

        // Money Supply & Banking
        series.extend(Self::money_supply_indicators());

        // International Trade
        series.extend(Self::trade_indicators());

        // Housing Market
        series.extend(Self::housing_indicators());

        // Manufacturing & Industrial Production
        series.extend(Self::manufacturing_indicators());

        // Consumer Spending & Confidence
        series.extend(Self::consumer_indicators());

        // Business Investment & Confidence
        series.extend(Self::business_indicators());

        // Government Finance
        series.extend(Self::government_indicators());

        // International Economic Indicators
        series.extend(Self::international_indicators());

        Self { series }
    }

    /// GDP and Economic Growth Indicators
    fn gdp_indicators() -> Vec<SeriesDefinition> {
        vec![
            SeriesDefinition {
                id: "GDPC1".to_string(),
                title: "Real Gross Domestic Product".to_string(),
                description: "Inflation-adjusted measure of the value of all goods and services produced in the economy".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Quarterly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Billions of Chained 2017 Dollars".to_string(),
                category: EconomicCategory::GDP,
                tags: vec!["gdp".to_string(), "real".to_string(), "output".to_string()],
                priority: 1,
                is_active: true,
                start_date: Some("1947-01-01".to_string()),
                notes: Some("Quarterly data, seasonally adjusted annual rate".to_string()),
            },
            SeriesDefinition {
                id: "GDP".to_string(),
                title: "Gross Domestic Product".to_string(),
                description: "Current dollar value of all goods and services produced in the economy".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Quarterly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Billions of Dollars".to_string(),
                category: EconomicCategory::GDP,
                tags: vec!["gdp".to_string(), "nominal".to_string(), "current dollar".to_string()],
                priority: 1,
                is_active: true,
                start_date: Some("1947-01-01".to_string()),
                notes: None,
            },
            SeriesDefinition {
                id: "A939RX0Q048SBEA".to_string(),
                title: "Real GDP Per Capita".to_string(),
                description: "Real GDP divided by population, measures economic output per person".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Quarterly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Chained 2017 Dollars".to_string(),
                category: EconomicCategory::GDP,
                tags: vec!["gdp".to_string(), "per capita".to_string(), "productivity".to_string()],
                priority: 2,
                is_active: true,
                start_date: Some("1947-01-01".to_string()),
                notes: None,
            },
            SeriesDefinition {
                id: "GDPPOT".to_string(),
                title: "Real Potential Gross Domestic Product".to_string(),
                description: "Estimate of the level of GDP that would be produced with full employment".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Quarterly,
                seasonal_adjustment: SeasonalAdjustment::NotSeasonallyAdjusted,
                units: "Billions of Chained 2017 Dollars".to_string(),
                category: EconomicCategory::GDP,
                tags: vec!["gdp".to_string(), "potential".to_string(), "full employment".to_string()],
                priority: 2,
                is_active: true,
                start_date: Some("1949-01-01".to_string()),
                notes: Some("CBO estimate of potential GDP".to_string()),
            },
        ]
    }

    /// Employment and Labor Market Indicators
    fn employment_indicators() -> Vec<SeriesDefinition> {
        vec![
            SeriesDefinition {
                id: "UNRATE".to_string(),
                title: "Unemployment Rate".to_string(),
                description: "Percentage of labor force that is unemployed and actively seeking employment".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Percent".to_string(),
                category: EconomicCategory::Employment,
                tags: vec!["unemployment".to_string(), "labor force".to_string(), "jobs".to_string()],
                priority: 1,
                is_active: true,
                start_date: Some("1948-01-01".to_string()),
                notes: None,
            },
            SeriesDefinition {
                id: "PAYEMS".to_string(),
                title: "All Employees, Total Nonfarm".to_string(),
                description: "Total number of employees on nonfarm payrolls".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Thousands of Persons".to_string(),
                category: EconomicCategory::Employment,
                tags: vec!["employment".to_string(), "payrolls".to_string(), "jobs".to_string()],
                priority: 1,
                is_active: true,
                start_date: Some("1939-01-01".to_string()),
                notes: Some("From the Bureau of Labor Statistics Employment Situation".to_string()),
            },
            SeriesDefinition {
                id: "CIVPART".to_string(),
                title: "Labor Force Participation Rate".to_string(),
                description: "Percentage of working-age population that is in the labor force".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Percent".to_string(),
                category: EconomicCategory::Employment,
                tags: vec!["labor force".to_string(), "participation".to_string(), "workforce".to_string()],
                priority: 2,
                is_active: true,
                start_date: Some("1948-01-01".to_string()),
                notes: None,
            },
            SeriesDefinition {
                id: "AHETPI".to_string(),
                title: "Average Hourly Earnings of Production and Nonsupervisory Employees, Total Private".to_string(),
                description: "Average hourly earnings for production and nonsupervisory employees".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Dollars per Hour".to_string(),
                category: EconomicCategory::Employment,
                tags: vec!["wages".to_string(), "earnings".to_string(), "compensation".to_string()],
                priority: 2,
                is_active: true,
                start_date: Some("1964-01-01".to_string()),
                notes: None,
            },
            SeriesDefinition {
                id: "ICSA".to_string(),
                title: "Initial Claims".to_string(),
                description: "Initial claims for unemployment insurance".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Weekly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Number".to_string(),
                category: EconomicCategory::Employment,
                tags: vec!["unemployment claims".to_string(), "layoffs".to_string(), "weekly".to_string()],
                priority: 2,
                is_active: true,
                start_date: Some("1967-01-07".to_string()),
                notes: Some("Weekly data, seasonally adjusted".to_string()),
            },
        ]
    }

    /// Inflation and Price Indicators
    fn inflation_indicators() -> Vec<SeriesDefinition> {
        vec![
            SeriesDefinition {
                id: "CPIAUCSL".to_string(),
                title: "Consumer Price Index for All Urban Consumers: All Items".to_string(),
                description: "Measure of the average change in prices paid by urban consumers for goods and services".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Index 1982-84=100".to_string(),
                category: EconomicCategory::Inflation,
                tags: vec!["cpi".to_string(), "inflation".to_string(), "prices".to_string()],
                priority: 1,
                is_active: true,
                start_date: Some("1947-01-01".to_string()),
                notes: None,
            },
            SeriesDefinition {
                id: "CPILFESL".to_string(),
                title: "Consumer Price Index for All Urban Consumers: All Items Less Food and Energy".to_string(),
                description: "Core CPI excluding volatile food and energy prices".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Index 1982-84=100".to_string(),
                category: EconomicCategory::Inflation,
                tags: vec!["core cpi".to_string(), "inflation".to_string(), "core prices".to_string()],
                priority: 1,
                is_active: true,
                start_date: Some("1957-01-01".to_string()),
                notes: Some("Excludes food and energy for less volatile measure".to_string()),
            },
            SeriesDefinition {
                id: "PCEPI".to_string(),
                title: "Personal Consumption Expenditures: Chain-type Price Index".to_string(),
                description: "Measure of prices paid by consumers, preferred by Federal Reserve for inflation targeting".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Index 2017=100".to_string(),
                category: EconomicCategory::Inflation,
                tags: vec!["pce".to_string(), "inflation".to_string(), "fed target".to_string()],
                priority: 1,
                is_active: true,
                start_date: Some("1959-01-01".to_string()),
                notes: Some("Federal Reserve's preferred inflation measure".to_string()),
            },
            SeriesDefinition {
                id: "PCEPILFE".to_string(),
                title: "Personal Consumption Expenditures Excluding Food and Energy (Chain-Type Price Index)".to_string(),
                description: "Core PCE price index excluding food and energy".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Index 2017=100".to_string(),
                category: EconomicCategory::Inflation,
                tags: vec!["core pce".to_string(), "inflation".to_string(), "fed target".to_string()],
                priority: 1,
                is_active: true,
                start_date: Some("1959-01-01".to_string()),
                notes: Some("Core PCE, Federal Reserve's preferred core inflation measure".to_string()),
            },
            SeriesDefinition {
                id: "PPIFIS".to_string(),
                title: "Producer Price Index by Commodity: Final Demand".to_string(),
                description: "Average change in selling prices received by domestic producers for their output".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Index Nov 2009=100".to_string(),
                category: EconomicCategory::Inflation,
                tags: vec!["ppi".to_string(), "producer prices".to_string(), "wholesale".to_string()],
                priority: 2,
                is_active: true,
                start_date: Some("2009-11-01".to_string()),
                notes: Some("Measures inflation at the producer level".to_string()),
            },
        ]
    }

    /// Interest Rates and Monetary Policy Indicators
    fn interest_rate_indicators() -> Vec<SeriesDefinition> {
        vec![
            SeriesDefinition {
                id: "FEDFUNDS".to_string(),
                title: "Federal Funds Effective Rate".to_string(),
                description:
                    "Interest rate at which depository institutions trade federal funds overnight"
                        .to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::NotSeasonallyAdjusted,
                units: "Percent".to_string(),
                category: EconomicCategory::InterestRates,
                tags: vec![
                    "fed funds".to_string(),
                    "interest rates".to_string(),
                    "monetary policy".to_string(),
                ],
                priority: 1,
                is_active: true,
                start_date: Some("1954-07-01".to_string()),
                notes: Some("Key policy interest rate set by Federal Reserve".to_string()),
            },
            SeriesDefinition {
                id: "DFF".to_string(),
                title: "Federal Funds Effective Rate (Daily)".to_string(),
                description: "Daily federal funds effective rate".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Daily,
                seasonal_adjustment: SeasonalAdjustment::NotSeasonallyAdjusted,
                units: "Percent".to_string(),
                category: EconomicCategory::InterestRates,
                tags: vec![
                    "fed funds".to_string(),
                    "daily".to_string(),
                    "monetary policy".to_string(),
                ],
                priority: 2,
                is_active: true,
                start_date: Some("1954-07-01".to_string()),
                notes: Some("Daily version of federal funds rate".to_string()),
            },
            SeriesDefinition {
                id: "GS10".to_string(),
                title: "10-Year Treasury Constant Maturity Rate".to_string(),
                description: "Yield on 10-year Treasury securities at constant maturity"
                    .to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Daily,
                seasonal_adjustment: SeasonalAdjustment::NotSeasonallyAdjusted,
                units: "Percent".to_string(),
                category: EconomicCategory::InterestRates,
                tags: vec![
                    "treasury".to_string(),
                    "10 year".to_string(),
                    "long term".to_string(),
                ],
                priority: 1,
                is_active: true,
                start_date: Some("1962-01-02".to_string()),
                notes: Some("Benchmark long-term interest rate".to_string()),
            },
            SeriesDefinition {
                id: "GS2".to_string(),
                title: "2-Year Treasury Constant Maturity Rate".to_string(),
                description: "Yield on 2-year Treasury securities at constant maturity".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Daily,
                seasonal_adjustment: SeasonalAdjustment::NotSeasonallyAdjusted,
                units: "Percent".to_string(),
                category: EconomicCategory::InterestRates,
                tags: vec![
                    "treasury".to_string(),
                    "2 year".to_string(),
                    "short term".to_string(),
                ],
                priority: 2,
                is_active: true,
                start_date: Some("1976-06-01".to_string()),
                notes: Some("Short-term Treasury rate".to_string()),
            },
            SeriesDefinition {
                id: "T10Y2Y".to_string(),
                title: "10-Year Treasury Constant Maturity Minus 2-Year Treasury Constant Maturity"
                    .to_string(),
                description: "Yield curve spread between 10-year and 2-year Treasury rates"
                    .to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Daily,
                seasonal_adjustment: SeasonalAdjustment::NotSeasonallyAdjusted,
                units: "Percent".to_string(),
                category: EconomicCategory::InterestRates,
                tags: vec![
                    "yield curve".to_string(),
                    "spread".to_string(),
                    "recession indicator".to_string(),
                ],
                priority: 2,
                is_active: true,
                start_date: Some("1976-06-01".to_string()),
                notes: Some("Yield curve spread, recession indicator when negative".to_string()),
            },
        ]
    }

    /// Money Supply and Banking Indicators
    fn money_supply_indicators() -> Vec<SeriesDefinition> {
        vec![
            SeriesDefinition {
                id: "M1SL".to_string(),
                title: "M1 Money Stock".to_string(),
                description: "Narrow measure of money supply including currency and checkable deposits".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Billions of Dollars".to_string(),
                category: EconomicCategory::Money,
                tags: vec!["money supply".to_string(), "m1".to_string(), "liquidity".to_string()],
                priority: 2,
                is_active: true,
                start_date: Some("1959-01-01".to_string()),
                notes: Some("Narrow money supply measure".to_string()),
            },
            SeriesDefinition {
                id: "M2SL".to_string(),
                title: "M2 Money Stock".to_string(),
                description: "Broad measure of money supply including M1 plus savings deposits and money market funds".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Billions of Dollars".to_string(),
                category: EconomicCategory::Money,
                tags: vec!["money supply".to_string(), "m2".to_string(), "broad money".to_string()],
                priority: 2,
                is_active: true,
                start_date: Some("1959-01-01".to_string()),
                notes: Some("Broad money supply measure".to_string()),
            },
        ]
    }

    /// International Trade Indicators
    fn trade_indicators() -> Vec<SeriesDefinition> {
        vec![
            SeriesDefinition {
                id: "BOPGSTB".to_string(),
                title: "Trade Balance: Goods and Services, Balance of Payments Basis".to_string(),
                description: "Difference between exports and imports of goods and services"
                    .to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Millions of Dollars".to_string(),
                category: EconomicCategory::Trade,
                tags: vec![
                    "trade balance".to_string(),
                    "exports".to_string(),
                    "imports".to_string(),
                ],
                priority: 2,
                is_active: true,
                start_date: Some("1992-01-01".to_string()),
                notes: Some("Monthly trade balance data".to_string()),
            },
            SeriesDefinition {
                id: "EXPGS".to_string(),
                title: "Exports of Goods and Services".to_string(),
                description: "Total exports of goods and services from the United States"
                    .to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Billions of Dollars".to_string(),
                category: EconomicCategory::Trade,
                tags: vec!["exports".to_string(), "international trade".to_string()],
                priority: 3,
                is_active: true,
                start_date: Some("1992-01-01".to_string()),
                notes: None,
            },
            SeriesDefinition {
                id: "IMPGS".to_string(),
                title: "Imports of Goods and Services".to_string(),
                description: "Total imports of goods and services to the United States".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Billions of Dollars".to_string(),
                category: EconomicCategory::Trade,
                tags: vec!["imports".to_string(), "international trade".to_string()],
                priority: 3,
                is_active: true,
                start_date: Some("1992-01-01".to_string()),
                notes: None,
            },
        ]
    }

    /// Housing Market Indicators
    fn housing_indicators() -> Vec<SeriesDefinition> {
        vec![
            SeriesDefinition {
                id: "HOUST".to_string(),
                title: "Housing Starts: Total New Privately Owned Housing Units Started"
                    .to_string(),
                description: "Number of new privately owned housing units started".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Thousands of Units".to_string(),
                category: EconomicCategory::Housing,
                tags: vec![
                    "housing starts".to_string(),
                    "construction".to_string(),
                    "real estate".to_string(),
                ],
                priority: 2,
                is_active: true,
                start_date: Some("1959-01-01".to_string()),
                notes: Some("Leading indicator of housing market activity".to_string()),
            },
            SeriesDefinition {
                id: "CSUSHPISA".to_string(),
                title: "S&P/Case-Shiller U.S. National Home Price Index".to_string(),
                description: "Measure of U.S. residential real estate prices".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Index Jan 2000=100".to_string(),
                category: EconomicCategory::Housing,
                tags: vec![
                    "home prices".to_string(),
                    "case shiller".to_string(),
                    "real estate".to_string(),
                ],
                priority: 2,
                is_active: true,
                start_date: Some("1987-01-01".to_string()),
                notes: Some("Widely followed home price index".to_string()),
            },
            SeriesDefinition {
                id: "MORTGAGE30US".to_string(),
                title: "30-Year Fixed Rate Mortgage Average in the United States".to_string(),
                description: "Average interest rate on 30-year fixed rate mortgages".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Weekly,
                seasonal_adjustment: SeasonalAdjustment::NotSeasonallyAdjusted,
                units: "Percent".to_string(),
                category: EconomicCategory::Housing,
                tags: vec!["mortgage rates".to_string(), "housing finance".to_string()],
                priority: 2,
                is_active: true,
                start_date: Some("1971-04-02".to_string()),
                notes: Some("From Freddie Mac Primary Mortgage Market Survey".to_string()),
            },
        ]
    }

    /// Manufacturing and Industrial Production Indicators
    fn manufacturing_indicators() -> Vec<SeriesDefinition> {
        vec![
            SeriesDefinition {
                id: "INDPRO".to_string(),
                title: "Industrial Production Index".to_string(),
                description: "Measure of real output of manufacturing, mining, and electric and gas utilities".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Index 2017=100".to_string(),
                category: EconomicCategory::Manufacturing,
                tags: vec!["industrial production".to_string(), "manufacturing".to_string(), "output".to_string()],
                priority: 2,
                is_active: true,
                start_date: Some("1919-01-01".to_string()),
                notes: Some("Federal Reserve measure of industrial output".to_string()),
            },
            SeriesDefinition {
                id: "TCU".to_string(),
                title: "Capacity Utilization: Total Industry".to_string(),
                description: "Percentage of total industrial capacity being utilized".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Percent of Capacity".to_string(),
                category: EconomicCategory::Manufacturing,
                tags: vec!["capacity utilization".to_string(), "manufacturing".to_string(), "efficiency".to_string()],
                priority: 3,
                is_active: true,
                start_date: Some("1967-01-01".to_string()),
                notes: Some("Measure of how fully industrial capacity is being used".to_string()),
            },
            SeriesDefinition {
                id: "NAPM".to_string(),
                title: "ISM Manufacturing: PMI Composite Index".to_string(),
                description: "Purchasing Managers' Index for manufacturing sector".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::NotSeasonallyAdjusted,
                units: "Index".to_string(),
                category: EconomicCategory::Manufacturing,
                tags: vec!["pmi".to_string(), "manufacturing".to_string(), "business conditions".to_string()],
                priority: 2,
                is_active: true,
                start_date: Some("1948-01-01".to_string()),
                notes: Some("Values above 50 indicate expansion, below 50 contraction".to_string()),
            },
        ]
    }

    /// Consumer Spending and Confidence Indicators
    fn consumer_indicators() -> Vec<SeriesDefinition> {
        vec![
            SeriesDefinition {
                id: "PCE".to_string(),
                title: "Personal Consumption Expenditures".to_string(),
                description: "Total spending by consumers on goods and services".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Billions of Dollars".to_string(),
                category: EconomicCategory::Consumer,
                tags: vec![
                    "consumer spending".to_string(),
                    "consumption".to_string(),
                    "pce".to_string(),
                ],
                priority: 2,
                is_active: true,
                start_date: Some("1959-01-01".to_string()),
                notes: Some("Primary measure of consumer spending".to_string()),
            },
            SeriesDefinition {
                id: "UMCSENT".to_string(),
                title: "University of Michigan: Consumer Sentiment".to_string(),
                description: "Index of consumer confidence based on surveys".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::NotSeasonallyAdjusted,
                units: "Index 1966:Q1=100".to_string(),
                category: EconomicCategory::Consumer,
                tags: vec![
                    "consumer confidence".to_string(),
                    "sentiment".to_string(),
                    "expectations".to_string(),
                ],
                priority: 2,
                is_active: true,
                start_date: Some("1978-01-01".to_string()),
                notes: Some("University of Michigan consumer sentiment survey".to_string()),
            },
            SeriesDefinition {
                id: "RSAFS".to_string(),
                title: "Advance Retail Sales: Retail and Food Services".to_string(),
                description: "Total retail sales including food services".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Millions of Dollars".to_string(),
                category: EconomicCategory::Consumer,
                tags: vec![
                    "retail sales".to_string(),
                    "consumer spending".to_string(),
                    "commerce".to_string(),
                ],
                priority: 2,
                is_active: true,
                start_date: Some("1992-01-01".to_string()),
                notes: Some("Advance estimate of retail sales".to_string()),
            },
        ]
    }

    /// Business Investment and Confidence Indicators
    fn business_indicators() -> Vec<SeriesDefinition> {
        vec![
            SeriesDefinition {
                id: "GPDI".to_string(),
                title: "Gross Private Domestic Investment".to_string(),
                description: "Total private investment in equipment, structures, and inventories"
                    .to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Quarterly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Billions of Dollars".to_string(),
                category: EconomicCategory::Business,
                tags: vec![
                    "investment".to_string(),
                    "business spending".to_string(),
                    "capex".to_string(),
                ],
                priority: 3,
                is_active: true,
                start_date: Some("1947-01-01".to_string()),
                notes: Some("Includes equipment, structures, and inventory investment".to_string()),
            },
            SeriesDefinition {
                id: "NEWORDER".to_string(),
                title: "Manufacturers' New Orders: Durable Goods".to_string(),
                description: "New orders placed with manufacturers for durable goods".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Monthly,
                seasonal_adjustment: SeasonalAdjustment::SeasonallyAdjusted,
                units: "Millions of Dollars".to_string(),
                category: EconomicCategory::Business,
                tags: vec![
                    "new orders".to_string(),
                    "manufacturing".to_string(),
                    "business activity".to_string(),
                ],
                priority: 3,
                is_active: true,
                start_date: Some("1992-01-01".to_string()),
                notes: Some("Leading indicator of manufacturing activity".to_string()),
            },
        ]
    }

    /// Government Finance Indicators
    fn government_indicators() -> Vec<SeriesDefinition> {
        vec![
            SeriesDefinition {
                id: "FYFSGDA188S".to_string(),
                title: "Federal Surplus or Deficit [-] as Percent of Gross Domestic Product"
                    .to_string(),
                description: "Federal government budget balance as percentage of GDP".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Annual,
                seasonal_adjustment: SeasonalAdjustment::NotSeasonallyAdjusted,
                units: "Percent of GDP".to_string(),
                category: EconomicCategory::Government,
                tags: vec![
                    "budget deficit".to_string(),
                    "fiscal policy".to_string(),
                    "government".to_string(),
                ],
                priority: 3,
                is_active: true,
                start_date: Some("1929-01-01".to_string()),
                notes: Some("Negative values indicate deficit".to_string()),
            },
            SeriesDefinition {
                id: "GFDEGDQ188S".to_string(),
                title: "Federal Debt: Total Public Debt as Percent of Gross Domestic Product"
                    .to_string(),
                description: "Total federal debt as percentage of GDP".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Quarterly,
                seasonal_adjustment: SeasonalAdjustment::NotSeasonallyAdjusted,
                units: "Percent of GDP".to_string(),
                category: EconomicCategory::Government,
                tags: vec![
                    "federal debt".to_string(),
                    "debt to gdp".to_string(),
                    "fiscal".to_string(),
                ],
                priority: 3,
                is_active: true,
                start_date: Some("1966-01-01".to_string()),
                notes: Some("Key measure of government debt sustainability".to_string()),
            },
        ]
    }

    /// International Economic Indicators
    fn international_indicators() -> Vec<SeriesDefinition> {
        vec![
            SeriesDefinition {
                id: "DEXUSEU".to_string(),
                title: "U.S. / Euro Foreign Exchange Rate".to_string(),
                description: "U.S. dollars per euro exchange rate".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Daily,
                seasonal_adjustment: SeasonalAdjustment::NotSeasonallyAdjusted,
                units: "U.S. Dollars to One Euro".to_string(),
                category: EconomicCategory::International,
                tags: vec![
                    "exchange rate".to_string(),
                    "euro".to_string(),
                    "currency".to_string(),
                ],
                priority: 3,
                is_active: true,
                start_date: Some("1999-01-04".to_string()),
                notes: Some("Daily exchange rate data".to_string()),
            },
            SeriesDefinition {
                id: "DEXCHUS".to_string(),
                title: "China / U.S. Foreign Exchange Rate".to_string(),
                description: "Chinese yuan per U.S. dollar exchange rate".to_string(),
                source: DataSource::FRED,
                frequency: DataFrequency::Daily,
                seasonal_adjustment: SeasonalAdjustment::NotSeasonallyAdjusted,
                units: "Chinese Yuan to One U.S. Dollar".to_string(),
                category: EconomicCategory::International,
                tags: vec![
                    "exchange rate".to_string(),
                    "yuan".to_string(),
                    "china".to_string(),
                ],
                priority: 3,
                is_active: true,
                start_date: Some("1981-01-02".to_string()),
                notes: Some("Important for U.S.-China trade analysis".to_string()),
            },
        ]
    }

    /// Get all series by category
    pub fn get_by_category(&self, category: &EconomicCategory) -> Vec<&SeriesDefinition> {
        self.series
            .iter()
            .filter(|s| s.category == *category)
            .collect()
    }

    /// Get all series by source
    pub fn get_by_source(&self, source: &DataSource) -> Vec<&SeriesDefinition> {
        self.series.iter().filter(|s| s.source == *source).collect()
    }

    /// Get all series by priority level
    pub fn get_by_priority(&self, priority: u8) -> Vec<&SeriesDefinition> {
        self.series
            .iter()
            .filter(|s| s.priority == priority)
            .collect()
    }

    /// Get all high-priority series (priority 1-2)
    pub fn get_high_priority(&self) -> Vec<&SeriesDefinition> {
        self.series.iter().filter(|s| s.priority <= 2).collect()
    }

    /// Get all active series
    pub fn get_active(&self) -> Vec<&SeriesDefinition> {
        self.series.iter().filter(|s| s.is_active).collect()
    }

    /// Get total number of series in catalog
    pub fn len(&self) -> usize {
        self.series.len()
    }

    /// Check if catalog is empty
    pub fn is_empty(&self) -> bool {
        self.series.is_empty()
    }

    /// Get series statistics by category
    pub fn get_category_stats(&self) -> HashMap<EconomicCategory, usize> {
        let mut stats = HashMap::new();
        for series in &self.series {
            *stats.entry(series.category.clone()).or_insert(0) += 1;
        }
        stats
    }

    /// Get series statistics by source
    pub fn get_source_stats(&self) -> HashMap<DataSource, usize> {
        let mut stats = HashMap::new();
        for series in &self.series {
            *stats.entry(series.source.clone()).or_insert(0) += 1;
        }
        stats
    }
}

impl Default for ComprehensiveSeriesCatalog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_creation() {
        let catalog = ComprehensiveSeriesCatalog::new();
        assert!(!catalog.is_empty());
        assert!(catalog.len() >= 35); // Should have at least 35+ series (currently 39)
    }

    #[test]
    fn test_category_filtering() {
        let catalog = ComprehensiveSeriesCatalog::new();
        let gdp_series = catalog.get_by_category(&EconomicCategory::GDP);
        assert!(!gdp_series.is_empty());

        let employment_series = catalog.get_by_category(&EconomicCategory::Employment);
        assert!(!employment_series.is_empty());
    }

    #[test]
    fn test_priority_filtering() {
        let catalog = ComprehensiveSeriesCatalog::new();
        let high_priority = catalog.get_high_priority();
        assert!(!high_priority.is_empty());

        let priority_1 = catalog.get_by_priority(1);
        assert!(!priority_1.is_empty());
    }

    #[test]
    fn test_source_filtering() {
        let catalog = ComprehensiveSeriesCatalog::new();
        let fred_series = catalog.get_by_source(&DataSource::FRED);
        assert!(!fred_series.is_empty());
    }

    #[test]
    fn test_statistics() {
        let catalog = ComprehensiveSeriesCatalog::new();
        let category_stats = catalog.get_category_stats();
        assert!(!category_stats.is_empty());

        let source_stats = catalog.get_source_stats();
        assert!(!source_stats.is_empty());
    }

    #[test]
    fn test_key_series_present() {
        let catalog = ComprehensiveSeriesCatalog::new();

        // Check that key economic indicators are present
        let gdp_series = catalog.series.iter().find(|s| s.id == "GDPC1");
        assert!(gdp_series.is_some());

        let unemployment = catalog.series.iter().find(|s| s.id == "UNRATE");
        assert!(unemployment.is_some());

        let cpi = catalog.series.iter().find(|s| s.id == "CPIAUCSL");
        assert!(cpi.is_some());

        let fed_funds = catalog.series.iter().find(|s| s.id == "FEDFUNDS");
        assert!(fed_funds.is_some());
    }
}
