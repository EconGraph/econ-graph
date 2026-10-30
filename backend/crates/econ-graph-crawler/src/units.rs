//! Units of measure: a fixed vocabulary, a strict parser from source text into it, and where a
//! series' units came from.
//!
//! # Vocabulary
//!
//! A [`Unit`] is a [`UnitKind`], a [`Scale`], and, where the source states them, an ISO 4217
//! currency code, an index base (`2017=100`) and the reference year of a constant-price amount.
//! The scale describes the stored values: a series whose text is `Billions of Dollars` stores
//! `21000` for 21 trillion dollars. An adapter that rescales values (BEA multiplies by
//! `10^UNIT_MULT`) stores units text that describes the rescaled values.
//!
//! # Parsing
//!
//! [`parse`] recognises only the phrases listed here. It normalises case and whitespace and
//! nothing else, and returns `None` for anything it does not recognise, including phrases that
//! are ambiguous (`Rate`, `Percent or rate`, `Level`), units outside the vocabulary (`Mt CO2e`,
//! `+1 or 0`), and a scale on a kind that cannot have one (`Millions of percent`). It never
//! guesses. `Dollars` alone is a currency with no code unless the caller's [`UnitsContext`]
//! names one: FRED, BEA and the Bank of Canada all just say `dollars`.
//!
//! A phrase is `[qualifier, ][scale of ]body[, scale]`:
//!
//! - qualifier: `Month-end`, which says when a value is observed, not what it measures.
//! - scale: `Thousands of` / `Millions of` / `Billions of` / `Trillions of` before the body, or
//!   `, Thousands` / `, In Thousands` / ` - In Thousands` (and the other scales) after it.
//!   Without one, the scale is [`Scale::Units`].
//! - body, by kind:
//!   - currency: an optional price basis (`current`, `constant 2015`, `chained`,
//!     `chained 2017`, `chained (2017)`) and a currency (`dollars`, `US dollars`, `U.S. dollars`,
//!     `US$`, `Canadian dollars`, `euro`, `yen`, `Swiss francs`, `pounds sterling`,
//!     `national currency`). `constant` and `chained` make it
//!     [`UnitKind::ConstantPriceCurrency`].
//!   - count: `count`, `number`, `persons`, `people`, `employees`, `all employees`.
//!   - percent: `percent`, `%`, `percent per annum`, `% per year`, `unemployment rate`, and
//!     `percent of ...` / `% of ...` (a share of something).
//!   - percent change: `percent change`, `% change`, `percent change, annual rate`,
//!     `percent change at annual rate`, `percent change from year ago`.
//!   - percentage points: `percentage points`.
//!   - index: `index`, `Fisher price index`, `Fisher quantity index`, or `index` followed by a
//!     base, bare or in parentheses: `Index 2017=100`, `Index (January 1991 = 100)`,
//!     `Index 1982-84=100`, `Index (1991Q1 = 100)`.
//!   - rate: `per 1000 people`, `per 100000 people`, `per million people`,
//!     `per 1000 live births`, `per 100000 live births`.
//!   - years: `years`.
//!
//! Only currency and count take a scale.
//!
//! # Checking source text
//!
//! When a fetched series is stored, [`check_and_record`] parses its units text and compares it
//! with the units the source's own structured fields state, if the adapter supplied them
//! ([`StructuredUnit`]). It logs and counts what it finds, by source, and never changes the
//! stored units text or values:
//!
//! - the text does not parse: `crawler_units_unrecognised_total`, logged at `error` when the
//!   source also gave structured units (both describe the same values), else at `debug`;
//! - the text parses but disagrees with the structured units: `crawler_units_mismatch_total`,
//!   logged at `error` with both;
//! - they agree: `crawler_units_agree_total`.
//!
//! It returns the unit stored in `economic_series.units_parsed` ([`stored_unit`]): each field
//! the structured units state, else the text's. Each stored series is checked once, so a crawl
//! logs at most one line per series.

use std::fmt;

use econ_graph_metrics::crawler::CRAWLER_UNITS_METRICS;
use serde::{Deserialize, Serialize};

use crate::source::SourceId;

/// What kind of quantity a series measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitKind {
    /// An amount of money at the prices of each period (`Billions of Dollars`, `current US$`).
    Currency,
    /// An amount of money at the prices of a reference period: constant or chained
    /// (`Chained 2017 Dollars`, `constant 2015 US$`).
    ConstantPriceCurrency,
    /// A percentage level or share (`Percent`, `% of GDP`, an unemployment rate).
    Percent,
    /// A percentage change between periods (`Percent change, annual rate`).
    PercentChange,
    /// A difference between two percentages (`Percentage Points`).
    PercentagePoints,
    /// An index number (`Index 2017=100`).
    Index,
    /// A number of people or things (`Thousands of Persons`, `Count`).
    Count,
    /// Events per fixed number of a population (`per 1000 people`).
    Rate,
    /// A duration in years (`years`, as in life expectancy).
    Years,
}

