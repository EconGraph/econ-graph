// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! The securities the crawler fetches from Massive, a market data vendor: the S&P 500
//! constituents, read at runtime from `sp500.csv` (not compiled in).
//!
//! The file is `$MASSIVE_SYMBOLS_PATH`, or when that is unset [`SYMBOLS_FILE`] in the crawler
//! data directory ([`crate::reference::data_dir`], `/app/data` in the images). Its header is
//! `share_class_figi,cik10,ticker,exchange,currency,name,valid_from,valid_to`; `data/massive/
//! README.md` says where the rows come from and how to regenerate them.
//!
//! A security is its share-class FIGI. Each row says which ticker, exchange and name it had from
//! `valid_from` to `valid_to` (inclusive; empty while current). A ticker change is two rows for
//! one FIGI, so earlier prices keep the ticker they traded under: [`SymbolTable::active_on`]
//! picks the row in force on a date.
//!
//! [`SymbolTable::load`] checks every row and fails on the first bad one, naming its row number
//! (the first data row is row 1) and file line. It also rejects a repeated
//! `(share_class_figi, valid_from)` and overlapping rows for one FIGI, so at most one row per
//! security is active on any date.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use chrono::NaiveDate;

/// Environment variable naming the symbol file, overriding [`default_path`]'s data-directory file.
pub const SYMBOLS_PATH_ENV: &str = "MASSIVE_SYMBOLS_PATH";

/// The symbol file's path relative to the crawler data directory.
pub const SYMBOLS_FILE: &str = "massive/sp500.csv";

/// The file's header, in order.
pub const HEADER: [&str; 8] = [
    "share_class_figi",
    "cik10",
    "ticker",
    "exchange",
    "currency",
    "name",
    "valid_from",
    "valid_to",
];

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

impl SymbolRow {
    /// Whether the row applies on `date`.
    pub fn is_active_on(&self, date: NaiveDate) -> bool {
        self.valid_from <= date && self.valid_to.is_none_or(|to| date <= to)
    }
}

/// The rows of a symbol file, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolTable {
    rows: Vec<SymbolRow>,
}

/// `$MASSIVE_SYMBOLS_PATH` if set and non-empty, otherwise [`SYMBOLS_FILE`] in the crawler data
/// directory.
pub fn default_path() -> PathBuf {
    path_from_env(std::env::var_os(SYMBOLS_PATH_ENV))
}

fn path_from_env(value: Option<OsString>) -> PathBuf {
    value.filter(|v| !v.is_empty()).map_or_else(
        || crate::reference::data_dir().join(SYMBOLS_FILE),
        PathBuf::from,
    )
}

impl SymbolTable {
    /// Reads and checks the symbol file at `path`.
    pub fn load(path: &Path) -> anyhow::Result<SymbolTable> {
        let text = std::fs::read_to_string(path).with_context(|| {
            format!(
                "reading Massive symbol file {} (set {SYMBOLS_PATH_ENV})",
                path.display()
            )
        })?;
        Self::parse(&text).with_context(|| format!("Massive symbol file {}", path.display()))
    }

    /// Reads and checks the symbol file at [`default_path`].
    pub fn load_default() -> anyhow::Result<SymbolTable> {
        Self::load(&default_path())
    }

    /// Parses and checks symbol file text.
    pub fn parse(text: &str) -> anyhow::Result<SymbolTable> {
        let mut reader = csv::ReaderBuilder::new().from_reader(text.as_bytes());
        let header = reader.headers().context("reading header")?;
        if header.iter().ne(HEADER) {
            bail!(
                "expected header {}, got {}",
                HEADER.join(","),
                header.iter().collect::<Vec<_>>().join(",")
            );
        }
        let mut rows = Vec::new();
        let mut keys: HashMap<(String, NaiveDate), usize> = HashMap::new();
        for (i, record) in reader.records().enumerate() {
            let n = i + 1;
            let record = record.map_err(|e| anyhow!("row {n}: {e}"))?;
            let line = record.position().map_or(0, |p| p.line());
            let row = parse_row(&record).map_err(|e| anyhow!("row {n} (line {line}): {e}"))?;
            let key = (row.share_class_figi.clone(), row.valid_from);
            if let Some(first) = keys.insert(key, n) {
                bail!(
                    "row {n} (line {line}): duplicate of row {first}: share_class_figi {} with valid_from {}",
                    row.share_class_figi,
                    row.valid_from
                );
            }
            rows.push(row);
        }
        if rows.is_empty() {
            bail!("no rows");
        }
        check_no_overlap(&rows)?;
        Ok(SymbolTable { rows })
    }

