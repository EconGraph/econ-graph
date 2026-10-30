//! Executable examples shared by company and workspace consumers. No database is required.
use econ_graph_core::models::{
    AnchorResolution, AnnotationAnchor, ChartAnnotation, FactSubject, FinancialAnnotation,
    SubjectRef,
};
use serde_json::{json, Value};
use uuid::Uuid;

const FIXTURE: &str = include_str!("fixtures/research-anchor-v1.json");

fn anchor() -> AnnotationAnchor {
    AnnotationAnchor::from_json(FIXTURE).unwrap()
}

#[test]
fn company_and_workspace_serialize_the_same_versioned_fixture() {
    let company = anchor();
    let workspace = AnnotationAnchor::from_json(&serde_json::to_string(&company).unwrap()).unwrap();
    assert_eq!(company, workspace);
    assert_eq!(
        serde_json::to_value(company).unwrap(),
        serde_json::from_str::<Value>(FIXTURE).unwrap()
    );
}

#[test]
fn restatements_and_economic_dimensions_do_not_retarget_filed_notes() {
    let original = anchor();
    let AnnotationAnchor::V1 {
        target: original_target,
        ..
    } = &original;
    let original_key = original_target.canonical_key().unwrap();
    for (pointer, replacement) in [
        ("/target/filing/accession", json!("0000320193-25-000001")),
        ("/target/subject/period/start", json!("2024-01-01")),
        (
            "/target/subject/unit",
            json!("{http://www.xbrl.org/2003/iso4217}EUR"),
        ),
        (
            "/target/subject/concept_namespace",
            json!("https://example.invalid/custom"),
        ),
        (
            "/target/subject/dimensions",
            json!({"axis": "other-segment"}),
        ),
    ] {
        let mut value = serde_json::to_value(&original).unwrap();
        *value.pointer_mut(pointer).unwrap() = replacement;
        let changed = AnnotationAnchor::from_json(&value.to_string()).unwrap();
        let AnnotationAnchor::V1 { target, .. } = changed;
        assert_ne!(original_key, target.canonical_key().unwrap(), "{pointer}");
    }
    // Economic subject intentionally ignores which filing reports the fact.
    let SubjectRef::FiledFact { subject, .. } = original_target else {
        panic!()
    };
    let economic = SubjectRef::Fact {
        subject: subject.clone(),
    };
    assert_ne!(original_key, economic.canonical_key().unwrap());
}

#[test]
fn canonical_keys_sort_dimensions_and_do_not_use_ambiguous_delimiters() {
    let first: FactSubject = serde_json::from_value(json!({
        "cik":"0000320193", "concept_namespace":"https://example.invalid/ns", "concept":"Revenue",
        "period":{"kind":"instant","date":"2024-09-28"}, "unit":"USD",
        "dimensions":{"z":"a:b", "a":"x"}
    }))
    .unwrap();
    let mut second = first.clone();
    second.dimensions.clear();
    second.dimensions.insert("a".into(), "x".into());
    second.dimensions.insert("z".into(), "a:b".into());
    assert_eq!(
        SubjectRef::Fact { subject: first }.canonical_key().unwrap(),
        SubjectRef::Fact { subject: second }
            .canonical_key()
            .unwrap()
    );
}

#[test]
fn invalid_versions_periods_and_issuer_mismatches_are_rejected() {
    for (pointer, replacement) in [
        ("/version", json!("2")),
        ("/target/subject/period/start", json!("2025-01-01")),
        ("/target/subject/cik", json!("0000000001")),
        ("/target/filing/accession", json!("bad-accession")),
        ("/target/subject/unit", json!("")),
    ] {
        let mut value: Value = serde_json::from_str(FIXTURE).unwrap();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            AnnotationAnchor::from_json(&value.to_string()).is_err(),
            "{pointer}"
        );
    }
}

#[test]
fn legacy_financial_adapter_preserves_both_stored_uuids() {
    let statement_id = Uuid::from_u128(42);
    let line_item_id = Uuid::from_u128(43);
    let note: FinancialAnnotation = serde_json::from_value(json!({
        "id":Uuid::from_u128(44), "statement_id":statement_id, "line_item_id":line_item_id,
        "annotation_type":"Comment", "title":"original filing", "content":"retain me",
        "status":"Active", "created_by":Uuid::from_u128(45), "assigned_to":null,
        "priority":null, "tags":null, "created_at":"2024-01-01T00:00:00Z",
        "updated_at":"2024-01-01T00:00:00Z", "resolved_at":null
    }))
    .unwrap();
    let converted = AnnotationAnchor::try_from(&note).unwrap();
    assert_eq!(
        converted,
        AnnotationAnchor::V1 {
            target: SubjectRef::LegacyFinancial {
                statement_id,
                line_item_id: Some(line_item_id)
            },
            chart_id: None,
            observation_date: None,
            observation_value: None,
        }
    );
}

#[test]
fn legacy_series_adapter_keeps_non_derived_id_and_chart_context() {
    let legacy_id = Uuid::from_u128(99);
    let chart_id = Uuid::from_u128(100);
    let note: ChartAnnotation = serde_json::from_value(json!({
        "id":Uuid::from_u128(101), "user_id":Uuid::from_u128(102), "series_id":legacy_id.to_string(),
        "chart_id":chart_id, "annotation_date":"2024-01-01", "annotation_value":"12345678901234567890.000000000000000001",
        "title":"legacy", "description":null, "color":null, "annotation_type":null,
        "visibility":"Private", "is_pinned":null, "tags":null, "created_at":null,"updated_at":null
    })).unwrap();
    let AnnotationAnchor::V1 {
        target,
        chart_id: converted_chart,
        observation_value,
        ..
    } = AnnotationAnchor::try_from(&note).unwrap();
    assert_eq!(target, SubjectRef::Series { id: legacy_id });
    assert_eq!(converted_chart, Some(chart_id));
    assert_eq!(
        observation_value.unwrap().to_string(),
        "12345678901234567890.000000000000000001"
    );
}

#[test]
fn missing_and_inaccessible_do_not_leak_target_existence_or_mutate_notes() {
    let note = anchor();
    for state in [
        AnchorResolution::<()>::Missing,
        AnchorResolution::Tombstoned,
        AnchorResolution::Inaccessible,
    ] {
        assert_eq!(state.public_status(), "not_found");
    }
    assert_eq!(
        AnchorResolution::<()>::Unavailable.public_status(),
        "unavailable"
    );
    assert_eq!(note, anchor());
}

#[test]
fn ambiguous_duplicate_dimension_axes_are_rejected() {
    let duplicate = r#"{"cik":"0000320193","concept_namespace":"ns","concept":"Revenue",
        "period":{"kind":"instant","date":"2024-01-01"},"unit":"USD",
        "dimensions":{"axis":"first","axis":"second"}}"#;
    assert!(serde_json::from_str::<FactSubject>(duplicate).is_err());
}
