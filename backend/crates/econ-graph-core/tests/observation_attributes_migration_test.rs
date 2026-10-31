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
        if next.name().version().to_string() == "20261031010000" {
            break;
        }
        conn.run_migration(next.as_ref()).unwrap();
    }
    conn.batch_execute(include_str!("fixtures/train3/data.sql"))
        .unwrap();
    let before = fingerprint(&mut conn);
    conn.run_pending_migrations(DATA_MIGRATIONS).unwrap();
    assert_eq!(before, fingerprint(&mut conn));
    let points = data_points::table
        .select(DataPoint::as_select())
        .load::<DataPoint>(&mut conn)
        .unwrap();
    assert_eq!(points.len(), 3);
    assert!(points.iter().all(|p| p.attributes.is_none()));
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
    assert_eq!(read.attributes, Some(attributes));
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
