//! Execute real GraphQL requests against an isolated, migrated database.

use crate::graphql::schema::create_schema;
use econ_graph_core::database::{create_pool, run_migrations, AppPool, DataPool};
use econ_graph_core::test_utils::ScratchDatabase;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
enum Endpoint {
    Search,
    List,
}

impl Endpoint {
    /// Root response key for this resolver.
    fn field(self) -> &'static str {
        match self {
            Self::Search => "searchSeries",
            Self::List => "seriesList",
        }
    }

    /// Collection key inside the resolver's result (search has no connection wrapper).
    fn nodes(self) -> &'static str {
        match self {
            Self::Search => "series",
            Self::List => "nodes",
        }
    }

    /// Build a source-scoped request, quoting the cursor as a GraphQL string literal.
    fn query(self, source: Uuid, first: i32, after: Option<&str>) -> String {
        let after = after
            .map(|cursor| format!(", after: {}", serde_json::to_string(cursor).unwrap()))
            .unwrap_or_default();
        match self {
            Self::Search => format!(
                "{{ searchSeries(query: \"Cursorprobe\", source: \"{source}\", first: {first}{after}) \
                 {{ series {{ id }} }} }}"
            ),
            Self::List => format!(
                "{{ seriesList(filter: {{sourceId: \"{source}\"}}, \
                 pagination: {{first: {first}{after}}}) \
                 {{ nodes {{ id }} totalCount pageInfo {{ hasNextPage endCursor }} }} }}"
            ),
        }
    }

    /// Read returned series IDs in their pagination order.
    fn ids(self, data: &Value) -> Vec<String> {
        data[self.field()][self.nodes()]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["id"].as_str().unwrap().to_owned())
            .collect()
    }
}

/// Invalid continuations return only errors; valid retries still traverse every series once.
#[tokio::test]
async fn pagination_cursors_reject_invalid_input_and_preserve_traversal() {
    // No silent skip: this regression needs an actual database to verify resolver behavior.
    let db = ScratchDatabase::create("pagination");
    run_migrations(&db.url).await.unwrap();
    let pool = create_pool(&db.url).await.unwrap();
    let source = Uuid::new_v4();
    let dataset = Uuid::new_v4();
    {
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await.unwrap();
        for sql in [
            format!(
                "INSERT INTO data_sources (id, name, base_url) \
                     VALUES ('{source}', 'Cursorprobe', 'https://example.test')"
            ),
            format!(
                "INSERT INTO datasets (id, source_id, code, name) \
                     VALUES ('{dataset}', '{source}', 'cursor', 'Cursorprobe')"
            ),
            format!(
                "INSERT INTO economic_series \
                     (source_id, dataset_id, external_id, title, frequency, end_date) \
                     SELECT '{source}', '{dataset}', 'cursor-' || i, \
                     'Cursorprobe ' || i, 'Monthly', DATE '2026-01-01' \
                     FROM generate_series(1, 7) i"
            ),
        ] {
            diesel::sql_query(sql).execute(&mut conn).await.unwrap();
        }
    }
    let schema = create_schema(AppPool(pool.clone()), Some(DataPool(pool.clone())));

    for endpoint in [Endpoint::Search, Endpoint::List] {
        let response = schema.execute(endpoint.query(source, 100, None)).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let expected = endpoint.ids(&response.data.into_json().unwrap());
        assert_eq!(expected.len(), 7);

        let mut seen = Vec::new();
        let mut after = None;
        for offset in [0, 3, 6] {
            // Inject invalid continuations between valid pages. No records or success cursor
            // may be returned, and retrying the valid cursor must still continue correctly.
            for invalid in ["banana", "", "-1", "2147483648", "18446744073709551616"] {
                let response = schema
                    .execute(endpoint.query(source, 3, Some(invalid)))
                    .await;
                assert_eq!(response.errors.len(), 1, "{endpoint:?}: {invalid}");
                assert_bad_user_input(&response.errors[0]);
                assert_eq!(
                    response.errors[0].message,
                    format!("`after` is not a valid cursor: {invalid:?}")
                );
                assert!(
                    response.data == async_graphql::Value::Null,
                    "invalid continuation returned data"
                );
            }

            let response = schema
                .execute(endpoint.query(source, 3, after.as_deref()))
                .await;
            assert!(response.errors.is_empty(), "{:?}", response.errors);
            let data = response.data.into_json().unwrap();
            let ids = endpoint.ids(&data);
            assert_eq!(ids, expected[offset..(offset + 3).min(7)]);
            seen.extend(ids);
            if let Endpoint::List = endpoint {
                assert_eq!(data["seriesList"]["totalCount"], 7);
                assert_eq!(data["seriesList"]["pageInfo"]["hasNextPage"], offset < 6);
                after = data["seriesList"]["pageInfo"]["endCursor"]
                    .as_str()
                    .map(str::to_owned);
            } else {
                // SearchResult exposes no pageInfo; search clients supply the offset.
                after = Some((offset + 3).to_string());
            }
        }
        assert_eq!(
            seen, expected,
            "missing or repeated records for {endpoint:?}"
        );
        let unique: std::collections::HashSet<_> = seen.iter().collect();
        assert_eq!(unique.len(), 7);

        // Explicit zero is valid; offsets at and beyond the end return empty success pages.
        for cursor in ["0", "7", "99", "2147483647"] {
            let response = schema
                .execute(endpoint.query(source, 3, Some(cursor)))
                .await;
            assert!(
                response.errors.is_empty(),
                "{cursor}: {:?}",
                response.errors
            );
            let data = response.data.into_json().unwrap();
            if cursor == "0" {
                assert_eq!(endpoint.ids(&data), expected[..3]);
            } else {
                assert!(endpoint.ids(&data).is_empty());
                if let Endpoint::List = endpoint {
                    assert_eq!(data["seriesList"]["totalCount"], 7);
                    assert_eq!(data["seriesList"]["pageInfo"]["hasNextPage"], false);
                    assert!(data["seriesList"]["pageInfo"]["endCursor"].is_null());
                }
            }
        }
    }

    // The input error agrees with seriesData's existing cursor contract.
    for invalid in ["banana", "", "-1", "18446744073709551616"] {
        let cursor = serde_json::to_string(invalid).unwrap();
        let response = schema
            .execute(format!(
                "{{ seriesData(seriesId: \"{}\", after: {cursor}) {{ nodes {{ date }} }} }}",
                Uuid::new_v4()
            ))
            .await;
        assert_eq!(response.errors.len(), 1);
        assert_bad_user_input(&response.errors[0]);
        assert_eq!(
            response.errors[0].message,
            format!("`after` is not a valid cursor: {invalid:?}")
        );
        assert_eq!(response.data, async_graphql::Value::Null);
    }
}

