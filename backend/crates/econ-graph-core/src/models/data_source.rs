use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::schema::data_sources;

/// Data source model representing external data providers
#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = data_sources)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DataSource {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub base_url: String,
    pub api_key_required: bool,
    pub rate_limit_per_minute: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_visible: bool,
    pub is_enabled: bool,
    pub requires_admin_approval: bool,
    pub crawl_frequency_hours: i32,
    pub last_crawl_at: Option<DateTime<Utc>>,
    pub crawl_status: Option<String>,
    pub crawl_error_message: Option<String>,
    pub api_documentation_url: Option<String>,
    pub api_key_name: Option<String>,
}

/// New data source for insertion
#[derive(Debug, Clone, Insertable, Validate, Deserialize)]
#[diesel(table_name = data_sources)]
pub struct NewDataSource {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    #[validate(url)]
    pub base_url: String,
    pub api_key_required: bool,
    #[validate(range(min = 1, max = 10000))]
    pub rate_limit_per_minute: i32,
    pub is_visible: bool,
    pub is_enabled: bool,
    pub requires_admin_approval: bool,
    pub crawl_frequency_hours: i32,
    #[validate(url)]
    pub api_documentation_url: Option<String>,
    pub api_key_name: Option<String>,
}

/// Data source update model
#[derive(Debug, Clone, AsChangeset, Validate, Deserialize)]
#[diesel(table_name = data_sources)]
pub struct UpdateDataSource {
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    #[validate(url)]
    pub base_url: Option<String>,
    pub api_key_required: Option<bool>,
    #[validate(range(min = 1, max = 10000))]
    pub rate_limit_per_minute: Option<i32>,
    #[validate(url)]
    pub api_documentation_url: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Predefined data sources
impl DataSource {
    /// Create FRED (Federal Reserve Economic Data) source
    pub fn fred() -> NewDataSource {
        NewDataSource {
            name: "Federal Reserve Economic Data (FRED)".to_string(),
            description: Some(
                "Economic data from the Federal Reserve Bank of St. Louis".to_string(),
            ),
            base_url: "https://api.stlouisfed.org/fred".to_string(),
            api_key_required: true,
            rate_limit_per_minute: 120,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 6,
            api_documentation_url: Some("https://fred.stlouisfed.org/docs/api/fred/".to_string()),
            api_key_name: Some("FRED_API_KEY".to_string()),
        }
    }

    /// Create BLS (Bureau of Labor Statistics) source
    pub fn bls() -> NewDataSource {
        NewDataSource {
            name: "Bureau of Labor Statistics (BLS)".to_string(),
            description: Some(
                "Labor statistics and economic indicators from the U.S. Bureau of Labor Statistics"
                    .to_string(),
            ),
            base_url: "https://api.bls.gov/publicAPI/v2".to_string(),
            api_key_required: true,
            rate_limit_per_minute: 500,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 12,
            api_documentation_url: Some(
                "https://www.bls.gov/developers/api_signature_v2.htm".to_string(),
            ),
            api_key_name: Some("BLS_API_KEY".to_string()),
        }
    }

    /// Create Census Bureau source
    pub fn census() -> NewDataSource {
        NewDataSource {
            name: "U.S. Census Bureau".to_string(),
            description: Some(
                "Demographic and economic data from the U.S. Census Bureau".to_string(),
            ),
            base_url: "https://api.census.gov/data".to_string(),
            api_key_required: false, // Census API doesn't require authentication
            rate_limit_per_minute: 500,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some(
                "https://www.census.gov/data/developers/data-sets.html".to_string(),
            ),
            api_key_name: None,
        }
    }

    /// Create World Bank source
    pub fn world_bank() -> NewDataSource {
        NewDataSource {
            name: "World Bank Open Data".to_string(),
            description: Some(
                "Global economic and development indicators from the World Bank".to_string(),
            ),
            base_url: "https://api.worldbank.org/v2".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 1000,
            is_visible: false,
            is_enabled: false,
            requires_admin_approval: true,
            crawl_frequency_hours: 24,
            api_documentation_url: Some("https://datahelpdesk.worldbank.org/knowledgebase/articles/898581-api-basic-call-structures".to_string()),
            api_key_name: None,
        }
    }

