//! A populated pre-attribute database upgrades without inventing historical provenance.
use diesel::{connection::SimpleConnection, prelude::*};
use diesel_migrations::MigrationHarness;
use econ_graph_core::{
    database::DATA_MIGRATIONS, schema::data_points, test_utils::ScratchDatabase, DataPoint,
};
use serde_json::json;

#[derive(QueryableByName)]
struct Digest {
    #[diesel(sql_type = diesel::sql_types::Text)]
    value: String,
}
/// Execute the runner's actual SQL, rather than duplicating its observation projection.
fn semantic_snapshot(conn: &mut PgConnection) -> serde_json::Value {
    #[derive(QueryableByName)]
    struct Snapshot {
        #[diesel(sql_type = diesel::sql_types::Text)]
        jsonb_build_object: String,
    }
    let row = diesel::sql_query(include_str!(
        "../../../../scripts/train3/semantic-snapshot.sql"
    ))
    .get_result::<Snapshot>(conn)
    .unwrap();
    serde_json::from_str(&row.jsonb_build_object).unwrap()
}

fn fingerprint(conn: &mut PgConnection) -> String {
    diesel::sql_query("SELECT md5(string_agg((to_jsonb(p)-'attributes')::text, ',' ORDER BY id)) AS value FROM data_points p")
        .get_result::<Digest>(conn).unwrap().value
}

#[test]
fn nullable_attributes_preserve_the_populated_history_and_accept_only_objects() {
    let database = ScratchDatabase::create("attributes_migration");
    let mut conn = database.connect();
    loop {
        let pending = conn.pending_migrations(DATA_MIGRATIONS).unwrap();
        let next = pending.first().unwrap();
        if next.name().version().to_string() == "20261101000110" {
            break;
        }
        conn.run_migration(next.as_ref()).unwrap();
    }
    conn.batch_execute(include_str!("fixtures/train3/data.sql"))
        .unwrap();
    let before = fingerprint(&mut conn);
    let pending = conn.pending_migrations(DATA_MIGRATIONS).unwrap();
    assert_eq!(pending[0].name().version().to_string(), "20261101000110");
    conn.run_migration(pending[0].as_ref()).unwrap();
    let constraint_state = |conn: &mut PgConnection| {
        diesel::sql_query("SELECT convalidated::text AS value FROM pg_constraint WHERE conrelid='data_points'::regclass AND conname='data_points_attributes_object'")
            .get_result::<Digest>(conn).unwrap().value
    };
    assert_eq!(constraint_state(&mut conn), "false");
    assert!(
        diesel::sql_query("UPDATE data_points SET attributes='[]'::jsonb")
            .execute(&mut conn)
            .is_err(),
        "NOT VALID must still reject invalid new writes"
    );

    // Hold a real writer's table lock while the separate validation transaction runs.
    // Reintroducing ACCESS EXCLUSIVE validation would fail the bounded lock acquisition.
    let mut writer = database.connect();
    writer
        .batch_execute("BEGIN; UPDATE data_points SET attributes='{}'::jsonb")
        .unwrap();
    conn.batch_execute("SET lock_timeout='1s'").unwrap();
    let pending = conn.pending_migrations(DATA_MIGRATIONS).unwrap();
    assert_eq!(pending[0].name().version().to_string(), "20261101000111");
    conn.run_pending_migrations(DATA_MIGRATIONS).unwrap();
    conn.batch_execute("SET lock_timeout=DEFAULT").unwrap();
    writer.batch_execute("ROLLBACK").unwrap();
    assert_eq!(constraint_state(&mut conn), "true");
    assert_eq!(before, fingerprint(&mut conn));
    let points = data_points::table
        .select(DataPoint::as_select())
        .load::<DataPoint>(&mut conn)
        .unwrap();
    assert_eq!(points.len(), 3);
    assert!(points.iter().all(|p| p.attributes.is_none()));
    let unknown_snapshot = semantic_snapshot(&mut conn);
    assert_eq!(unknown_snapshot["format"], 2);
    assert!(unknown_snapshot["observations"]
        .as_array()
        .unwrap()
        .iter()
        .all(|point| point.get("attributes") == Some(&serde_json::Value::Null)));

    let target = points[0].id;
    let attributes = json!({"source":"BLS","footnotes":[{"code":"P","text":"literal\n\"雪\"","unknown":{"future":null}}],"estimated":null});
    diesel::update(data_points::table.find(target))
        .set(data_points::attributes.eq(Some(attributes.clone())))
        .execute(&mut conn)
        .unwrap();
    let read = data_points::table
        .find(target)
        .select(DataPoint::as_select())
        .first::<DataPoint>(&mut conn)
        .unwrap();
    assert_eq!(read.attributes, Some(attributes.clone()));
    let captured_snapshot = semantic_snapshot(&mut conn);
    let captured_row = captured_snapshot["observations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|point| point["id"] == target.to_string())
        .unwrap();
    assert_eq!(captured_row["attributes"], attributes);
    assert_ne!(unknown_snapshot, captured_snapshot);

    // SQL NULL and a captured empty object remain different storage states.
    diesel::update(data_points::table.find(target))
        .set(data_points::attributes.eq(Some(json!({}))))
        .execute(&mut conn)
        .unwrap();
    let captured: Option<serde_json::Value> = data_points::table
        .find(target)
        .select(data_points::attributes)
        .first(&mut conn)
        .unwrap();
    assert_eq!(captured, Some(json!({})));
    let empty_snapshot = semantic_snapshot(&mut conn);
    let empty_row = empty_snapshot["observations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|point| point["id"] == target.to_string())
        .unwrap();
    assert_eq!(empty_row["attributes"], json!({}));
    assert_ne!(
        empty_snapshot, unknown_snapshot,
        "semantic evidence must distinguish SQL NULL from captured-empty attributes"
    );
    assert_ne!(
        empty_snapshot, captured_snapshot,
        "semantic evidence must detect lost footnotes"
    );

    for invalid in [json!(null), json!([]), json!("not an object")] {
        assert!(diesel::update(data_points::table.find(target))
            .set(data_points::attributes.eq(Some(invalid)))
            .execute(&mut conn)
            .is_err());
    }
    let original = points.iter().find(|point| point.id == target).unwrap();
    let final_point = data_points::table
        .find(target)
        .select(DataPoint::as_select())
        .first::<DataPoint>(&mut conn)
        .unwrap();
    assert_eq!(final_point.id, original.id);
    assert_eq!(final_point.value, original.value);
    assert_eq!(final_point.date, original.date);
    assert_eq!(final_point.revision_date, original.revision_date);
}
