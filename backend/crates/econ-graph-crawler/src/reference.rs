// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Reference data shared by adapters, read from data files at runtime (not compiled in).
//!
//! The files live in the directory named by `CRAWLER_DATA_DIR`. When it is unset they are read
//! from this crate's `data/` directory in the source tree, which is what `cargo run` and
//! `cargo test` use. The crawler-worker image copies `data/` to `/app/data` and sets
//! `CRAWLER_DATA_DIR` to it.
//!
//! Each file is read once per process and cached, including a failure to read it, so the
//! worker checks them at startup ([`bls_series`], [`boc_series`], [`wdi_indicators`],
//! [`wto_indicators`]) rather than on its first job.
//!
//! Dataset definitions ([`datasets`]) are cached the same way; the worker loads them at startup
//! through [`DatasetCatalog::load`](crate::dataset::DatasetCatalog::load).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::dataset::{parse_dataset_file, DatasetDef};
use crate::error::CrawlError;
use crate::source::SourceId;

/// Environment variable naming the reference data directory.
pub const DATA_DIR_ENV: &str = "CRAWLER_DATA_DIR";

/// File name of the World Development Indicators list in the data directory.
pub const WDI_INDICATORS_FILE: &str = "wdi_indicators.csv";

/// Directory under [`data_dir`] holding one `<source>.toml` of dataset definitions per source.
pub const DATASETS_DIR: &str = "datasets";

/// File name of the BLS series list in the data directory.
pub const BLS_SERIES_FILE: &str = "bls_series.csv";

/// File name of the curated WTO Timeseries indicator list in the data directory.
pub const WTO_INDICATORS_FILE: &str = "wto_indicators.csv";

/// File name of the curated Bank of Canada Valet series list in the data directory.
pub const BOC_SERIES_FILE: &str = "boc_series.csv";

/// A BLS series the crawler discovers and fetches. Only the id is curated here: its title, units
/// and frequency come from BLS (see [`crate::sources::bls`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlsSeries {
    /// BLS series id, e.g. `CUUR0000SA0`.
    pub id: String,
}

/// The reference data directory: `$CRAWLER_DATA_DIR`, or this crate's `data/` directory.
pub fn data_dir() -> PathBuf {
    std::env::var_os(DATA_DIR_ENV)
        .filter(|v| !v.is_empty())
        .map_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).join("data"),
            PathBuf::from,
        )
}

/// One World Development Indicator the World Bank adapter crawls. Its name and description are
/// not shipped here: [`crate::sources::world_bank`]'s
/// [`code_lists`](crate::adapter::SourceAdapter::code_lists) fetches them, one indicator at a
/// time, from the World Bank's own `/indicator/{id}` endpoint.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WdiIndicator {
    /// Indicator code, e.g. `NY.GDP.PCAP.CD`.
    pub id: String,
    /// Unit stored as each series' units, e.g. `current US$`.
    pub unit: String,
}

/// The indicators from `wdi_indicators.csv` in [`data_dir`], read on first use and cached.
///
/// A missing, malformed or empty file, a duplicate id, or an id that could not be part of a
/// canonical series id (empty, or containing `/` or whitespace) is a `Permanent` error, with the
/// path in the message.
pub fn wdi_indicators() -> Result<&'static [WdiIndicator], CrawlError> {
    static INDICATORS: OnceLock<Result<Vec<WdiIndicator>, String>> = OnceLock::new();
    INDICATORS
        .get_or_init(|| load_wdi_indicators(&data_dir().join(WDI_INDICATORS_FILE)))
        .as_deref()
        .map_err(|e| CrawlError::Permanent(e.clone()))
}

