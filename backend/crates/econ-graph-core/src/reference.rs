// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Reference data shared across crates, read from data files at runtime (not compiled in).
//!
//! The files live in the directory named by `REFERENCE_DATA_DIR`. When it is unset they are read
//! from this crate's `data/` directory in the source tree, which is what `cargo run` and
//! `cargo test` use. The backend and crawler-worker images copy `data/` to `/app/reference` and
//! set `REFERENCE_DATA_DIR` to it.
//!
//! Each file is read once per process and cached, including a failure to read it, so binaries
//! check it at startup ([`areas`]) rather than on first use.
//!
//! `countries.csv` holds one row per area: every ISO 3166-1 country plus Kosovo, keyed by ISO
//! alpha-3, and World Bank aggregates (World, Euro area, income groups, ...) keyed by their
//! World Bank code, with no ISO codes. It is built by `backend/scripts/build_countries_csv.py`.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;

/// Environment variable naming the reference data directory.
pub const DATA_DIR_ENV: &str = "REFERENCE_DATA_DIR";

/// File name of the country and aggregate table in the data directory.
pub const COUNTRIES_FILE: &str = "countries.csv";

/// Fewest rows with an ISO 3166-1 numeric code the table must hold (ISO 3166-1 lists 249
/// countries), so a truncated file is rejected rather than silently dropping countries.
pub const MIN_ISO_COUNTRIES: usize = 249;

/// Columns of `countries.csv`, in order.
const COLUMNS: [&str; 10] = [
    "key",
    "kind",
    "iso2",
    "iso3",
    "iso_numeric",
    "wb_code",
    "sdmx_ref_area",
    "name",
    "region",
    "income_group",
];

/// A reference data file that is missing, malformed or incomplete. The message names the file.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ReferenceError(String);

/// Whether an area is a country or an aggregate of countries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AreaKind {
    /// An ISO 3166-1 country or territory, or Kosovo.
    Country,
    /// A group of countries published as one area, e.g. `WLD` (World) or `HIC` (High income).
    Aggregate,
}

/// One row of `countries.csv`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Area {
    /// Stable key: ISO alpha-3 for countries (e.g. `USA`), World Bank code for aggregates
    /// (e.g. `EMU`). Datasets key their area dimension by it.
    pub key: String,
    /// Country or aggregate.
    pub kind: AreaKind,
    /// ISO 3166-1 alpha-2, e.g. `US` (`XK` for Kosovo). `None` for aggregates.
    pub iso2: Option<String>,
    /// ISO 3166-1 alpha-3, e.g. `USA` (`XKX` for Kosovo). `None` for aggregates.
    pub iso3: Option<String>,
    /// ISO 3166-1 numeric, e.g. `840` (written `840`, `004`, ...). `None` for aggregates and
    /// Kosovo.
    pub iso_numeric: Option<u16>,
    /// World Bank economy or aggregate code, e.g. `USA`, `EMU`. `None` for Taiwan, and after
    /// the build script's `--world-bank` run for every country the World Bank does not publish
    /// (before it, other countries default to their ISO alpha-3).
    pub wb_code: Option<String>,
    /// SDMX `REF_AREA` code (ISO alpha-2, the CL_AREA convention), e.g. `US`.
    pub sdmx_ref_area: Option<String>,
    /// Display name, e.g. `United States`.
    pub name: String,
    /// World Bank region code, e.g. `NAC`, where known.
    pub region: Option<String>,
    /// World Bank income group code, e.g. `HIC`, where known.
    pub income_group: Option<String>,
}

/// The rows of `countries.csv`, with lookups by each code.
///
/// Lookups ignore ASCII case (`us`, `US`).
#[derive(Debug)]
pub struct Areas {
    rows: Vec<Area>,
    by_key: HashMap<String, usize>,
    by_iso2: HashMap<String, usize>,
    by_iso3: HashMap<String, usize>,
    by_iso_numeric: HashMap<u16, usize>,
    by_wb_code: HashMap<String, usize>,
    by_sdmx_ref_area: HashMap<String, usize>,
}

impl Areas {
    /// Every row, in file order.
    pub fn all(&self) -> &[Area] {
        &self.rows
    }

