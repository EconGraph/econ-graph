//! Declarative, bounded selection from BEA's current upstream table catalog.
//!
//! Historical dimension labels are intentionally not selection input: they are additive and
//! include tables the source has removed. Changing these rules establishes a new lifecycle scope.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::CrawlError;
use crate::reference::data_dir;
use crate::sources::bea::{BeaDataset, BeaTable, Frequency};

/// Runtime file under the crawler reference directory.
pub const BEA_RULES_FILE: &str = "catalog_selection/bea.toml";

/// A bounded collection of table-family rules, evaluated against the source catalog.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BeaSelection {
    /// Maximum selected tables before any table's series are requested.
    #[serde(skip_serializing)]
    pub max_tables: usize,
    /// Maximum expanded series before any are persisted or enqueued.
    #[serde(skip_serializing)]
    pub max_series: usize,
    /// Optional table names excluded even when a rule matches.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Each rule combines dataset, table prefix, optional title text and frequencies.
    pub rule: Vec<BeaRule>,
}

/// The dataset and table family a source publishes; titles and actual names stay upstream-owned.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BeaRule {
    pub dataset: String,
    pub prefix: String,
    #[serde(default)]
    pub title_contains: Option<String>,
    pub frequencies: Vec<String>,
}

impl BeaSelection {
    pub fn parse(text: &str) -> Result<Self, CrawlError> {
        let rules: Self = toml::from_str(text)
            .map_err(|e| CrawlError::Permanent(format!("BEA selection rules: {e}")))?;
        if rules.max_tables == 0 || rules.max_series == 0 || rules.rule.is_empty() {
            return Err(CrawlError::Permanent(
                "BEA selection needs nonzero limits and at least one rule".into(),
            ));
        }
        for r in &rules.rule {
            let regional = r.dataset == "Regional";
            if !(regional || r.dataset == "NIPA")
                || r.prefix.is_empty()
                || !r.prefix.bytes().all(|b| b.is_ascii_alphanumeric())
                || r.frequencies.is_empty()
                || r.frequencies
                    .iter()
                    .any(|f| Frequency::from_code(f).is_none())
                || (regional && r.frequencies != ["A"])
                || r.title_contains
                    .as_ref()
                    .is_some_and(|s| s.trim().is_empty())
            {
                return Err(CrawlError::Permanent(format!(
                    "BEA invalid selection rule for {} {}",
                    r.dataset, r.prefix
                )));
            }
        }
        Ok(rules)
    }

    /// Stable selection scope; changing operational limits preserves absence evidence.
    pub fn fingerprint(&self) -> String {
        let digest = Sha256::digest(serde_json::to_vec(self).expect("serializable selection"));
        let hash: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        format!("bea-rules-v1:{hash}")
    }

    /// Matches only names in the current complete catalog, never the accumulated label cache.
    pub fn select(
        &self,
        catalogs: &HashMap<BeaDataset, HashMap<String, String>>,
    ) -> Result<Vec<BeaTable>, CrawlError> {
        let excluded: HashSet<&str> = self.exclude.iter().map(String::as_str).collect();
        let mut selected: BTreeMap<(String, String), Vec<Frequency>> = BTreeMap::new();
        for rule in &self.rule {
            let dataset = if rule.dataset == "NIPA" {
                BeaDataset::Nipa
            } else {
                BeaDataset::Regional
            };
            let catalog = catalogs.get(&dataset).ok_or_else(|| {
                CrawlError::Transient(format!(
                    "BEA {}: current table catalog unavailable; title not yet known",
                    rule.dataset
                ))
            })?;
            for (name, title) in catalog {
                if !name.starts_with(&rule.prefix)
                    || excluded.contains(name.as_str())
                    || rule
                        .title_contains
                        .as_ref()
                        .is_some_and(|s| !title.to_lowercase().contains(&s.to_lowercase()))
                {
                    continue;
                }
                let frequencies = selected
                    .entry((rule.dataset.clone(), name.clone()))
                    .or_default();
                for code in &rule.frequencies {
                    let f = Frequency::from_code(code).expect("validated frequency");
                    if !frequencies.contains(&f) {
                        frequencies.push(f);
                    }
                }
                if selected.len() > self.max_tables {
                    return Err(volume_error("tables", selected.len(), self.max_tables));
                }
            }
        }
        // An empty result may mean a truncated catalog or a broken rule. Never certify it as a
        // complete disappearance; a later healthy refresh must establish what happened.
        if selected.is_empty() {
            return Err(CrawlError::Transient(
                "BEA selection matched no current tables".into(),
            ));
        }
        Ok(selected
            .into_iter()
            .map(|((dataset, table_name), frequencies)| BeaTable {
                dataset: if dataset == "NIPA" {
                    BeaDataset::Nipa
                } else {
                    BeaDataset::Regional
                },
                table_name,
                frequencies,
            })
            .collect())
    }

    /// Whether an already-stored canonical series belongs to these rules. Retained titles
    /// may positively establish a title predicate, but cannot invent current membership.
    pub fn includes_stored_series(
        &self,
        dataset: BeaDataset,
        table: &str,
        frequency: Frequency,
        retained_title: Option<&str>,
    ) -> bool {
        !self.exclude.iter().any(|excluded| excluded == table)
            && self.rule.iter().any(|rule| {
                rule.dataset == dataset.api_name()
                    && table.starts_with(&rule.prefix)
                    && rule.frequencies.iter().any(|f| f == frequency.code())
                    && rule.title_contains.as_ref().is_none_or(|needle| {
                        retained_title.is_some_and(|title| {
                            title.to_lowercase().contains(&needle.to_lowercase())
                        })
                    })
            })
    }

