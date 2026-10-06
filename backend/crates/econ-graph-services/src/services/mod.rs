pub mod collaboration_service;
pub mod cross_section_service;
pub mod global_analysis_service;
pub mod queue_service;
pub mod search_service;
pub mod series_service;

// #[cfg(test)]
// mod __tests__;

pub use collaboration_service::*;

/// Use category patterns for recognized frequencies and exact equality for custom labels.
fn frequency_filter(raw: &str) -> (Option<Vec<String>>, Option<String>) {
    use econ_graph_core::models::SeriesFrequency;
    let classified = SeriesFrequency::classify_raw(raw);
    let frequency = if classified == SeriesFrequency::Irregular {
        SeriesFrequency::from(raw.to_owned())
    } else {
        classified
    };
    let patterns = frequency.sql_like_patterns();
    if patterns.is_empty() {
        (None, Some(raw.to_owned()))
    } else {
        (Some(patterns), None)
    }
}

#[cfg(test)]
mod frequency_filter_tests {
    use super::frequency_filter;

    #[test]
    fn accepts_source_labels_and_canonical_aliases() {
        for raw in ["Weekly", "Weekly, Ending Friday", "w"] {
            assert_eq!(frequency_filter(raw), (Some(vec!["weekly%".into()]), None));
        }
    }

    #[test]
    fn preserves_custom_labels_for_exact_equality() {
        for raw in ["Irregular", "Event-based", "custom_%\\label"] {
            assert_eq!(frequency_filter(raw), (None, Some(raw.to_owned())));
        }
    }
}
