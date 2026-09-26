// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! **Static catalogs** — hardcoded series lists for sources that have no live integration.
//!
//! These ten sources (BOC, BOE, BOJ, ECB, ILO, OECD, RBA, SNB, UN_STATS, WTO) were previously
//! "discovered" by modules under `econ-graph-services/src/services/series_discovery/` that made
//! no HTTP calls and returned fixed lists. They are ported here verbatim (same external ids,
//! titles, descriptions, units, frequencies and data URLs) and clearly marked as static so
//! nobody mistakes them for live integrations:
//!
//! - [`StaticCatalogAdapter::discover`](crate::SourceAdapter::discover) returns the hardcoded
//!   list and never touches the network.
//! - [`StaticCatalogAdapter::fetch_series`](crate::SourceAdapter::fetch_series) always fails
//!   with [`CrawlError::Permanent`]: fetching observations is **not implemented** for these
//!   sources.
//! - [`StaticCatalogAdapter::is_static_catalog`] / [`IS_STATIC_CATALOG`] and
//!   [`is_static_catalog_source`] let status pages and the UI flag them.
//!
//! Because no HTTP is made, the testkit's `new(base_url)` constructor convention does not apply;
//! the constructors are one per source ([`StaticCatalogAdapter::boc`], ...). The `data_url`
//! values point at the real upstream sites and are informational only.
//!
//! Where the old lists reused one external id for several unrelated series, only the first
//! entry is kept: the old discovery upserted by `(source, external_id)`, so the later entries
//! were never stored anyway. Each catalog's doc comment lists what was dropped.

use async_trait::async_trait;
use chrono::NaiveDate;

use crate::adapter::{CrawlCtx, DiscoveredSeries, FetchedSeries, SourceAdapter};
use crate::error::CrawlError;
use crate::source::SourceId;

/// Marker: every [`StaticCatalogAdapter`] serves a hardcoded catalog, not live data.
pub const IS_STATIC_CATALOG: bool = true;

/// Sources whose only adapter is a [`StaticCatalogAdapter`].
pub const STATIC_CATALOG_SOURCES: [SourceId; 10] = [
    SourceId::Boc,
    SourceId::Boe,
    SourceId::Boj,
    SourceId::Ecb,
    SourceId::Ilo,
    SourceId::Oecd,
    SourceId::Rba,
    SourceId::Snb,
    SourceId::UnStats,
    SourceId::Wto,
];

/// Whether `source` is served by a static catalog (no live discovery, no data fetching).
pub fn is_static_catalog_source(source: SourceId) -> bool {
    STATIC_CATALOG_SOURCES.contains(&source)
}

/// One hardcoded catalog entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogEntry {
    /// The source's identifier for the series.
    pub external_id: &'static str,
    /// Human-readable title.
    pub title: &'static str,
    /// Longer description.
    pub description: &'static str,
    /// Units of measure.
    pub units: &'static str,
    /// Observation frequency.
    pub frequency: &'static str,
    /// Geographic coverage (kept from the old modules; not part of [`DiscoveredSeries`]).
    pub geographic_level: &'static str,
    /// Link to the series on the source's website (informational; never requested).
    pub data_url: &'static str,
}

impl CatalogEntry {
    fn to_discovered(self) -> DiscoveredSeries {
        DiscoveredSeries {
            external_id: self.external_id.to_string(),
            title: self.title.to_string(),
            description: Some(self.description.to_string()),
            units: Some(self.units.to_string()),
            frequency: Some(self.frequency.to_string()),
            data_url: Some(self.data_url.to_string()),
        }
    }
}

/// A [`SourceAdapter`] backed by a hardcoded catalog. **Not a live integration**: discovery
/// returns a fixed list without any HTTP, and [`fetch_series`](SourceAdapter::fetch_series)
/// always returns [`CrawlError::Permanent`].
#[derive(Debug, Clone, Copy)]
pub struct StaticCatalogAdapter {
    source: SourceId,
    entries: &'static [CatalogEntry],
}

