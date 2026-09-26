//! SEC EDGAR XBRL Crawler
//!
//! This crate provides functionality for crawling SEC EDGAR filings and downloading
//! XBRL financial data. HTTP goes through the shared `econ_graph_crawler::HttpFetcher`
//! (SEC rate limit, retries, metrics); [`handler::SecFilingHandler`] runs company crawls as
//! `crawl_queue` `fetch_filing` jobs.

pub mod config_loader;
pub mod crawler;
pub mod dts_manager;
pub mod financial_ratio_calculator;
pub mod handler;
pub mod models;
pub mod storage;
pub mod submissions;
pub mod utils;
pub mod xbrl_parser;
pub mod xbrl_parser_tests;

pub use config_loader::{
    ConceptMappingsConfig, FinancialAnalysisConfig, RatioBenchmarksConfig, RatioFormulasConfig,
    RatioInterpretationsConfig,
};
pub use crawler::{normalize_cik, sec_http_fetcher, CompanyCrawl, SecEdgarCrawler, SecEndpoints};
pub use dts_manager::DtsManager;
pub use financial_ratio_calculator::{
    CalculatedRatio, FinancialRatioCalculator, RatioCalculationConfig,
};
pub use handler::{enqueue_filings, SecFilingHandler};
pub use models::*;
pub use storage::XbrlStorage;
pub use xbrl_parser::{
    DocumentType, FinancialRatio, TaxonomyConcept, ValidationReport, XbrlParseResult, XbrlParser,
    XbrlParserConfig,
};

/// Re-export commonly used types
pub use anyhow::Result;
pub use bigdecimal::BigDecimal;
pub use chrono::{DateTime, NaiveDate, Utc};
pub use uuid::Uuid;
