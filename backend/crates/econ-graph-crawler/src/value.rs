// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Observation values: parsing source text into exact decimals, and the size limits every
//! stored value must meet.
//!
//! `data_points.value` is unconstrained `NUMERIC`, so a value is stored exactly as the source
//! wrote it: every digit, and the source's own scale (`1.50` stays `1.50`). Adapters parse value
//! text with [`parse_observation_value`], never through a binary float, so no digit is lost or
//! invented on the way. The limits below only reject text no economic series produces (a
//! corrupt or hostile response), so one bad cell can't make the database store or the API serve
//! an enormous number.
//!
//! [`BigDecimal`] has no NaN or infinity, and its parser rejects `NaN`, `inf` and the like, so a
//! value that parses is finite.

use std::str::FromStr;

use bigdecimal::BigDecimal;

use crate::adapter::FetchedSeries;
use crate::error::CrawlError;

/// Longest value text accepted, in bytes (after trimming).
pub const MAX_VALUE_TEXT_LEN: usize = 128;

/// Most digits a value may have before the decimal point. The largest published aggregates
/// (world GDP in local-currency units) have about 20.
pub const MAX_INTEGER_DIGITS: i64 = 40;

/// Most significant digits a value may have after the decimal point (trailing zeros don't
/// count).
pub const MAX_FRACTION_DIGITS: i64 = 30;

/// Parses `text` (a plain decimal such as `-12.345` or `1.9E14`, surrounding whitespace
/// allowed) into the exact decimal it writes, checked against [`check_observation_value`].
/// The caller handles source-specific markers (missing-value symbols, thousands separators)
/// first.
pub fn parse_observation_value(text: &str) -> Result<BigDecimal, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("empty value".to_string());
    }
    if text.len() > MAX_VALUE_TEXT_LEN {
        return Err(format!(
            "value is {} characters long (at most {MAX_VALUE_TEXT_LEN})",
            text.len()
        ));
    }
    // BigDecimal's parser accepts a few forms that aren't plain decimals; only digits, one
    // sign, one point and one exponent marker are numbers here.
    if !text
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.' | b'e' | b'E'))
        || !text.bytes().any(|b| b.is_ascii_digit())
    {
        return Err(format!("{text:?} is not a decimal number"));
    }
    let value =
        BigDecimal::from_str(text).map_err(|e| format!("{text:?} is not a decimal number: {e}"))?;
    check_observation_value(&value)?;
    Ok(value)
}

/// Whether `value` is within the stored-value limits: at most [`MAX_INTEGER_DIGITS`] digits
/// before the point and [`MAX_FRACTION_DIGITS`] significant digits after it.
pub fn check_observation_value(value: &BigDecimal) -> Result<(), String> {
    let normalized = value.normalized();
    let (digits, scale) = normalized.as_bigint_and_exponent();
    if digits == 0.into() {
        return Ok(());
    }
    let significant = i64::try_from(normalized.digits()).unwrap_or(i64::MAX);
    // `digits * 10^-scale`: integer digits = significant - scale, fraction digits = scale.
    let integer_digits = significant.saturating_sub(scale);
    if integer_digits > MAX_INTEGER_DIGITS {
        return Err(format!(
            "value has {integer_digits} integer digits (at most {MAX_INTEGER_DIGITS})"
        ));
    }
    if scale > MAX_FRACTION_DIGITS {
        return Err(format!(
            "value has {scale} fractional digits (at most {MAX_FRACTION_DIGITS})"
        ));
    }
    Ok(())
}

/// Checks every value of `fetched` against [`check_observation_value`]; a series with any
/// value outside the limits is a [`CrawlError::Parse`] for `external_id`, and nothing of it is
/// written. Run on every fetched series before it is persisted, whichever adapter produced it.
pub fn check_fetched_values(external_id: &str, fetched: &FetchedSeries) -> Result<(), CrawlError> {
    for point in &fetched.points {
        if let Some(value) = &point.value {
            check_observation_value(value).map_err(|e| {
                CrawlError::Parse(format!("{external_id}: {} value: {e}", point.date))
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact(text: &str) -> String {
        parse_observation_value(text).unwrap().to_string()
    }

    #[test]
    fn keeps_every_digit_and_the_source_scale() {
        // World Bank NY.GDP.MKTP.PP.CD for WLD is about 1.9e14 with a fractional part.
        assert_eq!(exact("190358563845627.40625"), "190358563845627.40625");
        assert_eq!(exact("0.123456789012345678"), "0.123456789012345678");
        assert_eq!(exact("1.50"), "1.50");
        assert_eq!(exact(" -42 "), "-42");
        assert_eq!(
            parse_observation_value("1.9E14").unwrap(),
            BigDecimal::from(190_000_000_000_000i64)
        );
        // 17 significant digits: more than an f64 holds exactly.
        assert_eq!(exact("12345678901234567"), "12345678901234567");
    }

    #[test]
    fn rejects_non_numbers_and_non_finite_text() {
        for bad in [
            "", " ", "NaN", "nan", "inf", "-inf", "Infinity", "1,234", "0x10", "1e", "--1", ".",
            "1.2.3", "12abc",
        ] {
            assert!(parse_observation_value(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn enforces_size_limits() {
        let forty = "9".repeat(40);
        assert!(parse_observation_value(&forty).is_ok());
        assert!(parse_observation_value(&format!("{forty}9")).is_err());
        assert!(parse_observation_value(&format!("0.{}", "1".repeat(30))).is_ok());
        assert!(parse_observation_value(&format!("0.{}", "1".repeat(31))).is_err());
        // Trailing zeros are not significant.
        assert!(parse_observation_value(&format!("1.{}", "0".repeat(60))).is_ok());
        assert!(parse_observation_value("1e40").is_err());
        assert!(parse_observation_value("1e39").is_ok());
        assert!(parse_observation_value("1e-31").is_err());
        assert!(parse_observation_value("1e999999999").is_err());
        assert!(parse_observation_value("0e999999999").is_ok());
        assert!(parse_observation_value(&"1".repeat(MAX_VALUE_TEXT_LEN + 1)).is_err());
    }
}