impl StaticCatalogAdapter {
    /// Bank of Canada (static catalog).
    pub fn boc() -> Self {
        Self::new(SourceId::Boc, BOC_CATALOG)
    }
    /// Bank of England (static catalog).
    pub fn boe() -> Self {
        Self::new(SourceId::Boe, BOE_CATALOG)
    }
    /// Bank of Japan (static catalog).
    pub fn boj() -> Self {
        Self::new(SourceId::Boj, BOJ_CATALOG)
    }
    /// European Central Bank (static catalog).
    pub fn ecb() -> Self {
        Self::new(SourceId::Ecb, ECB_CATALOG)
    }
    /// International Labour Organization (static catalog).
    pub fn ilo() -> Self {
        Self::new(SourceId::Ilo, ILO_CATALOG)
    }
    /// OECD (static catalog).
    pub fn oecd() -> Self {
        Self::new(SourceId::Oecd, OECD_CATALOG)
    }
    /// Reserve Bank of Australia (static catalog).
    pub fn rba() -> Self {
        Self::new(SourceId::Rba, RBA_CATALOG)
    }
    /// Swiss National Bank (static catalog).
    pub fn snb() -> Self {
        Self::new(SourceId::Snb, SNB_CATALOG)
    }
    /// UN Statistics Division (static catalog).
    pub fn un_stats() -> Self {
        Self::new(SourceId::UnStats, UN_STATS_CATALOG)
    }
    /// World Trade Organization (static catalog).
    pub fn wto() -> Self {
        Self::new(SourceId::Wto, WTO_CATALOG)
    }

    /// All ten static-catalog adapters, in [`STATIC_CATALOG_SOURCES`] order.
    pub fn all() -> [Self; 10] {
        [
            Self::boc(),
            Self::boe(),
            Self::boj(),
            Self::ecb(),
            Self::ilo(),
            Self::oecd(),
            Self::rba(),
            Self::snb(),
            Self::un_stats(),
            Self::wto(),
        ]
    }

    const fn new(source: SourceId, entries: &'static [CatalogEntry]) -> Self {
        Self { source, entries }
    }

    /// Always `true`: this adapter serves a hardcoded list, not live data.
    pub const fn is_static_catalog(&self) -> bool {
        IS_STATIC_CATALOG
    }

    /// The hardcoded entries.
    pub fn entries(&self) -> &'static [CatalogEntry] {
        self.entries
    }
}

#[async_trait]
impl SourceAdapter for StaticCatalogAdapter {
    fn id(&self) -> SourceId {
        self.source
    }

    /// Returns the hardcoded catalog. Makes no HTTP requests.
    async fn discover(&self, _ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        Ok(self.entries.iter().map(|e| e.to_discovered()).collect())
    }

    /// Always fails: fetching data is not implemented for static catalogs.
    async fn fetch_series(
        &self,
        _ctx: &CrawlCtx,
        _external_id: &str,
        _since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        Err(CrawlError::Permanent(format!(
            "{} is a static catalog: fetching data is not implemented",
            self.source.as_str()
        )))
    }
}

// ---------------------------------------------------------------------------------------------
// Catalog data (generated from the old series_discovery modules; do not edit by hand without
// a reason — these ids are what existing `economic_series` rows were keyed on).
// ---------------------------------------------------------------------------------------------

/// Ported from `econ-graph-services/src/services/series_discovery/boc.rs` (3 entries there, 3 distinct external ids).
pub const BOC_CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        external_id: "V39079",
        title: "Canada - Overnight Rate Target",
        description: "The target for the overnight rate set by the Bank of Canada",
        units: "Percent per annum",
        frequency: "Daily",
        geographic_level: "Canada",
        data_url: "https://www.bankofcanada.ca/valet/observations/V39079",
    },
    CatalogEntry {
        external_id: "V41690914",
        title: "Canada - Consumer Price Index",
        description: "Consumer Price Index for Canada",
        units: "Index (2002=100)",
        frequency: "Monthly",
        geographic_level: "Canada",
        data_url: "https://www.bankofcanada.ca/valet/observations/V41690914",
    },
    CatalogEntry {
        external_id: "V62700578",
        title: "Canada - GDP",
        description: "Gross Domestic Product for Canada",
        units: "Millions of Canadian dollars",
        frequency: "Quarterly",
        geographic_level: "Canada",
        data_url: "https://www.bankofcanada.ca/valet/observations/V62700578",
    },
];

