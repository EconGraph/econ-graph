//! Series discovery service for automated cataloging of economic data series
//!
//! This module provides a unified interface for discovering and cataloging
//! economic data series from multiple government and international organization APIs.

pub mod bea;
pub mod bls;
pub mod boc;
pub mod boe;
pub mod boj;
pub mod census;
pub mod ecb;
pub mod fhfa;
pub mod fred;
pub mod ilo;
pub mod imf;
pub mod oecd;
pub mod rba;
pub mod snb;
pub mod unstats;
pub mod world_bank;
pub mod wto;

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::models::{DataSource, EconomicSeries, NewEconomicSeries};
use reqwest::Client;
use uuid::Uuid;

/// Series discovery service for automated cataloging
pub struct SeriesDiscoveryService {
    client: Client,
    fred_api_key: Option<String>,
    bls_api_key: Option<String>,
    census_api_key: Option<String>,
    bea_api_key: Option<String>,
}

impl SeriesDiscoveryService {
    pub fn new(
        fred_api_key: Option<String>,
        bls_api_key: Option<String>,
        census_api_key: Option<String>,
        bea_api_key: Option<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            fred_api_key,
            bls_api_key,
            census_api_key,
            bea_api_key,
        }
    }

    /// Discover all FRED series by searching through categories
    pub async fn discover_fred_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        fred::discover_fred_series(&self.client, &self.fred_api_key, pool).await
    }

    /// Discover BLS series using the BLS API v2 surveys endpoint
    pub async fn discover_bls_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        bls::discover_bls_series(&self.client, &self.bls_api_key, pool).await
    }

    /// Discover Census series using the Census Data API
    pub async fn discover_census_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        census::discover_census_series(&self.client, &self.census_api_key, pool).await
    }

    /// Discover BEA series using the BEA Data API
    pub async fn discover_bea_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        bea::discover_bea_series(&self.client, &self.bea_api_key, pool).await
    }

    /// Discover World Bank series using the World Bank Indicators API
    pub async fn discover_world_bank_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        world_bank::discover_world_bank_series(&self.client, pool).await
    }

    /// Discover IMF series using the IMF Data API
    pub async fn discover_imf_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        imf::discover_imf_series(&self.client, pool).await
    }

    /// Discover ECB series using the ECB Statistical Data Warehouse API
    pub async fn discover_ecb_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        ecb::discover_ecb_series(&self.client, pool).await
    }

    /// Discover OECD series using the OECD REST API
    pub async fn discover_oecd_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        oecd::discover_oecd_series(&self.client, pool).await
    }

    /// Discover Bank of England series using the BoE Statistical Database
    pub async fn discover_boe_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        boe::discover_boe_series(&self.client, pool).await
    }

    /// Discover World Trade Organization series using the WTO API
    pub async fn discover_wto_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        wto::discover_wto_series(&self.client, pool).await
    }

    /// Discover Bank of Japan series using the BoJ Statistical Database
    pub async fn discover_boj_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        boj::discover_boj_series(&self.client, pool).await
    }

    /// Discover Reserve Bank of Australia series using the RBA Statistical Database
    pub async fn discover_rba_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        rba::discover_rba_series(&self.client, pool).await
    }

    /// Discover Bank of Canada series using the BoC Statistical Database
    pub async fn discover_boc_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        boc::discover_boc_series(&self.client, pool).await
    }

    /// Discover Swiss National Bank series using the SNB Statistical Database
    pub async fn discover_snb_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        snb::discover_snb_series(&self.client, pool).await
    }

    /// Discover UN Statistics Division series using the UN Stats API
    pub async fn discover_unstats_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        unstats::discover_unstats_series(&self.client, pool).await
    }

    /// Discover International Labour Organization series using the ILO API
    pub async fn discover_ilo_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        ilo::discover_ilo_series(&self.client, pool).await
    }

    /// Discover FHFA series using the FHFA House Price Index API
    pub async fn discover_fhfa_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        fhfa::discover_fhfa_series(&self.client, pool).await
    }

    /// Search FRED series by query
    pub async fn search_fred_series(&self, query: &str) -> AppResult<Vec<fred::FredSeriesInfo>> {
        fred::search_fred_series(&self.client, &self.fred_api_key, query).await
    }

    /// Discover all series from all sources
    pub async fn discover_all_series(&self, pool: &DatabasePool) -> AppResult<Vec<String>> {
        let mut all_series = Vec::new();

        // Discover from each source
        if let Ok(fred_series) = self.discover_fred_series(pool).await {
            all_series.extend(fred_series);
        }

        if let Ok(bls_series) = self.discover_bls_series(pool).await {
            all_series.extend(bls_series);
        }

        if let Ok(census_series) = self.discover_census_series(pool).await {
            all_series.extend(census_series);
        }

        if let Ok(bea_series) = self.discover_bea_series(pool).await {
            all_series.extend(bea_series);
        }

        if let Ok(world_bank_series) = self.discover_world_bank_series(pool).await {
            all_series.extend(world_bank_series);
        }

        if let Ok(imf_series) = self.discover_imf_series(pool).await {
            all_series.extend(imf_series);
        }

        if let Ok(ecb_series) = self.discover_ecb_series(pool).await {
            all_series.extend(ecb_series);
        }

        if let Ok(oecd_series) = self.discover_oecd_series(pool).await {
            all_series.extend(oecd_series);
        }

        if let Ok(boe_series) = self.discover_boe_series(pool).await {
            all_series.extend(boe_series);
        }

        if let Ok(wto_series) = self.discover_wto_series(pool).await {
            all_series.extend(wto_series);
        }

        if let Ok(boj_series) = self.discover_boj_series(pool).await {
            all_series.extend(boj_series);
        }

        if let Ok(rba_series) = self.discover_rba_series(pool).await {
            all_series.extend(rba_series);
        }

        if let Ok(boc_series) = self.discover_boc_series(pool).await {
            all_series.extend(boc_series);
        }

        if let Ok(snb_series) = self.discover_snb_series(pool).await {
            all_series.extend(snb_series);
        }

        if let Ok(unstats_series) = self.discover_unstats_series(pool).await {
            all_series.extend(unstats_series);
        }

        if let Ok(ilo_series) = self.discover_ilo_series(pool).await {
            all_series.extend(ilo_series);
        }

        if let Ok(fhfa_series) = self.discover_fhfa_series(pool).await {
            all_series.extend(fhfa_series);
        }

        Ok(all_series)
    }
}
