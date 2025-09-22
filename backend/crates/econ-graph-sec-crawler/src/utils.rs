use anyhow::Result;
use chrono::{Datelike, NaiveDate, Utc};
use std::str::FromStr;

/// **Date Utilities**
///
/// Utility functions for date parsing and manipulation in SEC EDGAR context.

/// Parse a date string in various SEC EDGAR formats
pub fn parse_sec_date(date_str: &str) -> Result<NaiveDate> {
    // Try common SEC date formats
    let formats = [
        "%Y-%m-%d", // 2023-12-31
        "%Y%m%d",   // 20231231
        "%m/%d/%Y", // 12/31/2023
        "%m-%d-%Y", // 12-31-2023
    ];

    for format in &formats {
        if let Ok(date) = NaiveDate::parse_from_str(date_str, format) {
            return Ok(date);
        }
    }

    Err(anyhow::anyhow!("Unable to parse date: {}", date_str))
}

/// Format a date for SEC EDGAR URLs
pub fn format_sec_date(date: &NaiveDate) -> String {
    date.format("%Y%m%d").to_string()
}

/// **CIK Utilities**
///
/// Utility functions for working with SEC Central Index Keys (CIK).

/// Pad a CIK to 10 digits with leading zeros
pub fn pad_cik(cik: &str) -> String {
    format!("{:0>10}", cik)
}

/// Remove leading zeros from a CIK
pub fn unpad_cik(cik: &str) -> String {
    cik.trim_start_matches('0').to_string()
}

/// Validate CIK format
pub fn is_valid_cik(cik: &str) -> bool {
    cik.chars().all(|c| c.is_ascii_digit()) && cik.len() <= 10
}

/// **Accession Number Utilities**
///
/// Utility functions for working with SEC accession numbers.

/// Parse accession number components
pub fn parse_accession_number(accession: &str) -> Result<AccessionComponents> {
    // Format: 0000320193-23-000006
    let parts: Vec<&str> = accession.split('-').collect();

    if parts.len() != 3 {
        return Err(anyhow::anyhow!(
            "Invalid accession number format: {}",
            accession
        ));
    }

    let cik = parts[0].to_string();
    let year = parts[1].to_string();
    let sequence = parts[2].to_string();

    Ok(AccessionComponents {
        cik,
        year,
        sequence,
    })
}

/// Construct accession number from components
pub fn build_accession_number(cik: &str, year: &str, sequence: &str) -> String {
    format!("{}-{}-{}", pad_cik(cik), year, sequence)
}

/// **URL Construction Utilities**
///
/// Utility functions for constructing SEC EDGAR URLs.

/// Build XBRL file URL from accession number
pub fn build_xbrl_url(accession: &str) -> Result<String> {
    let components = parse_accession_number(accession)?;
    let cik_unpadded = unpad_cik(&components.cik);
    let accession_clean = accession.replace("-", "");

    Ok(format!(
        "https://www.sec.gov/Archives/edgar/data/{}/{}/{}.xbrl",
        cik_unpadded, accession_clean, accession
    ))
}

/// Build company submissions URL from CIK
pub fn build_submissions_url(cik: &str) -> String {
    format!("https://data.sec.gov/submissions/CIK{}.json", pad_cik(cik))
}

/// Build company facts URL from CIK
pub fn build_company_facts_url(cik: &str) -> String {
    format!(
        "https://data.sec.gov/api/xbrl/companyfacts/CIK{}.json",
        pad_cik(cik)
    )
}

/// **File Size Utilities**
///
/// Utility functions for file size formatting and validation.