/// Ported from `econ-graph-services/src/services/series_discovery/boe.rs` (10 entries there, 3 distinct external ids).
///
/// The old list reused external ids for unrelated series; the old discovery upserted by
/// `(source, external_id)`, so only the first entry per id was ever stored. Dropped repeats:
/// - `IUDBEDR`: UK - Sterling Overnight Index Average (SONIA)
/// - `LPMVWYR`: UK - Retail Price Index (RPI)
/// - `LPMVWYR`: UK - GDP at current prices
/// - `LPMVWYR`: UK - GDP at constant prices
/// - `LPMVWYR`: UK - Unemployment rate
/// - `LPMVWYR`: UK - Employment rate
/// - `LPMVWYR`: UK - Financial Stability Report indicators
pub const BOE_CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        external_id: "IUDBEDR",
        title: "UK - Bank Rate",
        description: "Official Bank Rate set by the Bank of England's Monetary Policy Committee",
        units: "Percent per annum",
        frequency: "Monthly",
        geographic_level: "United Kingdom",
        data_url: "https://www.bankofengland.co.uk/boeapps/database/_iadb-fromshowcolumns.asp?csv.x=yes&Datefrom=01/Jan/1997&Dateto=01/Jan/2025&SeriesCodes=IUDBEDR&CSVF=TN&UsingCodes=Y&Filter=N&title=IUDBEDR&VPD=Y&VFD=N",
    },
    CatalogEntry {
        external_id: "LPMVWYR",
        title: "UK - Consumer Price Index (CPI)",
        description: "Consumer Price Index for the United Kingdom",
        units: "Index (2015=100)",
        frequency: "Monthly",
        geographic_level: "United Kingdom",
        data_url: "https://www.bankofengland.co.uk/boeapps/database/_iadb-fromshowcolumns.asp?csv.x=yes&Datefrom=01/Jan/1997&Dateto=01/Jan/2025&SeriesCodes=LPMVWYR&CSVF=TN&UsingCodes=Y&Filter=N&title=LPMVWYR&VPD=Y&VFD=N",
    },
    CatalogEntry {
        external_id: "XUDLBK67",
        title: "UK - Sterling effective exchange rate",
        description: "Sterling effective exchange rate index",
        units: "Index (2015=100)",
        frequency: "Daily",
        geographic_level: "United Kingdom",
        data_url: "https://www.bankofengland.co.uk/boeapps/database/_iadb-fromshowcolumns.asp?csv.x=yes&Datefrom=01/Jan/1997&Dateto=01/Jan/2025&SeriesCodes=XUDLBK67&CSVF=TN&UsingCodes=Y&Filter=N&title=XUDLBK67&VPD=Y&VFD=N",
    },
];

/// Ported from `econ-graph-services/src/services/series_discovery/boj.rs` (9 entries there, 9 distinct external ids).
pub const BOJ_CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        external_id: "BOJ_UNRATE",
        title: "Japan - Policy interest rate",
        description: "Policy interest rate set by the Bank of Japan",
        units: "Percent per annum",
        frequency: "Monthly",
        geographic_level: "Japan",
        data_url: "https://www.stat-search.boj.or.jp/ssi/mtshtml/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1",
    },
    CatalogEntry {
        external_id: "BOJ_TONAR",
        title: "Japan - Tokyo Overnight Average Rate (TONAR)",
        description: "Tokyo Overnight Average Rate - the benchmark interest rate for Japanese yen overnight unsecured transactions",
        units: "Percent per annum",
        frequency: "Daily",
        geographic_level: "Japan",
        data_url: "https://www.stat-search.boj.or.jp/ssi/mtshtml/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1",
    },
    CatalogEntry {
        external_id: "BOJ_CPI",
        title: "Japan - Consumer Price Index (CPI)",
        description: "Consumer Price Index for Japan",
        units: "Index (2015=100)",
        frequency: "Monthly",
        geographic_level: "Japan",
        data_url: "https://www.stat-search.boj.or.jp/ssi/mtshtml/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1",
    },
    CatalogEntry {
        external_id: "BOJ_CORE_CPI",
        title: "Japan - Core Consumer Price Index",
        description: "Core Consumer Price Index (excluding fresh food) for Japan",
        units: "Index (2015=100)",
        frequency: "Monthly",
        geographic_level: "Japan",
        data_url: "https://www.stat-search.boj.or.jp/ssi/mtshtml/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1",
    },
    CatalogEntry {
        external_id: "BOJ_GDP_CURRENT",
        title: "Japan - GDP at current prices",
        description: "Gross Domestic Product at current prices for Japan",
        units: "Trillions of yen",
        frequency: "Quarterly",
        geographic_level: "Japan",
        data_url: "https://www.stat-search.boj.or.jp/ssi/mtshtml/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1",
    },
    CatalogEntry {
        external_id: "BOJ_GDP_CONSTANT",
        title: "Japan - GDP at constant prices",
        description: "Gross Domestic Product at constant prices for Japan",
        units: "Trillions of yen",
        frequency: "Quarterly",
        geographic_level: "Japan",
        data_url: "https://www.stat-search.boj.or.jp/ssi/mtshtml/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1",
    },
    CatalogEntry {
        external_id: "BOJ_UNEMPLOYMENT",
        title: "Japan - Unemployment rate",
        description: "Unemployment rate for Japan",
        units: "Percent",
        frequency: "Monthly",
        geographic_level: "Japan",
        data_url: "https://www.stat-search.boj.or.jp/ssi/mtshtml/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1",
    },
    CatalogEntry {
        external_id: "BOJ_EMPLOYMENT",
        title: "Japan - Employment rate",
        description: "Employment rate for Japan",
        units: "Percent",
        frequency: "Monthly",
        geographic_level: "Japan",
        data_url: "https://www.stat-search.boj.or.jp/ssi/mtshtml/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1",
    },
    CatalogEntry {
        external_id: "BOJ_FINANCIAL_STABILITY",
        title: "Japan - Financial Stability indicators",
        description: "Key indicators from the Bank of Japan's Financial System Report",
        units: "Various",
        frequency: "Quarterly",
        geographic_level: "Japan",
        data_url: "https://www.stat-search.boj.or.jp/ssi/mtshtml/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1",
    },
];

