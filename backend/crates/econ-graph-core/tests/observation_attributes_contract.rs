//! Attribute contracts remain storage-neutral and preserve historical unknowns.
use chrono::{NaiveDate, Utc};
use econ_graph_core::{DataPoint, Observation};
use serde_json::json;
use uuid::Uuid;

#[test]
fn observation_conversion_retains_null_empty_and_unknown_provider_extensions() {
    for attributes in [
        None,
        Some(json!({})),
        Some(json!({
            "source":"BLS", "footnotes":[{"code":"P", "text":"preliminary\n雪", "future":{"raw":null}}],
            "status_flags":["P"], "preliminary":true, "estimated":null,
            "raw_value":"12345678901234567890.000000000000000001", "future":[]
        })),
    ] {
        let point = DataPoint {
            id: Uuid::from_u128(1),
            series_id: Uuid::from_u128(2),
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            value: None,
            revision_date: NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(),
            is_original_release: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            attributes: attributes.clone(),
        };
        let observation: Observation = point.into();
        assert_eq!(observation.attributes, attributes);
        let wire = serde_json::to_string(&observation).unwrap();
        let decoded: Observation = serde_json::from_str(&wire).unwrap();
        assert_eq!(decoded.attributes, attributes);
        let point: DataPoint = decoded.into();
        assert_eq!(point.attributes, attributes);
    }
}
