# Crawler Data Sources Investigation

**Date**: September 26, 2025  
**Status**: Investigation Complete  
**Branch**: `backend-federation-crawler-integration`

## Executive Summary

This document provides a comprehensive investigation of the current crawler system and data sources supported by EconGraph. The investigation reveals a sophisticated multi-source data architecture with significant geographic components and spatial correlation opportunities.

## Current Crawler Architecture

### **1. Multi-Source Crawler System**

The EconGraph system supports **17+ major data sources** across economic and financial domains:

#### **Economic Data Sources (econ-graph-crawler)**
1. **FRED** (Federal Reserve Economic Data) - US economic indicators
2. **BLS** (Bureau of Labor Statistics) - US labor market data  
3. **Census Bureau** - US demographic and economic data
4. **BEA** (Bureau of Economic Analysis) - US national accounts
5. **World Bank** - Global development indicators
6. **IMF** (International Monetary Fund) - Global economic data
7. **ECB** (European Central Bank) - Euro area economic data
8. **OECD** (Organisation for Economic Co-operation and Development)
9. **Bank of England** - UK monetary policy and economic data
10. **WTO** (World Trade Organization) - International trade data
11. **Bank of Japan** - Japanese monetary policy and economic data
12. **Reserve Bank of Australia** - Australian economic data
13. **Bank of Canada** - Canadian monetary policy and economic data
14. **Swiss National Bank** - Swiss economic and financial data
15. **UN Statistics Division** - Global economic and social data
16. **ILO** (International Labour Organization) - Global labor market data
17. **FHFA** (Federal Housing Finance Agency) - US housing data

#### **Financial Data Sources (econ-graph-sec-crawler)**
1. **SEC EDGAR XBRL** - Corporate financial filings
   - 10-K, 10-Q, 8-K filings
   - XBRL taxonomy processing
   - Financial statement parsing
   - Company submissions crawling

#### **Queue-Based Processing**
- Background processing of queued items
- Retry mechanisms with exponential backoff
- Priority-based processing
- Rate limiting and compliance

### **2. Crawler Discovery & Processing Pipeline**

```rust
// Current crawler pattern
async fn crawl_data_source(data_source: &DataSource) -> Result<(usize, usize)> {
    // 1. Discover series using source-specific methods
    let discovered_series = match data_source.name.to_lowercase().as_str() {
        "fred" => discovery_service.discover_fred_series(pool).await?,
        "bls" => discovery_service.discover_bls_series(pool).await?,
        "census" => discovery_service.discover_census_series(pool).await?,
        // ... 15+ more sources
    };
    
    // 2. Download and process data
    let processed_series = series_downloader.download_series_data(discovered_series).await?;
    
    // 3. Store in database with metadata
    catalog_downloader.store_series_metadata(processed_series).await?;
}
```

## Geographic Data Analysis

### **1. Current Geographic Support**

The system has **comprehensive geographic infrastructure**:

#### **Country-Level Data**
```rust
pub struct Country {
    pub id: Uuid,
    pub iso_code: String,        // ISO 3166-1 alpha-3 (USA, GBR, etc.)
    pub iso_code_2: String,      // ISO 3166-1 alpha-2 (US, GB, etc.)
    pub name: String,
    pub region: String,          // North America, Europe, Asia, etc.
    pub sub_region: String,      // Western Europe, Southeast Asia, etc.
    pub income_group: String,    // High income, Upper middle income, etc.
    pub population: Option<i64>,
    pub gdp_usd: Option<BigDecimal>,
    pub gdp_per_capita_usd: Option<BigDecimal>,
    pub latitude: Option<BigDecimal>,   // Spatial coordinates
    pub longitude: Option<BigDecimal>,  // Spatial coordinates
    pub currency_code: Option<String>,
    pub is_active: bool,
}
```

#### **Spatial Correlation Infrastructure**
```rust
pub struct CountryCorrelation {
    pub country_a_id: Uuid,
    pub country_b_id: Uuid,
    pub indicator_category: String,
    pub correlation_coefficient: Decimal,
    pub time_period_start: NaiveDate,
    pub time_period_end: NaiveDate,
    pub sample_size: i32,
    pub p_value: Option<Decimal>,
    pub is_significant: bool,
    pub calculated_at: DateTime<Utc>,
}
```

### **2. Census Bureau Geographic Hierarchy**

**Census data provides the most detailed geographic breakdown**:

```rust
// Census geographic levels
pub struct BdsGeography {
    pub name: String,
    pub geo_level_display: Option<String>,
    pub geo_level_id: Option<String>,
    pub requires: Option<HashMap<String, String>>,
    pub wildcard: Option<bool>,
}

// Supported geographic levels:
// - "us" - United States (national)
// - "state" - State level  
// - "county" - County level
// - "metro" - Metropolitan areas
// - "cbsa" - Core Based Statistical Areas
```

