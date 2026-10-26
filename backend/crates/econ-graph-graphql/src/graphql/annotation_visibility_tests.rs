//! Integration tests for AUTH-8: annotation visibility (and ECO-410: annotation input checks).
//!
//! Real Postgres, real rows: alice and bob are actual `users` rows (annotations have a
//! `user_id` foreign key), and every case runs the GraphQL schema end to end rather than
//! calling the service layer directly, so it also exercises `require_role`/`current_user`
//! on the mutation and query resolvers.

use crate::graphql::context::GraphQLContext;
use crate::graphql::schema::create_schema_with_data;
use crate::graphql::TEST_DB_LOCK as DB_LOCK;
use econ_graph_core::models::User;
use econ_graph_core::DatabasePool;
use std::sync::Arc;

/// A real Postgres pool with migrations applied and this test's tables emptied, or `None`
/// (with a skip notice) when `DATABASE_URL` isn't set.
async fn db() -> Option<(DatabasePool, tokio::sync::MutexGuard<'static, ()>)> {
    use diesel_async::RunQueryDsl;
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set; skipping DB-backed annotation visibility tests");
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
    diesel::sql_query("DELETE FROM annotation_comments")
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("DELETE FROM chart_annotations")
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("DELETE FROM users WHERE email LIKE '%@auth8.test'")
        .execute(&mut conn)
        .await
        .unwrap();
    diesel::sql_query("DELETE FROM economic_series WHERE external_id LIKE 'auth8-%'")
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    Some((pool, guard))
}

/// Creates a real `users` row for `name`, e.g. "alice" or "bob".
async fn make_user(pool: &DatabasePool, name: &str) -> User {
    User::get_or_create_for_subject(
        pool,
        uuid::Uuid::new_v4(),
        Some(&format!("{name}@auth8.test")),
        true,
        Some(name),
    )
    .await
    .expect("create test user")
}

/// Creates a real `economic_series` row (annotations must be on an existing series) and
/// returns its id.
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
    .bind::<Text, _>(format!("auth8-{}", uuid::Uuid::new_v4()))
    .bind::<SqlUuid, _>(dataset)
    .get_result::<Id>(&mut conn)
    .await
    .expect("create test series")
    .id
    .to_string()
}

/// Runs `query` against the real schema, as `user` (or anonymous when `None`); never staff.
async fn run_as(pool: &DatabasePool, user: Option<User>, query: &str) -> async_graphql::Response {
    let schema = create_schema_with_data(
        econ_graph_core::database::AppPool(pool.clone()),
        Some(econ_graph_core::database::DataPool(pool.clone())),
        Arc::new(GraphQLContext::for_test_user(user, false)),
    );
    schema.execute(query).await
}

/// A `createAnnotation` mutation string for `series_id`, with the given title and visibility.
fn create_mutation(series_id: &str, title: &str, is_public: bool) -> String {
    format!(
        r#"mutation {{ createAnnotation(input: {{ seriesId: "{series_id}", annotationDate: "2024-01-01", title: "{title}", content: "c", annotationType: "note", isPublic: {is_public} }}) {{ id }} }}"#
    )
}

/// Extracts the created annotation's id from a `createAnnotation` response.
fn annotation_id(resp: &async_graphql::Response) -> String {
    let data = resp.data.clone().into_json().unwrap();
    data["createAnnotation"]["id"].as_str().unwrap().to_string()
}

/// Extracts the `id` of every node in a response's `field` array (e.g. `annotationsForSeries`).
fn ids_in(resp: &async_graphql::Response, field: &str) -> Vec<String> {
    resp.data.clone().into_json().unwrap()[field]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap().to_string())
        .collect()
}