/// Format file size in human-readable format
pub fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: u64 = 1024;

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= THRESHOLD as f64 && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD as f64;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Parse file size from string (e.g., "2.5MB", "1024KB")
pub fn parse_file_size(size_str: &str) -> Result<u64> {
    let size_str = size_str.trim().to_uppercase();

    if size_str.ends_with("B") {
        let number_part = &size_str[..size_str.len() - 1];
        let number: f64 = number_part
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid file size format: {}", size_str))?;

        if size_str.ends_with("TB") {
            Ok((number * 1024.0 * 1024.0 * 1024.0 * 1024.0) as u64)
        } else if size_str.ends_with("GB") {
            Ok((number * 1024.0 * 1024.0 * 1024.0) as u64)
        } else if size_str.ends_with("MB") {
            Ok((number * 1024.0 * 1024.0) as u64)
        } else if size_str.ends_with("KB") {
            Ok((number * 1024.0) as u64)
        } else {
            Ok(number as u64)
        }
    } else {
        // Assume bytes if no unit specified
        Ok(size_str
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid file size format: {}", size_str))?)
    }
}

/// **Retry Utilities**
///
/// Utility functions for retry logic and backoff.

/// Calculate exponential backoff delay
pub fn calculate_backoff_delay(attempt: u32, base_delay: u64) -> u64 {
    let delay = base_delay * 2_u64.pow(attempt);
    // Cap at 5 minutes
    delay.min(300)
}

/// **Validation Utilities**
///
/// Utility functions for data validation.

/// Validate SEC form type
pub fn is_valid_form_type(form_type: &str) -> bool {
    const VALID_FORMS: &[&str] = &[
        "10-K", "10-Q", "8-K", "20-F", "6-K", "11-K", "DEF 14A", "PRE 14A", "4", "3", "5", "144",
        "S-1", "S-3", "S-4", "S-8", "F-1", "F-3", "F-4", "POS AM", "POS EX", "POS PRE", "POS UPD",
        "POS ASR", "POS COR",
    ];

    VALID_FORMS.contains(&form_type)
}

/// Validate fiscal year
pub fn is_valid_fiscal_year(year: i32) -> bool {
    year >= 1900 && year <= 2100
}

/// Validate fiscal quarter
pub fn is_valid_fiscal_quarter(quarter: i32) -> bool {
    quarter >= 1 && quarter <= 4
}

/// Get fiscal quarter from date
pub fn get_fiscal_quarter(date: &NaiveDate) -> i32 {
    match date.month() {
        1..=3 => 1,
        4..=6 => 2,
        7..=9 => 3,
        10..=12 => 4,
        _ => 1, // Should never happen
    }
}

/// **Data Structures**

/// Components of a SEC accession number
#[derive(Debug, Clone, PartialEq)]
pub struct AccessionComponents {
    pub cik: String,
    pub year: String,
    pub sequence: String,
}

/// **Error Types**

#[derive(Debug, thiserror::Error)]
pub enum UtilsError {
    #[error("Invalid date format: {0}")]
    InvalidDateFormat(String),

    #[error("Invalid CIK format: {0}")]
    InvalidCikFormat(String),

    #[error("Invalid accession number format: {0}")]
    InvalidAccessionFormat(String),

    #[error("Invalid file size format: {0}")]
    InvalidFileSizeFormat(String),

    #[error("Invalid form type: {0}")]
    InvalidFormType(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sec_date() {
        assert_eq!(
            parse_sec_date("2023-12-31").unwrap(),
            NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()
        );
        assert_eq!(
            parse_sec_date("20231231").unwrap(),
            NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()
        );
        assert_eq!(
            parse_sec_date("12/31/2023").unwrap(),
            NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()
        );
    }

    #[test]
    fn test_pad_cik() {
        assert_eq!(pad_cik("320193"), "0000320193");
        assert_eq!(pad_cik("0000320193"), "0000320193");
    }

    #[test]
    fn test_unpad_cik() {
        assert_eq!(unpad_cik("0000320193"), "320193");
        assert_eq!(unpad_cik("320193"), "320193");
    }

    #[test]
    fn test_is_valid_cik() {
        assert!(is_valid_cik("320193"));
        assert!(is_valid_cik("0000320193"));
        assert!(!is_valid_cik("abc123"));
        assert!(!is_valid_cik("12345678901")); // Too long
    }

    #[test]
    fn test_parse_accession_number() {
        let components = parse_accession_number("0000320193-23-000006").unwrap();
        assert_eq!(components.cik, "0000320193");
        assert_eq!(components.year, "23");
        assert_eq!(components.sequence, "000006");
    }

    #[test]
    fn test_build_accession_number() {
        assert_eq!(
            build_accession_number("320193", "23", "000006"),
            "0000320193-23-000006"
        );
    }

    #[test]
    fn test_build_xbrl_url() {
        let url = build_xbrl_url("0000320193-23-000006").unwrap();
        assert_eq!(
            url,
            "https://www.sec.gov/Archives/edgar/data/320193/000032019323000006/0000320193-23-000006.xbrl"
        );
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(0), "0 B");
        assert_eq!(format_file_size(1024), "1.0 KB");
        assert_eq!(format_file_size(1048576), "1.0 MB");
        assert_eq!(format_file_size(1073741824), "1.0 GB");
    }

    #[test]
    fn test_parse_file_size() {
        assert_eq!(parse_file_size("1024").unwrap(), 1024);
        assert_eq!(parse_file_size("1KB").unwrap(), 1024);
        assert_eq!(parse_file_size("1MB").unwrap(), 1048576);
        assert_eq!(parse_file_size("1GB").unwrap(), 1073741824);
    }

    #[test]
    fn test_calculate_backoff_delay() {
        assert_eq!(calculate_backoff_delay(0, 1), 1);
        assert_eq!(calculate_backoff_delay(1, 1), 2);
        assert_eq!(calculate_backoff_delay(2, 1), 4);
        assert_eq!(calculate_backoff_delay(10, 1), 300); // Capped at 5 minutes
    }

    #[test]
    fn test_is_valid_form_type() {
        assert!(is_valid_form_type("10-K"));
        assert!(is_valid_form_type("10-Q"));
        assert!(is_valid_form_type("8-K"));
        assert!(!is_valid_form_type("INVALID"));
    }

    #[test]
    fn test_is_valid_fiscal_year() {
        assert!(is_valid_fiscal_year(2023));
        assert!(is_valid_fiscal_year(1900));
        assert!(is_valid_fiscal_year(2100));
        assert!(!is_valid_fiscal_year(1899));
        assert!(!is_valid_fiscal_year(2101));
    }

    #[test]
    fn test_is_valid_fiscal_quarter() {
        assert!(is_valid_fiscal_quarter(1));
        assert!(is_valid_fiscal_quarter(2));
        assert!(is_valid_fiscal_quarter(3));
        assert!(is_valid_fiscal_quarter(4));
        assert!(!is_valid_fiscal_quarter(0));
        assert!(!is_valid_fiscal_quarter(5));
    }
}
