//! Integration tests for annotation creation: ECO-439 idempotency and ECO-520 exact values.
//!
//! Real Postgres and the real schema, end to end. "Replicas" are separate schemas on separate
//! connection pools, which share nothing but the database, as separate backend pods do.

use crate::graphql::context::GraphQLContext;
use crate::graphql::schema::create_schema_with_data;
use crate::graphql::TEST_DB_LOCK as DB_LOCK;
use econ_graph_core::models::User;
use econ_graph_core::DatabasePool;
use std::sync::Arc;

/// A migrated Postgres pool with this file's rows removed, or `None` (with a skip notice)
/// when `DATABASE_URL` isn't set.
async fn db() -> Option<(String, DatabasePool, tokio::sync::MutexGuard<'static, ()>)> {
    use diesel_async::RunQueryDsl;
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set; skipping DB-backed annotation idempotency tests");
        return None;
    };
    let guard = DB_LOCK.lock().await;
    econ_graph_core::database::run_migrations(&url)
        .await
        .expect("migrations");
    let pool = econ_graph_core::database::create_pool(&url)
        .await
        .expect("pool");
    let mut conn = pool.get().await.unwrap();
    // Annotations go with their users (ON DELETE CASCADE).
    diesel::sql_query("DELETE FROM users WHERE email LIKE '%@eco439.test'")
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("DELETE FROM economic_series WHERE external_id LIKE 'eco439-%'")
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    Some((url, pool, guard))
}

/// Creates a real `users` row for `name`.
async fn make_user(pool: &DatabasePool, name: &str) -> User {
    User::get_or_create_for_subject(
        pool,
        uuid::Uuid::new_v4(),
        Some(&format!("{name}@eco439.test")),
        true,
        Some(name),
    )
    .await
    .expect("create test user")
}

/// Creates a real `economic_series` row and returns its id.
async fn make_series(pool: &DatabasePool) -> String {
    use diesel::sql_types::{Text, Uuid as SqlUuid};
    use diesel_async::RunQueryDsl;
    #[derive(diesel::QueryableByName)]
    struct Id {
        #[diesel(sql_type = SqlUuid)]
        id: uuid::Uuid,
    }
    let mut conn = pool.get().await.unwrap();
    let source = diesel::sql_query("SELECT id FROM data_sources ORDER BY name LIMIT 1")
        .get_result::<Id>(&mut conn)
        .await
        .expect("the baseline migration seeds data sources")
        .id;
    let dataset = econ_graph_core::test_utils::test_dataset_id(&mut conn, source).await;
    diesel::sql_query(
        "INSERT INTO economic_series (source_id, external_id, title, frequency, is_active, dataset_id) \
         VALUES ($1, $2, 'Annotated series', 'Monthly', TRUE, $3) RETURNING id",
    )
    .bind::<SqlUuid, _>(source)
    .bind::<Text, _>(format!("eco439-{}", uuid::Uuid::new_v4()))
    .bind::<SqlUuid, _>(dataset)
    .get_result::<Id>(&mut conn)
    .await
    .expect("create test series")
    .id
    .to_string()
}

/// Runs `query` against the real schema on `pool`, as `user`.
async fn run_as(pool: &DatabasePool, user: &User, query: &str) -> async_graphql::Response {
    let schema = create_schema_with_data(
        econ_graph_core::database::AppPool(pool.clone()),
        Some(econ_graph_core::database::DataPool(pool.clone())),
        Arc::new(GraphQLContext::for_test_user(Some(user.clone()), false)),
    );
    schema.execute(query).await
}