impl UnitKind {
    /// The kind's name: lower case, words joined by `_`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Currency => "currency",
            Self::ConstantPriceCurrency => "constant_price_currency",
            Self::Percent => "percent",
            Self::PercentChange => "percent_change",
            Self::PercentagePoints => "percentage_points",
            Self::Index => "index",
            Self::Count => "count",
            Self::Rate => "rate",
            Self::Years => "years",
        }
    }

    /// Whether values of this kind can be stated in thousands, millions, ...
    const fn takes_scale(self) -> bool {
        matches!(
            self,
            Self::Currency | Self::ConstantPriceCurrency | Self::Count
        )
    }
}

/// The power of ten the stored values are in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scale {
    /// Values as they are: no multiplier.
    Units,
    /// Values in thousands.
    Thousands,
    /// Values in millions.
    Millions,
    /// Values in billions (10^9).
    Billions,
    /// Values in trillions (10^12).
    Trillions,
}

impl Scale {
    /// The scale's name, lower case.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Units => "units",
            Self::Thousands => "thousands",
            Self::Millions => "millions",
            Self::Billions => "billions",
            Self::Trillions => "trillions",
        }
    }

    const NAMED: [(Self, &'static str); 4] = [
        (Self::Thousands, "thousands"),
        (Self::Millions, "millions"),
        (Self::Billions, "billions"),
        (Self::Trillions, "trillions"),
    ];
}

/// A unit of measure in the fixed vocabulary. Stored as JSON in `economic_series.units_parsed`,
/// e.g. `{"kind": "currency", "currency": "USD", "scale": "billions"}`; a field that is `None`
/// is left out.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Unit {
    /// What is measured.
    pub kind: UnitKind,
    /// ISO 4217 code of a currency kind, when the source names the currency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// The power of ten the stored values are in.
    pub scale: Scale,
    /// For an index: the period it equals its base value in, and that value (`2017=100`,
    /// `1982-1984=100`, `January 1991=100`, `1991Q1=100`), when the source states it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_base: Option<String>,
    /// For a constant-price currency: the year whose prices it uses, when the source states it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_reference_year: Option<i32>,
}

impl Unit {
    /// A unit of `kind` in `scale`, with no currency, index base or reference year.
    pub fn new(kind: UnitKind, scale: Scale) -> Self {
        Self {
            kind,
            currency: None,
            scale,
            index_base: None,
            price_reference_year: None,
        }
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "kind={} scale={}",
            self.kind.as_str(),
            self.scale.as_str()
        )?;
        if let Some(c) = &self.currency {
            write!(f, " currency={c}")?;
        }
        if let Some(b) = &self.index_base {
            write!(f, " index_base={b}")?;
        }
        if let Some(y) = self.price_reference_year {
            write!(f, " price_reference_year={y}")?;
        }
        Ok(())
    }
}

/// Where a series' units came from (`economic_series.units_source`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum UnitsSource {
    /// From a field of the source's own that holds units (FRED's `units`, BEA's `CL_UNIT`), or
    /// from the adapter's curated list of what it crawls.
    #[default]
    Structured,
    /// Parsed by the crawler out of free text that also says other things (a description).
    ParsedFromText,
    /// The series has no units.
    Unknown,
}

impl UnitsSource {
    /// The stored value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Structured => "structured",
            Self::ParsedFromText => "parsed_from_text",
            Self::Unknown => "unknown",
        }
    }

    /// The source for a stored value; `None` for anything else.
    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "structured" => Some(Self::Structured),
            "parsed_from_text" => Some(Self::ParsedFromText),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// What the parser may assume about a source's texts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnitsContext {
    /// ISO 4217 code that a bare `dollars` means for this source (`"CAD"` for the Bank of
    /// Canada). `None` leaves such a currency without a code.
    pub dollars: Option<&'static str>,
}

/// Parses a units phrase into the vocabulary, with no assumptions about the source. See the
/// [module docs](self) for the phrases it recognises.
pub fn parse(text: &str) -> Option<Unit> {
    parse_with(text, &UnitsContext::default())
}