**Census Data Structure**:
```rust
pub struct BdsDataPoint {
    pub variable: String,        // ESTAB, FIRM, JOB_CREATION, etc.
    pub year: i32,
    pub value: Option<i64>,
    pub geography: String,       // Geographic code (numeric)
}
```

### **3. Spatial Correlation Opportunities**

#### **US County-Level Analysis**
- **Adjacent Counties**: High spatial correlation for economic indicators
- **Metropolitan Areas**: Cross-county economic relationships
- **State Boundaries**: Economic spillover effects
- **Regional Patterns**: Similar economic cycles across geographic regions

#### **International Analysis**
- **Trade Relationships**: Cross-border economic impacts
- **Currency Zones**: ECB data across Eurozone countries
- **Regional Blocs**: OECD, ASEAN, NAFTA economic correlations
- **Global Supply Chains**: WTO trade data spatial relationships

## XBRL Financial Data Integration

### **1. SEC EDGAR XBRL System**

**XBRL is fundamentally different** from economic time series data:

#### **Data Structure**
- **Filing-Based**: Data organized by company filings (10-K, 10-Q, 8-K)
- **Taxonomy-Driven**: Uses XBRL taxonomies (US-GAAP, IFRS)
- **Contextual**: Data points have complex contexts (periods, scenarios, segments)
- **Hierarchical**: Financial statements have parent-child relationships

#### **Current Implementation**
```rust
pub struct SecFiling {
    pub accession_number: String,
    pub filing_date: DateTime<Utc>,
    pub period_end_date: DateTime<Utc>,
    pub fiscal_year: i32,
    pub fiscal_quarter: i32,
    pub xbrl_processing_status: String,
    pub xbrl_file_oid: i64,  // PostgreSQL Large Object reference
    pub is_amended: bool,
    pub is_restated: bool,
}

pub struct FinancialLineItem {
    pub statement_id: Uuid,
    pub taxonomy_concept: String,  // XBRL concept name
    pub standard_label: String,
    pub custom_label: Option<String>,
    pub value: BigDecimal,
    pub unit: String,
    pub context_ref: String,
    pub statement_type: String,
    pub statement_section: String,
    pub precision: i32,
    pub decimals: i32,
    pub is_credit: bool,
    pub is_debit: bool,
}
```

#### **XBRL Processing Pipeline**
1. **Download XBRL Files**: From SEC EDGAR API
2. **Arelle Integration**: Python-based XBRL parser
3. **Taxonomy Mapping**: Convert XBRL concepts to standardized terms
4. **Context Resolution**: Handle periods, scenarios, segments
5. **Financial Statement Assembly**: Reconstruct statements from line items

### **2. XBRL vs. Economic Data Integration**

**XBRL is Fundamentally Different** - it's not time series data at all:

#### **XBRL Characteristics**
- **Schema Per Company**: Each company defines its own XBRL taxonomy
- **Schema Evolution**: Companies change their reporting structure over time
- **Complex Contexts**: Data points have multiple contexts (periods, scenarios, segments)
- **Hierarchical Relationships**: Parent-child relationships between line items
- **Variable Structure**: No standard schema across companies
- **Filing-Based**: Data organized by SEC filings, not by time series

#### **Economic Data Characteristics**
- **Standardized Schema**: Consistent structure across all series
- **Time Series**: Regular intervals with predictable patterns
- **Geographic Dimensions**: Consistent geographic hierarchy
- **Stable Structure**: Schema rarely changes
- **Time-Based**: Data organized by date/time

#### **Integration Challenges**
- **Schema Incompatibility**: XBRL schemas vary by company and filing
- **Different Storage Patterns**: XBRL needs document storage, not time series
- **Different Query Patterns**: Company/filing queries vs. time/geography queries
- **Different Processing**: XBRL parsing vs. time series aggregation
- **Different APIs**: File-based uploads vs. bulk time series data

## Data Source Characteristics

### **1. API Characteristics**

#### **Rate Limiting Requirements**
- **SEC EDGAR**: 10 requests/second (strict enforcement)
- **FRED**: 120 requests/minute (API key required)
- **Census**: No authentication, but has usage limits
- **World Bank**: 120 requests/minute
- **OECD**: 100 requests/minute

#### **Data Volume Patterns**
- **FRED**: ~800,000 series, high-frequency updates
- **Census**: ~50,000 series, annual updates
- **SEC EDGAR**: ~10,000 companies, quarterly filings
- **World Bank**: ~1,000 indicators, ~200 countries