/// A `createAnnotation` mutation titled `title`, with `key` as its idempotency key if any.
fn create(series_id: &str, title: &str, key: Option<&str>) -> String {
    let key = key
        .map(|k| format!(r#", idempotencyKey: "{k}""#))
        .unwrap_or_default();
    format!(
        r#"mutation {{ createAnnotation(input: {{ seriesId: "{series_id}", annotationDate: "2024-01-01", title: "{title}", content: "c", annotationType: "note"{key} }}) {{ id title }} }}"#
    )
}

/// The created annotation's id, failing on any error.
fn created_id(resp: &async_graphql::Response) -> String {
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);
    resp.data.clone().into_json().unwrap()["createAnnotation"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// How many annotations `user` has stored.
async fn stored_for(pool: &DatabasePool, user: &User) -> i64 {
    use diesel::{ExpressionMethods, QueryDsl};
    use diesel_async::RunQueryDsl;
    use econ_graph_core::schema::chart_annotations;
    chart_annotations::table
        .filter(chart_annotations::user_id.eq(user.id))
        .count()
        .get_result(&mut pool.get().await.unwrap())
        .await
        .unwrap()
}

/// A retry with the same key, on another replica, returns the original annotation and stores
/// nothing new, even when the retry's fields differ; a new key creates a new annotation.
#[tokio::test]
async fn a_retried_create_stores_one_annotation() {
    let Some((url, pool, _guard)) = db().await else {
        return;
    };
    let other_replica = econ_graph_core::database::create_pool(&url).await.unwrap();
    let alice = make_user(&pool, "alice").await;
    let series = make_series(&pool).await;
    let key = uuid::Uuid::new_v4().to_string();

    let first = created_id(&run_as(&pool, &alice, &create(&series, "Recession", Some(&key))).await);
    let retry = run_as(
        &other_replica,
        &alice,
        &create(&series, "Changed on retry", Some(&key)),
    )
    .await;
    assert_eq!(created_id(&retry), first);
    assert_eq!(
        retry.data.into_json().unwrap()["createAnnotation"]["title"],
        "Recession",
        "a retry returns what the first attempt stored"
    );
    assert_eq!(stored_for(&pool, &alice).await, 1);

    let next_key = uuid::Uuid::new_v4().to_string();
    let next =
        created_id(&run_as(&pool, &alice, &create(&series, "Recovery", Some(&next_key))).await);
    assert_ne!(next, first);
    assert_eq!(stored_for(&pool, &alice).await, 2);
}

/// Attempts with one key that race each other, on different replicas, store one annotation
/// and all return it.
#[tokio::test]
async fn concurrent_attempts_with_one_key_store_one_annotation() {
    let Some((url, pool, _guard)) = db().await else {
        return;
    };
    let alice = make_user(&pool, "alice").await;
    let series = make_series(&pool).await;
    let key = uuid::Uuid::new_v4().to_string();

    let mut attempts = Vec::new();
    for _ in 0..8 {
        let replica = econ_graph_core::database::create_pool(&url).await.unwrap();
        let (alice, query) = (alice.clone(), create(&series, "Raced", Some(&key)));
        attempts.push(tokio::spawn(async move {
            created_id(&run_as(&replica, &alice, &query).await)
        }));
    }
    let mut ids = Vec::new();
    for attempt in attempts {
        ids.push(attempt.await.unwrap());
    }
    ids.dedup();
    assert_eq!(
        ids.len(),
        1,
        "every attempt returns the same annotation: {ids:?}"
    );
    assert_eq!(stored_for(&pool, &alice).await, 1);
}

/// Keys are scoped to the acting user: bob sending alice's key creates his own annotation and
/// never gets hers. Without a key every create stores a new annotation, as before.
#[tokio::test]
async fn keys_are_scoped_per_user_and_optional() {
    let Some((_url, pool, _guard)) = db().await else {
        return;
    };
    let alice = make_user(&pool, "alice").await;
    let bob = make_user(&pool, "bob").await;
    let series = make_series(&pool).await;
    let key = "shared-key";

    let hers = created_id(&run_as(&pool, &alice, &create(&series, "Alice's", Some(key))).await);
    let his = run_as(&pool, &bob, &create(&series, "Bob's", Some(key))).await;
    assert_ne!(created_id(&his), hers);
    assert_eq!(
        his.data.into_json().unwrap()["createAnnotation"]["title"],
        "Bob's"
    );
    assert_eq!(stored_for(&pool, &alice).await, 1);
    assert_eq!(stored_for(&pool, &bob).await, 1);

    for _ in 0..2 {
        created_id(&run_as(&pool, &bob, &create(&series, "No key", None)).await);
    }
    assert_eq!(stored_for(&pool, &bob).await, 3);

    let too_long = "k".repeat(129);
    let refused = run_as(&pool, &bob, &create(&series, "Long key", Some(&too_long))).await;
    assert_eq!(refused.errors.len(), 1, "{:?}", refused.errors);
    assert!(
        refused.errors[0].message.contains("Idempotency key"),
        "{}",
        refused.errors[0].message
    );
    assert_eq!(stored_for(&pool, &bob).await, 3);
}

/// Annotation Y values must preserve the large aggregates and fractional precision that
/// observations can hold, through GraphQL input, Postgres storage, and a subsequent query.
#[tokio::test]
async fn annotation_values_round_trip_large_and_precise_decimals() {
    use diesel::sql_types::{Text, Uuid as SqlUuid};
    use diesel_async::RunQueryDsl;

    #[derive(diesel::QueryableByName)]
    struct StoredValue {
        #[diesel(sql_type = Text)]
        value: String,
    }

    let Some((_url, pool, _guard)) = db().await else {
        return;
    };
    let alice = make_user(&pool, "exact-values").await;
    let series = make_series(&pool).await;

    let redundant_zeros = format!("1.{}", "0".repeat(40_000));
    for (value, expected) in [
        ("190358563845627.40625", "190358563845627.40625"),
        ("-190358563845627.40625", "-190358563845627.40625"),
        (
            "0.123456789012345678901234567890",
            "0.123456789012345678901234567890",
        ),
        ("0e-999999999", "0"),
        (redundant_zeros.as_str(), "1"),
    ] {
        let mutation = format!(
            r#"mutation {{ createAnnotation(input: {{ seriesId: "{series}", annotationDate: "2024-01-01", annotationValue: "{value}", title: "Exact value", content: "c", annotationType: "note" }}) {{ id annotationValue }} }}"#
        );
        let response = run_as(&pool, &alice, &mutation).await;
        let id = created_id(&response);
        assert_eq!(
            response.data.into_json().unwrap()["createAnnotation"]["annotationValue"],
            expected,
            "create response must preserve the exact value"
        );

        let stored: StoredValue = diesel::sql_query(
            "SELECT annotation_value::text AS value FROM chart_annotations WHERE id = $1",
        )
        .bind::<SqlUuid, _>(uuid::Uuid::parse_str(&id).unwrap())
        .get_result(&mut pool.get().await.unwrap())
        .await
        .expect("read stored annotation value");
        assert_eq!(
            stored.value, expected,
            "Postgres must store the exact decimal"
        );

        let query =
            format!(r#"{{ annotationsForSeries(seriesId: "{series}") {{ id annotationValue }} }}"#);
        let response = run_as(&pool, &alice, &query).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().unwrap();
        let annotation = data["annotationsForSeries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|annotation| annotation["id"] == id)
            .expect("query returns the created annotation");
        assert_eq!(annotation["annotationValue"], expected);
    }
    assert_eq!(stored_for(&pool, &alice).await, 5);
}

/// Unconstrained NUMERIC storage still enforces the observation value limits at the API.
#[tokio::test]
async fn annotation_values_outside_observation_limits_store_nothing() {
    let Some((_url, pool, _guard)) = db().await else {
        return;
    };
    let alice = make_user(&pool, "value-limits").await;
    let series = make_series(&pool).await;

    for value in ["1e40", "-1e40", "1e-31", "-1e-31"] {
        let mutation = format!(
            r#"mutation {{ createAnnotation(input: {{ seriesId: "{series}", annotationDate: "2024-01-01", annotationValue: "{value}", title: "Out of bounds", content: "c", annotationType: "note" }}) {{ id }} }}"#
        );
        let response = run_as(&pool, &alice, &mutation).await;
        assert_eq!(response.errors.len(), 1, "{value}: {:?}", response.errors);
        assert!(
            response.errors[0].message.contains("digits"),
            "{value}: {}",
            response.errors[0].message
        );
        assert_eq!(stored_for(&pool, &alice).await, 0);
    }
}
