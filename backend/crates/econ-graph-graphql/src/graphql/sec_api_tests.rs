use super::{context::GraphQLContext, Mutation, Query};
use async_graphql::{EmptySubscription, Request, Schema, Variables};
use serde_json::json;
use std::sync::Arc;

/// Validate the actual client documents against the compiled schema, without a database or SEC network calls.
#[tokio::test]
async fn sec_api_client_documents_validate_and_mutations_require_authentication() {
    let schema = Schema::build(Query, Mutation, EmptySubscription)
        .data(Arc::new(GraphQLContext::new(None)))
        .finish();
    let sources = [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../admin-frontend/src/hooks/useSecCrawler.ts"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../admin-frontend/src/hooks/useCompanySearch.ts"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../frontend/src/hooks/useFinancialData.ts"
        )),
    ];
    let mut document_count = 0;
    for source in sources {
        for block in source.split("gql`").skip(1) {
            let document = block.split('`').next().unwrap();
            let input = if document.contains("TriggerSecCrawl") {
                json!({"cik":"0000320193", "formTypes":"10-K", "startDate":"2023-01-01", "endDate":"2023-12-31", "excludeAmended":true, "excludeRestated":false, "maxFileSize":1024})
            } else if document.contains("ImportSecRss") {
                json!({"rssUrl":"https://www.sec.gov/feed", "maxFilings":10, "formTypes":"10-K"})
            } else {
                json!({"query":"Apple", "limit":10, "includeInactive":false})
            };
            let variables = Variables::from_json(json!({
                "input": input, "id":"00000000-0000-0000-0000-000000000001",
                "companyId":"00000000-0000-0000-0000-000000000001", "pagination":{"first":10}
            }));
            let response = schema
                .execute(Request::new(document).variables(variables))
                .await;
            assert_eq!(
                response.errors.len(),
                1,
                "{document}: {:?}",
                response.errors
            );
            // Resolver errors have a path; validation errors occur before execution and do not.
            assert!(
                !response.errors[0].path.is_empty(),
                "{document}: {:?}",
                response.errors
            );
            if document.contains("mutation ") {
                assert_eq!(response.errors[0].message, "Authentication required");
            } else {
                assert!(
                    response.errors[0].message.contains("DatabasePool")
                        || response.errors[0].message.contains("Pool<"),
                    "{:?}",
                    response.errors
                );
            }
            document_count += 1;
        }
    }
    assert_eq!(document_count, 6);
}
