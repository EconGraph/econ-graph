use anyhow::{Context, Result};
use arrow_array::{
    Array, ArrayRef, BooleanArray, Date32Array, Int64Array, RecordBatch, StringArray,
    TimestampMicrosecondArray,
};
use chrono::{DateTime, NaiveDate, Utc};
use econ_graph_core::models::Observation;
use iceberg::spec::{NestedField, PrimitiveType, Schema, Type};
use std::{collections::BTreeMap, sync::Arc};

pub fn schema() -> Result<Schema> {
    let defs = [
        ("id", PrimitiveType::String, false),
        ("series_id", PrimitiveType::String, false),
        ("date", PrimitiveType::Date, false),
        ("revision_date", PrimitiveType::Date, false),
        ("value", PrimitiveType::String, true),
        ("is_original_release", PrimitiveType::Boolean, false),
        ("created_at", PrimitiveType::Timestamptz, false),
        ("updated_at", PrimitiveType::Timestamptz, false),
        ("attributes", PrimitiveType::String, true),
        ("_commit_generation", PrimitiveType::Long, false),
    ];
    Ok(Schema::builder()
        .with_fields(
            defs.into_iter()
                .enumerate()
                .map(|(i, (name, ty, optional))| {
                    Arc::new(if optional {
                        NestedField::optional(i as i32 + 1, name, Type::Primitive(ty))
                    } else {
                        NestedField::required(i as i32 + 1, name, Type::Primitive(ty))
                    })
                })
                .collect::<Vec<_>>(),
        )
        .build()?)
}
pub fn to_batch(schema: &Schema, rows: &[Observation], generation: i64) -> Result<RecordBatch> {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
    let values: Vec<_> = rows
        .iter()
        .map(|r| r.value.as_ref().map(ToString::to_string))
        .collect();
    let attrs: Vec<_> = rows
        .iter()
        .map(|r| {
            serde_json::to_value(&r.attributes).map(|v| {
                if v.is_null() {
                    None
                } else {
                    Some(v.to_string())
                }
            })
        })
        .collect::<std::result::Result<_, _>>()?;
    let arrays: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(
            rows.iter().map(|r| r.id.to_string()).collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from(
            rows.iter()
                .map(|r| r.series_id.to_string())
                .collect::<Vec<_>>(),
        )),
        Arc::new(Date32Array::from(
            rows.iter()
                .map(|r| (r.date - epoch).num_days() as i32)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Date32Array::from(
            rows.iter()
                .map(|r| (r.revision_date - epoch).num_days() as i32)
                .collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from(values)),
        Arc::new(BooleanArray::from(
            rows.iter()
                .map(|r| r.is_original_release)
                .collect::<Vec<_>>(),
        )),
        Arc::new(
            TimestampMicrosecondArray::from(
                rows.iter()
                    .map(|r| r.created_at.timestamp_micros())
                    .collect::<Vec<_>>(),
            )
            .with_timezone("+00:00"),
        ),
        Arc::new(
            TimestampMicrosecondArray::from(
                rows.iter()
                    .map(|r| r.updated_at.timestamp_micros())
                    .collect::<Vec<_>>(),
            )
            .with_timezone("+00:00"),
        ),
        Arc::new(StringArray::from(attrs)),
        Arc::new(Int64Array::from(vec![generation; rows.len()])),
    ];
    Ok(RecordBatch::try_new(
        Arc::new(iceberg::arrow::schema_to_arrow_schema(schema)?),
        arrays,
    )?)
}
pub fn from_batches(batches: &[RecordBatch]) -> Result<Vec<Observation>> {
    let mut rows = vec![];
    for b in batches {
        for i in 0..b.num_rows() {
            let string = |name: &str| -> Result<Option<String>> {
                let a = b
                    .column_by_name(name)
                    .context(format!("missing {name}"))?
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .context(format!("invalid {name}"))?;
                Ok((!a.is_null(i)).then(|| a.value(i).to_string()))
            };
            let date = |name: &str| -> Result<NaiveDate> {
                let a = b
                    .column_by_name(name)
                    .context(format!("missing {name}"))?
                    .as_any()
                    .downcast_ref::<Date32Array>()
                    .context(format!("invalid {name}"))?;
                Ok(NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
                    + chrono::Duration::days(a.value(i).into()))
            };
            let stamp = |name: &str| -> Result<DateTime<Utc>> {
                let a = b
                    .column_by_name(name)
                    .context(format!("missing {name}"))?
                    .as_any()
                    .downcast_ref::<TimestampMicrosecondArray>()
                    .context(format!("invalid {name}"))?;
                DateTime::from_timestamp_micros(a.value(i)).context("timestamp out of range")
            };
            let attributes = string("attributes")?
                .map(|s| serde_json::from_str::<serde_json::Value>(&s))
                .transpose()?
                .unwrap_or(serde_json::Value::Null);
            // Deserializing through the public DTO keeps the additive attribute boundary lossless.
            rows.push(serde_json::from_value(serde_json::json!({"id":string("id")?.context("null id")?,"series_id":string("series_id")?.context("null series_id")?,"date":date("date")?,"revision_date":date("revision_date")?,"value":string("value")?,"is_original_release":b.column_by_name("is_original_release").context("missing original flag")?.as_any().downcast_ref::<BooleanArray>().context("invalid original flag")?.value(i),"created_at":stamp("created_at")?,"updated_at":stamp("updated_at")?,"attributes":attributes}))?);
        }
    }
    Ok(rows)
}
/// Stable content digest independent of file layout and snapshot IDs. JSON lines are ordered
/// by (series UUID,date,revision date,observation UUID), with decimal strings and explicit nulls.
pub fn canonical_rows(rows: &[Observation]) -> Result<Vec<u8>> {
    let mut rows = rows.to_vec();
    rows.sort_by_key(|r| (r.series_id, r.date, r.revision_date, r.id));
    let mut out = vec![];
    for row in rows {
        let ordered: BTreeMap<String, serde_json::Value> =
            serde_json::from_value(serde_json::to_value(row)?)?;
        serde_json::to_writer(&mut out, &ordered)?;
        out.push(b'\n');
    }
    Ok(out)
}

pub fn schema_with_dimensions(names: &[String]) -> Result<Schema> {
    let base = schema()?;
    let mut fields = base.as_struct().fields().to_vec();
    for (i, name) in names.iter().enumerate() {
        fields.push(Arc::new(NestedField::optional(
            11 + i as i32,
            format!("dim_{name}"),
            Type::Primitive(PrimitiveType::String),
        )));
    }
    Ok(Schema::builder().with_fields(fields).build()?)
}
pub fn to_batch_with_dimensions(
    table_schema: &Schema,
    rows: &[Observation],
    dimensions: &BTreeMap<String, String>,
    generation: i64,
) -> Result<RecordBatch> {
    let base = schema()?;
    let batch = to_batch(&base, rows, generation)?;
    let mut arrays = batch.columns().to_vec();
    for field in table_schema.as_struct().fields().iter().skip(10) {
        let name = field
            .name
            .strip_prefix("dim_")
            .context("unexpected dimension field")?;
        arrays.push(Arc::new(StringArray::from(vec![
            dimensions
                .get(name)
                .cloned();
            rows.len()
        ])));
    }
    Ok(RecordBatch::try_new(
        Arc::new(iceberg::arrow::schema_to_arrow_schema(table_schema)?),
        arrays,
    )?)
}
