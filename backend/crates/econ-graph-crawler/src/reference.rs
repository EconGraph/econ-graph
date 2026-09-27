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
//! worker checks it at startup ([`us_states`]) rather than on its first job.
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

/// File name of the U.S. states table in the data directory.
pub const US_STATES_FILE: &str = "us_states.csv";

/// File name of the World Development Indicators list in the data directory.
pub const WDI_INDICATORS_FILE: &str = "wdi_indicators.csv";

/// Directory under [`data_dir`] holding one `<source>.toml` of dataset definitions per source.
pub const DATASETS_DIR: &str = "datasets";

/// Rows the states table must hold: the 50 states and DC.
pub const US_STATE_COUNT: usize = 51;

/// A U.S. state or the District of Columbia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsState {
    /// Two-digit state FIPS code, e.g. `06`.
    pub fips: String,
    /// USPS postal code, e.g. `CA`.
    pub postal: String,
    /// Name, e.g. `California`.
    pub name: String,
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

/// One World Development Indicator the World Bank adapter crawls.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WdiIndicator {
    /// Indicator code, e.g. `NY.GDP.PCAP.CD`.
    pub id: String,
    /// Name, e.g. `GDP per capita (current US$)`.
    pub name: String,
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

/// Parses `id,name,unit` rows after a header line (`#` comment lines skipped; fields may be
/// quoted) and checks them.
fn parse_wdi_indicators(text: &str) -> Result<Vec<WdiIndicator>, String> {
    let mut reader = csv::ReaderBuilder::new()
        .comment(Some(b'#'))
        .trim(csv::Trim::All)
        .from_reader(text.as_bytes());
    let header = reader.headers().map_err(|e| e.to_string())?.clone();
    if header.iter().ne(["id", "name", "unit"]) {
        return Err(format!(
            "expected header id,name,unit, got {}",
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
        if row.name.is_empty() || row.unit.is_empty() {
            return Err(format!("indicator {id}: empty name or unit"));
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

/// The states and DC from `us_states.csv` in [`data_dir`], read on first use and cached.
///
/// A missing, malformed or incomplete file (not [`US_STATE_COUNT`] rows) is a `Permanent` error,
/// with the path in the message.
pub fn us_states() -> Result<&'static [UsState], CrawlError> {
    static STATES: OnceLock<Result<Vec<UsState>, String>> = OnceLock::new();
    STATES
        .get_or_init(|| load_us_states(&data_dir().join(US_STATES_FILE)))
        .as_deref()
        .map_err(|e| CrawlError::Permanent(e.clone()))
}

/// The dataset definitions file for `source`: `datasets/<source>.toml` in [`data_dir`], where
/// `<source>` is the lowercase [`SourceId::as_str`] (e.g. `world_bank.toml`).
pub fn datasets_file(source: SourceId) -> PathBuf {
    data_dir()
        .join(DATASETS_DIR)
        .join(format!("{}.toml", source.as_str().to_ascii_lowercase()))
}

/// `source`'s dataset definitions from [`datasets_file`], parsed and validated, read on first use
/// and cached (including a failure) like [`us_states`].
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

fn load_datasets(path: &Path) -> Result<Vec<DatasetDef>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("reading {}: {e} (set {DATA_DIR_ENV})", path.display()))?;
    parse_dataset_file(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Reads, parses and checks a states table.
fn load_us_states(path: &Path) -> Result<Vec<UsState>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("reading {}: {e} (set {DATA_DIR_ENV})", path.display()))?;
    parse_us_states(&text)
        .and_then(check_complete)
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Rejects a table without exactly [`US_STATE_COUNT`] rows, so a truncated file cannot silently
/// drop states from discovery and scheduling.
fn check_complete(states: Vec<UsState>) -> Result<Vec<UsState>, String> {
    if states.len() == US_STATE_COUNT {
        Ok(states)
    } else {
        Err(format!(
            "expected {US_STATE_COUNT} rows (50 states and DC), got {}",
            states.len()
        ))
    }
}

/// Parses `fips,postal,name` rows after a header line; blank lines and `#` comments are skipped.
fn parse_us_states(text: &str) -> Result<Vec<UsState>, String> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'));
    match lines.next() {
        Some((_, "fips,postal,name")) => {}
        Some((n, other)) => {
            return Err(format!(
                "line {n}: expected header fips,postal,name, got {other:?}"
            ))
        }
        None => return Err("no header line".into()),
    }
    let mut states = Vec::new();
    let (mut fips_seen, mut postal_seen) = (HashSet::new(), HashSet::new());
    for (n, line) in lines {
        let mut cols = line.splitn(3, ',').map(str::trim);
        let (Some(fips), Some(postal), Some(name)) = (cols.next(), cols.next(), cols.next()) else {
            return Err(format!("line {n}: expected fips,postal,name, got {line:?}"));
        };
        if fips.len() != 2 || !fips.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("line {n}: FIPS code {fips:?} is not two digits"));
        }
        if postal.len() != 2 || !postal.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(format!(
                "line {n}: postal code {postal:?} is not two capital letters"
            ));
        }
        if name.is_empty() {
            return Err(format!("line {n}: empty name"));
        }
        if !fips_seen.insert(fips.to_string()) || !postal_seen.insert(postal.to_string()) {
            return Err(format!(
                "line {n}: duplicate FIPS or postal code ({fips}, {postal})"
            ));
        }
        states.push(UsState {
            fips: fips.into(),
            postal: postal.into(),
            name: name.into(),
        });
    }
    if states.is_empty() {
        return Err("no states".into());
    }
    Ok(states)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped file parses and holds the 50 states and DC.
    #[test]
    fn shipped_states_file_is_valid() {
        let states = load_us_states(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("data")
                .join(US_STATES_FILE),
        )
        .unwrap();
        assert_eq!(states.len(), US_STATE_COUNT);
        let ca = states.iter().find(|s| s.postal == "CA").unwrap();
        assert_eq!((ca.fips.as_str(), ca.name.as_str()), ("06", "California"));
        assert!(states.iter().any(|s| s.fips == "11" && s.postal == "DC"));
    }

    /// The shipped indicator list parses and holds the curated set (about 50).
    #[test]
    fn shipped_wdi_indicators_file_is_valid() {
        let list = load_wdi_indicators(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("data")
                .join(WDI_INDICATORS_FILE),
        )
        .unwrap();
        assert!((45..=60).contains(&list.len()), "{}", list.len());
        let gdp = list.iter().find(|i| i.id == "NY.GDP.PCAP.CD").unwrap();
        assert_eq!(gdp.name, "GDP per capita (current US$)");
        assert_eq!(gdp.unit, "current US$");
        // Quoted names keep their commas.
        assert!(list.iter().any(|i| i.name == "Population, total"));
    }

    #[test]
    fn rejects_malformed_wdi_indicators() {
        for (text, needle) in [
            ("code,name,unit\n", "expected header"),
            ("id,name,unit\n", "no indicators"),
            ("id,name,unit\na/b,X,u\n", "'/'"),
            ("id,name,unit\nA,X,\n", "empty name or unit"),
            ("id,name,unit\nA,X,u\nA,Y,u\n", "twice"),
            ("id,name,unit\nA,X\n", "found record with 2 fields"),
        ] {
            let e = parse_wdi_indicators(text).unwrap_err();
            assert!(e.contains(needle), "{text:?}: {e}");
        }
        let ok = parse_wdi_indicators("# c\nid,name,unit\n A ,\"B, c\", u \n").unwrap();
        assert_eq!(
            ok,
            [WdiIndicator {
                id: "A".into(),
                name: "B, c".into(),
                unit: "u".into(),
            }]
        );
    }

    /// Comments and blank lines are skipped; names may contain spaces.
    #[test]
    fn parses_rows_after_header() {
        let s = parse_us_states("# c\n\nfips,postal,name\n11,DC,District of Columbia\n").unwrap();
        assert_eq!(
            s,
            [UsState {
                fips: "11".into(),
                postal: "DC".into(),
                name: "District of Columbia".into(),
            }]
        );
    }

    /// Malformed files are rejected with the offending line.
    #[test]
    fn rejects_malformed_files() {
        for (text, needle) in [
            ("", "no header"),
            ("name,fips\n", "expected header"),
            ("fips,postal,name\n", "no states"),
            ("fips,postal,name\n6,CA,California\n", "line 2"),
            ("fips,postal,name\n06,ca,California\n", "postal"),
            ("fips,postal,name\n06,CA\n", "expected fips,postal,name"),
            ("fips,postal,name\n06,CA,\n", "empty name"),
            ("fips,postal,name\n06,CA,California\n06,CB,X\n", "duplicate"),
        ] {
            let e = parse_us_states(text).unwrap_err();
            assert!(e.contains(needle), "{text:?}: {e}");
        }
    }

    /// A table missing any state is rejected, even if every row is valid.
    #[test]
    fn rejects_incomplete_tables() {
        let one = parse_us_states("fips,postal,name\n06,CA,California\n").unwrap();
        let e = check_complete(one).unwrap_err();
        assert!(e.contains("expected 51 rows") && e.contains("got 1"), "{e}");
    }

    /// Every shipped dataset file parses and validates.
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

    /// A missing file names its path and the environment variable.
    #[test]
    fn missing_file_names_path_and_env() {
        let e = load_us_states(Path::new("/nonexistent/us_states.csv")).unwrap_err();
        assert!(
            e.contains("/nonexistent/us_states.csv") && e.contains(DATA_DIR_ENV),
            "{e}"
        );
    }
}