/// Ported from `econ-graph-services/src/services/series_discovery/ecb.rs` (10 entries there, 4 distinct external ids).
///
/// The old list reused external ids for unrelated series; the old discovery upserted by
/// `(source, external_id)`, so only the first entry per id was ever stored. Dropped repeats:
/// - `ICP.M.U2.N.000000.4.ANR`: Euro area - Deposit facility rate
/// - `ICP.M.U2.N.000000.4.ANR`: Euro area - Marginal lending facility rate
/// - `ICP.M.U2.N.000000.4.ANR`: Euro area - HICP (all items)
/// - `ICP.M.U2.N.000000.4.ANR`: Euro area - HICP (excluding energy and food)
/// - `MNA.Q.N.I8.W2.S1.S1.B.B1GQ._Z._Z._Z.EUR.LR.N`: Euro area - GDP at constant prices
/// - `LFSI.M.20.S.UNEHRT.TOTAL0.15_74.T`: Euro area - Employment rate
pub const ECB_CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        external_id: "ICP.M.U2.N.000000.4.ANR",
        title: "Euro area - Main refinancing operations rate",
        description: "Interest rate for main refinancing operations in the euro area",
        units: "Percent per annum",
        frequency: "Monthly",
        geographic_level: "Euro area",
        data_url: "https://sdw-wsrest.ecb.europa.eu/service/data/ICP/M.U2.N.000000.4.ANR",
    },
    CatalogEntry {
        external_id: "MNA.Q.N.I8.W2.S1.S1.B.B1GQ._Z._Z._Z.EUR.LR.N",
        title: "Euro area - GDP at current prices",
        description: "Gross Domestic Product at current prices in the euro area",
        units: "Millions of euro",
        frequency: "Quarterly",
        geographic_level: "Euro area",
        data_url: "https://sdw-wsrest.ecb.europa.eu/service/data/MNA/Q.N.I8.W2.S1.S1.B.B1GQ._Z._Z._Z.EUR.LR.N",
    },
    CatalogEntry {
        external_id: "LFSI.M.20.S.UNEHRT.TOTAL0.15_74.T",
        title: "Euro area - Unemployment rate",
        description: "Unemployment rate in the euro area",
        units: "Percent",
        frequency: "Monthly",
        geographic_level: "Euro area",
        data_url: "https://sdw-wsrest.ecb.europa.eu/service/data/LFSI/M.20.S.UNEHRT.TOTAL0.15_74.T",
    },
    CatalogEntry {
        external_id: "BTS.M.U2.N.000000.4.ANR",
        title: "Euro area - Balance of trade",
        description: "Balance of trade in goods and services in the euro area",
        units: "Millions of euro",
        frequency: "Monthly",
        geographic_level: "Euro area",
        data_url: "https://sdw-wsrest.ecb.europa.eu/service/data/BTS/M.U2.N.000000.4.ANR",
    },
];