fn load_wdi_indicators(path: &Path) -> Result<Vec<WdiIndicator>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("reading {}: {e} (set {DATA_DIR_ENV})", path.display()))?;
    parse_wdi_indicators(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Parses `id,unit` rows after a header line (`#` comment lines skipped; fields may be quoted)
/// and checks them.
fn parse_wdi_indicators(text: &str) -> Result<Vec<WdiIndicator>, String> {
    let mut reader = csv::ReaderBuilder::new()
        .comment(Some(b'#'))
        .trim(csv::Trim::All)
        .from_reader(text.as_bytes());
    let header = reader.headers().map_err(|e| e.to_string())?.clone();
    if header.iter().ne(["id", "unit"]) {
        return Err(format!(
            "expected header id,unit, got {}",
            header.iter().collect::<Vec<_>>().join(",")
        ));
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for record in reader.deserialize::<WdiIndicator>() {
        let row = record.map_err(|e| e.to_string())?;
        let id = &row.id;
        if id.is_empty() || id.contains('/') || id.contains(char::is_whitespace) {
            return Err(format!("indicator id {id:?} is empty or has '/' or spaces"));
        }
        if row.unit.is_empty() {
            return Err(format!("indicator {id}: empty unit"));
        }
        if !seen.insert(id.clone()) {
            return Err(format!("indicator {id} is listed twice"));
        }
        out.push(row);
    }
    if out.is_empty() {
        return Err("no indicators".into());
    }
    Ok(out)
}

/// One WTO Timeseries indicator the WTO adapter crawls, with the product/sector and partner
/// economy that select its headline series. Its name, unit and frequency are not shipped here:
/// [`crate::sources::wto`] reads them from the WTO's own `/indicators` endpoint.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WtoIndicator {
    /// WTO indicator code, e.g. `ITS_MTV_AX`.
    pub id: String,
    /// Product/sector code sent as `pc` (e.g. `TO`, total merchandise), or empty for an indicator
    /// requested without one (the API's default).
    pub product: String,
    /// Partner economy code sent as `p` (e.g. `000`, the world), or empty for an indicator
    /// requested without one (the API's default).
    pub partner: String,
}

/// The indicators from `wto_indicators.csv` in [`data_dir`], read on first use and cached.
///
/// A missing, malformed or empty file, a duplicate id, or an id, product or partner that is not
/// made of ASCII letters, digits and `_` (so it can break neither a request nor a canonical
/// series id) is a `Permanent` error, with the path in the message.
pub fn wto_indicators() -> Result<&'static [WtoIndicator], CrawlError> {
    static INDICATORS: OnceLock<Result<Vec<WtoIndicator>, String>> = OnceLock::new();
    INDICATORS
        .get_or_init(|| load_wto_indicators(&data_dir().join(WTO_INDICATORS_FILE)))
        .as_deref()
        .map_err(|e| CrawlError::Permanent(e.clone()))
}