/// The annotation's current title, as its owner sees it.
async fn title_as(pool: &DatabasePool, owner: &User, series_id: &str, id: &str) -> String {
    let resp = run_as(
        pool,
        Some(owner.clone()),
        &format!(r#"{{ annotationsForSeries(seriesId: "{series_id}") {{ id title }} }}"#),
    )
    .await;
    let annotations = resp.data.into_json().unwrap();
    annotations["annotationsForSeries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == id)
        .unwrap_or_else(|| panic!("{id} not found for its owner"))["title"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Public annotations are visible to anyone; private ones only to their author.
#[tokio::test]
async fn bob_sees_alice_public_but_not_private() {
    let Some((pool, _guard)) = db().await else {
        return;
    };
    let alice = make_user(&pool, "alice").await;
    let bob = make_user(&pool, "bob").await;
    let series_id = make_series(&pool).await;

    let pub_resp = run_as(
        &pool,
        Some(alice.clone()),
        &create_mutation(&series_id, "public one", true),
    )
    .await;
    assert!(pub_resp.errors.is_empty(), "{:?}", pub_resp.errors);
    let public_id = annotation_id(&pub_resp);

    let priv_resp = run_as(
        &pool,
        Some(alice.clone()),
        &create_mutation(&series_id, "private one", false),
    )
    .await;
    assert!(priv_resp.errors.is_empty(), "{:?}", priv_resp.errors);
    let private_id = annotation_id(&priv_resp);

    let query =
        format!(r#"{{ annotationsForSeries(seriesId: "{series_id}") {{ id visibility }} }}"#);

    // bob sees only alice's public annotation.
    let resp = run_as(&pool, Some(bob.clone()), &query).await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);
    assert_eq!(
        ids_in(&resp, "annotationsForSeries"),
        vec![public_id.clone()]
    );

    // Anonymous sees only the public one too.
    let resp = run_as(&pool, None, &query).await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);
    assert_eq!(
        ids_in(&resp, "annotationsForSeries"),
        vec![public_id.clone()]
    );

    // alice sees both of her own.
    let resp = run_as(&pool, Some(alice), &query).await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);
    let mut ids = ids_in(&resp, "annotationsForSeries");
    ids.sort();
    let mut expected = vec![public_id, private_id];
    expected.sort();
    assert_eq!(ids, expected);
}

/// Bob can't comment on, edit, or delete alice's annotation, whether it is public (he isn't
/// its owner) or private (he can't even see it, so it is reported as not found).
#[tokio::test]
async fn bob_cannot_comment_edit_or_delete_alices_annotation() {
    let Some((pool, _guard)) = db().await else {
        return;
    };
    let alice = make_user(&pool, "alice").await;
    let bob = make_user(&pool, "bob").await;
    let series_id = make_series(&pool).await;

    let pub_resp = run_as(
        &pool,
        Some(alice.clone()),
        &create_mutation(&series_id, "public one", true),
    )
    .await;
    let public_id = annotation_id(&pub_resp);

    let priv_resp = run_as(
        &pool,
        Some(alice.clone()),
        &create_mutation(&series_id, "private one", false),
    )
    .await;
    let private_id = annotation_id(&priv_resp);

    // Comment: allowed on the public one (any signed-in caller may comment), refused as
    // not-found on the private one bob cannot see.
    let resp = run_as(
        &pool,
        Some(bob.clone()),
        &format!(r#"mutation {{ addComment(input: {{ annotationId: "{public_id}", content: "hi" }}) {{ id }} }}"#),
    )
    .await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);

    let resp = run_as(
        &pool,
        Some(bob.clone()),
        &format!(r#"mutation {{ addComment(input: {{ annotationId: "{private_id}", content: "hi" }}) {{ id }} }}"#),
    )
    .await;
    assert_eq!(resp.errors.len(), 1, "{:?}", resp.errors);
    assert!(resp.errors[0].message.to_lowercase().contains("not found"));

    // Edit: bob isn't the owner of either. The public one exists but isn't his
    // (Unauthorized); the private one he can't even see (NotFound) - the distinction
    // matters, since leaking Unauthorized for the private one would confirm it exists.
    for (id, expected) in [(&public_id, "unauthorized"), (&private_id, "not found")] {
        let resp = run_as(
            &pool,
            Some(bob.clone()),
            &format!(
                r#"mutation {{ updateAnnotation(input: {{ annotationId: "{id}", title: "mine now" }}) {{ id }} }}"#
            ),
        )
        .await;
        assert_eq!(resp.errors.len(), 1, "{id}: {:?}", resp.errors);
        let msg = resp.errors[0].message.to_lowercase();
        assert!(msg.contains(expected), "{id}: {msg}");
    }
    // Neither title actually changed.
    assert_eq!(
        title_as(&pool, &alice, &series_id, &public_id).await,
        "public one"
    );
    assert_eq!(
        title_as(&pool, &alice, &series_id, &private_id).await,
        "private one"
    );

    // Delete: same story, and both annotations are still there afterward.
    for (id, expected) in [(&public_id, "unauthorized"), (&private_id, "not found")] {
        let resp = run_as(
            &pool,
            Some(bob.clone()),
            &format!(r#"mutation {{ deleteAnnotation(input: {{ annotationId: "{id}" }}) }}"#),
        )
        .await;
        assert_eq!(resp.errors.len(), 1, "{id}: {:?}", resp.errors);
        let msg = resp.errors[0].message.to_lowercase();
        assert!(msg.contains(expected), "{id}: {msg}");
    }
    let resp = run_as(
        &pool,
        Some(alice),
        &format!(r#"{{ annotationsForSeries(seriesId: "{series_id}") {{ id }} }}"#),
    )
    .await;
    let mut ids = ids_in(&resp, "annotationsForSeries");
    ids.sort();
    let mut expected = vec![public_id, private_id];
    expected.sort();
    assert_eq!(
        ids, expected,
        "bob's refused deletes must not have removed anything"
    );
}

/// Alice, as the annotation's author, can update her own annotation, including flipping its
/// visibility - and once it's private, bob stops seeing it.
#[tokio::test]
async fn alice_can_update_her_own_annotation() {
    let Some((pool, _guard)) = db().await else {
        return;
    };
    let alice = make_user(&pool, "alice").await;
    let bob = make_user(&pool, "bob").await;
    let series_id = make_series(&pool).await;

    let create_resp = run_as(
        &pool,
        Some(alice.clone()),
        &create_mutation(&series_id, "original title", true),
    )
    .await;
    assert!(create_resp.errors.is_empty(), "{:?}", create_resp.errors);
    let id = annotation_id(&create_resp);

    let resp = run_as(
        &pool,
        Some(alice.clone()),
        &format!(
            r#"mutation {{ updateAnnotation(input: {{ annotationId: "{id}", title: "new title", isPublic: false }}) {{ id title visibility }} }}"#
        ),
    )
    .await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);
    let updated = resp.data.into_json().unwrap();
    assert_eq!(updated["updateAnnotation"]["title"], "new title");
    assert_eq!(updated["updateAnnotation"]["visibility"], "PRIVATE");

    // Now that it's private, bob no longer sees it.
    let resp = run_as(
        &pool,
        Some(bob),
        &format!(r#"{{ annotationsForSeries(seriesId: "{series_id}") {{ id }} }}"#),
    )
    .await;
    assert!(ids_in(&resp, "annotationsForSeries").is_empty());

    // alice still sees her (now private, retitled) annotation.
    assert_eq!(title_as(&pool, &alice, &series_id, &id).await, "new title");
}

/// Comments on a private annotation are hidden from everyone but its author, exactly as the
/// annotation itself is.
#[tokio::test]
async fn comments_on_a_private_annotation_are_hidden_from_others() {
    let Some((pool, _guard)) = db().await else {
        return;
    };
    let alice = make_user(&pool, "alice").await;
    let bob = make_user(&pool, "bob").await;
    let series_id = make_series(&pool).await;

    let priv_resp = run_as(
        &pool,
        Some(alice.clone()),
        &create_mutation(&series_id, "private one", false),
    )
    .await;
    let private_id = annotation_id(&priv_resp);

    let resp = run_as(
        &pool,
        Some(alice.clone()),
        &format!(r#"mutation {{ addComment(input: {{ annotationId: "{private_id}", content: "only alice should see this" }}) {{ id }} }}"#),
    )
    .await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);

    let query =
        format!(r#"{{ commentsForAnnotation(annotationId: "{private_id}") {{ id content }} }}"#);

    // alice, its author, sees the comment.
    let resp = run_as(&pool, Some(alice), &query).await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);
    let comments = resp.data.into_json().unwrap()["commentsForAnnotation"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(comments, 1);

    // bob does not: the annotation (and so its comments) is reported as not found.
    let resp = run_as(&pool, Some(bob), &query).await;
    assert_eq!(resp.errors.len(), 1, "{:?}", resp.errors);
    assert!(resp.errors[0].message.to_lowercase().contains("not found"));

    // Anonymous: same.
    let resp = run_as(&pool, None, &query).await;
    assert_eq!(resp.errors.len(), 1, "{:?}", resp.errors);
    assert!(resp.errors[0].message.to_lowercase().contains("not found"));
}

/// ECO-410: an annotation must be on a series that exists, on a sane date, with a value that
/// fits the column. A refused one gets a clear message, not a database error, and isn't written.
#[tokio::test]
async fn annotations_on_unknown_series_or_absurd_dates_are_refused() {
    use diesel::QueryDsl;
    use diesel_async::RunQueryDsl;
    let Some((pool, _guard)) = db().await else {
        return;
    };
    let alice = make_user(&pool, "alice").await;
    let series_id = make_series(&pool).await;
    let create = |series: &str, date: &str, value: &str| {
        format!(
            r#"mutation {{ createAnnotation(input: {{ seriesId: "{series}", annotationDate: "{date}", annotationValue: "{value}", title: "t", content: "c", annotationType: "note" }}) {{ id }} }}"#
        )
    };

    let unknown = uuid::Uuid::new_v4();
    for (query, expected) in [
        (
            create(&unknown.to_string(), "2024-01-01", "1"),
            format!("Series not found: {unknown}"),
        ),
        (
            create(&series_id, "0001-01-01", "1"),
            "Date 0001-01-01 is outside".to_string(),
        ),
        (
            create(&series_id, "2099-12-31", "1"),
            "Date 2099-12-31 is outside".to_string(),
        ),
        (
            create(&series_id, "2024-01-01", "1e40"),
            "integer digits".to_string(),
        ),
        (
            create(&series_id, "2024-01-01", "1e-31"),
            "fractional digits".to_string(),
        ),
    ] {
        let resp = run_as(&pool, Some(alice.clone()), &query).await;
        assert_eq!(resp.errors.len(), 1, "{query}: {:?}", resp.errors);
        assert!(
            resp.errors[0].message.contains(&expected),
            "{query}: {}",
            resp.errors[0].message
        );
    }

    let mut conn = pool.get().await.unwrap();
    let written: i64 = econ_graph_core::schema::chart_annotations::table
        .count()
        .get_result(&mut conn)
        .await
        .unwrap();
    assert_eq!(written, 0);
    drop(conn);

    // Values that overflowed DECIMAL(20, 6) now fit the observation-aligned bounds.
    for value in ["1e20", "99999999999999.9999999"] {
        let resp = run_as(
            &pool,
            Some(alice.clone()),
            &create(&series_id, "2024-01-01", value),
        )
        .await;
        assert!(resp.errors.is_empty(), "{value}: {:?}", resp.errors);
    }

    // The same request on the real series, a sane date and a value that fits is accepted.
    let resp = run_as(&pool, Some(alice), &create(&series_id, "2024-01-01", "1.5")).await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);
}
