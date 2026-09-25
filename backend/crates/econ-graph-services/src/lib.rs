// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! # EconGraph Services
//!
//! Business logic, data processing, and service implementations for the `EconGraph` system.
//! This crate contains all the core business logic, external API integrations, and data
//! processing services that power the economic data platform.
//!
//! ## Services
//!
//! - **Search**: full-text series search (`search_service`)
//! - **Series**: series queries (`series_service`)
//! - **Global analysis**: cross-country analysis (`global_analysis_service`)
//! - **Queue**: `crawl_queue` statistics and admin helpers (`queue_service`)
//! - **Collaboration**: chart annotations and sharing (`collaboration_service`)
//!
//! Data collection (source adapters, the queue worker, the `crawler` CLI) lives in the
//! `econ-graph-crawler` crate, not here.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use econ_graph_services::services::search_service::SearchService;
//! ```

pub mod services;

// Re-export commonly used services
pub use services::*;