fn load_wto_indicators(path: &Path) -> Result<Vec<WtoIndicator>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("reading {}: {e} (set {DATA_DIR_ENV})", path.display()))?;
    parse_wto_indicators(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Parses `id,product,partner` rows after a header line (`#` comment lines skipped; product and
/// partner may be empty) and checks them.
fn parse_wto_indicators(text: &str) -> Result<Vec<WtoIndicator>, String> {
    let mut reader = csv::ReaderBuilder::new()
        .comment(Some(b'#'))
        .trim(csv::Trim::All)
        .from_reader(text.as_bytes());
    let header = reader.headers().map_err(|e| e.to_string())?.clone();
    if header.iter().ne(["id", "product", "partner"]) {
        return Err(format!(
            "expected header id,product,partner, got {}",
            header.iter().collect::<Vec<_>>().join(",")
        ));
    }
    let code_like = |s: &str| s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for record in reader.deserialize::<WtoIndicator>() {
        let row = record.map_err(|e| e.to_string())?;
        let id = &row.id;
        if id.is_empty() || !code_like(id) {
            return Err(format!(
                "indicator id {id:?} is empty or not letters, digits and _"
            ));
        }
        for (what, v) in [("product", &row.product), ("partner", &row.partner)] {
            if !code_like(v) {
                return Err(format!(
                    "indicator {id}: {what} {v:?} is not letters, digits and _"
                ));
            }
        }
        if !seen.insert(id.clone()) {
            return Err(format!("indicator {id} is listed twice"));
        }
        out.push(row);
    }
    if out.is_empty() {
        return Err("no indicators".into());
    }
    Ok(out)
}

/// The dataset definitions file for `source`: `datasets/<source>.toml` in [`data_dir`], where
/// `<source>` is the lowercase [`SourceId::as_str`] (e.g. `world_bank.toml`).
pub fn datasets_file(source: SourceId) -> PathBuf {
    data_dir()
        .join(DATASETS_DIR)
        .join(format!("{}.toml", source.as_str().to_ascii_lowercase()))
}

/// `source`'s dataset definitions from [`datasets_file`], parsed and validated, read on first use
/// and cached (including a failure) like [`bls_series`].
///
/// A missing or invalid file is a `Permanent` error, with the path in the message.
pub fn datasets(source: SourceId) -> Result<&'static [DatasetDef], CrawlError> {
    type Cache = Mutex<HashMap<SourceId, &'static Result<Vec<DatasetDef>, String>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(Cache::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // One entry per source, so leaking gives the 'static lifetime at a bounded cost.
    let entry = *cache
        .entry(source)
        .or_insert_with(|| Box::leak(Box::new(load_datasets(&datasets_file(source)))));
    entry
        .as_deref()
        .map_err(|e| CrawlError::Permanent(e.clone()))
}

/// The definition of `source`'s dataset `code` (see [`datasets`]), for adapters that build
/// canonical external ids with [`DatasetDef::external_id`] or [`DatasetDef::series`].
pub fn dataset(source: SourceId, code: &str) -> Result<&'static DatasetDef, CrawlError> {
    datasets(source)?
        .iter()
        .find(|d| d.code == code)
        .ok_or_else(|| {
            CrawlError::Permanent(format!(
                "{source} dataset {code} is not defined in {}",
                datasets_file(source).display()
            ))
        })
}

pub(crate) fn load_datasets(path: &Path) -> Result<Vec<DatasetDef>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("reading {}: {e} (set {DATA_DIR_ENV})", path.display()))?;
    parse_dataset_file(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The BLS series from `bls_series.csv` in [`data_dir`], read on first use and cached.
///
/// A missing or malformed file is a `Permanent` error, with the path in the message.
pub fn bls_series() -> Result<&'static [BlsSeries], CrawlError> {
    static SERIES: OnceLock<Result<Vec<BlsSeries>, String>> = OnceLock::new();
    SERIES
        .get_or_init(|| load_bls_series(&data_dir().join(BLS_SERIES_FILE)))
        .as_deref()
        .map_err(|e| CrawlError::Permanent(e.clone()))
}

/// Reads and parses a BLS series list.
fn load_bls_series(path: &Path) -> Result<Vec<BlsSeries>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("reading {}: {e} (set {DATA_DIR_ENV})", path.display()))?;
    parse_bls_series(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Parses one `series_id` per line after a `series_id` header line; blank lines and `#` comments
/// are skipped.
fn parse_bls_series(text: &str) -> Result<Vec<BlsSeries>, String> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'));
    match lines.next() {
        Some((_, "series_id")) => {}
        Some((n, other)) => {
            return Err(format!(
                "line {n}: expected header series_id, got {other:?}"
            ))
        }
        None => return Err("no header line".into()),
    }
    let mut series = Vec::new();
    let mut seen = HashSet::new();
    for (n, id) in lines {
        if !id
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            return Err(format!(
                "line {n}: series id {id:?} is not capital letters and digits"
            ));
        }
        if !seen.insert(id.to_string()) {
            return Err(format!("line {n}: duplicate series id {id}"));
        }
        series.push(BlsSeries { id: id.into() });
    }
    if series.is_empty() {
        return Err("no series".into());
    }
    Ok(series)
}

