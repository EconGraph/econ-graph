//! Source adapters retain each accession independently; FY/FP/frame never select revisions.
use std::collections::BTreeMap;
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use econ_graph_core::models::{CompanyFact, CompanyFactSubject, ResearchPeriod};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::normalize_cik;

#[derive(Debug)]
pub struct ParsedCompanyFacts {
    pub cik: String,
    pub name: String,
    pub facts: Vec<CompanyFact>,
}

/// Parse JSON using serde_json's arbitrary_precision feature before constructing BigDecimal.
/// A conversion through f64 would silently corrupt both fractional and large integer facts.
pub fn parse_companyfacts(bytes: &[u8]) -> Result<ParsedCompanyFacts> {
    let root: Value = serde_json::from_slice(bytes).context("invalid companyfacts JSON")?;
    let raw_cik = root.get("cik").context("missing companyfacts CIK")?;
    let cik = normalize_cik(raw_cik.as_str().unwrap_or(&raw_cik.to_string()))?;
    let name = required_text(&root, "entityName")?.to_owned();
    let taxonomies = root
        .get("facts")
        .and_then(Value::as_object)
        .context("missing facts map")?;
    let mut facts: BTreeMap<String, CompanyFact> = BTreeMap::new();
    for (taxonomy, concepts) in taxonomies {
        for (concept, data) in concepts.as_object().context("invalid concepts map")? {
            let units = data
                .get("units")
                .and_then(Value::as_object)
                .context("missing units map")?;
            for (unit, rows) in units {
                for row in rows.as_array().context("invalid fact array")? {
                    let end = date(required_text(row, "end")?)?;
                    let period = match row.get("start") {
                        None => ResearchPeriod::Instant { date: end },
                        Some(start) => ResearchPeriod::Duration {
                            start: date(start.as_str().context("invalid start date")?)?,
                            end,
                        },
                    };
                    let subject = CompanyFactSubject {
                        cik: cik.clone(),
                        taxonomy: taxonomy.clone(),
                        concept: concept.clone(),
                        period,
                        unit: unit.clone(),
                    };
                    let numeric = row
                        .get("val")
                        .and_then(Value::as_number)
                        .context("fact val must be a JSON number")?;
                    let value = BigDecimal::from_str(&numeric.to_string())
                        .context("invalid decimal fact")?;
                    let mut fact = CompanyFact::new(
                        subject,
                        required_text(row, "accn")?.to_owned(),
                        date(required_text(row, "filed")?)?,
                        value,
                        required_text(row, "form")?.to_owned(),
                    )?;
                    fact.fiscal_year = row
                        .get("fy")
                        .map(|v| {
                            v.as_i64()
                                .and_then(|n| i32::try_from(n).ok())
                                .context("invalid fiscal year")
                        })
                        .transpose()?;
                    fact.fiscal_period = optional_text(row, "fp")?;
                    fact.frame = optional_text(row, "frame")?;
                    fact.label = optional_text(data, "label")?;
                    fact.description = optional_text(data, "description")?;
                    let key = fact.id.to_string();
                    if let Some(old) = facts.get_mut(&key) {
                        if old.value != fact.value
                            || old.filed != fact.filed
                            || old.form != fact.form
                        {
                            bail!(
                                "conflicting facts within accession {} for {}",
                                fact.accession,
                                concept
                            );
                        }
                        // SEC can repeat the same fact with different FY/FP/frame annotations.
                        // Keep deterministic metadata without splitting the economic identity.
                        old.fiscal_year = old.fiscal_year.min(fact.fiscal_year);
                        old.fiscal_period = old.fiscal_period.clone().min(fact.fiscal_period);
                        old.frame = old.frame.clone().max(fact.frame);
                    } else {
                        facts.insert(key, fact);
                    }
                }
            }
        }
    }
    Ok(ParsedCompanyFacts {
        cik,
        name,
        facts: facts.into_values().collect(),
    })
}