    pub fn check_series_count(&self, count: usize) -> Result<(), CrawlError> {
        if count > self.max_series {
            Err(volume_error("series", count, self.max_series))
        } else {
            Ok(())
        }
    }
}

fn volume_error(kind: &str, count: usize, max: usize) -> CrawlError {
    econ_graph_metrics::crawler::CRAWLER_CATALOG_METRICS
        .guard_rejections_total
        .with_label_values(&["BEA", "volume"])
        .inc();
    CrawlError::Permanent(format!(
        "BEA catalog volume guard: selected {count} {kind}, limit {max}; no series enqueued"
    ))
}

/// Loads the operator-controlled selection rules once per worker process.
pub fn bea_rules() -> Result<&'static BeaSelection, CrawlError> {
    static RULES: OnceLock<Result<BeaSelection, CrawlError>> = OnceLock::new();
    RULES
        .get_or_init(|| {
            let path = data_dir().join(BEA_RULES_FILE);
            let text = std::fs::read_to_string(&path)
                .map_err(|e| CrawlError::Permanent(format!("reading {}: {e}", path.display())))?;
            BeaSelection::parse(&text)
        })
        .as_ref()
        .map_err(Clone::clone)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rules() -> BeaSelection {
        BeaSelection::parse("max_tables=2\nmax_series=4\n[[rule]]\ndataset='Regional'\nprefix='SAGDP2'\nfrequencies=['A']").unwrap()
    }
    fn catalog(names: &[&str]) -> HashMap<BeaDataset, HashMap<String, String>> {
        [(
            BeaDataset::Regional,
            names
                .iter()
                .map(|name| (name.to_string(), "GDP".to_string()))
                .collect(),
        )]
        .into_iter()
        .collect()
    }
    #[test]
    fn rename_and_new_table_are_selected_without_config_changes() {
        let r = rules();
        assert_eq!(
            r.select(&catalog(&["SAGDP2N"])).unwrap()[0].table_name,
            "SAGDP2N"
        );
        let selected = r
            .select(&catalog(&["SAGDP2", "SAGDP2NEW", "SAINC1"]))
            .unwrap();
        assert_eq!(
            selected
                .iter()
                .map(|t| t.table_name.as_str())
                .collect::<Vec<_>>(),
            ["SAGDP2", "SAGDP2NEW"]
        );
    }
    #[test]
    fn bounds_reject_instead_of_truncating_and_accept_exact_limit() {
        let r = rules();
        assert_eq!(r.select(&catalog(&["SAGDP2", "SAGDP2N"])).unwrap().len(), 2);
        assert!(r
            .select(&catalog(&["SAGDP2", "SAGDP2N", "SAGDP2X"]))
            .unwrap_err()
            .to_string()
            .contains("volume guard"));
        assert!(r.check_series_count(4).is_ok());
        assert!(r.check_series_count(5).is_err());
    }
    #[test]
    fn exclusions_title_filters_and_overlapping_rules_are_deterministic() {
        let mut r = rules();
        r.exclude = vec!["SAGDP2N".into()];
        r.rule.push(r.rule[0].clone());
        r.rule[0].title_contains = Some("gdp".into());
        let selected = r.select(&catalog(&["SAGDP2N", "SAGDP2"])).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].frequencies, [Frequency::Annual]);
        let fingerprint = r.fingerprint();
        r.max_tables += 1;
        assert_eq!(fingerprint, r.fingerprint());
        r.max_series += 1;
        assert_eq!(fingerprint, r.fingerprint());
        r.rule[0].prefix.push('N');
        assert_ne!(fingerprint, r.fingerprint());
    }
    #[test]
    fn stored_scope_membership_requires_every_rule_predicate() {
        let mut r = rules();
        r.rule[0].title_contains = Some("GDP".into());
        assert!(r.includes_stored_series(
            BeaDataset::Regional,
            "SAGDP2N",
            Frequency::Annual,
            Some("Historic GDP by state")
        ));
        assert!(!r.includes_stored_series(
            BeaDataset::Regional,
            "SAGDP2N",
            Frequency::Annual,
            None
        ));
        assert!(!r.includes_stored_series(
            BeaDataset::Regional,
            "SAGDP2N",
            Frequency::Annual,
            Some("Personal income")
        ));
        assert!(!r.includes_stored_series(
            BeaDataset::Nipa,
            "SAGDP2N",
            Frequency::Annual,
            Some("GDP")
        ));
        assert!(!r.includes_stored_series(
            BeaDataset::Regional,
            "SAINC1",
            Frequency::Annual,
            Some("GDP")
        ));
        assert!(!r.includes_stored_series(
            BeaDataset::Regional,
            "SAGDP2N",
            Frequency::Quarterly,
            Some("GDP")
        ));
        r.exclude.push("SAGDP2N".into());
        assert!(!r.includes_stored_series(
            BeaDataset::Regional,
            "SAGDP2N",
            Frequency::Annual,
            Some("GDP")
        ));
    }

    #[test]
    fn invalid_config_and_missing_catalog_fail_closed() {
        for text in ["max_tables=0\nmax_series=1\nrule=[]", "max_tables=1\nmax_series=1\n[[rule]]\ndataset='Regional'\nprefix=''\nfrequencies=['A']"] {
            assert!(BeaSelection::parse(text).is_err());
        }
        assert!(rules().select(&HashMap::new()).is_err());
        assert!(rules().select(&catalog(&["SAINC1"])).is_err());
    }
}
