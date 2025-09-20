//! SEC EDGAR XBRL Crawler
//!
//! This crate provides functionality for crawling SEC EDGAR filings and downloading
//! XBRL financial data. It includes comprehensive error handling, rate limiting,
//! retry logic, and progress tracking for reliable data acquisition.

pub mod config_loader;
pub mod crawler;
pub mod dts_manager;
pub mod financial_ratio_calculator;
pub mod models;
pub mod rate_limiter;
pub mod storage;
pub mod utils;
pub mod xbrl_parser;
pub mod xbrl_parser_tests;

pub use config_loader::{
    ConceptMappingsConfig, FinancialAnalysisConfig, RatioBenchmarksConfig, RatioFormulasConfig,
    RatioInterpretationsConfig,
};
pub use crawler::SecEdgarCrawler;
pub use dts_manager::DtsManager;
pub use financial_ratio_calculator::{
    CalculatedRatio, FinancialRatioCalculator, RatioCalculationConfig,
};
pub use models::*;
pub use rate_limiter::SecRateLimiter;
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