    /// The area with this key (ISO alpha-3 or World Bank aggregate code).
    pub fn by_key(&self, key: &str) -> Option<&Area> {
        self.get(&self.by_key, key)
    }

    /// The country with this ISO alpha-2 code.
    pub fn by_iso2(&self, code: &str) -> Option<&Area> {
        self.get(&self.by_iso2, code)
    }

    /// The country with this ISO alpha-3 code.
    pub fn by_iso3(&self, code: &str) -> Option<&Area> {
        self.get(&self.by_iso3, code)
    }

    /// The country with this ISO numeric code (`4` finds Afghanistan, written `004`).
    pub fn by_iso_numeric(&self, code: u16) -> Option<&Area> {
        self.by_iso_numeric.get(&code).map(|&i| &self.rows[i])
    }

    /// The area with this World Bank code.
    pub fn by_wb_code(&self, code: &str) -> Option<&Area> {
        self.get(&self.by_wb_code, code)
    }

    /// The area with this SDMX `REF_AREA` code.
    pub fn by_sdmx_ref_area(&self, code: &str) -> Option<&Area> {
        self.get(&self.by_sdmx_ref_area, code)
    }

    fn get(&self, index: &HashMap<String, usize>, code: &str) -> Option<&Area> {
        index
            .get(&code.to_ascii_uppercase())
            .map(|&i| &self.rows[i])
    }
}

/// The reference data directory: `$REFERENCE_DATA_DIR`, or this crate's `data/` directory.
pub fn data_dir() -> PathBuf {
    data_dir_from(std::env::var_os(DATA_DIR_ENV))
}

fn data_dir_from(env: Option<OsString>) -> PathBuf {
    env.filter(|v| !v.is_empty()).map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("data"),
        PathBuf::from,
    )
}

/// The table from `countries.csv` in [`data_dir`], read on first use and cached.
///
/// A missing, malformed or incomplete file is an error naming the path.
pub fn areas() -> Result<&'static Areas, ReferenceError> {
    static AREAS: OnceLock<Result<Areas, ReferenceError>> = OnceLock::new();
    AREAS
        .get_or_init(|| load_areas(&data_dir().join(COUNTRIES_FILE)))
        .as_ref()
        .map_err(Clone::clone)
}

/// Reads, parses and checks a countries table.
pub fn load_areas(path: &Path) -> Result<Areas, ReferenceError> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        ReferenceError(format!(
            "reading {}: {e} (set {DATA_DIR_ENV} to the reference data directory)",
            path.display()
        ))
    })?;
    parse_areas(&text).map_err(|e| ReferenceError(format!("{}: {e}", path.display())))
}

/// Raw row, before `iso_numeric` is checked for its three-digit form.
#[derive(Deserialize)]
struct RawArea {
    key: String,
    kind: AreaKind,
    iso2: Option<String>,
    iso3: Option<String>,
    iso_numeric: Option<String>,
    wb_code: Option<String>,
    sdmx_ref_area: Option<String>,
    name: String,
    region: Option<String>,
    income_group: Option<String>,
}