/// Ported from `econ-graph-services/src/services/series_discovery/ilo.rs` (3 entries there, 3 distinct external ids).
pub const ILO_CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        external_id: "ILO_UNEMPLOYMENT",
        title: "ILO - Unemployment Rate",
        description: "Unemployment rate from International Labour Organization",
        units: "Percent",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://www.ilo.org/global/statistics-and-databases/lang--en/index.htm",
    },
    CatalogEntry {
        external_id: "ILO_EMPLOYMENT",
        title: "ILO - Employment Rate",
        description: "Employment rate from International Labour Organization",
        units: "Percent",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://www.ilo.org/global/statistics-and-databases/lang--en/index.htm",
    },
    CatalogEntry {
        external_id: "ILO_LABOR_FORCE",
        title: "ILO - Labor Force Participation Rate",
        description: "Labor force participation rate from International Labour Organization",
        units: "Percent",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://www.ilo.org/global/statistics-and-databases/lang--en/index.htm",
    },
];

/// Ported from `econ-graph-services/src/services/series_discovery/oecd.rs` (12 entries there, 12 distinct external ids).
pub const OECD_CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        external_id: "SNA_TABLE1.1.GDP.B1_GE.CPCAR_M",
        title: "OECD - GDP at current prices",
        description: "Gross Domestic Product at current prices for OECD countries",
        units: "Millions of national currency",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://sdmx.oecd.org/public/rest/data/OECD.SNA_TABLE1,DSD_SNA_TABLE1@DF_SNA_TABLE1,1.0/1.GDP.B1_GE.CPCAR_M",
    },
    CatalogEntry {
        external_id: "SNA_TABLE1.1.GDP.B1_GE.CPMP_NAC",
        title: "OECD - GDP at constant prices",
        description: "Gross Domestic Product at constant prices for OECD countries",
        units: "Millions of national currency",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://sdmx.oecd.org/public/rest/data/OECD.SNA_TABLE1,DSD_SNA_TABLE1@DF_SNA_TABLE1,1.0/1.GDP.B1_GE.CPMP_NAC",
    },
    CatalogEntry {
        external_id: "SNA_TABLE1.1.GDP.B1_GE.CPMP_PPP",
        title: "OECD - GDP at constant prices (PPP)",
        description: "Gross Domestic Product at constant prices using purchasing power parity for OECD countries",
        units: "Millions of US dollars",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://sdmx.oecd.org/public/rest/data/OECD.SNA_TABLE1,DSD_SNA_TABLE1@DF_SNA_TABLE1,1.0/1.GDP.B1_GE.CPMP_PPP",
    },
    CatalogEntry {
        external_id: "PRICES_CPI.CPI.TOTIDX.M",
        title: "OECD - Consumer Price Index",
        description: "Consumer Price Index for OECD countries",
        units: "Index (2015=100)",
        frequency: "Monthly",
        geographic_level: "Country",
        data_url: "https://sdmx.oecd.org/public/rest/data/OECD.PRICES_CPI,DSD_PRICES_CPI@DF_PRICES_CPI,1.0/1.CPI.TOTIDX.M",
    },
    CatalogEntry {
        external_id: "PRICES_CPI.CPI.TOTIDX.Q",
        title: "OECD - Consumer Price Index (Quarterly)",
        description: "Consumer Price Index for OECD countries (quarterly)",
        units: "Index (2015=100)",
        frequency: "Quarterly",
        geographic_level: "Country",
        data_url: "https://sdmx.oecd.org/public/rest/data/OECD.PRICES_CPI,DSD_PRICES_CPI@DF_PRICES_CPI,1.0/1.CPI.TOTIDX.Q",
    },
    CatalogEntry {
        external_id: "LFS_SEXAGE_I_R.UNEM_RT.AGE15_64.T",
        title: "OECD - Unemployment rate",
        description: "Unemployment rate for OECD countries",
        units: "Percent",
        frequency: "Monthly",
        geographic_level: "Country",
        data_url: "https://sdmx.oecd.org/public/rest/data/OECD.LFS_SEXAGE_I_R,DSD_LFS_SEXAGE_I_R@DF_LFS_SEXAGE_I_R,1.0/1.UNEM_RT.AGE15_64.T",
    },
    CatalogEntry {
        external_id: "LFS_SEXAGE_I_R.EMP_RT.AGE15_64.T",
        title: "OECD - Employment rate",
        description: "Employment rate for OECD countries",
        units: "Percent",
        frequency: "Monthly",
        geographic_level: "Country",
        data_url: "https://sdmx.oecd.org/public/rest/data/OECD.LFS_SEXAGE_I_R,DSD_LFS_SEXAGE_I_R@DF_LFS_SEXAGE_I_R,1.0/1.EMP_RT.AGE15_64.T",
    },
    CatalogEntry {
        external_id: "TIS_GOODS_SERVICES.TIS_GOODS_SERVICES.TIS_GOODS_SERVICES.TIS_GOODS_SERVICES",
        title: "OECD - Trade in goods and services",
        description: "Trade in goods and services for OECD countries",
        units: "Millions of US dollars",
        frequency: "Monthly",
        geographic_level: "Country",
        data_url: "https://sdmx.oecd.org/public/rest/data/OECD.TIS_GOODS_SERVICES,DSD_TIS_GOODS_SERVICES@DF_TIS_GOODS_SERVICES,1.0/1.TIS_GOODS_SERVICES.TIS_GOODS_SERVICES.TIS_GOODS_SERVICES.TIS_GOODS_SERVICES",
    },
    CatalogEntry {
        external_id: "PDB_LV.1.1.GDP.TOTIDX",
        title: "OECD - Labour productivity",
        description: "Labour productivity for OECD countries",
        units: "Index (2015=100)",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://sdmx.oecd.org/public/rest/data/OECD.PDB_LV,DSD_PDB_LV@DF_PDB_LV,1.0/1.1.GDP.TOTIDX",
    },
    CatalogEntry {
        external_id: "EDULIT_IND.EDULIT_IND.EDULIT_IND.EDULIT_IND",
        title: "OECD - Education indicators",
        description: "Education indicators for OECD countries",
        units: "Various",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://sdmx.oecd.org/public/rest/data/OECD.EDULIT_IND,DSD_EDULIT_IND@DF_EDULIT_IND,1.0/1.EDULIT_IND.EDULIT_IND.EDULIT_IND.EDULIT_IND",
    },
    CatalogEntry {
        external_id: "HEALTH_STAT.HEALTH_STAT.HEALTH_STAT.HEALTH_STAT",
        title: "OECD - Health statistics",
        description: "Health statistics for OECD countries",
        units: "Various",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://sdmx.oecd.org/public/rest/data/OECD.HEALTH_STAT,DSD_HEALTH_STAT@DF_HEALTH_STAT,1.0/1.HEALTH_STAT.HEALTH_STAT.HEALTH_STAT.HEALTH_STAT",
    },
    CatalogEntry {
        external_id: "GREEN_GROWTH.GREEN_GROWTH.GREEN_GROWTH.GREEN_GROWTH",
        title: "OECD - Green growth indicators",
        description: "Green growth indicators for OECD countries",
        units: "Various",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://sdmx.oecd.org/public/rest/data/OECD.GREEN_GROWTH,DSD_GREEN_GROWTH@DF_GREEN_GROWTH,1.0/1.GREEN_GROWTH.GREEN_GROWTH.GREEN_GROWTH.GREEN_GROWTH",
    },
];