/// [`parse`] with what the caller knows about the source's texts.
pub fn parse_with(text: &str, ctx: &UnitsContext) -> Option<Unit> {
    let normalised = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let mut s = normalised.as_str();
    for qualifier in QUALIFIERS {
        if let Some(rest) = s.strip_prefix(qualifier) {
            s = rest;
            break;
        }
    }
    let (scale, body) = split_scale(s);
    let unit = parse_body(body, scale, ctx)?;
    (unit.scale == Scale::Units || unit.kind.takes_scale()).then_some(unit)
}

/// Leading words that say when a value is observed, not what it measures.
const QUALIFIERS: &[&str] = &["month-end, "];

/// Splits a scale off the front (`millions of x`) or the back (`x, in millions`) of `s`.
fn split_scale(s: &str) -> (Scale, &str) {
    for (scale, word) in Scale::NAMED {
        if let Some(rest) = s.strip_prefix(word).and_then(|r| r.strip_prefix(" of ")) {
            return (scale, rest);
        }
        for suffix in [
            format!(", in {word}"),
            format!(", {word}"),
            format!(" - in {word}"),
        ] {
            if let Some(rest) = s.strip_suffix(suffix.as_str()) {
                return (scale, rest);
            }
        }
    }
    (Scale::Units, s)
}

const PERCENT: &[&str] = &[
    "percent",
    "%",
    "percent per annum",
    "% per year",
    "unemployment rate",
];
const PERCENT_CHANGE: &[&str] = &[
    "percent change",
    "% change",
    "percent change, annual rate",
    "percent change at annual rate",
    "percent change from year ago",
];
const PERCENTAGE_POINTS: &[&str] = &["percentage points", "percentage point"];
const COUNT: &[&str] = &[
    "count",
    "number",
    "persons",
    "people",
    "employees",
    "all employees",
];
const RATE: &[&str] = &[
    "per 1000 people",
    "per 100000 people",
    "per million people",
    "per 1000 live births",
    "per 100000 live births",
];
const YEARS: &[&str] = &["years"];
const INDEX_NAMES: &[&str] = &["index", "fisher price index", "fisher quantity index"];

/// Currencies by name. `None` is a currency the text does not identify.
const CURRENCIES: &[(&str, Option<&str>)] = &[
    ("us dollars", Some("USD")),
    ("u.s. dollars", Some("USD")),
    ("us$", Some("USD")),
    ("united states dollars", Some("USD")),
    ("canadian dollars", Some("CAD")),
    ("euro", Some("EUR")),
    ("euros", Some("EUR")),
    ("yen", Some("JPY")),
    ("japanese yen", Some("JPY")),
    ("swiss francs", Some("CHF")),
    ("pounds sterling", Some("GBP")),
    ("national currency", None),
];

fn parse_body(body: &str, scale: Scale, ctx: &UnitsContext) -> Option<Unit> {
    let simple: [(&[&str], UnitKind); 6] = [
        (PERCENT, UnitKind::Percent),
        (PERCENT_CHANGE, UnitKind::PercentChange),
        (PERCENTAGE_POINTS, UnitKind::PercentagePoints),
        (COUNT, UnitKind::Count),
        (RATE, UnitKind::Rate),
        (YEARS, UnitKind::Years),
    ];
    for (phrases, kind) in simple {
        if phrases.contains(&body) {
            return Some(Unit::new(kind, scale));
        }
    }
    // A share of something: "% of GDP", "percent of total electricity production".
    if ["% of ", "percent of "]
        .iter()
        .any(|p| body.strip_prefix(p).is_some_and(|rest| !rest.is_empty()))
    {
        return Some(Unit::new(UnitKind::Percent, scale));
    }
    if INDEX_NAMES.contains(&body) {
        return Some(Unit::new(UnitKind::Index, scale));
    }
    if let Some(base) = body.strip_prefix("index ") {
        let base = base
            .strip_prefix('(')
            .and_then(|b| b.strip_suffix(')'))
            .unwrap_or(base);
        let mut unit = Unit::new(UnitKind::Index, scale);
        unit.index_base = Some(index_base(base)?);
        return Some(unit);
    }
    currency(body, scale, ctx)
}

fn currency(body: &str, scale: Scale, ctx: &UnitsContext) -> Option<Unit> {
    let (kind, year, name) = if let Some(rest) = body.strip_prefix("current ") {
        (UnitKind::Currency, None, rest)
    } else if let Some(rest) = body.strip_prefix("constant ") {
        let (year, name) = rest.split_once(' ')?;
        (UnitKind::ConstantPriceCurrency, Some(year_of(year)?), name)
    } else if let Some(rest) = body.strip_prefix("chained ") {
        match rest
            .split_once(' ')
            .and_then(|(y, name)| Some((year_of(y.trim_matches(['(', ')']))?, name)))
        {
            Some((year, name)) => (UnitKind::ConstantPriceCurrency, Some(year), name),
            None => (UnitKind::ConstantPriceCurrency, None, rest),
        }
    } else {
        (UnitKind::Currency, None, body)
    };
    let code = match name {
        "dollars" | "dollar" => ctx.dollars,
        _ => CURRENCIES.iter().find(|(n, _)| *n == name)?.1,
    };
    let mut unit = Unit::new(kind, scale);
    unit.currency = code.map(str::to_string);
    unit.price_reference_year = year;
    Some(unit)
}

