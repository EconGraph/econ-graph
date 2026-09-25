//! Wire format of the EDGAR submissions API (`GET {data}/submissions/CIK##########.json`).
//!
//! The real response nests recent filings column-wise under `filings.recent` (one array per
//! field). [`crate::models::CompanySubmissionsResponse`] does not match that shape, so the
//! crawler decodes into [`EdgarSubmissions`] and converts each row to the crate's
//! [`FilingInfo`] (every vector holding exactly one element, which is how the crawler reads it).

use serde::Deserialize;

use crate::models::FilingInfo;

/// Top level of the submissions document (fields the crawler uses).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgarSubmissions {
    /// CIK as sent by EDGAR (a string, e.g. `"320193"` or `"0000320193"`; numbers accepted too).
    #[serde(default)]
    pub cik: serde_json::Value,
    /// Company name.
    pub name: String,
    #[serde(default)]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub sic: Option<String>,
    #[serde(default)]
    pub sic_description: Option<String>,
    #[serde(default)]
    pub tickers: Vec<String>,
    #[serde(default)]
    pub filings: EdgarFilings,
}

/// `filings` object.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EdgarFilings {
    /// Most recent filings (at least one year or 1000 filings), column-wise.
    #[serde(default)]
    pub recent: EdgarRecentFilings,
}

/// Column-wise recent filings: element `i` of every vector describes filing `i`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EdgarRecentFilings {
    pub accession_number: Vec<String>,
    pub filing_date: Vec<String>,
    pub report_date: Vec<String>,
    pub acceptance_date_time: Vec<String>,
    pub act: Vec<String>,
    pub form: Vec<String>,
    pub file_number: Vec<String>,
    pub film_number: Vec<String>,
    pub items: Vec<String>,
    pub size: Vec<u64>,
    #[serde(rename = "isXBRL")]
    pub is_xbrl: Vec<u32>,
    #[serde(rename = "isInlineXBRL")]
    pub is_inline_xbrl: Vec<u32>,
    pub primary_document: Vec<String>,
    pub primary_doc_description: Vec<String>,
}

impl EdgarRecentFilings {
    /// One [`FilingInfo`] per filing. Rows without an accession number are dropped; missing
    /// columns become empty strings / zeros.
    pub fn to_filing_infos(&self) -> Vec<FilingInfo> {
        fn s(v: &[String], i: usize) -> Vec<String> {
            vec![v.get(i).cloned().unwrap_or_default()]
        }
        fn n<T: Copy + Default>(v: &[T], i: usize) -> Vec<T> {
            vec![v.get(i).copied().unwrap_or_default()]
        }
        (0..self.accession_number.len())
            .filter(|&i| !self.accession_number[i].is_empty())
            .map(|i| FilingInfo {
                accession_number: s(&self.accession_number, i),
                filing_date: s(&self.filing_date, i),
                report_date: s(&self.report_date, i),
                acceptance_date_time: s(&self.acceptance_date_time, i),
                act: s(&self.act, i),
                form: s(&self.form, i),
                file_number: s(&self.file_number, i),
                film_number: s(&self.film_number, i),
                items: s(&self.items, i),
                size: n(&self.size, i),
                is_xbrl: n(&self.is_xbrl, i),
                is_inline_xbrl: n(&self.is_inline_xbrl, i),
                primary_document: s(&self.primary_document, i),
                primary_doc_description: s(&self.primary_doc_description, i),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_edgar_shape_and_splits_rows() {
        let raw = include_str!("../test_data/sec_mock/submissions_CIK0009999901.json");
        let subs: EdgarSubmissions = serde_json::from_str(raw).unwrap();
        assert_eq!(subs.name, "Testco Holdings Inc.");
        let rows = subs.filings.recent.to_filing_infos();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].form, vec!["10-K".to_string()]);
        assert_eq!(rows[0].is_xbrl, vec![1]);
        assert_eq!(rows[2].form, vec!["8-K".to_string()]);
    }
}