/// Parses the CSV text (header line first; `#` comment lines skipped) and checks every row.
fn parse_areas(text: &str) -> Result<Areas, String> {
    let mut reader = csv::ReaderBuilder::new()
        .comment(Some(b'#'))
        .from_reader(text.as_bytes());
    let header = reader.headers().map_err(|e| e.to_string())?.clone();
    if header.iter().ne(COLUMNS) {
        return Err(format!(
            "expected header {}, got {}",
            COLUMNS.join(","),
            header.iter().collect::<Vec<_>>().join(",")
        ));
    }
    let mut areas = Areas {
        rows: Vec::new(),
        by_key: HashMap::new(),
        by_iso2: HashMap::new(),
        by_iso3: HashMap::new(),
        by_iso_numeric: HashMap::new(),
        by_wb_code: HashMap::new(),
        by_sdmx_ref_area: HashMap::new(),
    };
    for record in reader.deserialize::<RawArea>() {
        let raw = record.map_err(|e| e.to_string())?;
        let key = raw.key.clone();
        let area = check_row(raw).map_err(|e| format!("row {key:?}: {e}"))?;
        let i = areas.rows.len();
        index(&mut areas.by_key, "key", Some(&area.key), i)?;
        index(&mut areas.by_iso2, "iso2", area.iso2.as_ref(), i)?;
        index(&mut areas.by_iso3, "iso3", area.iso3.as_ref(), i)?;
        index(&mut areas.by_wb_code, "wb_code", area.wb_code.as_ref(), i)?;
        index(
            &mut areas.by_sdmx_ref_area,
            "sdmx_ref_area",
            area.sdmx_ref_area.as_ref(),
            i,
        )?;
        if let Some(n) = area.iso_numeric {
            if areas.by_iso_numeric.insert(n, i).is_some() {
                return Err(format!("duplicate iso_numeric {n:03}"));
            }
        }
        areas.rows.push(area);
    }
    if areas.by_iso_numeric.len() < MIN_ISO_COUNTRIES {
        return Err(format!(
            "expected at least {MIN_ISO_COUNTRIES} ISO 3166-1 countries, got {}",
            areas.by_iso_numeric.len()
        ));
    }
    Ok(areas)
}

/// Adds `code` to `map`, rejecting a duplicate.
fn index(
    map: &mut HashMap<String, usize>,
    column: &str,
    code: Option<&String>,
    i: usize,
) -> Result<(), String> {
    match code {
        Some(code) if map.insert(code.clone(), i).is_some() => {
            Err(format!("duplicate {column} {code:?}"))
        }
        _ => Ok(()),
    }
}

/// Checks one row's codes against its kind.
fn check_row(raw: RawArea) -> Result<Area, String> {
    check_code("key", Some(&raw.key), 3, u8::is_ascii_uppercase)?;
    check_code("iso2", raw.iso2.as_ref(), 2, u8::is_ascii_uppercase)?;
    check_code("iso3", raw.iso3.as_ref(), 3, u8::is_ascii_uppercase)?;
    check_code(
        "iso_numeric",
        raw.iso_numeric.as_ref(),
        3,
        u8::is_ascii_digit,
    )?;
    for (column, code) in [
        ("wb_code", &raw.wb_code),
        ("sdmx_ref_area", &raw.sdmx_ref_area),
        ("region", &raw.region),
        ("income_group", &raw.income_group),
    ] {
        // Upper case only, since lookups upper-case the query.
        check_code(column, code.as_ref(), 0, |b| {
            b.is_ascii_uppercase() || b.is_ascii_digit()
        })?;
    }
    if raw.name.trim().is_empty() {
        return Err("empty name".into());
    }
    match raw.kind {
        AreaKind::Country => {
            if raw.iso3.as_deref() != Some(raw.key.as_str()) || raw.iso2.is_none() {
                return Err("a country needs iso2 and iso3, and its key must be its iso3".into());
            }
        }
        AreaKind::Aggregate => {
            if raw.iso2.is_some() || raw.iso3.is_some() || raw.iso_numeric.is_some() {
                return Err("an aggregate must not have ISO codes".into());
            }
            if raw.wb_code.as_deref() != Some(raw.key.as_str()) {
                return Err("an aggregate's key must be its wb_code".into());
            }
        }
    }
    Ok(Area {
        key: raw.key,
        kind: raw.kind,
        iso2: raw.iso2,
        iso3: raw.iso3,
        // Checked above: three ASCII digits.
        iso_numeric: raw.iso_numeric.map(|n| n.parse().expect("three digits")),
        wb_code: raw.wb_code,
        sdmx_ref_area: raw.sdmx_ref_area,
        name: raw.name,
        region: raw.region,
        income_group: raw.income_group,
    })
}

