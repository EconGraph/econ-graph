// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! The ticker symbol table: which tickers to fetch, keyed by CIK, and when each was active.

use chrono::NaiveDate;

/// One security under one ticker for a date range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRow {
    /// OpenFIGI share-class FIGI, e.g. `BBG001S5N8V8` (Apple). Series are keyed on it.
    pub share_class_figi: String,
    /// SEC CIK zero-padded to 10 digits, e.g. `0000320193`. GOOG and GOOGL share one.
    pub cik10: String,
    /// Ticker as Massive spells it, share classes with a dot: `AAPL`, `BRK.B`.
    pub ticker: String,
    /// Primary exchange MIC, e.g. `XNYS`, `XNAS`.
    pub exchange: String,
    /// ISO 4217 currency of the prices, `USD` for the S&P 500.
    pub currency: String,
    /// Company name, with the share class where there are several.
    pub name: String,
    /// First date the row applies.
    pub valid_from: NaiveDate,
    /// Last date the row applies (inclusive), or `None` while it is current.
    pub valid_to: Option<NaiveDate>,
}