/// The app-plane user list must not silently restart at page one either.
#[tokio::test]
async fn users_pagination_rejects_invalid_after_and_handles_large_offsets() {
    use crate::graphql::{context::GraphQLContext, schema::create_schema_with_data};
    use econ_graph_auth::Role;
    use econ_graph_core::{models::User, schema::users};
    use std::sync::Arc;

    let db = ScratchDatabase::create("user_pagination");
    run_migrations(&db.url).await.unwrap();
    let pool = create_pool(&db.url).await.unwrap();
    let caller = {
        use diesel::{QueryDsl, SelectableHelper};
        use diesel_async::RunQueryDsl;

        let mut conn = pool.get().await.unwrap();
        diesel::sql_query(
            "INSERT INTO users (email, name) VALUES ('cursor@example.test', 'Cursor caller')",
        )
        .execute(&mut conn)
        .await
        .unwrap();
        users::table
            .select(User::as_select())
            .first::<User>(&mut conn)
            .await
            .unwrap()
    };
    let schema = create_schema_with_data(
        AppPool(pool),
        None,
        Arc::new(GraphQLContext::signed_in(
            caller.clone(),
            [Role::AdminUsersRead],
        )),
    );
    for invalid in ["banana", "", "-1", "9223372036854775808"] {
        let after = serde_json::to_string(invalid).unwrap();
        let response = schema
            .execute(format!(
                "{{ users(pagination: {{ first: 1, after: {after} }}) {{ nodes {{ id }} }} }}"
            ))
            .await;
        assert_eq!(response.errors.len(), 1);
        assert_bad_user_input(&response.errors[0]);
        assert_eq!(response.data, async_graphql::Value::Null);
    }
    for after in ["0", "1", "99", "9223372036854775807"] {
        let response = schema
            .execute(format!(
                "{{ users(pagination: {{ first: 1, after: \"{after}\" }}) \
                 {{ nodes {{ id }} totalCount pageInfo {{ hasNextPage }} }} }}"
            ))
            .await;
        assert!(response.errors.is_empty(), "{after}: {:?}", response.errors);
        let data = response.data.into_json().unwrap();
        assert_eq!(data["users"]["totalCount"], 1);
        assert_eq!(data["users"]["pageInfo"]["hasNextPage"], false);
        let nodes = data["users"]["nodes"].as_array().unwrap();
        if after == "0" {
            assert_eq!(nodes[0]["id"], caller.id.to_string());
        } else {
            assert!(nodes.is_empty());
        }
    }
}

fn assert_bad_user_input(error: &async_graphql::ServerError) {
    assert_eq!(
        error.extensions.as_ref().and_then(|ext| ext.get("code")),
        Some(&async_graphql::Value::from("BAD_USER_INPUT"))
    );
    assert!(error.message.contains("`after`"));
}
