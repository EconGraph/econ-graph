// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! # EconGraph Auth
//!
//! Authentication, authorization, and user management for the EconGraph system.
//! This crate provides secure authentication and authorization services.

pub mod auth;

// Re-export commonly used auth types
pub use auth::*;
