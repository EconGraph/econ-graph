use anyhow::{anyhow, Result};
use chrono::{NaiveDate, Utc};
use regex::Regex;
use std::collections::HashSet;
use std::str::FromStr;

/// Validation errors for data models
#[derive(Debug, thiserror::Error)]
#[allow(clippy::enum_variant_names)]
pub enum ValidationError {
    #[error("Invalid external ID format: {0}")]
    InvalidExternalId(String),
    #[error("Invalid frequency: {0}")]
    InvalidFrequency(String),
    #[error("Invalid seasonal adjustment: {0}")]
    InvalidSeasonalAdjustment(String),
    #[error("Invalid date range: start date {start} is after end date {end}")]
    InvalidDateRange { start: NaiveDate, end: NaiveDate },
    #[error(
        "Invalid string length: {field} must be between {min} and {max} characters, got {actual}"
    )]
    InvalidStringLength {
        field: String,
        min: usize,
        max: usize,
        actual: usize,
    },
    #[error("Invalid decimal value: {0}")]
    InvalidDecimalValue(String),
}

/// Validation trait for data models
pub trait Validate {
    fn validate(&self) -> Result<()>;
}

/// Validates external ID format
pub fn validate_external_id(external_id: &str) -> Result<()> {
    // External IDs should be alphanumeric with underscores and hyphens
    let re = Regex::new(r"^[A-Z0-9_][A-Z0-9_-]*$")?;

    if external_id.is_empty() {
        return Err(anyhow!(ValidationError::InvalidExternalId(
            "External ID cannot be empty".to_string()
        )));
    }

    if external_id.len() > 100 {
        return Err(anyhow!(ValidationError::InvalidExternalId(
            "External ID cannot exceed 100 characters".to_string()
        )));
    }

    if !re.is_match(external_id) {
        return Err(anyhow!(ValidationError::InvalidExternalId(
            format!("External ID '{}' must contain only uppercase letters, numbers, underscores, and hyphens", external_id)
        )));
    }

    Ok(())
}

/// Validates frequency values
pub fn validate_frequency(frequency: &str) -> Result<()> {
    let valid_frequencies = HashSet::from([
        "daily",
        "weekly",
        "biweekly",
        "monthly",
        "quarterly",
        "semiannual",
        "annual",
        "irregular",
        "other",
    ]);

    if !valid_frequencies.contains(frequency.to_lowercase().as_str()) {
        return Err(anyhow!(ValidationError::InvalidFrequency(format!(
            "Frequency '{}' is not valid. Must be one of: {:?}",
            frequency, valid_frequencies
        ))));
    }

    Ok(())
}

/// Validates seasonal adjustment values
pub fn validate_seasonal_adjustment(adjustment: &str) -> Result<()> {
    let valid_adjustments = HashSet::from([
        "seasonally_adjusted",
        "not_seasonally_adjusted",
        "seasonally_adjusted_annual_rate",
    ]);

    if !valid_adjustments.contains(adjustment) {
        return Err(anyhow!(ValidationError::InvalidSeasonalAdjustment(
            format!(
                "Seasonal adjustment '{}' is not valid. Must be one of: {:?}",
                adjustment, valid_adjustments
            )
        )));
    }

    Ok(())
}

/// Validates string length constraints
pub fn validate_string_length(field: &str, value: &str, min: usize, max: usize) -> Result<()> {
    let len = value.len();
    if len < min || len > max {
        return Err(anyhow!(ValidationError::InvalidStringLength {
            field: field.to_string(),
            min,
            max,
            actual: len,
        }));
    }
    Ok(())
}

/// Validates date ranges
pub fn validate_date_range(
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> Result<()> {
    if let (Some(start), Some(end)) = (start_date, end_date) {
        if start > end {
            return Err(anyhow!(ValidationError::InvalidDateRange { start, end }));
        }
    }
    Ok(())
}

/// Validates that dates are not in the future
pub fn validate_not_future_date(date: NaiveDate) -> Result<()> {
    let today = Utc::now().date_naive();
    if date > today {
        return Err(anyhow!("Date {} cannot be in the future", date));
    }
    Ok(())
}

/// Validates decimal values are reasonable
pub fn validate_decimal_value(value: &rust_decimal::Decimal) -> Result<()> {
    // Check for reasonable bounds (adjust as needed for your use case)
    let max_value = rust_decimal::Decimal::from_str("999999999999.99")?;
    let min_value = rust_decimal::Decimal::from_str("-999999999999.99")?;

    if *value > max_value || *value < min_value {
        return Err(anyhow!(ValidationError::InvalidDecimalValue(format!(
            "Decimal value {} is outside acceptable range",
            value
        ))));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn test_validate_external_id() {
        // Valid IDs
        assert!(validate_external_id("GDP_Q1_2024").is_ok());
        assert!(validate_external_id("UNEMPLOYMENT_RATE").is_ok());
        assert!(validate_external_id("CPI-ALL-ITEMS").is_ok());

        // Invalid IDs
        assert!(validate_external_id("").is_err());
        assert!(validate_external_id("invalid id").is_err()); // space
        assert!(validate_external_id("invalid-id!").is_err()); // special char
        assert!(validate_external_id("invalid_id@").is_err()); // special char
        assert!(validate_external_id("123invalid").is_err()); // starts with number
    }

    #[test]
    fn test_validate_frequency() {
        // Valid frequencies
        assert!(validate_frequency("daily").is_ok());
        assert!(validate_frequency("MONTHLY").is_ok());
        assert!(validate_frequency("quarterly").is_ok());

        // Invalid frequencies
        assert!(validate_frequency("hourly").is_err());
        assert!(validate_frequency("biannual").is_err());
    }

    #[test]
    fn test_validate_seasonal_adjustment() {
        // Valid adjustments
        assert!(validate_seasonal_adjustment("seasonally_adjusted").is_ok());
        assert!(validate_seasonal_adjustment("not_seasonally_adjusted").is_ok());

        // Invalid adjustments
        assert!(validate_seasonal_adjustment("adjusted").is_err());
        assert!(validate_seasonal_adjustment("seasonal").is_err());
    }

    #[test]
    fn test_validate_string_length() {
        // Valid lengths
        assert!(validate_string_length("title", "Valid Title", 1, 100).is_ok());

        // Invalid lengths
        assert!(validate_string_length("title", "", 1, 100).is_err()); // too short
        assert!(validate_string_length("title", &"a".repeat(101), 1, 100).is_err());
        // too long
    }

    #[test]
    fn test_validate_date_range() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
        let invalid_end = NaiveDate::from_ymd_opt(2023, 12, 31).unwrap();

        // Valid ranges
        assert!(validate_date_range(Some(start), Some(end)).is_ok());
        assert!(validate_date_range(None, Some(end)).is_ok());
        assert!(validate_date_range(Some(start), None).is_ok());

        // Invalid range
        assert!(validate_date_range(Some(start), Some(invalid_end)).is_err());
    }

    #[test]
    fn test_validate_decimal_value() {
        // Valid values
        assert!(validate_decimal_value(&Decimal::from_str("123.45").unwrap()).is_ok());
        assert!(validate_decimal_value(&Decimal::from_str("-123.45").unwrap()).is_ok());
        assert!(validate_decimal_value(&Decimal::from_str("0").unwrap()).is_ok());

        // Invalid values (outside reasonable bounds)
        assert!(validate_decimal_value(&Decimal::from_str("1000000000000.00").unwrap()).is_err());
        assert!(validate_decimal_value(&Decimal::from_str("-1000000000000.00").unwrap()).is_err());
    }
}
