use super::{bearer, database, unreachable_pool};
use crate::{build_routes, Databases};
use econ_graph_auth::oidc::MIN_REFRESH_INTERVAL;
use econ_graph_auth::testkit::{SigningKey, TestIssuer};
use econ_graph_auth::OidcVerifier;
use econ_graph_core::AppPool;
use econ_graph_graphql::graphql::schema::create_schema_with_data;
use econ_graph_services::services::series_check::DataPlaneClient;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;
use warp::Filter;

/// Build the production HTTP routes with the supplied app pool and OIDC verifier.
fn routes(
    pool: AppPool,
    verifier: Arc<OidcVerifier>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    routes_with_data_plane(pool, verifier, None)
}

/// As [`routes`], with annotations' series checked through `data_plane`.
fn routes_with_data_plane(
    pool: AppPool,
    verifier: Arc<OidcVerifier>,
    data_plane: Option<Arc<DataPlaneClient>>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let databases = Databases {
        app: pool,
        data: None,
    };
    let schema = create_schema_with_data(databases.app.clone(), None, ());
    build_routes(databases, data_plane, schema, Some(verifier), &[], false)
}

/// A stand-in data plane that places every series in the FRED catalog.
fn fred_data_plane() -> Arc<DataPlaneClient> {
    let answer = warp::post().map(|| {
        warp::reply::json(&json!({"data": {
            "seriesCatalog": "Federal Reserve Economic Data (FRED)",
            "seriesExists": true,
        }}))
    });
    let (addr, server) = warp::serve(answer).bind_ephemeral(([127, 0, 0, 1], 0));
    tokio::spawn(server);
    Arc::new(
        DataPlaneClient::new(&econ_graph_core::config::DataGraphqlConfig {
            forward_authorization: false,
            allow_insecure_authorization: false,
            url: format!("http://{addr}/data/graphql"),
            host: None,
            timeout: Duration::from_secs(5),
        })
        .unwrap(),
    )
}

/// Send a bearer-authenticated GraphQL POST and decode its HTTP status and JSON body.
async fn post<F>(routes: &F, token: &str, query: &str) -> (u16, Value)
where
    F: Filter + Clone + 'static,
    F::Extract: warp::Reply,
{
    let reply = warp::test::request()
        .method("POST")
        .path("/graphql")
        .header("authorization", bearer(token))
        .json(&json!({ "query": query }))
        .reply(routes)
        .await;
    if reply.status() == 401 {
        assert_eq!(
            reply.headers()["www-authenticate"],
            r#"Bearer error="invalid_token""#
        );
    }
    (
        reply.status().as_u16(),
        serde_json::from_slice(reply.body()).unwrap(),
    )
}

/// Require the complete invalid-token response, including HTTP 401 and no data.
fn invalid((status, body): (u16, Value)) {
    assert_eq!(status, 401, "{body}");
    assert_eq!(
        body,
        json!({
            "data": null,
            "errors": [{"message": "Invalid or expired access token",
                        "extensions": {"code": "UNAUTHENTICATED"}}]
        })
    );
}

/// Reproduce ECO-512 over the production HTTP filter, including the default throttle.
/// All tokens and RSA keys are prepared before the first request starts the window.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_kids_stay_401_within_and_after_refresh_throttle() {
    let issuer = TestIssuer::start().await;
    let claims = issuer.claims(Uuid::new_v4(), &["annotation:create"]);
    let key = SigningKey::generate("never-published");
    let token = key.sign(&claims);
    let other = SigningKey::generate("another-unknown").sign(&claims);
    let mut foreign = claims.clone();
    foreign["iss"] = json!("https://foreign.example/realms/other");
    let foreign_unknown = key.sign(&foreign);
    let foreign_known = issuer.sign(&foreign);
    let verifier = Arc::new(
        issuer
            .verifier()
            .with_min_refresh_interval(MIN_REFRESH_INTERVAL),
    );
    let routes = routes(unreachable_pool(), verifier.clone());
    let write = format!(
        r#"mutation {{ deleteAnnotation(input: {{ annotationId: "{}" }}) }}"#,
        Uuid::new_v4()
    );
    let private = format!(
        r#"{{ annotationsForSeries(seriesId: "{}") {{ id description }} }}"#,
        Uuid::new_v4()
    );

    for _ in 0..3 {
        for token in [&token, &other, &foreign_unknown, &foreign_known] {
            for query in ["{ __typename }", &write, &private] {
                invalid(post(&routes, token, query).await);
            }
        }
    }
    assert_eq!(
        issuer.jwks_requests(),
        1,
        "negative lookups must be throttled"
    );
    // An invalid kid must not poison the valid key cache.
    verifier.verify(&issuer.sign(&claims)).await.unwrap();
    tokio::time::sleep(MIN_REFRESH_INTERVAL + Duration::from_millis(50)).await;
    for _ in 0..3 {
        invalid(post(&routes, &token, "{ __typename }").await);
    }
    assert_eq!(issuer.jwks_requests(), 2);
}