/// The curated Bank of Canada Valet series names from `boc_series.csv` in [`data_dir`], read on
/// first use and cached. Only the names are curated: each series' title, description, unit and
/// frequency come from Valet (see [`crate::sources::boc`]).
///
/// A missing or malformed file is a `Permanent` error, with the path in the message.
pub fn boc_series() -> Result<&'static [String], CrawlError> {
    static SERIES: OnceLock<Result<Vec<String>, String>> = OnceLock::new();
    SERIES
        .get_or_init(|| load_boc_series(&data_dir().join(BOC_SERIES_FILE)))
        .as_deref()
        .map_err(|e| CrawlError::Permanent(e.clone()))
}

/// Reads, parses and checks a Bank of Canada series list.
fn load_boc_series(path: &Path) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("reading {}: {e} (set {DATA_DIR_ENV})", path.display()))?;
    parse_boc_series(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Whether `name` can be a Valet series name: at most 100 ASCII letters, digits, `.`, `_` and
/// `-`, with at least one letter or digit. Every name in Valet's series list fits. Such a name
/// is one URL path segment that needs no percent-encoding: it has no `/`, `?`, `#` or `%`, and
/// requiring a letter or digit rules out the dot segments `.` and `..`, which a URL parser
/// would resolve against the path (`/observations/../json` is `/json`).
pub fn is_valet_series_name(name: &str) -> bool {
    name.len() <= 100
        && name.bytes().any(|b| b.is_ascii_alphanumeric())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// Parses `series_id` rows after a header line; blank lines and `#` comments are skipped.
fn parse_boc_series(text: &str) -> Result<Vec<String>, String> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'));
    match lines.next() {
        Some((_, "series_id")) => {}
        Some((n, other)) => {
            return Err(format!(
                "line {n}: expected header series_id, got {other:?}"
            ))
        }
        None => return Err("no header line".into()),
    }
    let mut ids = Vec::new();
    let mut seen = HashSet::new();
    for (n, id) in lines {
        if !is_valet_series_name(id) {
            return Err(format!(
                "line {n}: series id {id:?} is not a Valet series name"
            ));
        }
        if !seen.insert(id.to_string()) {
            return Err(format!("line {n}: duplicate series id {id:?}"));
        }
        ids.push(id.to_string());
    }
    if ids.is_empty() {
        return Err("no series ids".into());
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped Bank of Canada list parses and is a short headline list.
    #[test]
    fn shipped_boc_series_file_is_valid() {
        let ids = load_boc_series(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("data")
                .join(BOC_SERIES_FILE),
        )
        .unwrap();
        assert!((10..=50).contains(&ids.len()), "{} ids", ids.len());
        for id in ["V39079", "AVG.INTWO", "FXUSDCAD", "BD.CDN.10YR.DQ.YLD"] {
            assert!(ids.iter().any(|s| s == id), "{id} missing");
        }
    }

    /// Valet names are accepted; anything that isn't one URL path segment free of dot segments
    /// and reserved characters is not.
    #[test]
    fn valet_series_names() {
        for ok in [
            "V39079",
            "AVG.INTWO",
            "BD.CDN.10YR.DQ.YLD",
            "CPI_TRIM",
            "A-1",
            "x",
            ".V1",
            "V1..",
        ] {
            assert!(is_valet_series_name(ok), "{ok:?}");
        }
        let long = "A".repeat(101);
        for bad in [
            "",
            ".",
            "..",
            "...",
            "._-",
            "-",
            "a/b",
            "../json",
            "a?b",
            "a#b",
            "a%2F",
            "a b",
            "é",
            long.as_str(),
        ] {
            assert!(!is_valet_series_name(bad), "{bad:?}");
        }
        assert!(is_valet_series_name(&"A".repeat(100)));
    }

    /// Comments, blank lines and surrounding spaces are skipped; malformed lists are rejected
    /// with the offending line.
    #[test]
    fn parses_and_checks_boc_series_files() {
        let ok =
            parse_boc_series("# c\n\nseries_id\nV39079\n BD.CDN.2YR.DQ.YLD \nCPI_TRIM\n").unwrap();
        assert_eq!(ok, ["V39079", "BD.CDN.2YR.DQ.YLD", "CPI_TRIM"]);
        for (text, needle) in [
            ("", "no header"),
            ("id\n", "expected header"),
            ("series_id\n", "no series ids"),
            (
                "series_id\nFX/USD\n",
                "line 2: series id \"FX/USD\" is not a Valet",
            ),
            ("series_id\nFX USD\n", "not a Valet series name"),
            ("series_id\nFXUSDCAD,x\n", "not a Valet series name"),
            ("series_id\nFXUSDCAD\nFXUSDCAD\n", "duplicate"),
        ] {
            let e = parse_boc_series(text).unwrap_err();
            assert!(e.contains(needle), "{text:?}: {e}");
        }
        let e = load_boc_series(Path::new("/nonexistent/boc_series.csv")).unwrap_err();
        assert!(
            e.contains("/nonexistent/boc_series.csv") && e.contains(DATA_DIR_ENV),
            "{e}"
        );
    }

    /// The shipped indicator list parses and holds the curated set (about 150).
    #[test]
    fn shipped_wdi_indicators_file_is_valid() {
        let list = load_wdi_indicators(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("data")
                .join(WDI_INDICATORS_FILE),
        )
        .unwrap();
        assert!((140..=170).contains(&list.len()), "{}", list.len());
        let gdp = list.iter().find(|i| i.id == "NY.GDP.PCAP.CD").unwrap();
        assert_eq!(gdp.unit, "current US$");
    }

    #[test]
    fn rejects_malformed_wdi_indicators() {
        for (text, needle) in [
            ("code,unit\n", "expected header"),
            ("id,unit\n", "no indicators"),
            ("id,unit\na/b,u\n", "'/'"),
            ("id,unit\nA,\n", "empty unit"),
            ("id,unit\nA,u\nA,u\n", "twice"),
            ("id,unit\nA,u,X\n", "found record with 3 fields"),
        ] {
            let e = parse_wdi_indicators(text).unwrap_err();
            assert!(e.contains(needle), "{text:?}: {e}");
        }
        let ok = parse_wdi_indicators("# c\nid,unit\n A , u \n").unwrap();
        assert_eq!(
            ok,
            [WdiIndicator {
                id: "A".into(),
                unit: "u".into(),
            }]
        );
    }

    /// The shipped WTO list parses and holds the headline trade and tariff indicators.
    #[test]
    fn shipped_wto_indicators_file_is_valid() {
        let list = load_wto_indicators(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("data")
                .join(WTO_INDICATORS_FILE),
        )
        .unwrap();
        for id in [
            "ITS_MTV_AX",
            "ITS_MTV_AM",
            "ITS_CS_AX6",
            "ITS_CS_AM6",
            "TP_A_0010",
        ] {
            assert!(list.iter().any(|i| i.id == id), "{id}");
        }
        let exports = list.iter().find(|i| i.id == "ITS_MTV_AX").unwrap();
        assert_eq!(
            (exports.product.as_str(), exports.partner.as_str()),
            ("TO", "000")
        );
    }

    #[test]
    fn rejects_malformed_wto_indicators() {
        const H: &str = "id,product,partner\n";
        for (text, needle) in [
            ("id,unit\n".to_string(), "expected header"),
            (H.to_string(), "no indicators"),
            (format!("{H}A/B,,\n"), "not letters"),
            (format!("{H}A B,,\n"), "not letters"),
            (format!("{H}A,T.O,\n"), "product"),
            (format!("{H}A,,0,0\n"), "found record with 4 fields"),
            (format!("{H}A,,\nA,TO,\n"), "twice"),
        ] {
            let e = parse_wto_indicators(&text).unwrap_err();
            assert!(e.contains(needle), "{text:?}: {e}");
        }
        let ok = parse_wto_indicators(&format!("# c\n{H} A , TO , 000 \nB,,\n")).unwrap();
        assert_eq!(
            ok,
            [
                WtoIndicator {
                    id: "A".into(),
                    product: "TO".into(),
                    partner: "000".into(),
                },
                WtoIndicator {
                    id: "B".into(),
                    product: String::new(),
                    partner: String::new(),
                },
            ]
        );
    }

    /// The shipped BLS list parses, covers every survey the adapter promises, and has a LAUS
    /// unemployment rate (seasonally adjusted and not) for every state and DC (the Census state
    /// file fixture).
    #[test]
    fn shipped_bls_series_file_is_valid() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let series = load_bls_series(&dir.join(BLS_SERIES_FILE)).unwrap();
        assert!(series.len() >= 250, "{} series", series.len());
        for prefix in ["CUSR", "CUUR", "CES", "CEU", "LNS", "LNU", "LASST", "LAUST"] {
            assert!(
                series.iter().any(|s| s.id.starts_with(prefix)),
                "no {prefix} series"
            );
        }
        let states = crate::sources::census::parse_state_file(include_str!(
            "../tests/fixtures/census/state.txt"
        ))
        .unwrap();
        for st in &states {
            for id in [
                format!("LASST{}0000000000003", st.fips),
                format!("LAUST{}0000000000003", st.fips),
            ] {
                assert!(series.iter().any(|s| s.id == id), "{id} ({})", st.name);
            }
        }
    }

    /// Every shipped dataset file parses and validates: today `fred.toml` and `bls.toml`
    /// (DS-4); Census BDS (DS-5) adds more.
    #[test]
    fn shipped_dataset_files_are_valid() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join(DATASETS_DIR);
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "toml") {
                let stem = path
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_ascii_uppercase();
                assert!(
                    stem.parse::<SourceId>().is_ok(),
                    "{}: not named after a source",
                    path.display()
                );
                load_datasets(&path).unwrap();
            }
        }
    }

    /// Comments and blank lines are skipped.
    #[test]
    fn parses_bls_rows() {
        let s =
            parse_bls_series("# c\n\nseries_id\n# CES\nCES4000000001\n  LNS14000000  \n").unwrap();
        assert_eq!(
            s,
            [
                BlsSeries {
                    id: "CES4000000001".into()
                },
                BlsSeries {
                    id: "LNS14000000".into()
                },
            ]
        );
    }

    /// Malformed BLS lists are rejected with the offending line.
    #[test]
    fn rejects_malformed_bls_files() {
        const H: &str = "series_id\n";
        for (text, needle) in [
            (String::new(), "no header"),
            (
                "series_id,frequency,units,title\n".to_string(),
                "expected header",
            ),
            (H.to_string(), "no series"),
            (format!("{H}CUUR0000SA0,Monthly\n"), "line 2"),
            (format!("{H}cuur0000sa0\n"), "capital letters"),
            (format!("{H}CUUR0000SA0\nCUUR0000SA0\n"), "duplicate"),
        ] {
            let e = parse_bls_series(&text).unwrap_err();
            assert!(e.contains(needle), "{text:?}: {e}");
        }
        let e = load_bls_series(Path::new("/nonexistent/bls_series.csv")).unwrap_err();
        assert!(
            e.contains("/nonexistent/bls_series.csv") && e.contains(DATA_DIR_ENV),
            "{e}"
        );
    }

    #[test]
    fn datasets_file_is_lowercase_source() {
        assert!(datasets_file(SourceId::WorldBank).ends_with("datasets/world_bank.toml"));
    }

    /// A missing dataset file names its path and the environment variable.
    #[test]
    fn missing_dataset_file_names_path_and_env() {
        let e = load_datasets(Path::new("/nonexistent/fred.toml")).unwrap_err();
        assert!(
            e.contains("/nonexistent/fred.toml") && e.contains(DATA_DIR_ENV),
            "{e}"
        );
    }
}