#### **Update Frequencies**
- **Daily**: FRED (some series), financial markets
- **Monthly**: BLS, BEA, most economic indicators
- **Quarterly**: SEC filings, GDP data
- **Annually**: Census, World Bank development indicators

### **2. Geographic Data Patterns**

#### **US Data (Census, BLS, BEA)**
- **County Level**: 3,000+ counties with economic data
- **State Level**: 50 states + DC
- **Metropolitan Areas**: 300+ metro areas
- **Spatial Correlation**: High correlation between adjacent counties

#### **International Data (World Bank, IMF, OECD)**
- **Country Level**: 200+ countries
- **Regional Groupings**: Economic blocs and trade zones
- **Currency Zones**: Eurozone, dollar pegs, etc.
- **Development Levels**: Income groups and development status

## Implications for Crawler Integration Design

### **1. Data Volume Considerations**

#### **Bulk Processing Requirements**
- **Census Data**: Large batches of county-level data
- **SEC Filings**: Thousands of XBRL files per quarter
- **World Bank**: Country-level indicators across time
- **FRED**: High-frequency updates for key series

#### **Storage Patterns**
- **Time-Based Partitioning**: Works well for economic time series
- **Filing-Based Partitioning**: Better for SEC XBRL data
- **Geographic Partitioning**: Could optimize spatial queries
- **Hybrid Approaches**: Different strategies for different data types

### **2. Geographic Query Optimization**

#### **Spatial Indexing Opportunities**
- **PostGIS Integration**: For geographic queries and spatial joins
- **Hierarchical Queries**: Country → State → County → Metro
- **Correlation Queries**: Find spatially correlated series
- **Regional Aggregation**: Sum/count across geographic boundaries

#### **Partitioning Strategies**
```sql
-- Geographic partitioning example
CREATE TABLE economic_data (
    series_id UUID,
    date DATE,
    value DECIMAL,
    geography_code VARCHAR(10)
) PARTITION BY HASH (geography_code);

-- Time + geography partitioning
CREATE TABLE economic_data (
    series_id UUID,
    date DATE,
    value DECIMAL,
    geography_code VARCHAR(10)
) PARTITION BY RANGE (date) SUBPARTITION BY HASH (geography_code);
```

### **3. Arrow Flight Integration Benefits**

#### **Geographic Data Efficiency**
- **Spatial Queries**: Arrow can efficiently handle geographic filters
- **Correlation Analysis**: Vectorized operations for correlation calculations
- **Aggregation**: Fast geographic and temporal aggregations
- **Cross-Source Joins**: Efficient joins across different data sources

#### **XBRL Data Challenges**
- **Complex Schema**: XBRL has complex, nested structures
- **Variable Schema**: Different companies use different taxonomies
- **Context Handling**: Multiple contexts per data point
- **File-Based**: XBRL is inherently file-based, not time-series

## Recommendations for Crawler Integration

### **1. Multi-API Strategy**

#### **Primary APIs by Data Type**
- **Economic Time Series**: Arrow Flight RPC (bulk, efficient)
- **XBRL Filings**: HTTP REST API (file-based, complex)
- **Geographic Queries**: GraphQL with spatial extensions
- **Real-time Updates**: WebSocket or Server-Sent Events

#### **Storage Strategy**
- **Economic Data**: Arrow Flight → Parquet → Iceberg (time-partitioned)
- **XBRL Data**: HTTP REST → PostgreSQL Large Objects (filing-based)
- **Geographic Metadata**: PostgreSQL with PostGIS extensions
- **Correlation Data**: Arrow Flight → Parquet → Iceberg (computed)

### **2. Geographic Data Enhancements**

#### **Enhanced Data Models**
```rust
pub struct GeographicEconomicSeries {
    pub series_id: Uuid,
    pub external_id: String,
    pub title: String,
    pub geography_type: GeographyType,  // Country, State, County, Metro
    pub geography_code: String,
    pub parent_geography: Option<Uuid>, // Hierarchical relationships
    pub spatial_bounds: Option<BoundingBox>,
    pub frequency: String,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

pub enum GeographyType {
    Country,
    State,
    County,
    MetropolitanArea,
    CongressionalDistrict,
    ZipCode,
}

pub struct BoundingBox {
    pub min_latitude: Decimal,
    pub max_latitude: Decimal,
    pub min_longitude: Decimal,
    pub max_longitude: Decimal,
}
```