/// Ported from `econ-graph-services/src/services/series_discovery/rba.rs` (3 entries there, 2 distinct external ids).
///
/// The old list reused external ids for unrelated series; the old discovery upserted by
/// `(source, external_id)`, so only the first entry per id was ever stored. Dropped repeats:
/// - `G1`: Australia - GDP
pub const RBA_CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        external_id: "F1.1",
        title: "Australia - Cash Rate Target",
        description: "The cash rate target set by the Reserve Bank Board",
        units: "Percent per annum",
        frequency: "Monthly",
        geographic_level: "Australia",
        data_url: "https://www.rba.gov.au/statistics/f01-hist.html",
    },
    CatalogEntry {
        external_id: "G1",
        title: "Australia - Consumer Price Index",
        description: "Consumer Price Index for Australia",
        units: "Index (2011-12=100)",
        frequency: "Quarterly",
        geographic_level: "Australia",
        data_url: "https://www.rba.gov.au/statistics/g01-hist.html",
    },
];

/// Ported from `econ-graph-services/src/services/series_discovery/snb.rs` (3 entries there, 3 distinct external ids).
pub const SNB_CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        external_id: "ir",
        title: "Switzerland - SNB Policy Rate",
        description: "Swiss National Bank policy rate",
        units: "Percent per annum",
        frequency: "Daily",
        geographic_level: "Switzerland",
        data_url: "https://data.snb.ch/en/ir",
    },
    CatalogEntry {
        external_id: "gdp",
        title: "Switzerland - GDP",
        description: "Gross Domestic Product for Switzerland",
        units: "Billions of Swiss francs",
        frequency: "Quarterly",
        geographic_level: "Switzerland",
        data_url: "https://data.snb.ch/en/gdp",
    },
    CatalogEntry {
        external_id: "cpi",
        title: "Switzerland - Consumer Price Index",
        description: "Consumer Price Index for Switzerland",
        units: "Index (December 2020=100)",
        frequency: "Monthly",
        geographic_level: "Switzerland",
        data_url: "https://data.snb.ch/en/cpi",
    },
];

