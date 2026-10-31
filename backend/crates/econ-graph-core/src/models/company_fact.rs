//! Source-preserving SEC companyfacts evidence. See ADR 0001 for identity and revision rules.
//!
//! The API names a taxonomy *family*, not a filing's XBRL namespace. Do not equate
//! these subjects with raw-parser facts without an explicit, verified mapping.

use std::collections::BTreeMap;

use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{FactSubject, FilingRef, ResearchPeriod, SubjectRef};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanyFactSubject {
    pub cik: String,
    /// Source family, such as `us-gaap`, not an inferred namespace URI/version.
    pub taxonomy: String,
    pub concept: String,
    pub period: ResearchPeriod,
    /// Lossless SEC unit token, such as `USD`, `shares`, or `USD/shares`.
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanyFact {
    /// Deterministic identity of the economic subject and filing accession.
    pub id: Uuid,
    pub subject: CompanyFactSubject,
    pub accession: String,
    pub filed: NaiveDate,
    /// Exact source value. JSON represents it as a decimal string, never a float.
    pub value: BigDecimal,
    pub form: String,
    /// Filing fiscal focus; these fields do not identify the fact's economic period.
    pub fiscal_year: Option<i32>,
    pub fiscal_period: Option<String>,
    pub frame: Option<String>,
    pub label: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid company fact: {0}")]
pub struct CompanyFactError(pub &'static str);

fn valid_token(value: &str) -> bool {
    !value.is_empty() && value.trim() == value && !value.chars().any(char::is_control)
}

// Source tokens are percent-encoded so arbitrary units remain distinguishable URNs.
fn urn_token(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(encoded, "%{byte:02X}").expect("writing to a String cannot fail");
        }
    }
    encoded
}

fn decode_urn_token(value: &str) -> Result<String, CompanyFactError> {
    let mut decoded = Vec::new();
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = bytes.next().and_then(|b| char::from(b).to_digit(16));
            let low = bytes.next().and_then(|b| char::from(b).to_digit(16));
            match (high, low) {
                (Some(high), Some(low)) => decoded.push((high * 16 + low) as u8),
                _ => return Err(CompanyFactError("malformed companyfacts source URN")),
            }
        } else {
            decoded.push(byte);
        }
    }
    String::from_utf8(decoded).map_err(|_| CompanyFactError("source URN is not UTF-8"))
}

impl CompanyFactSubject {
    pub fn validate(&self) -> Result<(), CompanyFactError> {
        SubjectRef::Company {
            cik: self.cik.clone(),
        }
        .validate()
        .map_err(|_| CompanyFactError("CIK must contain ten digits and be nonzero"))?;
        if !valid_token(&self.taxonomy) || !valid_token(&self.concept) || !valid_token(&self.unit) {
            return Err(CompanyFactError(
                "taxonomy, concept and unit must be nonempty source tokens",
            ));
        }
        if matches!(self.period, ResearchPeriod::Duration { start, end } if start > end) {
            return Err(CompanyFactError("duration starts after its end"));
        }
        Ok(())
    }

    pub fn canonical_key(&self) -> Result<String, CompanyFactError> {
        self.validate()?;
        Ok(format!(
            "sec-companyfacts:subject:v1:{}",
            serde_json::to_string(self).expect("subject serialization cannot fail")
        ))
    }

    /// Explicit logical source namespaces allow the shared note anchor to preserve
    /// companyfacts identity. These URNs never claim to be original XBRL namespaces.
    /// Empty dimensions mean whole-entity companyfacts evidence, not all raw contexts.
    pub fn reference(&self) -> Result<FactSubject, CompanyFactError> {
        self.validate()?;
        Ok(FactSubject {
            cik: self.cik.clone(),
            concept_namespace: format!(
                "urn:econgraph:sec-companyfacts:taxonomy:{}",
                urn_token(&self.taxonomy)
            ),
            concept: self.concept.clone(),
            period: self.period.clone(),
            unit: format!(
                "urn:econgraph:sec-companyfacts:unit:{}",
                urn_token(&self.unit)
            ),
            dimensions: BTreeMap::new(),
        })
    }