    /// The rows that apply on `date`: at most one per security.
    pub fn active_on(&self, date: NaiveDate) -> impl Iterator<Item = &SymbolRow> {
        self.rows.iter().filter(move |r| r.is_active_on(date))
    }

    /// Every row, current and closed, in file order.
    pub fn rows(&self) -> &[SymbolRow] {
        &self.rows
    }

    /// Number of rows, current and closed.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Always false for a loaded table, which needs at least one row.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

fn parse_row(record: &csv::StringRecord) -> anyhow::Result<SymbolRow> {
    // The reader rejects a record whose field count differs from the header's.
    let field = |i: usize| record.get(i).unwrap_or_default();
    let figi = field(0);
    if !(figi.len() == 12
        && figi.starts_with("BBG")
        && figi
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()))
    {
        bail!("share_class_figi {figi:?} is not a FIGI (BBG and 9 upper-case letters or digits)");
    }
    let cik10 = field(1);
    if !(cik10.len() == 10 && cik10.bytes().all(|b| b.is_ascii_digit())) {
        bail!("cik10 {cik10:?} is not 10 digits");
    }
    let ticker = field(2);
    if ticker.is_empty()
        || !ticker
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'.')
    {
        bail!("ticker {ticker:?} is empty or not upper-case letters, digits and '.'");
    }
    let exchange = field(3);
    if !(exchange.len() == 4 && exchange.bytes().all(|b| b.is_ascii_uppercase())) {
        bail!("exchange {exchange:?} is not a MIC (4 upper-case letters)");
    }
    let currency = field(4);
    if !(currency.len() == 3 && currency.bytes().all(|b| b.is_ascii_uppercase())) {
        bail!("currency {currency:?} is not an ISO 4217 code");
    }
    let name = field(5);
    if name.trim().is_empty() {
        bail!("empty name");
    }
    let date = |col: &str, s: &str| {
        NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|e| anyhow!("{col} {s:?} is not a YYYY-MM-DD date: {e}"))
    };
    let valid_from = date("valid_from", field(6))?;
    let valid_to = match field(7) {
        "" => None,
        s => Some(date("valid_to", s)?),
    };
    if valid_to.is_some_and(|to| to < valid_from) {
        bail!("valid_to {} is before valid_from {valid_from}", field(7));
    }
    Ok(SymbolRow {
        share_class_figi: figi.to_owned(),
        cik10: cik10.to_owned(),
        ticker: ticker.to_owned(),
        exchange: exchange.to_owned(),
        currency: currency.to_owned(),
        name: name.to_owned(),
        valid_from,
        valid_to,
    })
}