/// With real app rows, check valid sign-in and rotation as well as the anonymous outage
/// fallback's read/write boundaries. ScratchDatabase keeps existing databases untouched.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial]
async fn rotation_and_provider_outage_preserve_annotation_boundaries() {
    let Some((_db, pool)) = database().await else {
        return;
    };
    let issuer = TestIssuer::start().await;
    let sub = Uuid::new_v4();
    let claims = issuer.claims(
        sub,
        &[
            "annotation:create",
            "annotation:delete",
            "catalog.fred:read",
        ],
    );
    let valid = issuer.sign(&claims);
    let unknown = SigningKey::generate("missing").sign(&claims);
    let interval = Duration::from_millis(250);
    let verifier = Arc::new(issuer.verifier().with_min_refresh_interval(interval));
    let data_plane = fred_data_plane();
    let live = routes_with_data_plane(pool.clone(), verifier.clone(), Some(data_plane.clone()));
    let series = Uuid::new_v4();
    let create = format!(
        r#"mutation {{ createAnnotation(input: {{ seriesId: "{series}", annotationDate: "2024-01-01", title: "private", content: "secret", annotationType: "note", isPublic: false }}) {{ id }} }}"#
    );
    let read = format!(r#"{{ annotationsForSeries(seriesId: "{series}") {{ id description }} }}"#);
    let (status, body) = post(&live, &valid, &create).await;
    assert_eq!(status, 200);
    assert!(body.get("errors").is_none(), "{body}");
    let id = body["data"]["createAnnotation"]["id"].as_str().unwrap();
    let delete = format!(r#"mutation {{ deleteAnnotation(input: {{ annotationId: "{id}" }}) }}"#);
    assert_eq!(
        post(&live, &valid, &read).await.1["data"]["annotationsForSeries"][0]["description"],
        "secret"
    );
    // A new key first misses a healthy JWKS, then becomes published under the same kid.
    let rotated_key = SigningKey::generate("rotating");
    let rotated = rotated_key.sign(&claims);
    tokio::time::sleep(interval).await;
    invalid(post(&live, &rotated, "{ __typename }").await);
    issuer.publish(Arc::new(rotated_key), true);
    let fetches = issuer.jwks_requests();
    invalid(post(&live, &rotated, "{ __typename }").await);
    assert_eq!(
        issuer.jwks_requests(),
        fetches,
        "rotation is initially throttled"
    );
    // The same negatively cached kid is accepted once refresh is allowed.
    tokio::time::sleep(interval + Duration::from_millis(20)).await;
    let (status, body) = post(&live, &rotated, &read).await;
    assert_eq!(status, 200);
    assert_eq!(
        body["data"]["annotationsForSeries"][0]["description"],
        "secret"
    );

    for query in ["{ __typename }", &read, &delete] {
        for _ in 0..3 {
            invalid(post(&live, &unknown, query).await);
        }
    }
    // A verifier with no keys reaches an actual outage and falls back anonymously.
    let outage = routes_with_data_plane(
        pool,
        Arc::new(issuer.verifier().with_min_refresh_interval(interval)),
        Some(data_plane),
    );
    issuer.stop().await;
    for _ in 0..3 {
        assert_eq!(
            post(&outage, &valid, "{ __typename }").await,
            (200, json!({"data": {"__typename": "Query"}}))
        );
        let (status, body) = post(&outage, &valid, &read).await;
        assert_eq!(status, 200);
        assert_eq!(body, json!({"data": {"annotationsForSeries": []}}));
        let (status, body) = post(&outage, &valid, &delete).await;
        assert_eq!(status, 200);
        assert!(body["data"].is_null());
        assert_eq!(body["errors"][0]["message"], "Authentication required");
    }
    // Cached valid keys still authenticate while the provider is down; refused writes
    // have left the private row intact.
    assert_eq!(
        post(&live, &valid, &read).await.1["data"]["annotationsForSeries"][0]["description"],
        "secret"
    );
}
