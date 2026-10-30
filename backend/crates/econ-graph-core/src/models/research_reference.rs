//! Durable, storage-independent research targets shared by company and workspace consumers.
//!
//! References never imply authorization. Resolve them under the request's access policy;
//! unavailable targets leave the app-owned note and its original reference intact.

use std::collections::BTreeMap;

use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ChartAnnotation, FinancialAnnotation};

/// An economic period. An instant and a one-day duration remain distinct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResearchPeriod {
    Instant { date: NaiveDate },
    Duration { start: NaiveDate, end: NaiveDate },
}

/// Economic subject, independent of which filing reported its value.
/// Namespace URIs and unit measures are expanded names, never document-local prefixes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactSubject {
    pub cik: String,
    pub concept_namespace: String,
    pub concept: String,
    pub period: ResearchPeriod,
    pub unit: String,
    /// Expanded axis names to expanded members or canonical typed-member content.
    /// BTreeMap gives deterministic order regardless of source context ordering.
    #[serde(deserialize_with = "unique_dimensions")]
    pub dimensions: BTreeMap<String, String>,
}

fn unique_dimensions<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Dimensions;
    impl<'de> serde::de::Visitor<'de> for Dimensions {
        type Value = BTreeMap<String, String>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a map with unique dimension axes")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut dimensions = BTreeMap::new();
            while let Some((axis, member)) = map.next_entry::<String, String>()? {
                if dimensions.insert(axis, member).is_some() {
                    return Err(serde::de::Error::custom("duplicate dimension axis"));
                }
            }
            Ok(dimensions)
        }
    }
    deserializer.deserialize_map(Dimensions)
}

/// SEC accession is the immutable filing revision. An amendment has its own accession.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilingRef {
    pub cik: String,
    pub accession: String,
}

/// Explicit subject identities. Legacy rows keep their existing IDs without deriving new ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SubjectRef {
    Series {
        id: Uuid,
    },
    Company {
        cik: String,
    },
    Filing {
        filing: FilingRef,
    },
    Statement {
        filing: FilingRef,
        statement: String,
    },
    Fact {
        subject: FactSubject,
    },
    FiledFact {
        subject: FactSubject,
        filing: FilingRef,
    },
    Chart {
        id: Uuid,
    },
    /// An existing financial note stays pinned even if its row cannot be resolved.
    LegacyFinancial {
        statement_id: Uuid,
        line_item_id: Option<Uuid>,
    },
}

