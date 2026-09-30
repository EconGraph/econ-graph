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
//! worker checks them at startup ([`us_states`], [`bls_series`]) rather than on its first job.
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

/// Directory under [`data_dir`] holding one `<source>.toml` of dataset definitions per source.
pub const DATASETS_DIR: &str = "datasets";

/// Rows the states table must hold: the 50 states and DC.
pub const US_STATE_COUNT: usize = 51;

/// File name of the BLS series list in the data directory.
pub const BLS_SERIES_FILE: &str = "bls_series.csv";

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

/// A BLS series the crawler discovers and fetches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlsSeries {
    /// BLS series id, e.g. `CUUR0000SA0`.
    pub id: String,
    /// Frequency, e.g. `Monthly`.
    pub frequency: String,
    /// Units, e.g. `Percent`.
    pub units: String,
    /// Title.
    pub title: String,
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

pub(crate) fn load_datasets(path: &Path) -> Result<Vec<DatasetDef>, String> {
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

/// Parses `series_id,frequency,units,title` rows after a header line; blank lines and `#`
/// comments are skipped. The title is the last column and may contain commas.
fn parse_bls_series(text: &str) -> Result<Vec<BlsSeries>, String> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'));
    match lines.next() {
        Some((_, "series_id,frequency,units,title")) => {}
        Some((n, other)) => {
            return Err(format!(
                "line {n}: expected header series_id,frequency,units,title, got {other:?}"
            ))
        }
        None => return Err("no header line".into()),
    }
    let mut series = Vec::new();
    let mut seen = HashSet::new();
    for (n, line) in lines {
        let mut cols = line.splitn(4, ',').map(str::trim);
        let (Some(id), Some(frequency), Some(units), Some(title)) =
            (cols.next(), cols.next(), cols.next(), cols.next())
        else {
            return Err(format!(
                "line {n}: expected series_id,frequency,units,title, got {line:?}"
            ));
        };
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            return Err(format!(
                "line {n}: series id {id:?} is not capital letters and digits"
            ));
        }
        if frequency.is_empty() || units.is_empty() || title.is_empty() {
            return Err(format!("line {n}: empty frequency, units or title"));
        }
        if !seen.insert(id.to_string()) {
            return Err(format!("line {n}: duplicate series id {id}"));
        }
        series.push(BlsSeries {
            id: id.into(),
            frequency: frequency.into(),
            units: units.into(),
            title: title.into(),
        });
    }
    if series.is_empty() {
        return Err("no series".into());
    }
    Ok(series)
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

    /// The shipped BLS list parses, covers every survey the adapter promises, and has a LAUS
    /// unemployment rate for every state in the shipped states table.
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
        let states = load_us_states(&dir.join(US_STATES_FILE)).unwrap();
        for st in &states {
            for id in [
                format!("LASST{}0000000000003", st.fips),
                format!("LAUST{}0000000000003", st.fips),
            ] {
                let row = series.iter().find(|s| s.id == id);
                assert!(
                    row.is_some_and(|r| r.title.contains(&st.name)),
                    "{id} ({})",
                    st.name
                );
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

    /// Titles keep their commas; comments and blank lines are skipped.
    #[test]
    fn parses_bls_rows() {
        let s = parse_bls_series(
            "# c\n\nseries_id,frequency,units,title\nCES4000000001,Monthly,Thousands,All Employees, Trade\n",
        )
        .unwrap();
        assert_eq!(
            s,
            [BlsSeries {
                id: "CES4000000001".into(),
                frequency: "Monthly".into(),
                units: "Thousands".into(),
                title: "All Employees, Trade".into(),
            }]
        );
    }

    /// Malformed BLS lists are rejected with the offending line.
    #[test]
    fn rejects_malformed_bls_files() {
        const H: &str = "series_id,frequency,units,title\n";
        for (text, needle) in [
            (String::new(), "no header"),
            ("id,title\n".to_string(), "expected header"),
            (H.to_string(), "no series"),
            (format!("{H}CUUR0000SA0,Monthly,Index\n"), "line 2"),
            (
                format!("{H}cuur0000sa0,Monthly,Index,T\n"),
                "capital letters",
            ),
            (format!("{H}CUUR0000SA0,,Index,T\n"), "empty"),
            (
                format!("{H}CUUR0000SA0,Monthly,Index,T\nCUUR0000SA0,Monthly,Index,U\n"),
                "duplicate",
            ),
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
