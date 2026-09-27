//! SEC EDGAR XBRL Crawler
//!
//! This crate provides functionality for crawling SEC EDGAR filings and downloading
//! XBRL financial data. HTTP goes through the shared `econ_graph_crawler::HttpFetcher`
//! (SEC rate limit, retries, metrics); [`handler::SecFilingHandler`] runs company crawls as
//! `crawl_queue` `fetch_filing` jobs.
//!
//! The XBRL parser, DTS (taxonomy) download, financial ratio calculator and their JSON config
//! are unfinished and compiled out by default. Enable the `xbrl-parser` feature to build them.

#[cfg(feature = "xbrl-parser")]
pub mod config_loader;
pub mod crawler;
#[cfg(feature = "xbrl-parser")]
pub mod dts_manager;
#[cfg(feature = "xbrl-parser")]
pub mod financial_ratio_calculator;
pub mod handler;
pub mod models;
pub mod storage;
pub mod submissions;
pub mod utils;
#[cfg(feature = "xbrl-parser")]
pub mod xbrl_parser;
#[cfg(all(test, feature = "xbrl-parser"))]
mod xbrl_parser_tests;

#[cfg(feature = "xbrl-parser")]
pub use config_loader::{
    ConceptMappingsConfig, FinancialAnalysisConfig, RatioBenchmarksConfig, RatioFormulasConfig,
    RatioInterpretationsConfig,
};
pub use crawler::{normalize_cik, sec_http_fetcher, CompanyCrawl, SecEdgarCrawler, SecEndpoints};
#[cfg(feature = "xbrl-parser")]
pub use dts_manager::DtsManager;
#[cfg(feature = "xbrl-parser")]
pub use financial_ratio_calculator::{
    CalculatedRatio, FinancialRatioCalculator, RatioCalculationConfig,
};
pub use handler::{enqueue_filings, SecFilingHandler};
pub use models::*;
pub use storage::XbrlStorage;
#[cfg(feature = "xbrl-parser")]
pub use xbrl_parser::{
    DocumentType, FinancialRatio, TaxonomyConcept, ValidationReport, XbrlParseResult, XbrlParser,
    XbrlParserConfig,
};

/// Re-export commonly used types
pub use anyhow::Result;
pub use bigdecimal::BigDecimal;
pub use chrono::{DateTime, NaiveDate, Utc};
pub use uuid::Uuid;