/// V1 wire envelope. The enum rejects unsupported versions during deserialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "version", deny_unknown_fields)]
pub enum AnnotationAnchor {
    #[serde(rename = "1")]
    V1 {
        target: SubjectRef,
        /// Legacy series annotations may also belong to a chart; preserve both identities.
        chart_id: Option<Uuid>,
        observation_date: Option<NaiveDate>,
        observation_value: Option<BigDecimal>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid research reference: {0}")]
pub struct ReferenceError(pub &'static str);

fn nonempty(value: &str) -> bool {
    !value.is_empty() && value.trim() == value && !value.chars().any(char::is_control)
}

fn validate_cik(cik: &str) -> Result<(), ReferenceError> {
    if cik.len() != 10 || !cik.bytes().all(|b| b.is_ascii_digit()) || cik == "0000000000" {
        return Err(ReferenceError("CIK must contain ten digits and be nonzero"));
    }
    Ok(())
}

impl FilingRef {
    pub fn validate(&self) -> Result<(), ReferenceError> {
        validate_cik(&self.cik)?;
        let parts: Vec<_> = self.accession.split('-').collect();
        if parts.len() != 3
            || parts[0].len() != 10
            || parts[1].len() != 2
            || parts[2].len() != 6
            || !parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(ReferenceError("accession must use SEC's 10-2-6 digit form"));
        }
        // The accession's leading CIK can identify the filing agent, not the issuer.
        Ok(())
    }
}

impl FactSubject {
    pub fn validate(&self) -> Result<(), ReferenceError> {
        validate_cik(&self.cik)?;
        if !nonempty(&self.concept_namespace)
            || !nonempty(&self.concept)
            || !nonempty(&self.unit)
            || self
                .dimensions
                .iter()
                .any(|(axis, member)| !nonempty(axis) || !nonempty(member))
        {
            return Err(ReferenceError(
                "concept, unit and dimensions require canonical nonempty names",
            ));
        }
        if matches!(self.period, ResearchPeriod::Duration { start, end } if start > end) {
            return Err(ReferenceError("duration starts after its end"));
        }
        Ok(())
    }
}

impl SubjectRef {
    pub fn validate(&self) -> Result<(), ReferenceError> {
        match self {
            Self::Series { id } | Self::Chart { id } if id.is_nil() => {
                Err(ReferenceError("nil target ID"))
            }
            Self::Company { cik } => validate_cik(cik),
            Self::Filing { filing } => filing.validate(),
            Self::Statement { filing, statement } => {
                filing.validate()?;
                if !nonempty(statement) {
                    return Err(ReferenceError("empty statement key"));
                }
                Ok(())
            }
            Self::Fact { subject } => subject.validate(),
            Self::FiledFact { subject, filing } => {
                subject.validate()?;
                filing.validate()?;
                if subject.cik != filing.cik {
                    return Err(ReferenceError("fact and filing issuers differ"));
                }
                Ok(())
            }
            Self::LegacyFinancial {
                statement_id,
                line_item_id,
            } if statement_id.is_nil() || line_item_id.is_some_and(|id| id.is_nil()) => {
                Err(ReferenceError("nil financial target ID"))
            }
            _ => Ok(()),
        }
    }

    /// Versioned canonical key, never a replacement for an existing database UUID.
    /// Structured JSON avoids delimiter collisions; sorted dimensions avoid order dependence.
    pub fn canonical_key(&self) -> Result<String, ReferenceError> {
        self.validate()?;
        Ok(format!(
            "research:v1:{}",
            serde_json::to_string(self).expect("reference serialization is infallible")
        ))
    }
}

impl AnnotationAnchor {
    pub fn validate(&self) -> Result<(), ReferenceError> {
        match self {
            Self::V1 {
                target, chart_id, ..
            } => {
                target.validate()?;
                if chart_id.is_some_and(|id| id.is_nil()) {
                    return Err(ReferenceError("nil chart ID"));
                }
                Ok(())
            }
        }
    }

    /// Deserialize and validate at trust boundaries, rather than accepting raw wire types.
    pub fn from_json(input: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let anchor: Self = serde_json::from_str(input)?;
        anchor.validate()?;
        Ok(anchor)
    }
}

impl TryFrom<&ChartAnnotation> for AnnotationAnchor {
    type Error = ReferenceError;
    fn try_from(note: &ChartAnnotation) -> Result<Self, Self::Error> {
        let target = match (&note.series_id, note.chart_id) {
            (Some(id), _) => SubjectRef::Series {
                id: Uuid::parse_str(id)
                    .map_err(|_| ReferenceError("legacy series ID is not a UUID"))?,
            },
            (None, Some(id)) => SubjectRef::Chart { id },
            (None, None) => return Err(ReferenceError("annotation has no target")),
        };
        let anchor = Self::V1 {
            target,
            chart_id: note.chart_id,
            observation_date: Some(note.annotation_date),
            observation_value: note.annotation_value.clone(),
        };
        anchor.validate()?;
        Ok(anchor)
    }
}

impl TryFrom<&FinancialAnnotation> for AnnotationAnchor {
    type Error = ReferenceError;
    fn try_from(note: &FinancialAnnotation) -> Result<Self, Self::Error> {
        let anchor = Self::V1 {
            target: SubjectRef::LegacyFinancial {
                statement_id: note.statement_id,
                line_item_id: note.line_item_id,
            },
            chart_id: None,
            observation_date: None,
            observation_value: None,
        };
        anchor.validate()?;
        Ok(anchor)
    }
}

/// Internal resolution state. Privacy-sensitive APIs map Missing/Tombstoned/Inaccessible
/// to the same public not-found response; an upstream outage fails closed as Unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnchorResolution<T> {
    Found(T),
    Missing,
    Tombstoned,
    Inaccessible,
    Unavailable,
}

impl<T> AnchorResolution<T> {
    pub fn public_status(&self) -> &'static str {
        match self {
            Self::Found(_) => "found",
            Self::Missing | Self::Tombstoned | Self::Inaccessible => "not_found",
            Self::Unavailable => "unavailable",
        }
    }
}
