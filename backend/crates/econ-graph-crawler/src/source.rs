// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Identifiers for the external data sources the crawler talks to.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// An external data source.
///
/// The canonical string form (see [`SourceId::as_str`]) is the exact value stored in
/// `crawl_queue.source`, and is also the serde representation. Parsing via [`FromStr`]
/// accepts the canonical strings case-insensitively and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SourceId {
    /// Federal Reserve Economic Data (St. Louis Fed). `"FRED"`
    #[serde(rename = "FRED")]
    Fred,
    /// U.S. Bureau of Labor Statistics. `"BLS"`
    #[serde(rename = "BLS")]
    Bls,
    /// U.S. Bureau of Economic Analysis. `"BEA"`
    #[serde(rename = "BEA")]
    Bea,
    /// U.S. Census Bureau. `"CENSUS"`
    #[serde(rename = "CENSUS")]
    Census,
    /// World Bank. `"WORLD_BANK"`
    #[serde(rename = "WORLD_BANK")]
    WorldBank,
    /// International Monetary Fund. `"IMF"`
    #[serde(rename = "IMF")]
    Imf,
    /// European Central Bank. `"ECB"`
    #[serde(rename = "ECB")]
    Ecb,
    /// Organisation for Economic Co-operation and Development. `"OECD"`
    #[serde(rename = "OECD")]
    Oecd,
    /// Bank of England. `"BOE"`
    #[serde(rename = "BOE")]
    Boe,
    /// Bank of Japan. `"BOJ"`
    #[serde(rename = "BOJ")]
    Boj,
    /// Bank of Canada. `"BOC"`
    #[serde(rename = "BOC")]
    Boc,
    /// Reserve Bank of Australia. `"RBA"`
    #[serde(rename = "RBA")]
    Rba,
    /// Swiss National Bank. `"SNB"`
    #[serde(rename = "SNB")]
    Snb,
    /// United Nations Statistics Division. `"UN_STATS"`
    #[serde(rename = "UN_STATS")]
    UnStats,
    /// International Labour Organization. `"ILO"`
    #[serde(rename = "ILO")]
    Ilo,
    /// World Trade Organization. `"WTO"`
    #[serde(rename = "WTO")]
    Wto,
    /// U.S. Federal Housing Finance Agency. `"FHFA"`
    #[serde(rename = "FHFA")]
    Fhfa,
    /// U.S. Securities and Exchange Commission (EDGAR). `"SEC"`
    #[serde(rename = "SEC")]
    Sec,
}

impl SourceId {
    /// Every source, in declaration order.
    pub const ALL: [SourceId; 18] = [
        SourceId::Fred,
        SourceId::Bls,
        SourceId::Bea,
        SourceId::Census,
        SourceId::WorldBank,
        SourceId::Imf,
        SourceId::Ecb,
        SourceId::Oecd,
        SourceId::Boe,
        SourceId::Boj,
        SourceId::Boc,
        SourceId::Rba,
        SourceId::Snb,
        SourceId::UnStats,
        SourceId::Ilo,
        SourceId::Wto,
        SourceId::Fhfa,
        SourceId::Sec,
    ];

    /// Canonical upper-snake-case name, as stored in `crawl_queue.source`.
    pub const fn as_str(self) -> &'static str {
        match self {
            SourceId::Fred => "FRED",
            SourceId::Bls => "BLS",
            SourceId::Bea => "BEA",
            SourceId::Census => "CENSUS",
            SourceId::WorldBank => "WORLD_BANK",
            SourceId::Imf => "IMF",
            SourceId::Ecb => "ECB",
            SourceId::Oecd => "OECD",
            SourceId::Boe => "BOE",
            SourceId::Boj => "BOJ",
            SourceId::Boc => "BOC",
            SourceId::Rba => "RBA",
            SourceId::Snb => "SNB",
            SourceId::UnStats => "UN_STATS",
            SourceId::Ilo => "ILO",
            SourceId::Wto => "WTO",
            SourceId::Fhfa => "FHFA",
            SourceId::Sec => "SEC",
        }
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Returned when a string is not a canonical [`SourceId`] name.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown data source: {0:?}")]
pub struct UnknownSource(pub String);

impl FromStr for SourceId {
    type Err = UnknownSource;

    /// Parses a canonical name (e.g. `"WORLD_BANK"`), ignoring ASCII case.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        SourceId::ALL
            .into_iter()
            .find(|id| id.as_str().eq_ignore_ascii_case(s))
            .ok_or_else(|| UnknownSource(s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_round_trips_through_as_str() {
        for id in SourceId::ALL {
            assert_eq!(id.as_str().parse::<SourceId>(), Ok(id));
            assert_eq!(id.to_string(), id.as_str());
        }
    }

    #[test]
    fn parse_is_case_insensitive() {
        assert_eq!("world_bank".parse::<SourceId>(), Ok(SourceId::WorldBank));
        assert_eq!("Fred".parse::<SourceId>(), Ok(SourceId::Fred));
        assert_eq!("un_Stats".parse::<SourceId>(), Ok(SourceId::UnStats));
    }

    #[test]
    fn parse_rejects_non_canonical_names() {
        assert!("World Bank".parse::<SourceId>().is_err());
        assert!("WORLDBANK".parse::<SourceId>().is_err());
        assert!("".parse::<SourceId>().is_err());
    }

    #[test]
    fn canonical_names_are_unique_upper_snake_case() {
        let mut names: Vec<_> = SourceId::ALL.iter().map(|s| s.as_str()).collect();
        for n in &names {
            assert!(n.chars().all(|c| c.is_ascii_uppercase() || c == '_'), "{n}");
        }
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SourceId::ALL.len());
    }

    #[test]
    fn serde_uses_canonical_names() {
        for id in SourceId::ALL {
            let json = serde_json::to_string(&id).unwrap();
            assert_eq!(json, format!("\"{}\"", id.as_str()));
            assert_eq!(serde_json::from_str::<SourceId>(&json).unwrap(), id);
        }
    }
}