    /// Create BEA source
    pub fn bea() -> NewDataSource {
        NewDataSource {
            name: "Bureau of Economic Analysis (BEA)".to_string(),
            description: Some(
                "U.S. economic statistics including GDP, NIPA, ITA, and Regional data".to_string(),
            ),
            base_url: "https://apps.bea.gov/api/data".to_string(),
            api_key_required: true,
            rate_limit_per_minute: 1000,
            is_visible: false,
            is_enabled: false,
            requires_admin_approval: true,
            crawl_frequency_hours: 24,
            api_documentation_url: Some(
                "https://apps.bea.gov/api/bea_web_service_api_user_guide.htm".to_string(),
            ),
            api_key_name: Some("BEA_API_KEY".to_string()),
        }
    }

    /// Create IMF source
    pub fn imf() -> NewDataSource {
        NewDataSource {
            name: "International Monetary Fund (IMF)".to_string(),
            description: Some(
                "Global economic and financial data including IFS, BOP, GFS, and WEO".to_string(),
            ),
            base_url: "https://dataservices.imf.org/REST/SDMX_JSON.svc".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 1000,
            is_visible: false,
            is_enabled: false,
            requires_admin_approval: true,
            crawl_frequency_hours: 24,
            api_documentation_url: Some(
                "https://data.imf.org/en/Resource-Pages/IMF-API".to_string(),
            ),
            api_key_name: Some("IMF_API_KEY".to_string()),
        }
    }

    /// Create FHFA (Federal Housing Finance Agency) source
    pub fn fhfa() -> NewDataSource {
        NewDataSource {
            name: "Federal Housing Finance Agency (FHFA)".to_string(),
            description: Some(
                "House Price Index data measuring changes in single-family home values".to_string(),
            ),
            base_url: "https://api.fhfa.gov".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 1000,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some(
                "https://www.fhfa.gov/data/developer-information".to_string(),
            ),
            api_key_name: None,
        }
    }

    /// Create ECB (European Central Bank) source
    pub fn ecb() -> NewDataSource {
        NewDataSource {
            name: "European Central Bank (ECB)".to_string(),
            description: Some(
                "Eurozone monetary policy, inflation, GDP, employment, and trade data".to_string(),
            ),
            base_url: "https://sdw-wsrest.ecb.europa.eu/service".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 1000,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some("https://sdw-wsrest.ecb.europa.eu/help/".to_string()),
            api_key_name: None,
        }
    }

    /// Create OECD (Organisation for Economic Co-operation and Development) source
    pub fn oecd() -> NewDataSource {
        NewDataSource {
            name: "OECD (Organisation for Economic Co-operation and Development)".to_string(),
            description: Some(
                "Comprehensive economic, social, and environmental data for OECD countries and partner economies".to_string(),
            ),
            base_url: "https://sdmx.oecd.org/public/rest/data".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 1000,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some(
                "https://data-explorer.oecd.org/vis?fs[0]=Topic%2C1%7C1%7C1%7C0&fs[1]=Country%2C1%7C1%7C1%7C0&fs[2]=Measure%2C1%7C1%7C1%7C0&fs[3]=Time%2C1%7C1%7C1%7C0&pg=0&fc=Topic&lc=en&fs[4]=Subject%2C1%7C1%7C1%7C0".to_string(),
            ),
            api_key_name: None,
        }
    }

    /// Create Bank of England (BoE) source
    pub fn boe() -> NewDataSource {
        NewDataSource {
            name: "Bank of England (BoE)".to_string(),
            description: Some(
                "UK monetary policy, inflation, GDP, employment, financial stability, and exchange rate data".to_string(),
            ),
            base_url: "https://www.bankofengland.co.uk/boeapps/database".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 1000,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some(
                "https://www.bankofengland.co.uk/statistics/data".to_string(),
            ),
            api_key_name: None,
        }
    }

    /// Create World Trade Organization (WTO) source
    pub fn wto() -> NewDataSource {
        NewDataSource {
            name: "World Trade Organization (WTO)".to_string(),
            description: Some(
                "International trade data including merchandise and services trade for WTO member countries".to_string(),
            ),
            base_url: "https://api.wto.org/timeseries/v1".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 1000,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some(
                "https://www.wto.org/english/res_e/statis_e/data_explorer_e.htm".to_string(),
            ),
            api_key_name: None,
        }
    }

    /// Create Bank of Japan (BoJ) source
    pub fn boj() -> NewDataSource {
        NewDataSource {
            name: "Bank of Japan (BoJ)".to_string(),
            description: Some(
                "Japanese monetary policy, inflation, GDP, employment, and financial stability data".to_string(),
            ),
            base_url: "https://www.stat-search.boj.or.jp/ssi/mtshtml".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 1000,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some(
                "https://www.boj.or.jp/en/statistics/".to_string(),
            ),
            api_key_name: None,
        }
    }