/// Ported from `econ-graph-services/src/services/series_discovery/unstats.rs` (3 entries there, 3 distinct external ids).
pub const UN_STATS_CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        external_id: "UN_GDP",
        title: "UN - GDP per capita",
        description: "Gross Domestic Product per capita from UN Statistics Division",
        units: "Current US dollars",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://unstats.un.org/unsd/snaama/Basic",
    },
    CatalogEntry {
        external_id: "UN_POPULATION",
        title: "UN - Total Population",
        description: "Total population estimates from UN Statistics Division",
        units: "Persons",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://unstats.un.org/unsd/demographic-social/products/dyb",
    },
    CatalogEntry {
        external_id: "UN_LIFE_EXPECTANCY",
        title: "UN - Life Expectancy at Birth",
        description: "Life expectancy at birth from UN Statistics Division",
        units: "Years",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://unstats.un.org/unsd/demographic-social/products/dyb",
    },
];

/// Ported from `econ-graph-services/src/services/series_discovery/wto.rs` (5 entries there, 5 distinct external ids).
pub const WTO_CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        external_id: "MT_GOODS_EXP",
        title: "WTO - Merchandise exports",
        description: "Merchandise exports for WTO member countries",
        units: "Millions of US dollars",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://api.wto.org/timeseries/v1/data/MT_GOODS_EXP",
    },
    CatalogEntry {
        external_id: "MT_GOODS_IMP",
        title: "WTO - Merchandise imports",
        description: "Merchandise imports for WTO member countries",
        units: "Millions of US dollars",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://api.wto.org/timeseries/v1/data/MT_GOODS_IMP",
    },
    CatalogEntry {
        external_id: "ST_SERVICES_EXP",
        title: "WTO - Services exports",
        description: "Services exports for WTO member countries",
        units: "Millions of US dollars",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://api.wto.org/timeseries/v1/data/ST_SERVICES_EXP",
    },
    CatalogEntry {
        external_id: "ST_SERVICES_IMP",
        title: "WTO - Services imports",
        description: "Services imports for WTO member countries",
        units: "Millions of US dollars",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://api.wto.org/timeseries/v1/data/ST_SERVICES_IMP",
    },
    CatalogEntry {
        external_id: "TP_TARIFFS",
        title: "WTO - Applied tariffs",
        description: "Applied tariffs for WTO member countries",
        units: "Percent",
        frequency: "Annual",
        geographic_level: "Country",
        data_url: "https://api.wto.org/timeseries/v1/data/TP_TARIFFS",
    },
];

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::testkit::{test_ctx, MockSource};

    /// Expected distinct entries per source, counted from the old
    /// `econ-graph-services/src/services/series_discovery/*.rs` `discover_series()` lists
    /// (raw list length in the comment; duplicates by external id dropped, see module docs).
    const EXPECTED: [(SourceId, usize); 10] = [
        (SourceId::Boc, 3),     // boc.rs: 3 entries, 3 distinct ids
        (SourceId::Boe, 3),     // boe.rs: 10 entries, 3 distinct ids
        (SourceId::Boj, 9),     // boj.rs: 9 entries, 9 distinct ids
        (SourceId::Ecb, 4),     // ecb.rs: 10 entries, 4 distinct ids
        (SourceId::Ilo, 3),     // ilo.rs: 3 entries, 3 distinct ids
        (SourceId::Oecd, 12),   // oecd.rs: 12 entries, 12 distinct ids
        (SourceId::Rba, 2),     // rba.rs: 3 entries, 2 distinct ids
        (SourceId::Snb, 3),     // snb.rs: 3 entries, 3 distinct ids
        (SourceId::UnStats, 3), // unstats.rs: 3 entries, 3 distinct ids
        (SourceId::Wto, 5),     // wto.rs: 5 entries, 5 distinct ids
    ];

    #[test]
    fn constructors_cover_every_static_source() {
        let ids: Vec<_> = StaticCatalogAdapter::all().iter().map(|a| a.id()).collect();
        assert_eq!(ids, STATIC_CATALOG_SOURCES.to_vec());
        for a in StaticCatalogAdapter::all() {
            assert!(a.is_static_catalog());
            assert!(is_static_catalog_source(a.id()));
        }
        assert!(!is_static_catalog_source(SourceId::Fred));
        assert!(!is_static_catalog_source(SourceId::Bls));
    }

    #[test]
    fn catalogs_non_empty_unique_and_expected_counts() {
        for (adapter, (source, expected)) in StaticCatalogAdapter::all().iter().zip(EXPECTED) {
            assert_eq!(adapter.id(), source);
            let entries = adapter.entries();
            assert!(!entries.is_empty(), "{source} catalog is empty");
            assert_eq!(entries.len(), expected, "{source} entry count");
            let ids: HashSet<_> = entries.iter().map(|e| e.external_id).collect();
            assert_eq!(
                ids.len(),
                entries.len(),
                "{source} has duplicate external ids"
            );
            for e in entries {
                assert!(!e.external_id.is_empty() && !e.title.is_empty());
                assert!(e.data_url.starts_with("https://"), "{}", e.data_url);
            }
        }
    }

    #[test]
    fn spot_check_ported_values() {
        let boc = StaticCatalogAdapter::boc().entries()[0];
        assert_eq!(boc.external_id, "V39079");
        assert_eq!(boc.title, "Canada - Overnight Rate Target");
        assert_eq!(boc.units, "Percent per annum");
        assert_eq!(boc.frequency, "Daily");
        assert_eq!(
            boc.data_url,
            "https://www.bankofcanada.ca/valet/observations/V39079"
        );
        let boe_ids: Vec<_> = BOE_CATALOG.iter().map(|e| e.external_id).collect();
        assert_eq!(boe_ids, ["IUDBEDR", "LPMVWYR", "XUDLBK67"]);
        assert_eq!(BOE_CATALOG[0].title, "UK - Bank Rate");
    }

    #[tokio::test]
    async fn discover_returns_catalog_without_http() {
        // Static adapters never use ctx.http; a mock server that nothing points at stays idle,
        // and discover succeeds even though no upstream is reachable.
        let mock = MockSource::start().await;
        let ctx = test_ctx();
        for adapter in StaticCatalogAdapter::all() {
            let found = adapter.discover(&ctx).await.expect("static discover");
            assert_eq!(found.len(), adapter.entries().len());
            for (d, e) in found.iter().zip(adapter.entries()) {
                assert_eq!(d.external_id, e.external_id);
                assert_eq!(d.title, e.title);
                assert_eq!(d.description.as_deref(), Some(e.description));
                assert_eq!(d.units.as_deref(), Some(e.units));
                assert_eq!(d.frequency.as_deref(), Some(e.frequency));
                assert_eq!(d.data_url.as_deref(), Some(e.data_url));
            }
        }
        assert!(mock.received_requests().await.is_empty());
    }

    #[tokio::test]
    async fn fetch_series_is_permanent_error() {
        let ctx = test_ctx();
        for adapter in StaticCatalogAdapter::all() {
            let id = adapter.entries()[0].external_id;
            let err = adapter.fetch_series(&ctx, id, None).await.unwrap_err();
            assert!(matches!(err, CrawlError::Permanent(_)), "{err:?}");
            assert!(!err.is_retryable());
            let expected = format!(
                "{} is a static catalog: fetching data is not implemented",
                adapter.id().as_str()
            );
            assert!(err.to_string().contains(&expected), "{err}");
        }
    }

    #[test]
    fn default_registry_registers_static_catalogs() {
        let registry = crate::sources::default_registry();
        for source in STATIC_CATALOG_SOURCES {
            assert_eq!(registry.get(source).map(|a| a.id()), Some(source));
        }
    }
}
