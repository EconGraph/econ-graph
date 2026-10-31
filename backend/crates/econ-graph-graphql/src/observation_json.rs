//! Lossless JSON for provider attributes. The generic `Json<Value>` serializer treats
//! serde_json's arbitrary-precision number representation as a private-token object.
//! Map the value tree directly instead: GraphQL and serde_json share the Number type.

use async_graphql::{InputValueResult, Name, Scalar, ScalarType, Value};

/// Lossless JSON wrapper for captured observation attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationJson(pub serde_json::Value);

/// Provider JSON with exact native JSON numbers, including integers and fractional values
/// wider than IEEE-754 or 64-bit integers. Clients must use a precision-preserving JSON parser.
#[Scalar(name = "ObservationAttributes")]
impl ScalarType for ObservationJson {
    fn parse(value: Value) -> InputValueResult<Self> {
        Ok(Self(from_graphql(value)))
    }

    fn to_value(&self) -> Value {
        to_graphql(&self.0)
    }
}

fn to_graphql(value: &serde_json::Value) -> Value {
    match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(value) => Value::Boolean(*value),
        serde_json::Value::Number(value) => Value::Number(value.clone()),
        serde_json::Value::String(value) => Value::String(value.clone()),
        serde_json::Value::Array(values) => Value::List(values.iter().map(to_graphql).collect()),
        serde_json::Value::Object(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| (Name::new(key), to_graphql(value)))
                .collect(),
        ),
    }
}

fn from_graphql(value: Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Boolean(value) => serde_json::Value::Bool(value),
        Value::Number(value) => serde_json::Value::Number(value),
        Value::String(value) => serde_json::Value::String(value),
        Value::Enum(value) => serde_json::Value::String(value.to_string()),
        Value::Binary(value) => serde_json::Value::Array(
            value
                .iter()
                .map(|byte| serde_json::Value::Number((*byte).into()))
                .collect(),
        ),
        Value::List(values) => {
            serde_json::Value::Array(values.into_iter().map(from_graphql).collect())
        }
        Value::Object(values) => serde_json::Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key.to_string(), from_graphql(value)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_maps_numbers_directly_and_roundtrips_unknown_nested_fields() {
        let raw = r#"{"integer":1234567890123456789012345678901234567890,"fraction":0.123456789012345678901234567890123456789,"nested":[null,{"exponent":1e1000}],"known-empty":{},"unknown":null}"#;
        let original: serde_json::Value = serde_json::from_str(raw).unwrap();
        let scalar = ObservationJson(original.clone());
        let graphql = scalar.to_value();
        let Value::Object(fields) = &graphql else {
            panic!("expected object")
        };
        assert!(matches!(fields.get("integer"), Some(Value::Number(_))));
        assert!(matches!(fields.get("fraction"), Some(Value::Number(_))));
        assert_eq!(ObservationJson::parse(graphql.clone()).unwrap(), scalar);
        let encoded = serde_json::to_string(&graphql).unwrap();
        assert!(!encoded.contains("$serde_json::private::Number"));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&encoded).unwrap(),
            original
        );
    }
}