    /// Resolve only canonical companyfacts anchors. A raw-parser subject with an
    /// authoritative XBRL namespace or any dimension never aliases these facts.
    pub fn from_reference(reference: &FactSubject) -> Result<Self, CompanyFactError> {
        reference
            .validate()
            .map_err(|_| CompanyFactError("invalid research fact reference"))?;
        if !reference.dimensions.is_empty() {
            return Err(CompanyFactError(
                "companyfacts does not expose dimensioned contexts",
            ));
        }
        let taxonomy = reference
            .concept_namespace
            .strip_prefix("urn:econgraph:sec-companyfacts:taxonomy:")
            .ok_or(CompanyFactError("reference is not a companyfacts taxonomy"))?;
        let unit = reference
            .unit
            .strip_prefix("urn:econgraph:sec-companyfacts:unit:")
            .ok_or(CompanyFactError("reference is not a companyfacts unit"))?;
        let subject = Self {
            cik: reference.cik.clone(),
            taxonomy: decode_urn_token(taxonomy)?,
            concept: reference.concept.clone(),
            period: reference.period.clone(),
            unit: decode_urn_token(unit)?,
        };
        if subject.reference()? != *reference {
            return Err(CompanyFactError("companyfacts reference is not canonical"));
        }
        Ok(subject)
    }
}

impl CompanyFact {
    pub fn new(
        subject: CompanyFactSubject,
        accession: String,
        filed: NaiveDate,
        value: BigDecimal,
        form: String,
    ) -> Result<Self, CompanyFactError> {
        let key = revision_key(&subject, &accession)?;
        let fact = Self {
            id: Uuid::new_v5(&Uuid::NAMESPACE_URL, key.as_bytes()),
            subject,
            accession,
            filed,
            value,
            form,
            fiscal_year: None,
            fiscal_period: None,
            frame: None,
            label: None,
            description: None,
        };
        fact.validate()?;
        Ok(fact)
    }

    pub fn revision_key(&self) -> Result<String, CompanyFactError> {
        revision_key(&self.subject, &self.accession)
    }

    pub fn validate(&self) -> Result<(), CompanyFactError> {
        let key = self.revision_key()?;
        if self.id != Uuid::new_v5(&Uuid::NAMESPACE_URL, key.as_bytes()) {
            return Err(CompanyFactError(
                "fact ID does not match its revision identity",
            ));
        }
        if !valid_token(&self.form) {
            return Err(CompanyFactError("form must be a nonempty source token"));
        }
        Ok(())
    }

    pub fn reference(&self) -> Result<SubjectRef, CompanyFactError> {
        self.validate()?;
        Ok(SubjectRef::FiledFact {
            subject: self.subject.reference()?,
            filing: FilingRef {
                cik: self.subject.cik.clone(),
                accession: self.accession.clone(),
            },
        })
    }
}