/// A four-digit year.
fn year_of(s: &str) -> Option<i32> {
    if s.len() == 4 && s.bytes().all(|b| b.is_ascii_digit()) {
        s.parse().ok()
    } else {
        None
    }
}

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

/// An index base, `period=value`, in canonical form. `period` is a year (`2017`), a span of
/// years (`1982-84`, `1982-1984`, `2011-12`: the second year after the first), a month and year
/// (`January 1991`, `Jan 1991`) or a quarter (`1991Q1`); `value` is a positive whole number.
fn index_base(s: &str) -> Option<String> {
    let (period, value) = s.split_once('=')?;
    let value = value.trim();
    let positive = !value.is_empty()
        && value.bytes().all(|b| b.is_ascii_digit())
        && value.bytes().any(|b| b != b'0');
    if !positive {
        return None;
    }
    Some(format!(
        "{}={}",
        base_period(period.trim())?,
        value.trim_start_matches('0')
    ))
}

fn base_period(p: &str) -> Option<String> {
    if let Some(year) = year_of(p) {
        return Some(year.to_string());
    }
    if let Some((first, second)) = p.split_once('-') {
        let first = year_of(first)?;
        let second = match second.len() {
            4 => year_of(second)?,
            2 if second.bytes().all(|b| b.is_ascii_digit()) => {
                let y = first - first % 100 + second.parse::<i32>().ok()?;
                if y > first {
                    y
                } else {
                    y + 100
                }
            }
            _ => return None,
        };
        return (second > first).then(|| format!("{first}-{second}"));
    }
    if let Some((year, quarter)) = p.split_once('q') {
        let year = year_of(year)?;
        return matches!(quarter, "1" | "2" | "3" | "4").then(|| format!("{year}Q{quarter}"));
    }
    let (month, year) = p.split_once(' ')?;
    let year = year_of(year)?;
    let month = month.strip_suffix('.').unwrap_or(month);
    let full = MONTHS
        .iter()
        .find(|m| **m == month || (month.len() == 3 && m.starts_with(month)))?;
    let mut name = full.to_string();
    name[..1].make_ascii_uppercase();
    Some(format!("{name} {year}"))
}

/// What a source's structured fields say about a series' stored values. Each field is `None`
/// when the source says nothing about it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StructuredUnit {
    /// The kind of quantity.
    pub kind: Option<UnitKind>,
    /// ISO 4217 code of the currency.
    pub currency: Option<String>,
    /// The power of ten the stored values are in.
    pub scale: Option<Scale>,
    /// The index base, in [`Unit::index_base`]'s form.
    pub index_base: Option<String>,
}

impl fmt::Display for StructuredUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if let Some(k) = self.kind {
            parts.push(format!("kind={}", k.as_str()));
        }
        if let Some(s) = self.scale {
            parts.push(format!("scale={}", s.as_str()));
        }
        if let Some(c) = &self.currency {
            parts.push(format!("currency={c}"));
        }
        if let Some(b) = &self.index_base {
            parts.push(format!("index_base={b}"));
        }
        f.write_str(&parts.join(" "))
    }
}

/// The outcome of comparing a units text with the source's structured units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextCheck {
    /// The text is not in the vocabulary.
    Unrecognised,
    /// The text parses, and the source gave no structured units to compare it with.
    Parsed(Unit),
    /// The text parses and agrees with every field the structured units state.
    Agrees(Unit),
    /// The text parses and disagrees with the structured units in these fields (`kind`,
    /// `currency`, `scale`, `index_base`).
    Mismatch(Unit, Vec<&'static str>),
}

impl TextCheck {
    /// The text's unit, if it parsed.
    pub fn unit(&self) -> Option<&Unit> {
        match self {
            Self::Unrecognised => None,
            Self::Parsed(u) | Self::Agrees(u) | Self::Mismatch(u, _) => Some(u),
        }
    }
}

