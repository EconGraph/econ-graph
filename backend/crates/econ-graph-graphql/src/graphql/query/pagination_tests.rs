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
                assert_eq!(
                    response.errors[0].message,
                    format!("Invalid cursor: {invalid}")
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
        assert_eq!(
            response.errors[0].message,
            format!("Invalid cursor: {invalid}")
        );
        assert_eq!(response.data, async_graphql::Value::Null);
    }
}
