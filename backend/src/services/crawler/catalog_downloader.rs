//! Catalog downloader for data sources
//!
//! Handles downloading and storing series metadata catalogs from various data sources

use crate::database::DatabasePool;
use crate::error::AppResult;
use crate::services::series_discovery::SeriesDiscoveryService;
use reqwest::Client;

/// Service for downloading data source catalogs
pub struct CatalogDownloader {
    discovery_service: SeriesDiscoveryService,
}

impl CatalogDownloader {
    /// Create a new catalog downloader
    pub fn new(client: Client) -> Self {
        Self {
            discovery_service: SeriesDiscoveryService::new(),
        }
    }

    /// Download catalog for a specific data source
    pub async fn download_catalog(
        &self,
        pool: &DatabasePool,
        source_name: &str,
    ) -> AppResult<usize> {
        let source_name_upper = source_name.to_uppercase();

        let series_count = match source_name_upper.as_str() {
            "FRED" => {
                let series_ids = self.discovery_service.discover_fred_series(pool).await?;
                series_ids.len()
            }
            "BLS" => {
                let series_ids = self.discovery_service.discover_bls_series(pool).await?;
                series_ids.len()
            }
            "CENSUS" => {
                let series_ids = self.discovery_service.discover_census_series(pool).await?;
                series_ids.len()
            }
            "BEA" => {
                let series_ids = self.discovery_service.discover_bea_series(pool).await?;
                series_ids.len()
            }
            "WORLD_BANK" | "WORLDBANK" => {
                let series_ids = self
                    .discovery_service
                    .discover_world_bank_series(pool)
                    .await?;
                series_ids.len()
            }
            "IMF" => {
                let series_ids = self.discovery_service.discover_imf_series(pool).await?;
                series_ids.len()
            }
            "FHFA" => {
                let series_ids = self.discovery_service.discover_fhfa_series(pool).await?;
                series_ids.len()
            }
            "ECB" => {
                let series_ids = self.discovery_service.discover_ecb_series(pool).await?;
                series_ids.len()
            }
            "OECD" => {
                let series_ids = self.discovery_service.discover_oecd_series(pool).await?;
                series_ids.len()
            }
            "BOE" | "BANK_OF_ENGLAND" => {
                let series_ids = self.discovery_service.discover_boe_series(pool).await?;
                series_ids.len()
            }
            "WTO" => {
                let series_ids = self.discovery_service.discover_wto_series(pool).await?;
                series_ids.len()
            }
            "BOJ" | "BANK_OF_JAPAN" => {
                let series_ids = self.discovery_service.discover_boj_series(pool).await?;
                series_ids.len()
            }
            "RBA" | "RESERVE_BANK_OF_AUSTRALIA" => {
                let series_ids = self.discovery_service.discover_rba_series(pool).await?;
                series_ids.len()
            }
            "BOC" | "BANK_OF_CANADA" => {
                let series_ids = self.discovery_service.discover_boc_series(pool).await?;
                series_ids.len()
            }
            "SNB" | "SWISS_NATIONAL_BANK" => {
                let series_ids = self.discovery_service.discover_snb_series(pool).await?;
                series_ids.len()
            }
            "UN_STATS" | "UNSTATS" | "UN_STATISTICS" => {
                let series_ids = self.discovery_service.discover_unstats_series(pool).await?;
                series_ids.len()
            }
            "ILO" => {
                let series_ids = self.discovery_service.discover_ilo_series(pool).await?;
                series_ids.len()
            }
            _ => {
                return Err(crate::error::AppError::ValidationError(format!(
                    "Unknown data source: {}. Available sources: FRED, BLS, Census, BEA, World Bank, IMF, FHFA, ECB, OECD, BoE, WTO, BoJ, RBA, BoC, SNB, UN Stats, ILO",
                    source_name
                )));
            }
        };

        Ok(series_count)
    }

    /// Get list of available data sources
    pub fn get_available_sources() -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "FRED",
                "Federal Reserve Economic Data - US economic indicators",
            ),
            ("BLS", "Bureau of Labor Statistics - US labor market data"),
            (
                "Census",
                "US Census Bureau - US demographic and economic data",
            ),
            ("BEA", "Bureau of Economic Analysis - US national accounts"),
            ("World Bank", "World Bank - Global development indicators"),
            ("IMF", "International Monetary Fund - Global economic data"),
            ("FHFA", "Federal Housing Finance Agency - US housing data"),
            ("ECB", "European Central Bank - Euro area economic data"),
            (
                "OECD",
                "Organisation for Economic Co-operation and Development",
            ),
            (
                "BoE",
                "Bank of England - UK monetary policy and economic data",
            ),
            ("WTO", "World Trade Organization - International trade data"),
            (
                "BoJ",
                "Bank of Japan - Japanese monetary policy and economic data",
            ),
            (
                "RBA",
                "Reserve Bank of Australia - Australian economic data",
            ),
            (
                "BoC",
                "Bank of Canada - Canadian monetary policy and economic data",
            ),
            (
                "SNB",
                "Swiss National Bank - Swiss economic and financial data",
            ),
            (
                "UN Stats",
                "UN Statistics Division - Global economic and social data",
            ),
            (
                "ILO",
                "International Labour Organization - Global labor market data",
            ),
        ]
    }
}