#### **Spatial Query Extensions**
```graphql
query SpatialEconomicData($bounds: BoundingBox!, $indicator: String!) {
  economicDataInBounds(bounds: $bounds, indicator: $indicator) {
    series {
      id
      title
      geographyCode
      spatialBounds
    }
    dataPoints {
      date
      value
      geographyCode
    }
  }
}

query SpatialCorrelations($geography: String!, $radius: Float!) {
  spatialCorrelations(geography: $geography, radius: $radius) {
    targetSeries {
      id
      title
      geographyCode
    }
    correlatedSeries {
      series {
        id
        title
        geographyCode
      }
      correlation: correlationCoefficient
      distance: spatialDistance
    }
  }
}
```

### **3. XBRL Integration Strategy**

#### **Completely Separate XBRL Pipeline**
XBRL is so fundamentally different that it should be treated as a **separate system**:

- **Different Data Model**: XBRL is document-based, not time series
- **Different Storage**: PostgreSQL Large Objects for XBRL files, not Parquet/Iceberg
- **Different APIs**: File-based uploads, not bulk Arrow data
- **Different Processing**: XBRL parsing with Arelle, not time series aggregation
- **Different Queries**: Company/filing queries, not time/geography queries

#### **XBRL-Specific Architecture**
```rust
// XBRL is fundamentally different - document-based, not time series
pub struct XbrlDataService {
    // File-based uploads (not bulk Arrow data)
    async fn upload_xbrl_filing(&self, filing: XbrlFiling) -> Result<FilingResponse>;
    
    // Company-centric queries (not time series queries)
    async fn get_company_financials(&self, cik: String, period: DateRange) -> Result<FinancialStatements>;
    
    // Cross-company analysis (not geographic analysis)
    async fn compare_companies(&self, ciks: Vec<String>, metrics: Vec<String>) -> Result<CompanyComparison>;
    
    // Schema management (XBRL-specific)
    async fn get_company_taxonomy(&self, cik: String, filing_date: DateTime<Utc>) -> Result<XbrlTaxonomy>;
    async fn map_xbrl_concepts(&self, company_taxonomy: XbrlTaxonomy) -> Result<StandardizedMapping>;
}

// XBRL storage is completely different
pub struct XbrlStorage {
    // Document storage (not time series storage)
    async fn store_xbrl_file(&self, accession_number: &str, content: &[u8]) -> Result<XbrlDocument>;
    async fn retrieve_xbrl_file(&self, accession_number: &str) -> Result<Vec<u8>>;
    
    // XBRL-specific metadata
    async fn extract_financial_statements(&self, xbrl_doc: &XbrlDocument) -> Result<FinancialStatements>;
    async fn get_line_items(&self, statement_id: Uuid) -> Result<Vec<FinancialLineItem>>;
}
```

#### **Integration Points**
- **Separate Databases**: XBRL in PostgreSQL, economic data in Parquet/Iceberg
- **Cross-Reference Tables**: Link companies to economic indicators where relevant
- **Unified GraphQL**: Single GraphQL endpoint that can query both systems
- **Separate Crawlers**: XBRL crawler is completely independent from economic data crawler

## Conclusion

The current crawler system is **significantly more sophisticated** than initially apparent, supporting 17+ data sources with comprehensive geographic infrastructure. Key findings:

### **✅ Strengths**
- **Comprehensive Coverage**: Major economic and financial data sources
- **Geographic Infrastructure**: Country-level data with spatial coordinates
- **Spatial Correlation**: Built-in correlation analysis capabilities
- **Multi-Source Architecture**: Well-designed crawler abstraction

### **🔄 Opportunities**
- **County-Level Analysis**: Census data provides detailed US geographic breakdown
- **Spatial Optimization**: PostGIS integration for geographic queries
- **Cross-Source Correlations**: Economic + financial data integration
- **Arrow Flight Efficiency**: Better bulk data processing for large geographic datasets

### **⚠️ Challenges**
- **XBRL Complexity**: **Completely different data paradigm** - document-based, not time series
- **Schema Variability**: XBRL schemas change per company and filing
- **Storage Diversity**: XBRL needs document storage, economic data needs time series storage
- **API Incompatibility**: XBRL needs file-based APIs, economic data needs bulk Arrow APIs
- **Geographic Queries**: Current models don't fully leverage spatial relationships
- **Scale**: Large datasets require careful partitioning strategies

### **🎯 Next Steps**
1. **Separate XBRL System**: Treat XBRL as completely separate from economic data
2. **Economic Data API**: Arrow Flight RPC for bulk economic time series data
3. **XBRL API**: File-based HTTP REST API for XBRL document uploads
4. **Enhanced Geographic Models**: Add spatial indexing and hierarchical relationships
5. **Implement Spatial Queries**: PostGIS integration for geographic analysis
6. **Cross-System Integration**: Link economic and financial data where relevant

This investigation reveals that the crawler integration should be **much more sophisticated** than initially planned, with significant opportunities for geographic and spatial data analysis that could differentiate EconGraph in the market.
