// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Massive, a market data vendor: daily stock prices for the S&P 500 constituents.
//!
//! The data is licensed and readable only with `catalog.massive_market_data:read`, which is
//! granted to nobody by default.

pub mod adapter;
pub mod backfill;
pub mod calendar;
pub mod client;
pub mod dataset;
pub mod symbols;
