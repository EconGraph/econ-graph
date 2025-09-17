// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! # EconGraph Services
//!
//! Business logic, data processing, and service implementations for the EconGraph system.
//! This crate contains all the core business logic and external API integrations.

pub mod services;

// Re-export commonly used services
pub use services::*;