/// Parses `text` and compares it with `structured`. Currency and index base are compared only
/// when the text states them too: a text that leaves them out does not contradict them.
pub fn check_text(
    text: &str,
    structured: Option<&StructuredUnit>,
    ctx: &UnitsContext,
) -> TextCheck {
    let Some(unit) = parse_with(text, ctx) else {
        return TextCheck::Unrecognised;
    };
    let Some(s) = structured else {
        return TextCheck::Parsed(unit);
    };
    let mut differ = Vec::new();
    if s.kind.is_some_and(|k| k != unit.kind) {
        differ.push("kind");
    }
    // Both given and different: a text that leaves one out doesn't contradict it.
    let both_differ =
        |a: &Option<String>, b: &Option<String>| matches!((a, b), (Some(a), Some(b)) if a != b);
    if both_differ(&s.currency, &unit.currency) {
        differ.push("currency");
    }
    if s.scale.is_some_and(|sc| sc != unit.scale) {
        differ.push("scale");
    }
    if both_differ(&s.index_base, &unit.index_base) {
        differ.push("index_base");
    }
    if differ.is_empty() {
        TextCheck::Agrees(unit)
    } else {
        TextCheck::Mismatch(unit, differ)
    }
}

/// The unit to store for a series: each field the `structured` units state, else the text's
/// (`parsed`). `None` when neither gives a kind and a scale.
pub fn stored_unit(parsed: Option<&Unit>, structured: Option<&StructuredUnit>) -> Option<Unit> {
    let s = structured.cloned().unwrap_or_default();
    let kind = s.kind.or(parsed.map(|p| p.kind))?;
    let scale = s.scale.or(parsed.map(|p| p.scale))?;
    let mut unit = Unit::new(kind, scale);
    unit.currency = s
        .currency
        .or_else(|| parsed.and_then(|p| p.currency.clone()));
    unit.index_base = s
        .index_base
        .or_else(|| parsed.and_then(|p| p.index_base.clone()));
    // A reference year belongs to a constant-price kind; a structured kind can override that.
    unit.price_reference_year = parsed
        .filter(|p| p.kind == kind)
        .and_then(|p| p.price_reference_year);
    Some(unit)
}

/// Creates the units counters at 0 for every source, so the first mismatch after a restart is
/// an increase that `increase()` sees, not the first sample of a new series.
pub fn init_metrics() {
    let metrics = &*CRAWLER_UNITS_METRICS;
    for source in SourceId::ALL {
        let label = [source.as_str()];
        metrics.unrecognised_total.with_label_values(&label);
        metrics.mismatch_total.with_label_values(&label);
        metrics.agree_total.with_label_values(&label);
    }
}

/// Longest units text a log line quotes, in characters.
const LOGGED_TEXT_CHARS: usize = 200;

/// Checks a fetched series' units `text` against the vocabulary and the source's `structured`
/// units, logs and counts the outcome by source (see
/// [Checking source text](self#checking-source-text)), and returns the unit to store
/// ([`stored_unit`]). The check never changes what is stored. Logs only the source, the
/// series' external id and units, never a URL or a key.
pub fn check_and_record(
    source: SourceId,
    external_id: &str,
    text: Option<&str>,
    structured: Option<&StructuredUnit>,
) -> Option<Unit> {
    let Some(text) = text.map(str::trim).filter(|t| !t.is_empty()) else {
        return stored_unit(None, structured);
    };
    let quoted: String = text.chars().take(LOGGED_TEXT_CHARS).collect();
    let label = [source.as_str()];
    let metrics = &*CRAWLER_UNITS_METRICS;
    let outcome = check_text(text, structured, &UnitsContext::default());
    match (&outcome, structured) {
        (TextCheck::Unrecognised, Some(s)) => {
            metrics.unrecognised_total.with_label_values(&label).inc();
            tracing::error!(
                source = source.as_str(),
                series = external_id,
                units_text = %quoted,
                structured = %s,
                "units text not recognised; storing the source's structured units"
            );
        }
        (TextCheck::Unrecognised, None) => {
            metrics.unrecognised_total.with_label_values(&label).inc();
            tracing::debug!(
                source = source.as_str(),
                series = external_id,
                units_text = %quoted,
                "units text not recognised"
            );
        }
        (TextCheck::Mismatch(unit, fields), Some(s)) => {
            metrics.mismatch_total.with_label_values(&label).inc();
            tracing::error!(
                source = source.as_str(),
                series = external_id,
                units_text = %quoted,
                parsed = %unit,
                structured = %s,
                fields = %fields.join(","),
                "units text disagrees with the source's structured units; storing the structured \
                 units"
            );
        }
        (TextCheck::Agrees(_), _) => {
            metrics.agree_total.with_label_values(&label).inc();
        }
        _ => {}
    }
    stored_unit(outcome.unit(), structured)
}

#[cfg(test)]
mod tests;