fn required_text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    let s = v
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("missing/invalid {key}"))?;
    if s.is_empty() {
        bail!("empty {key}");
    }
    Ok(s)
}
fn optional_text(v: &Value, key: &str) -> Result<Option<String>> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        _ => bail!("invalid {key}"),
    }
}
fn date(s: &str) -> Result<NaiveDate> {
    Ok(NaiveDate::parse_from_str(s, "%Y-%m-%d")?)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionFiling {
    pub cik: String,
    pub accession: String,
    pub filed: NaiveDate,
    pub report_date: Option<NaiveDate>,
    pub form: String,
    pub primary_document: Option<String>,
    pub is_xbrl: bool,
}

#[derive(Debug)]
pub struct ParsedSubmissions {
    pub cik: String,
    pub name: String,
    pub ticker: Option<String>,
    pub sic: Option<String>,
    pub sic_description: Option<String>,
    pub entity_type: Option<String>,
    pub filings: Vec<SubmissionFiling>,
    pub historical_files: Vec<String>,
}

pub fn parse_submissions(bytes: &[u8], expected_cik: &str) -> Result<ParsedSubmissions> {
    let root: Value = serde_json::from_slice(bytes).context("invalid submissions JSON")?;
    let raw_cik = root.get("cik").context("missing submissions CIK")?;
    let cik = normalize_cik(raw_cik.as_str().unwrap_or(&raw_cik.to_string()))?;
    if cik != expected_cik {
        bail!("submissions CIK does not match companyfacts");
    }
    let name = required_text(&root, "name")?.to_owned();
    let filings = root.get("filings").context("missing submissions filings")?;
    let recent = parse_submission_rows(
        filings.get("recent").context("missing recent filings")?,
        &cik,
    )?;
    let historical_files = match filings.get("files") {
        None => Vec::new(),
        Some(v) => v
            .as_array()
            .context("invalid historical files")?
            .iter()
            .map(|f| Ok(required_text(f, "name")?.to_owned()))
            .collect::<Result<Vec<_>>>()?,
    };
    let ticker = root
        .get("tickers")
        .and_then(Value::as_array)
        .and_then(|v| v.first())
        .map(|v| v.as_str().context("invalid ticker").map(str::to_owned))
        .transpose()?;
    Ok(ParsedSubmissions {
        cik,
        name,
        ticker,
        sic: optional_text(&root, "sic")?,
        sic_description: optional_text(&root, "sicDescription")?,
        entity_type: optional_text(&root, "entityType")?,
        filings: recent,
        historical_files,
    })
}

pub fn parse_submission_rows(root: &Value, cik: &str) -> Result<Vec<SubmissionFiling>> {
    fn column<'a>(root: &'a Value, name: &str) -> Result<&'a Vec<Value>> {
        root.get(name)
            .and_then(Value::as_array)
            .with_context(|| format!("missing filings column {name}"))
    }
    let accessions = column(root, "accessionNumber")?;
    let filed = column(root, "filingDate")?;
    let report = column(root, "reportDate")?;
    let forms = column(root, "form")?;
    let documents = column(root, "primaryDocument")?;
    let xbrl = column(root, "isXBRL")?;
    for values in [filed, report, forms, documents, xbrl] {
        if values.len() != accessions.len() {
            bail!("misaligned submissions columns");
        }
    }
    (0..accessions.len())
        .map(|i| {
            let text = |v: &Value| {
                v.as_str()
                    .context("invalid filings column value")
                    .map(str::to_owned)
            };
            let accession = text(&accessions[i])?;
            if accession.len() != 20
                || !accession.bytes().enumerate().all(|(i, b)| {
                    if i == 10 || i == 13 {
                        b == b'-'
                    } else {
                        b.is_ascii_digit()
                    }
                })
            {
                bail!("invalid filing accession");
            }
            let report = text(&report[i])?;
            let doc = text(&documents[i])?;
            Ok(SubmissionFiling {
                cik: cik.to_owned(),
                accession,
                filed: date(&text(&filed[i])?)?,
                report_date: if report.is_empty() {
                    None
                } else {
                    Some(date(&report)?)
                },
                form: text(&forms[i])?,
                primary_document: if doc.is_empty() { None } else { Some(doc) },
                is_xbrl: match xbrl[i].as_u64() {
                    Some(0) => false,
                    Some(1) => true,
                    _ => bail!("invalid isXBRL"),
                },
            })
        })
        .collect()
}