fn revision_key(subject: &CompanyFactSubject, accession: &str) -> Result<String, CompanyFactError> {
    subject.validate()?;
    FilingRef {
        cik: subject.cik.clone(),
        accession: accession.to_owned(),
    }
    .validate()
    .map_err(|_| CompanyFactError("accession must use SEC's 10-2-6 digit form"))?;
    Ok(format!(
        "sec-companyfacts:revision:v1:{}",
        serde_json::to_string(&(subject, accession)).expect("revision serialization cannot fail")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn date(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
    }

    fn fact() -> CompanyFact {
        CompanyFact::new(
            CompanyFactSubject {
                cik: "0000320193".into(),
                taxonomy: "us-gaap".into(),
                concept: "Revenue".into(),
                period: ResearchPeriod::Duration {
                    start: date("2023-01-01"),
                    end: date("2023-12-31"),
                },
                unit: "USD".into(),
            },
            "0000320193-24-000001".into(),
            date("2024-02-01"),
            BigDecimal::from_str("123456789012345678901234567890.12345678901234567890").unwrap(),
            "10-K".into(),
        )
        .unwrap()
    }

    #[test]
    fn identity_preserves_economic_period_units_and_accession() {
        let original = fact();
        assert_eq!(original.id, fact().id);
        let mut comparative = original.clone();
        comparative.fiscal_year = Some(2024);
        comparative.fiscal_period = Some("FY".into());
        comparative.frame = Some("CY2023".into());
        assert_eq!(original.revision_key(), comparative.revision_key());
        for subject in [
            CompanyFactSubject {
                unit: "shares".into(),
                ..original.subject.clone()
            },
            CompanyFactSubject {
                taxonomy: "ifrs-full".into(),
                ..original.subject.clone()
            },
            CompanyFactSubject {
                period: ResearchPeriod::Instant {
                    date: date("2023-12-31"),
                },
                ..original.subject.clone()
            },
            CompanyFactSubject {
                period: ResearchPeriod::Duration {
                    start: date("2023-10-01"),
                    end: date("2023-12-31"),
                },
                ..original.subject.clone()
            },
        ] {
            assert_ne!(original.subject.canonical_key(), subject.canonical_key());
        }
        let amendment = CompanyFact::new(
            original.subject.clone(),
            "0000320193-24-000002".into(),
            date("2024-03-01"),
            original.value.clone(),
            "10-K/A".into(),
        )
        .unwrap();
        assert_ne!(original.id, amendment.id);
        assert_ne!(
            original.reference().unwrap(),
            amendment.reference().unwrap()
        );
    }

    #[test]
    fn decimal_json_roundtrip_is_exact_and_source_namespace_is_explicit() {
        let original = fact();
        let json = serde_json::to_value(&original).unwrap();
        assert!(json["value"].is_string());
        assert_eq!(original, serde_json::from_value(json).unwrap());
        let reference = original.subject.reference().unwrap();
        assert_eq!(
            reference.concept_namespace,
            "urn:econgraph:sec-companyfacts:taxonomy:us-gaap"
        );
        assert!(reference.dimensions.is_empty());
        reference.validate().unwrap();
        let mut per_share = original.subject;
        per_share.unit = "USD/shares".into();
        assert!(per_share
            .reference()
            .unwrap()
            .unit
            .ends_with("USD%2Fshares"));
        let mut literal_escape = per_share.clone();
        literal_escape.unit = "USD%2Fshares".into();
        assert_ne!(
            per_share.reference().unwrap().unit,
            literal_escape.reference().unwrap().unit
        );
        for taxonomy in ["custom/taxonomy", "custom%2Ftaxonomy", "custom:taxonomy"] {
            let source = CompanyFactSubject {
                taxonomy: taxonomy.into(),
                ..per_share.clone()
            };
            assert_eq!(
                source,
                CompanyFactSubject::from_reference(&source.reference().unwrap()).unwrap()
            );
        }
    }

    #[test]
    fn invalid_identity_is_rejected_and_agent_accession_is_allowed() {
        let mut value = fact();
        value.subject.cik = "320193".into();
        assert!(value.validate().is_err());
        let mut value = fact();
        value.id = Uuid::new_v4();
        assert!(value.validate().is_err());
        let original = fact();
        assert!(CompanyFact::new(
            original.subject,
            "0000950170-24-000001".into(),
            original.filed,
            original.value,
            original.form
        )
        .is_ok());
    }

    #[test]
    fn reference_resolution_rejects_aliases_and_dimensioned_parser_facts() {
        let mut subject = fact().subject;
        subject.unit = "USD/shares".into();
        let reference = subject.reference().unwrap();
        assert_eq!(
            CompanyFactSubject::from_reference(&reference).unwrap(),
            subject
        );
        let mut noncanonical = reference.clone();
        noncanonical.unit = noncanonical.unit.replace("%2F", "%2f");
        assert!(CompanyFactSubject::from_reference(&noncanonical).is_err());
        let mut parser = reference.clone();
        parser.concept_namespace = "http://fasb.org/us-gaap/2024".into();
        assert!(CompanyFactSubject::from_reference(&parser).is_err());
        let mut dimensional = reference.clone();
        dimensional
            .dimensions
            .insert("axis".into(), "member".into());
        assert!(CompanyFactSubject::from_reference(&dimensional).is_err());
        let mut broken = reference;
        broken.unit.push('%');
        assert!(CompanyFactSubject::from_reference(&broken).is_err());
    }
}
