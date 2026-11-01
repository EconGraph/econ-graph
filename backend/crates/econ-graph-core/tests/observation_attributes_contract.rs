//! Attribute contracts remain storage-neutral and preserve historical unknowns.
use chrono::{NaiveDate, Utc};
use econ_graph_core::{DataPoint, Observation, ObservationAttributes, ObservationFootnote};
use serde_json::json;
use uuid::Uuid;

#[test]
fn public_footnote_helper_preserves_absent_null_and_empty_fields() {
    for raw in [
        json!({}),
        json!({"code":"P"}),
        json!({"text":"preliminary"}),
        json!({"code":null}),
        json!({"text":null}),
        json!({"code":null,"text":null}),
        json!({"code":"","text":""}),
    ] {
        let footnote: ObservationFootnote = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(footnote.code(), raw.get("code").and_then(|v| v.as_str()));
        assert_eq!(footnote.text(), raw.get("text").and_then(|v| v.as_str()));
        let wire = serde_json::to_string(&footnote).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&wire).unwrap(),
            raw
        );
        let decoded: ObservationFootnote = serde_json::from_str(&wire).unwrap();
        assert_eq!(decoded, footnote);
    }
}

#[test]
fn public_attribute_helper_preserves_unknown_footnote_shapes_and_exact_numbers() {
    let raw = r#"{"source":"BLS","status_flags":null,"preliminary":null,
        "estimated":null,"raw_value":"1.0","footnotes":[
            {"code":{"future":true},"text":["unknown shape"],
             "extension":{"amount":123456789012345678901234567890.12345678901234567890123456789,
                          "exponent":1e1000,"items":[null,{"code":null}]}},
            {"code":null},{}
        ]}"#;
    let expected: serde_json::Value = serde_json::from_str(raw).unwrap();
    let attributes: ObservationAttributes = serde_json::from_str(raw).unwrap();
    let footnote = &attributes.footnotes.as_ref().unwrap()[0];
    assert_eq!(footnote.code(), None);
    assert_eq!(footnote.text(), None);
    assert_eq!(
        footnote.0["extension"]["amount"].to_string(),
        "123456789012345678901234567890.12345678901234567890123456789"
    );
    assert_eq!(footnote.0["extension"]["exponent"].to_string(), "1e+1000");
    let wire = serde_json::to_string(&attributes).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&wire).unwrap(),
        expected
    );
    assert_eq!(
        serde_json::from_str::<ObservationAttributes>(&wire).unwrap(),
        attributes
    );
}

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

#[test]
fn unknown_numeric_attributes_keep_hundreds_of_digits_and_large_exponents() {
    let integer = "9".repeat(240);
    let fraction = format!("0.{}1234567890123456789", "0".repeat(210));
    let raw = format!(
        r#"{{"provider_extension":{{"integer":{integer},"fraction":{fraction},"exponent":1e1000}},"nested":[{{"negative":-{integer}}},null]}}"#
    );
    let attributes: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(
        attributes["provider_extension"]["integer"]
            .as_number()
            .unwrap()
            .to_string(),
        integer
    );
    assert_eq!(
        attributes["provider_extension"]["fraction"]
            .as_number()
            .unwrap()
            .to_string(),
        fraction
    );
    assert_eq!(
        attributes["provider_extension"]["exponent"]
            .as_number()
            .unwrap()
            .to_string(),
        "1e+1000"
    );
    let point = DataPoint {
        id: Uuid::from_u128(1),
        series_id: Uuid::from_u128(2),
        date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        value: None,
        revision_date: NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(),
        is_original_release: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        attributes: Some(attributes.clone()),
    };
    let observation: Observation = point.into();
    let json = serde_json::to_string(&observation).unwrap();
    let decoded: Observation = serde_json::from_str(&json).unwrap();
    let restored: DataPoint = decoded.into();
    assert_eq!(restored.attributes, Some(attributes));
}