/// Rejects a code that isn't `len` characters (any non-zero length when `len` is 0) of `allowed`.
fn check_code(
    column: &str,
    code: Option<&String>,
    len: usize,
    allowed: fn(&u8) -> bool,
) -> Result<(), String> {
    match code {
        Some(c) if (len != 0 && c.len() != len) || !c.bytes().all(|b| allowed(&b)) => {
            Err(format!("invalid {column} {c:?}"))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const HEADER: &str =
        "key,kind,iso2,iso3,iso_numeric,wb_code,sdmx_ref_area,name,region,income_group\n";

    fn shipped() -> Areas {
        load_areas(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("data")
                .join(COUNTRIES_FILE),
        )
        .unwrap()
    }

    /// Parses `rows` after the header, padded with enough synthetic countries to pass the ISO
    /// completeness check.
    fn parse_rows(rows: &str) -> Result<Areas, String> {
        let mut text = format!("{HEADER}{rows}");
        // Padding codes stay clear of the test rows: keys Qxx, iso2 K..T plus a letter,
        // numeric 100-348.
        for n in 0..MIN_ISO_COUNTRIES {
            let l = |x: usize| char::from(b'A' + u8::try_from(x).unwrap());
            let key = format!("Q{}{}", l(n / 26), l(n % 26));
            let iso2 = format!("{}{}", l(10 + n / 26), l(n % 26));
            text.push_str(&format!("{key},country,{iso2},{key},{},,,Pad,,\n", 100 + n));
        }
        parse_areas(&text)
    }

    /// The shipped file loads and holds every ISO 3166-1 country, Kosovo and the aggregates.
    #[test]
    fn shipped_file_is_valid() {
        let areas = shipped();
        let countries: Vec<_> = areas
            .all()
            .iter()
            .filter(|a| a.kind == AreaKind::Country)
            .collect();
        assert_eq!(
            countries.iter().filter(|a| a.iso_numeric.is_some()).count(),
            MIN_ISO_COUNTRIES
        );
        assert_eq!(countries.len(), MIN_ISO_COUNTRIES + 1, "ISO plus Kosovo");
        for code in [
            "WLD", "EMU", "EUU", "HIC", "UMC", "LMC", "LIC", "MIC", "LMY",
        ] {
            let a = areas.by_key(code).unwrap();
            assert_eq!(a.kind, AreaKind::Aggregate, "{code}");
        }
    }

    /// Keys are unique, and so is each code used for lookups.
    #[test]
    fn shipped_keys_and_codes_are_unique() {
        let areas = shipped();
        let rows = areas.all();
        let unique = |f: fn(&Area) -> Option<String>| {
            let codes: Vec<_> = rows.iter().filter_map(f).collect();
            codes.iter().collect::<HashSet<_>>().len() == codes.len()
        };
        assert!(unique(|a| Some(a.key.clone())));
        assert!(unique(|a| a.iso2.clone()));
        assert!(unique(|a| a.iso3.clone()));
        assert!(unique(|a| a.iso_numeric.map(|n| n.to_string())));
        assert!(unique(|a| a.wb_code.clone()));
        assert!(unique(|a| a.sdmx_ref_area.clone()));
    }

    /// Spot-checks ISO 3166-1 countries across the alphabet and numeric range.
    #[test]
    fn shipped_iso_countries() {
        let areas = shipped();
        for (iso2, iso3, numeric, name) in [
            ("AF", "AFG", 4, "Afghanistan"),
            ("US", "USA", 840, "United States"),
            ("DE", "DEU", 276, "Germany"),
            ("CN", "CHN", 156, "China"),
            ("KR", "KOR", 410, "South Korea"),
            ("BQ", "BES", 535, "Bonaire, Sint Eustatius and Saba"),
            ("AQ", "ATA", 10, "Antarctica"),
            ("ZW", "ZWE", 716, "Zimbabwe"),
        ] {
            let a = areas.by_key(iso3).unwrap();
            assert_eq!(
                (a.iso2.as_deref(), a.iso_numeric, a.name.as_str()),
                (Some(iso2), Some(numeric), name),
                "{iso3}"
            );
            assert_eq!(a.sdmx_ref_area.as_deref(), Some(iso2));
        }
    }

    /// Kosovo and Taiwan are countries; Kosovo has no ISO numeric code and Taiwan no World Bank
    /// code.
    #[test]
    fn shipped_kosovo_and_taiwan() {
        let areas = shipped();
        let xkx = areas.by_key("XKX").unwrap();
        assert_eq!(xkx.kind, AreaKind::Country);
        assert_eq!(
            (xkx.iso2.as_deref(), xkx.iso_numeric, xkx.wb_code.as_deref()),
            (Some("XK"), None, Some("XKX"))
        );
        assert_eq!(xkx.name, "Kosovo");
        let twn = areas.by_key("TWN").unwrap();
        assert_eq!(twn.kind, AreaKind::Country);
        assert_eq!(
            (twn.iso2.as_deref(), twn.iso_numeric, twn.wb_code.as_deref()),
            (Some("TW"), Some(158), None)
        );
        assert_eq!(twn.name, "Taiwan");
    }

    /// Aggregates carry no ISO codes and are keyed by their World Bank code.
    #[test]
    fn shipped_aggregates_have_no_iso_codes() {
        let areas = shipped();
        let aggregates: Vec<_> = areas
            .all()
            .iter()
            .filter(|a| a.kind == AreaKind::Aggregate)
            .collect();
        assert!(!aggregates.is_empty());
        for a in aggregates {
            assert_eq!(
                (&a.iso2, &a.iso3, a.iso_numeric),
                (&None, &None, None),
                "{}",
                a.key
            );
            assert_eq!(a.wb_code.as_deref(), Some(a.key.as_str()));
        }
        assert_eq!(areas.by_wb_code("EMU").unwrap().name, "Euro area");
    }

    /// Each lookup finds the row, ignoring case; unknown codes find nothing.
    #[test]
    fn lookups() {
        let areas = shipped();
        let key = |a: Option<&Area>| a.map(|a| a.key.clone());
        let fra = Some("FRA".to_string());
        assert_eq!(key(areas.by_key("fra")), fra);
        assert_eq!(key(areas.by_iso2("FR")), fra);
        assert_eq!(key(areas.by_iso2("fr")), fra);
        assert_eq!(key(areas.by_iso3("FRA")), fra);
        assert_eq!(key(areas.by_iso_numeric(250)), fra);
        assert_eq!(key(areas.by_wb_code("FRA")), fra);
        assert_eq!(key(areas.by_sdmx_ref_area("FR")), fra);
        assert_eq!(key(areas.by_iso_numeric(4)).as_deref(), Some("AFG"));
        assert_eq!(key(areas.by_wb_code("WLD")).as_deref(), Some("WLD"));
        assert_eq!(key(areas.by_iso2("XK")).as_deref(), Some("XKX"));
        assert_eq!(key(areas.by_sdmx_ref_area("TW")).as_deref(), Some("TWN"));
        assert!(areas.by_iso2("ZZ").is_none());
        assert!(areas.by_iso3("WLD").is_none(), "aggregates have no iso3");
        assert!(areas.by_iso_numeric(999).is_none());
        assert!(areas.by_wb_code("TWN").is_none());
        assert!(areas.by_sdmx_ref_area("").is_none());
    }

    /// `areas()` reads the default directory when `REFERENCE_DATA_DIR` is unset in tests.
    #[test]
    fn cached_areas_load() {
        if std::env::var_os(DATA_DIR_ENV).is_none() {
            assert!(areas().unwrap().by_key("USA").is_some());
        }
    }

    /// The directory comes from the environment variable, or the crate's `data/` when it is
    /// unset or empty.
    #[test]
    fn data_dir_from_env() {
        let default = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        assert_eq!(data_dir_from(None), default);
        assert_eq!(data_dir_from(Some(OsString::new())), default);
        assert_eq!(
            data_dir_from(Some("/app/reference".into())),
            PathBuf::from("/app/reference")
        );
    }

    /// A missing file names its path and the environment variable.
    #[test]
    fn missing_file_names_path_and_env() {
        let e = load_areas(Path::new("/nonexistent/countries.csv"))
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("/nonexistent/countries.csv") && e.contains(DATA_DIR_ENV),
            "{e}"
        );
    }

    /// Comment lines, quoted names and empty optional fields parse.
    #[test]
    fn parses_rows() {
        let areas = parse_rows(
            "# comment\n\
             BES,country,BQ,BES,535,BES,BQ,\"Bonaire, Sint Eustatius and Saba\",LCN,HIC\n\
             XKX,country,XK,XKX,,XKX,XK,Kosovo,,\n\
             WLD,aggregate,,,,WLD,,World,,\n",
        )
        .unwrap();
        let bes = areas.by_key("BES").unwrap();
        assert_eq!(bes.name, "Bonaire, Sint Eustatius and Saba");
        assert_eq!(
            (bes.region.as_deref(), bes.income_group.as_deref()),
            (Some("LCN"), Some("HIC"))
        );
        assert_eq!(areas.by_key("XKX").unwrap().iso_numeric, None);
        assert_eq!(areas.by_key("WLD").unwrap().region, None);
    }

    /// Malformed rows are rejected with the offending row and column.
    #[test]
    fn rejects_malformed_rows() {
        for (rows, needle) in [
            (
                "USA,nation,US,USA,840,USA,US,United States,,\n",
                "unknown variant",
            ),
            ("USA,country,US,USA,840,USA,US,,,\n", "empty name"),
            (
                "USA,country,U,USA,840,USA,US,United States,,\n",
                "invalid iso2",
            ),
            (
                "USA,country,us,USA,840,USA,US,United States,,\n",
                "invalid iso2",
            ),
            (
                "USA,country,US,USA,84,USA,US,United States,,\n",
                "invalid iso_numeric",
            ),
            (
                "USA,country,US,USA,84a,USA,US,United States,,\n",
                "invalid iso_numeric",
            ),
            (
                "USA,country,US,USA,840,U-A,US,United States,,\n",
                "invalid wb_code",
            ),
            (
                "USA,country,US,USA,840,usa,US,United States,,\n",
                "invalid wb_code",
            ),
            (
                "USA,country,US,USA,840,USA,us,United States,,\n",
                "invalid sdmx_ref_area",
            ),
            (
                "USA,country,US,,840,USA,US,United States,,\n",
                "must be its iso3",
            ),
            (
                "USA,country,US,USB,840,USA,US,United States,,\n",
                "must be its iso3",
            ),
            (
                "USA,country,,USA,840,USA,US,United States,,\n",
                "needs iso2",
            ),
            (
                "EMU,aggregate,,,,EMU,,Euro area,,\nEMU,aggregate,,,,EMU,,Euro area,,\n",
                "duplicate key",
            ),
            (
                "EMU,aggregate,EU,,,EMU,,Euro area,,\n",
                "must not have ISO codes",
            ),
            (
                "EMU,aggregate,,,978,EMU,,Euro area,,\n",
                "must not have ISO codes",
            ),
            ("EMU,aggregate,,,,,,Euro area,,\n", "must be its wb_code"),
            (
                "USA,country,US,USA,840,USA,US,United States,,\n\
                 USB,country,US,USB,841,USB,UB,Other,,\n",
                "duplicate iso2",
            ),
            (
                "USA,country,US,USA,840,USA,US,United States,,\n\
                 USB,country,UB,USB,840,USB,UB,Other,,\n",
                "duplicate iso_numeric 840",
            ),
            (
                "USA,country,US,USA,840,USA,US,United States,,\n\
                 USB,country,UB,USB,841,USA,UB,Other,,\n",
                "duplicate wb_code",
            ),
            (
                "USA,country,US,USA,840,USA,US,United States,,\n\
                 USB,country,UB,USB,841,USB,US,Other,,\n",
                "duplicate sdmx_ref_area",
            ),
        ] {
            let e = parse_rows(rows).unwrap_err();
            assert!(e.contains(needle), "{rows:?}: {e}");
        }
    }

    /// A wrong header or a table short of the ISO countries is rejected.
    #[test]
    fn rejects_bad_header_and_incomplete_tables() {
        let e = parse_areas("key,name\nUSA,United States\n").unwrap_err();
        assert!(e.contains("expected header"), "{e}");
        let e = parse_areas(&format!(
            "{HEADER}USA,country,US,USA,840,USA,US,United States,,\n"
        ))
        .unwrap_err();
        assert!(e.contains("at least 249") && e.contains("got 1"), "{e}");
        let e = parse_areas("").unwrap_err();
        assert!(e.contains("expected header"), "{e}");
    }
}