    /// Create Reserve Bank of Australia (RBA) source
    pub fn rba() -> NewDataSource {
        NewDataSource {
            name: "Reserve Bank of Australia (RBA)".to_string(),
            description: Some(
                "Australian monetary policy, inflation, GDP, employment, and financial stability data".to_string(),
            ),
            base_url: "https://www.rba.gov.au/statistics".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 1000,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some(
                "https://www.rba.gov.au/statistics/".to_string(),
            ),
            api_key_name: None,
        }
    }

    /// Create Bank of Canada (BoC) source
    pub fn boc() -> NewDataSource {
        NewDataSource {
            name: "Bank of Canada (BoC)".to_string(),
            description: Some(
                "Canadian monetary policy, inflation, GDP, employment, and financial stability data".to_string(),
            ),
            base_url: "https://www.bankofcanada.ca/valet".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 1000,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some(
                "https://www.bankofcanada.ca/valet/docs/".to_string(),
            ),
            api_key_name: None,
        }
    }

    /// Create Swiss National Bank (SNB) source
    pub fn snb() -> NewDataSource {
        NewDataSource {
            name: "Swiss National Bank (SNB)".to_string(),
            description: Some(
                "Swiss monetary policy, inflation, GDP, employment, and financial stability data"
                    .to_string(),
            ),
            base_url: "https://data.snb.ch".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 1000,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some("https://data.snb.ch/en".to_string()),
            api_key_name: None,
        }
    }

    /// Create UN Statistics Division source
    pub fn unstats() -> NewDataSource {
        NewDataSource {
            name: "UN Statistics Division".to_string(),
            description: Some(
                "Global economic, social, and environmental data from the United Nations Statistics Division".to_string(),
            ),
            base_url: "https://unstats.un.org".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 1000,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some(
                "https://unstats.un.org/home/".to_string(),
            ),
            api_key_name: None,
        }
    }

    /// Create International Labour Organization (ILO) source
    pub fn ilo() -> NewDataSource {
        NewDataSource {
            name: "International Labour Organization (ILO)".to_string(),
            description: Some(
                "Global labor market and employment data from the International Labour Organization".to_string(),
            ),
            base_url: "https://www.ilo.org".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 1000,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some(
                "https://www.ilo.org/global/statistics-and-databases/lang--en/index.htm".to_string(),
            ),
            api_key_name: None,
        }
    }

    /// Find data source by name
    pub async fn find_by_name(
        pool: &crate::database::DatabasePool,
        name: &str,
    ) -> crate::error::AppResult<Option<Self>> {
        use crate::schema::data_sources::dsl;

        let mut conn = pool.get().await?;
        let name = name.to_string();

        let source = dsl::data_sources
            .filter(dsl::name.eq(name))
            .first::<Self>(&mut conn)
            .await
            .optional()?;

        Ok(source)
    }

    /// Find all data sources
    pub async fn find_all(
        pool: &crate::database::DatabasePool,
    ) -> crate::error::AppResult<Vec<Self>> {
        use crate::schema::data_sources::dsl;

        let mut conn = pool.get().await?;

        let sources = diesel_async::RunQueryDsl::load(dsl::data_sources, &mut conn).await?;

        Ok(sources)
    }

    /// Create a new data source
    pub async fn create(
        pool: &crate::database::DatabasePool,
        new_source: NewDataSource,
    ) -> crate::error::AppResult<Self> {
        use crate::schema::data_sources::dsl;

        // Validate the new data source
        new_source.validate()?;

        let mut conn = pool.get().await?;

        let source = diesel_async::RunQueryDsl::get_result(
            diesel::insert_into(dsl::data_sources).values(&new_source),
            &mut conn,
        )
        .await?;

        Ok(source)
    }

    /// Get or create a data source by name
    pub async fn get_or_create(
        pool: &crate::database::DatabasePool,
        new_source: NewDataSource,
    ) -> crate::error::AppResult<Self> {
        // Try to find existing source first
        match Self::find_by_name(pool, &new_source.name).await {
            Ok(Some(existing)) => Ok(existing),
            Ok(None) => {
                // Source doesn't exist, create it
                Self::create(pool, new_source).await
            }
            Err(e) => Err(e),
        }
    }
}

/// Data source with statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceWithStats {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub base_url: String,
    pub api_key_required: bool,
    pub rate_limit_per_minute: i32,
    pub series_count: i64,
    pub last_crawl: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for NewDataSource {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: None,
            base_url: String::new(),
            api_key_required: false,
            rate_limit_per_minute: 60,
            is_visible: false,
            is_enabled: false,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some("https://example.com/api/docs".to_string()),
            api_key_name: None,
        }
    }
}

