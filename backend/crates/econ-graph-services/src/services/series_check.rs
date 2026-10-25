//! Whether a series exists, for writes in the app database that name one (a new annotation's
//! series). `chart_annotations.series_id` has no foreign key, since the series may be in another
//! database, so this check is all that stops an annotation on a series that doesn't exist.
//!
//! The backend checks through the data database when it has one ([`SeriesCheck::Database`]),
//! otherwise through the data plane's GraphQL endpoint ([`SeriesCheck::DataPlane`],
//! `DATA_GRAPHQL_URL`), otherwise not at all ([`SeriesCheck::Skip`]). When the data plane
//! can't answer, the check fails closed with [`AppError::DataPlaneUnavailable`].

use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use econ_graph_core::{
    config::{redacted_url, DataGraphqlConfig},
    database::DatabasePool,
    error::{AppError, AppResult},
};

/// The data-plane query for "does this series exist". `seriesExists` answers from the
/// `economic_series` row alone, as [`SeriesCheck::Database`] does; `series(id)` would not do,
/// since it hides a series with no observations.
const SERIES_EXISTS_QUERY: &str = "query SeriesExists($id: ID!) { seriesExists(id: $id) }";

/// How [`SeriesCheck::series_exists`] finds out whether a series exists.
#[derive(Clone)]
pub enum SeriesCheck {
    /// Look it up in `economic_series` in the data database.
    Database(DatabasePool),
    /// Ask the data plane over HTTP.
    DataPlane(Arc<DataPlaneClient>),
    /// Don't check: this backend has neither a data database nor a data plane URL.
    Skip,
}

impl SeriesCheck {
    /// `Database` on `pool`, or `Skip` without one.
    pub fn from_pool(pool: Option<DatabasePool>) -> Self {
        pool.map_or(SeriesCheck::Skip, SeriesCheck::Database)
    }

    /// Whether the series `id` exists; `true` for [`SeriesCheck::Skip`].
    ///
    /// # Errors
    /// [`AppError::DataPlaneUnavailable`] when the data plane can't answer, or a database error
    /// from the data database.
    pub async fn series_exists(&self, id: Uuid) -> AppResult<bool> {
        match self {
            SeriesCheck::Database(pool) => {
                crate::services::series_service::series_exists(pool, id).await
            }
            SeriesCheck::DataPlane(client) => client.series_exists(id).await,
            SeriesCheck::Skip => Ok(true),
        }
    }
}

/// Asks the data plane's GraphQL endpoint about series, with no token: the gateway strips
/// credentials on that route anyway, and the data plane may not trust this backend's identity
/// provider.
#[derive(Clone)]
pub struct DataPlaneClient {
    http: reqwest::Client,
    url: String,
    /// `url` without anything that could be secret, for logs and error detail.
    log_url: String,
    host: Option<String>,
    timeout: Duration,
}

/// `error` and its sources, joined with ": ".
fn error_chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