/// Rejects two rows for one FIGI whose date ranges overlap.
fn check_no_overlap(rows: &[SymbolRow]) -> anyhow::Result<()> {
    let mut by_figi: HashMap<&str, Vec<(usize, &SymbolRow)>> = HashMap::new();
    for (i, row) in rows.iter().enumerate() {
        by_figi
            .entry(&row.share_class_figi)
            .or_default()
            .push((i + 1, row));
    }
    for (figi, mut list) in by_figi {
        list.sort_by_key(|(_, r)| r.valid_from);
        for pair in list.windows(2) {
            let ((a_n, a), (b_n, b)) = (pair[0], pair[1]);
            if a.valid_to.is_none_or(|to| to >= b.valid_from) {
                bail!(
                    "rows {a_n} and {b_n} overlap: share_class_figi {figi} has two rows active on {}",
                    b.valid_from
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const HEAD: &str = "share_class_figi,cik10,ticker,exchange,currency,name,valid_from,valid_to\n";

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn crate_path(rel: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
    }

    fn shipped() -> SymbolTable {
        SymbolTable::load(&crate_path("data").join(SYMBOLS_FILE)).unwrap()
    }

    fn parse_err(body: &str) -> String {
        format!(
            "{:#}",
            SymbolTable::parse(&format!("{HEAD}{body}")).unwrap_err()
        )
    }

    const GOOD: &str = "BBG001S5N8V8,0000320193,AAPL,XNAS,USD,Apple Inc.,2026-11-09,\n";

    #[test]
    fn parses_a_row() {
        let t = SymbolTable::parse(&format!(
            "{HEAD}{GOOD}BBG001S90346,0001067983,BRK.B,XNYS,USD,\"Berkshire Hathaway, Class B\",2020-01-01,2025-12-31\n"
        ))
        .unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(
            t.rows()[1],
            SymbolRow {
                share_class_figi: "BBG001S90346".into(),
                cik10: "0001067983".into(),
                ticker: "BRK.B".into(),
                exchange: "XNYS".into(),
                currency: "USD".into(),
                name: "Berkshire Hathaway, Class B".into(),
                valid_from: d("2020-01-01"),
                valid_to: Some(d("2025-12-31")),
            }
        );
        assert_eq!(t.rows()[0].valid_to, None);
    }

    /// Each malformed row fails the load, and the error names its row number and line.
    #[test]
    fn rejects_malformed_rows() {
        for (bad, needle) in [
            (
                "XYZ001S5N8V8,0000320193,AAPL,XNAS,USD,A,2026-11-09,\n",
                "share_class_figi",
            ),
            ("BBG001S5N8V8,320193,AAPL,XNAS,USD,A,2026-11-09,\n", "cik10"),
            (
                "BBG001S5N8V8,000032019X,AAPL,XNAS,USD,A,2026-11-09,\n",
                "cik10",
            ),
            (
                "BBG001S5N8V8,0000320193,,XNAS,USD,A,2026-11-09,\n",
                "ticker",
            ),
            (
                "BBG001S5N8V8,0000320193,brk-b,XNAS,USD,A,2026-11-09,\n",
                "ticker",
            ),
            (
                "BBG001S5N8V8,0000320193,AAPL,Nasdaq,USD,A,2026-11-09,\n",
                "exchange",
            ),
            (
                "BBG001S5N8V8,0000320193,AAPL,XNAS,usd,A,2026-11-09,\n",
                "currency",
            ),
            (
                "BBG001S5N8V8,0000320193,AAPL,XNAS,USD, ,2026-11-09,\n",
                "empty name",
            ),
            (
                "BBG001S5N8V8,0000320193,AAPL,XNAS,USD,A,11/09/2026,\n",
                "valid_from",
            ),
            (
                "BBG001S5N8V8,0000320193,AAPL,XNAS,USD,A,2026-11-09,2026-13-01\n",
                "valid_to",
            ),
            (
                "BBG001S5N8V8,0000320193,AAPL,XNAS,USD,A,2026-11-09,2026-11-08\n",
                "before valid_from",
            ),
            (
                "BBG001S5N8V8,0000320193,AAPL,XNAS,USD,A,2026-11-09\n",
                "found record with 7 fields",
            ),
            (
                "BBG001S5N8V8,0000320193,AAPL,XNAS,USD,A,2026-11-09,,x\n",
                "found record with 9 fields",
            ),
        ] {
            let e = parse_err(&format!(
                "BBG001S5P4H1,0000014693,BF.B,XNYS,USD,B,2026-11-09,\n{bad}"
            ));
            assert!(e.contains("row 2") && e.contains(needle), "{bad:?}: {e}");
        }
        let e = parse_err("BBG001S5N8V8,320193,AAPL,XNAS,USD,A,2026-11-09,\n");
        assert!(e.contains("row 1 (line 2)"), "{e}");
    }

    #[test]
    fn rejects_bad_header_and_empty_file() {
        let e = SymbolTable::parse("cik10,ticker,exchange,currency,name,valid_from,valid_to\n")
            .unwrap_err();
        assert!(format!("{e:#}").contains("expected header"), "{e:#}");
        let e = SymbolTable::parse(HEAD).unwrap_err();
        assert!(format!("{e:#}").contains("no rows"), "{e:#}");
    }

    #[test]
    fn rejects_duplicate_key() {
        let e = parse_err(&format!(
            "{GOOD}BBG001S5P4H1,0000014693,BF.B,XNYS,USD,B,2026-11-09,\n\
             BBG001S5N8V8,0000320193,AAPL2,XNAS,USD,Apple,2026-11-09,2026-11-10\n"
        ));
        assert!(
            e.contains("row 3") && e.contains("duplicate of row 1"),
            "{e}"
        );
    }

    #[test]
    fn rejects_overlapping_rows_for_one_figi() {
        let e = parse_err(
            "BBG001S5N8V8,0000320193,OLD,XNAS,USD,A,2020-01-01,2024-06-30\n\
             BBG001S5N8V8,0000320193,NEW,XNAS,USD,A,2024-06-30,\n",
        );
        assert!(e.contains("rows 1 and 2 overlap"), "{e}");
        let e = parse_err(
            "BBG001S5N8V8,0000320193,OLD,XNAS,USD,A,2020-01-01,\n\
             BBG001S5N8V8,0000320193,NEW,XNAS,USD,A,2024-07-01,\n",
        );
        assert!(e.contains("overlap"), "{e}");
    }

    /// A ticker change is two rows; each date sees the ticker in force then.
    #[test]
    fn active_on_honours_validity_dates() {
        let t = SymbolTable::parse(&format!(
            "{HEAD}BBG000000001,0000000001,OLD,XNYS,USD,Renamed Co,2020-01-01,2024-06-30\n\
             BBG000000001,0000000001,NEW,XNYS,USD,Renamed Co,2024-07-01,\n\
             BBG000000002,0000000002,GONE,XNAS,USD,Delisted Co,2021-03-01,2022-12-31\n\
             BBG000000003,0000000003,LATE,XNAS,USD,Added Co,2026-01-01,\n"
        ))
        .unwrap();
        let tickers = |date: &str| {
            t.active_on(d(date))
                .map(|r| r.ticker.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(tickers("2019-12-31"), Vec::<&str>::new());
        assert_eq!(tickers("2020-01-01"), ["OLD"]);
        assert_eq!(tickers("2021-03-01"), ["OLD", "GONE"]);
        assert_eq!(tickers("2022-12-31"), ["OLD", "GONE"]);
        assert_eq!(tickers("2023-01-01"), ["OLD"]);
        assert_eq!(tickers("2024-06-30"), ["OLD"]);
        assert_eq!(tickers("2024-07-01"), ["NEW"]);
        assert_eq!(tickers("2026-01-01"), ["NEW", "LATE"]);
    }

    #[test]
    fn load_names_missing_path_and_env() {
        let e = SymbolTable::load(Path::new("/nonexistent/sp500.csv")).unwrap_err();
        let e = format!("{e:#}");
        assert!(
            e.contains("/nonexistent/sp500.csv") && e.contains(SYMBOLS_PATH_ENV),
            "{e}"
        );
    }

    #[test]
    fn load_names_the_file_on_a_bad_row() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("symbols.csv");
        std::fs::write(&path, format!("{HEAD}{GOOD}bad\n")).unwrap();
        let e = format!("{:#}", SymbolTable::load(&path).unwrap_err());
        assert!(
            e.contains(&path.display().to_string()) && e.contains("row 2"),
            "{e}"
        );
    }

    #[test]
    fn path_from_env_overrides_data_dir() {
        assert_eq!(
            path_from_env(Some("/etc/symbols.csv".into())),
            PathBuf::from("/etc/symbols.csv")
        );
        let default = crate::reference::data_dir().join(SYMBOLS_FILE);
        assert_eq!(path_from_env(Some("".into())), default);
        assert_eq!(path_from_env(None), default);
    }

    /// The shipped file sits where the images put the data directory's files: `data/` is copied
    /// whole to `$CRAWLER_DATA_DIR` (backend/Dockerfile), so `massive/sp500.csv` ships with it.
    #[test]
    fn shipped_file_is_valid() {
        assert!(crate_path("data").join(SYMBOLS_FILE).is_file());
        let t = shipped();
        let today = t.active_on(d("2026-11-09")).collect::<Vec<_>>();
        assert!((495..=510).contains(&today.len()), "{}", today.len());
        assert!(today.iter().all(|r| r.currency == "USD"));
        let figis: HashSet<&str> = today.iter().map(|r| r.share_class_figi.as_str()).collect();
        assert_eq!(figis.len(), today.len(), "a FIGI repeats among active rows");
        let tickers: HashSet<&str> = today.iter().map(|r| r.ticker.as_str()).collect();
        assert_eq!(
            tickers.len(),
            today.len(),
            "a ticker repeats among active rows"
        );
        assert!(tickers.contains("BRK.B") && tickers.contains("BF.B"));
        let by = |t: &str| today.iter().find(|r| r.ticker == t).unwrap();
        let (goog, googl) = (by("GOOG"), by("GOOGL"));
        assert_eq!(goog.cik10, googl.cik10);
        assert_ne!(goog.share_class_figi, googl.share_class_figi);
    }

    /// Every row's CIK, with its ticker, is in the pinned copy of SEC's company_tickers.json
    /// (SEC writes share classes with a dash: BRK-B).
    #[test]
    fn shipped_ciks_are_in_pinned_sec_mapping() {
        let text =
            std::fs::read_to_string(crate_path("tests/fixtures/massive/sec_company_tickers.csv"))
                .unwrap();
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some("cik10,ticker"));
        let sec: HashSet<(&str, &str)> = lines.map(|l| l.split_once(',').unwrap()).collect();
        let ciks: HashSet<&str> = sec.iter().map(|(c, _)| *c).collect();
        for row in shipped().rows() {
            assert!(
                ciks.contains(row.cik10.as_str()),
                "{}: CIK {} not in SEC mapping",
                row.ticker,
                row.cik10
            );
            let sec_ticker = row.ticker.replace('.', "-");
            if row.valid_to.is_none() {
                assert!(
                    sec.contains(&(row.cik10.as_str(), sec_ticker.as_str())),
                    "{}: SEC does not map {sec_ticker} to {}",
                    row.ticker,
                    row.cik10
                );
            }
        }
    }

    /// Every current row's FIGI is the share-class FIGI OpenFIGI returned for its ticker, in the
    /// pinned responses (OpenFIGI writes share classes with a slash: BRK/B).
    #[test]
    fn shipped_figis_match_pinned_openfigi_responses() {
        let text =
            std::fs::read_to_string(crate_path("tests/fixtures/massive/openfigi_mapping.json"))
                .unwrap();
        let pinned: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
        let mut figi_for: HashMap<String, HashSet<String>> = HashMap::new();
        for entry in &pinned {
            let ticker = entry["job"]["idValue"].as_str().unwrap().replace('/', ".");
            for data in entry["result"]["data"].as_array().into_iter().flatten() {
                if let Some(f) = data["shareClassFIGI"].as_str() {
                    figi_for
                        .entry(ticker.clone())
                        .or_default()
                        .insert(f.to_owned());
                }
            }
        }
        for row in shipped().rows().iter().filter(|r| r.valid_to.is_none()) {
            let figis = figi_for
                .get(&row.ticker)
                .unwrap_or_else(|| panic!("{}: not in OpenFIGI responses", row.ticker));
            assert_eq!(
                figis.iter().collect::<Vec<_>>(),
                [&row.share_class_figi],
                "{}",
                row.ticker
            );
        }
    }
}