impl Default for UpdateDataSource {
    fn default() -> Self {
        Self {
            name: None,
            description: None,
            base_url: None,
            api_key_required: None,
            rate_limit_per_minute: None,
            api_documentation_url: Some("https://example.com/api/docs".to_string()),
            updated_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod _inline_tests {
    use super::*;

    #[test]
    fn test_predefined_data_sources() {
        // REQUIREMENT: The system should support Federal Reserve and BLS data sources
        // PURPOSE: Verify that predefined data sources have correct configuration for API compatibility
        // This ensures the crawler can connect to external APIs with proper rate limiting

        let fred = DataSource::fred();
        // Verify FRED configuration matches API requirements
        assert_eq!(fred.name, "Federal Reserve Economic Data (FRED)");
        assert!(fred.api_key_required, "FRED requires API key for access");
        assert_eq!(
            fred.rate_limit_per_minute, 120,
            "FRED rate limit should match API documentation"
        );

        let bls = DataSource::bls();
        // Verify BLS configuration matches API requirements
        assert_eq!(bls.name, "Bureau of Labor Statistics (BLS)");
        assert!(
            bls.api_key_required,
            "BLS requires API key for higher rate limits"
        );
        assert_eq!(
            bls.rate_limit_per_minute, 500,
            "BLS rate limit should match API documentation"
        );

        let world_bank = DataSource::world_bank();
        // Verify World Bank configuration - no API key required
        assert!(
            !world_bank.api_key_required,
            "World Bank API is publicly accessible"
        );
        assert_eq!(
            world_bank.rate_limit_per_minute, 1000,
            "World Bank allows higher rate limits"
        );
    }

    #[test]
    fn test_new_data_source_validation() {
        // REQUIREMENT: Data source configuration should be validated to prevent crawler failures
        // PURPOSE: Verify that data source validation prevents invalid configurations
        // This ensures crawlers don't fail due to malformed URLs or unrealistic rate limits

        let valid_source = NewDataSource {
            name: "Test Source".to_string(),
            description: Some("A test data source".to_string()),
            base_url: "https://api.example.com".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 100,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some("https://api.example.com/docs".to_string()),
            api_key_name: None,
        };

        // Verify valid configuration passes validation
        assert!(
            valid_source.validate().is_ok(),
            "Valid data source should pass validation"
        );

        // Test URL validation - prevents crawler connection failures
        let invalid_source = NewDataSource {
            name: "Test Source".to_string(),
            description: None,
            base_url: "not-a-url".to_string(), // Invalid URL format
            api_key_required: false,
            rate_limit_per_minute: 100,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some("https://example.com/api/docs".to_string()),
            api_key_name: None,
        };

        assert!(
            invalid_source.validate().is_err(),
            "Invalid URL should fail validation"
        );

        // Test rate limit validation - prevents unrealistic configurations
        let invalid_rate_limit = NewDataSource {
            name: "Test Source".to_string(),
            description: None,
            base_url: "https://api.example.com".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 50000, // Unrealistically high rate limit
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some("https://example.com/api/docs".to_string()),
            api_key_name: None,
        };

        assert!(
            invalid_rate_limit.validate().is_err(),
            "Excessive rate limit should fail validation"
        );
    }

    #[test]
    fn test_update_data_source_validation() {
        // REQUIREMENT: Data source updates should maintain data integrity
        // PURPOSE: Verify that data source updates are validated to prevent configuration corruption
        // This ensures existing crawlers continue to function after configuration changes

        let valid_update = UpdateDataSource {
            name: Some("Updated Source".to_string()),
            base_url: Some("https://api.updated.com".to_string()),
            rate_limit_per_minute: Some(200),
            ..Default::default()
        };

        // Verify valid updates pass validation
        assert!(
            valid_update.validate().is_ok(),
            "Valid update should pass validation"
        );

        // Test URL validation on updates - prevents breaking existing crawlers
        let invalid_update = UpdateDataSource {
            base_url: Some("invalid-url".to_string()), // Invalid URL format
            ..Default::default()
        };

        assert!(
            invalid_update.validate().is_err(),
            "Invalid URL update should fail validation"
        );
    }
}

#[cfg(test)]
mod tests;