/// Shows the redacted URL only.
impl std::fmt::Debug for DataPlaneClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataPlaneClient")
            .field("url", &self.log_url)
            .field("host", &self.host)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl DataPlaneClient {
    /// A client for `config`. Every request is bounded by `config.timeout` in all (connecting
    /// included) and follows no redirects.
    ///
    /// # Errors
    /// [`AppError::ConfigError`] when the HTTP client can't be built.
    pub fn new(config: &DataGraphqlConfig) -> AppResult<Self> {
        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| AppError::ConfigError(format!("data plane HTTP client: {e}")))?;
        Ok(Self {
            http,
            url: config.url.clone(),
            log_url: redacted_url(&config.url),
            host: config.host.clone(),
            timeout: config.timeout,
        })
    }

    /// The URL this client asks, as it may be logged ([`redacted_url`]): no user name,
    /// password, query string or fragment.
    pub fn log_url(&self) -> &str {
        &self.log_url
    }

    /// The `Host` header this client sends, when it overrides the URL's.
    pub fn host(&self) -> Option<&str> {
        self.host.as_deref()
    }

    /// How long one request may take.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Whether the data plane has the series `id`.
    ///
    /// # Errors
    /// [`AppError::DataPlaneUnavailable`] when the request fails or times out, the answer is
    /// not a success, or it is not a GraphQL response with a boolean `data.seriesExists` and no
    /// errors.
    pub async fn series_exists(&self, id: Uuid) -> AppResult<bool> {
        let unavailable = |detail: String| {
            tracing::warn!(series_id = %id, url = %self.log_url, "series check failed: {detail}");
            AppError::DataPlaneUnavailable(detail)
        };
        let mut request = self.http.post(&self.url).json(&serde_json::json!({
            "query": SERIES_EXISTS_QUERY,
            "operationName": "SeriesExists",
            "variables": { "id": id.to_string() },
        }));
        if let Some(host) = &self.host {
            request = request.header(reqwest::header::HOST, host);
        }
        let response = request.send().await.map_err(|e| {
            unavailable(if e.is_timeout() {
                format!("no answer within {:?}", self.timeout)
            } else {
                format!("request failed: {}", error_chain(&e.without_url()))
            })
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(unavailable(format!("HTTP {status}")));
        }
        let body: serde_json::Value = response.json().await.map_err(|e| {
            unavailable(format!(
                "unreadable answer: {}",
                error_chain(&e.without_url())
            ))
        })?;
        if let Some(errors) = body.get("errors").filter(|e| !e.is_null()) {
            return Err(unavailable(format!("GraphQL errors: {errors}")));
        }
        match body.pointer("/data/seriesExists") {
            Some(serde_json::Value::Bool(exists)) => Ok(*exists),
            _ => Err(unavailable(format!("unexpected answer: {body}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(url: String, host: Option<&str>, timeout: Duration) -> DataPlaneClient {
        DataPlaneClient::new(&DataGraphqlConfig {
            url,
            host: host.map(str::to_string),
            timeout,
        })
        .unwrap()
    }

    async fn answering(body: serde_json::Value) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        server
    }

    #[tokio::test]
    async fn asks_for_the_series_by_id_with_the_host_header_and_no_token() {
        let id = Uuid::new_v4();
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_partial_json(serde_json::json!({
                "operationName": "SeriesExists",
                "variables": { "id": id.to_string() },
            })))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "data": { "seriesExists": true } })),
            )
            .mount(&server)
            .await;
        let client = client(
            format!("{}/data/graphql", server.uri()),
            Some("br-1.localhost"),
            Duration::from_secs(2),
        );
        assert!(client.series_exists(id).await.unwrap());

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].url.path(), "/data/graphql");
        assert_eq!(requests[0].headers.get("host").unwrap(), "br-1.localhost");
        assert!(requests[0].headers.get("authorization").is_none());
        assert!(requests[0].headers.get("cookie").is_none());
    }

    #[tokio::test]
    async fn an_unknown_series_does_not_exist() {
        let server = answering(serde_json::json!({ "data": { "seriesExists": false } })).await;
        let client = client(server.uri(), None, Duration::from_secs(2));
        assert!(!client.series_exists(Uuid::new_v4()).await.unwrap());
    }

    /// Anything but a clean answer is "can't tell", never "doesn't exist" or "exists".
    #[tokio::test]
    async fn failures_are_unavailable() {
        let id = Uuid::new_v4();
        let mut servers = Vec::new();
        for template in [
            ResponseTemplate::new(503),
            ResponseTemplate::new(429),
            ResponseTemplate::new(302).insert_header("location", "http://elsewhere/"),
            ResponseTemplate::new(200).set_body_string("<html>gateway</html>"),
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": null,
                "errors": [{ "message": "Data is served by the data plane at /data/graphql" }]
            })),
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": {} })),
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "data": { "seriesExists": null } })),
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "data": { "seriesExists": "yes" } })),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(template)
                .mount(&server)
                .await;
            servers.push(server);
        }
        for server in &servers {
            let result = client(server.uri(), None, Duration::from_secs(2))
                .series_exists(id)
                .await;
            assert!(
                matches!(result, Err(AppError::DataPlaneUnavailable(_))),
                "{result:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_slow_or_down_data_plane_is_unavailable() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "data": { "seriesExists": false } }))
                    .set_delay(Duration::from_secs(5)),
            )
            .mount(&server)
            .await;
        let started = std::time::Instant::now();
        let result = client(server.uri(), None, Duration::from_millis(200))
            .series_exists(Uuid::new_v4())
            .await;
        assert!(matches!(result, Err(AppError::DataPlaneUnavailable(_))));
        assert!(started.elapsed() < Duration::from_secs(2));

        // Nothing listening.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let result = client(
            format!("http://127.0.0.1:{port}/data/graphql"),
            None,
            Duration::from_millis(500),
        )
        .series_exists(Uuid::new_v4())
        .await;
        assert!(matches!(result, Err(AppError::DataPlaneUnavailable(_))));
    }

    #[derive(Clone)]
    struct Buffer(Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for Buffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Even given a URL holding secrets (config refuses one, so this builds the client
    /// directly), a failed check's log line, its error detail and the client's `Debug` show
    /// only the redacted URL.
    #[tokio::test]
    async fn failed_checks_never_log_the_full_url() {
        let buffer = Buffer(Arc::new(std::sync::Mutex::new(Vec::new())));
        let writer = buffer.clone();
        let _guard = tracing::subscriber::set_default(
            tracing_subscriber::fmt()
                .with_ansi(false)
                .with_writer(move || writer.clone())
                .finish(),
        );

        let failing = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&failing)
            .await;
        let down_port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut details = Vec::new();
        for base in [failing.uri(), format!("http://127.0.0.1:{down_port}")] {
            let client = client(
                format!("{base}/data/graphql?api_key=s3cret#s3cret"),
                None,
                Duration::from_millis(500),
            );
            assert_eq!(client.log_url(), format!("{base}/data/graphql"));
            assert!(!format!("{client:?}").contains("s3cret"));
            let Err(AppError::DataPlaneUnavailable(detail)) =
                client.series_exists(Uuid::new_v4()).await
            else {
                panic!("expected DataPlaneUnavailable");
            };
            details.push(detail);
        }

        let logged = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
        assert_eq!(logged.matches("series check failed").count(), 2, "{logged}");
        assert!(logged.contains("/data/graphql"), "{logged}");
        assert!(!logged.contains("s3cret"), "{logged}");
        for detail in &details {
            assert!(!detail.contains("s3cret"), "{detail}");
        }
    }

    #[tokio::test]
    async fn skip_accepts_every_series() {
        assert!(SeriesCheck::Skip
            .series_exists(Uuid::new_v4())
            .await
            .unwrap());
    }
}
