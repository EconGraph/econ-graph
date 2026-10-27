use serde::{Deserialize, Serialize};
use std::env;

/// Application configuration. The database URLs are read separately: [`backend_database_urls`]
/// and [`data_database_url`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub cors: CorsConfig,
    pub crawler: CrawlerConfig,
    pub rate_limits: RateLimitConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    pub allowed_origins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlerConfig {
    pub fred_api_key: Option<String>,
    pub bls_api_key: Option<String>,
    pub max_concurrent_jobs: usize,
    pub queue_poll_interval_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub fred_rate_limit_per_minute: u32,
    pub bls_rate_limit_per_minute: u32,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        // Load .env file if it exists
        dotenvy::dotenv().ok();

        Ok(Config {
            server: ServerConfig {
                host: env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: env::var("BACKEND_PORT")
                    .unwrap_or_else(|_| "9876".to_string())
                    .parse()
                    .unwrap_or(9876),
            },

            cors: CorsConfig {
                allowed_origins: env::var("CORS_ALLOWED_ORIGINS")
                    .unwrap_or_else(|_| {
                        format!(
                            "http://localhost:{}",
                            env::var("FRONTEND_PORT").unwrap_or_else(|_| "3000".to_string())
                        )
                    })
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect(),
            },

            crawler: CrawlerConfig {
                fred_api_key: env::var("FRED_API_KEY").ok(),
                bls_api_key: env::var("BLS_API_KEY").ok(),
                max_concurrent_jobs: env::var("MAX_CONCURRENT_JOBS")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .unwrap_or(10),
                queue_poll_interval_seconds: env::var("QUEUE_POLL_INTERVAL_SECONDS")
                    .unwrap_or_else(|_| "5".to_string())
                    .parse()
                    .unwrap_or(5),
            },

            rate_limits: RateLimitConfig {
                fred_rate_limit_per_minute: env::var("FRED_RATE_LIMIT_PER_MINUTE")
                    .unwrap_or_else(|_| "120".to_string())
                    .parse()
                    .unwrap_or(120),
                bls_rate_limit_per_minute: env::var("BLS_RATE_LIMIT_PER_MINUTE")
                    .unwrap_or_else(|_| "500".to_string())
                    .parse()
                    .unwrap_or(500),
            },
        })
    }
}

/// The app database's URL variable: users and everything they create.
pub const APP_DATABASE_URL: &str = "APP_DATABASE_URL";

/// The data database's URL variable: sources, series, observations, crawl state, SEC data.
pub const DATA_DATABASE_URL: &str = "DATA_DATABASE_URL";

/// The single-database URL variable, which the binaries that read [`APP_DATABASE_URL`] and
/// [`DATA_DATABASE_URL`] refuse to fall back to.
pub const LEGACY_DATABASE_URL: &str = "DATABASE_URL";

/// Why a binary could not find its database URL.
#[derive(Clone, PartialEq, Eq, thiserror::Error)]
pub enum DatabaseUrlError {
    /// `DATABASE_URL` is set and the binary's own variable is not: the configuration predates the
    /// separate app and data databases, and guessing which one it names could write data into the
    /// wrong database.
    #[error("DATABASE_URL is no longer read; set APP_DATABASE_URL and/or DATA_DATABASE_URL")]
    LegacyDatabaseUrl,
    /// The binary's variable is unset and so is `DATABASE_URL`.
    #[error("{0} is not set")]
    Missing(&'static str),
}

/// The message itself, so a `main` that returns this error prints the documented startup error
/// rather than a variant name.
impl std::fmt::Debug for DatabaseUrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

/// The database URLs the backend reads: the app database always, the data database where the
/// deployment has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendDatabaseUrls {
    /// `APP_DATABASE_URL`.
    pub app: String,
    /// `DATA_DATABASE_URL`, or `None` when unset.
    pub data: Option<String>,
}

/// The backend's database URLs from the environment. `APP_DATABASE_URL` is required and
/// `DATA_DATABASE_URL` optional; both may name the same database. Empty values count as unset.
pub fn backend_database_urls() -> Result<BackendDatabaseUrls, DatabaseUrlError> {
    backend_database_urls_from(|name| env::var(name).ok())
}

/// The data database's URL from the environment, for the binaries that use only the data
/// database (`crawler`, `crawler-worker`, `seed-fixtures`, `sec-crawler`, `sec-company-crawler`).
/// Empty values count as unset.
pub fn data_database_url() -> Result<String, DatabaseUrlError> {
    data_database_url_from(|name| env::var(name).ok())
}

/// [`backend_database_urls`] with the environment read through `var`.
pub fn backend_database_urls_from(
    var: impl Fn(&str) -> Option<String>,
) -> Result<BackendDatabaseUrls, DatabaseUrlError> {
    Ok(BackendDatabaseUrls {
        app: required_url(&var, APP_DATABASE_URL)?,
        data: non_empty(&var, DATA_DATABASE_URL),
    })
}

/// [`data_database_url`] with the environment read through `var`.
pub fn data_database_url_from(
    var: impl Fn(&str) -> Option<String>,
) -> Result<String, DatabaseUrlError> {
    required_url(&var, DATA_DATABASE_URL)
}

/// The backend's app database pool size variable.
pub const APP_DB_POOL_SIZE: &str = "APP_DB_POOL_SIZE";

/// The backend's data database pool size variable.
pub const DATA_DB_POOL_SIZE: &str = "DATA_DB_POOL_SIZE";

/// Default connections in the backend's app database pool.
///
/// With the data pool's default, a backend opens at most 20 connections, as many as when it had
/// one pool. In Kubernetes (2 backend replicas plus 1 during a rolling update, and the
/// crawler-worker's 20) that stays at 80, under Postgres's `max_connections = 100`
/// (`k8s/manifests/postgresql-config.yaml`), even when both databases are on one server.
pub const DEFAULT_APP_DB_POOL_SIZE: u32 = 10;

/// Default connections in the backend's data database pool; see [`DEFAULT_APP_DB_POOL_SIZE`].
pub const DEFAULT_DATA_DB_POOL_SIZE: u32 = 10;

/// The backend's pool sizes. When `APP_DATABASE_URL` and `DATA_DATABASE_URL` are the same, the
/// backend opens one pool of `app + data` connections for both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendPoolSizes {
    /// `APP_DB_POOL_SIZE`, default [`DEFAULT_APP_DB_POOL_SIZE`].
    pub app: u32,
    /// `DATA_DB_POOL_SIZE`, default [`DEFAULT_DATA_DB_POOL_SIZE`].
    pub data: u32,
}

/// A pool size variable that is not a positive whole number.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{name} must be a positive whole number, not {value:?}")]
pub struct PoolSizeError {
    /// The variable.
    pub name: &'static str,
    /// Its value.
    pub value: String,
}

/// The backend's pool sizes from the environment. Unset or empty values take the defaults.
pub fn backend_pool_sizes() -> Result<BackendPoolSizes, PoolSizeError> {
    backend_pool_sizes_from(|name| env::var(name).ok())
}

/// [`backend_pool_sizes`] with the environment read through `var`.
pub fn backend_pool_sizes_from(
    var: impl Fn(&str) -> Option<String>,
) -> Result<BackendPoolSizes, PoolSizeError> {
    let size = |name: &'static str, default: u32| match non_empty(&var, name) {
        None => Ok(default),
        Some(value) => match value.trim().parse::<u32>() {
            Ok(n) if n > 0 => Ok(n),
            _ => Err(PoolSizeError { name, value }),
        },
    };
    Ok(BackendPoolSizes {
        app: size(APP_DB_POOL_SIZE, DEFAULT_APP_DB_POOL_SIZE)?,
        data: size(DATA_DB_POOL_SIZE, DEFAULT_DATA_DB_POOL_SIZE)?,
    })
}

/// In-cluster URL of the data plane's GraphQL endpoint, for the backend's server-side data
/// checks when it has no [`DATA_DATABASE_URL`].
pub const DATA_GRAPHQL_URL: &str = "DATA_GRAPHQL_URL";

/// `Host` header sent with [`DATA_GRAPHQL_URL`] requests: the backend's own route hostname, so a
/// request through the shared gateway takes the same route as the browser's.
pub const DATA_GRAPHQL_HOST: &str = "DATA_GRAPHQL_HOST";

/// Opt into forwarding verified user credentials to a trusted data plane.
pub const DATA_GRAPHQL_FORWARD_AUTHORIZATION: &str = "DATA_GRAPHQL_FORWARD_AUTHORIZATION";

/// Explicit exception for caller forwarding over a trusted private HTTP transport.
pub const DATA_GRAPHQL_ALLOW_INSECURE_AUTHORIZATION: &str =
    "DATA_GRAPHQL_ALLOW_INSECURE_AUTHORIZATION";

/// How long, in milliseconds, a [`DATA_GRAPHQL_URL`] request may take in all.
pub const DATA_GRAPHQL_TIMEOUT_MS: &str = "DATA_GRAPHQL_TIMEOUT_MS";

/// Default for [`DATA_GRAPHQL_TIMEOUT_MS`]: long enough for a one-row lookup through the
/// gateway, short enough that someone saving an annotation isn't left waiting on an outage.
pub const DEFAULT_DATA_GRAPHQL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Where the backend asks the data plane over HTTP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataGraphqlConfig {
    /// Forward the verified caller token only when the target trusts the same issuer/audience.
    pub forward_authorization: bool,
    /// Permit plaintext HTTP only when the operator explicitly trusts the private transport.
    pub allow_insecure_authorization: bool,
    /// `DATA_GRAPHQL_URL`: an `http` or `https` URL.
    pub url: String,
    /// `DATA_GRAPHQL_HOST`, or `None` to send the URL's own host.
    pub host: Option<String>,
    /// `DATA_GRAPHQL_TIMEOUT_MS`, default [`DEFAULT_DATA_GRAPHQL_TIMEOUT`].
    pub timeout: std::time::Duration,
}

/// Why the data plane's GraphQL settings are unusable.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DataGraphqlConfigError {
    /// `DATA_GRAPHQL_URL` is not an `http` or `https` URL with a host. The value is not echoed:
    /// a malformed URL may still hold a secret.
    #[error("DATA_GRAPHQL_URL is not an http(s) URL with a host")]
    InvalidUrl,
    /// `DATA_GRAPHQL_URL` has a user name or password. The check sends no credentials, and the
    /// URL is logged.
    #[error("DATA_GRAPHQL_URL must not contain a user name or password")]
    UrlWithCredentials,
    /// `DATA_GRAPHQL_URL` has a query string or fragment. The endpoint needs neither, and either
    /// could carry a secret into the logs.
    #[error("DATA_GRAPHQL_URL must not contain a query string or fragment")]
    UrlWithQueryOrFragment,
    /// `DATA_GRAPHQL_HOST` is not a host name (with an optional port).
    #[error("DATA_GRAPHQL_HOST={0:?} is not a host name")]
    InvalidHost(String),
    /// `DATA_GRAPHQL_TIMEOUT_MS` is not a positive whole number of milliseconds.
    #[error("DATA_GRAPHQL_TIMEOUT_MS={0:?} is not a positive number of milliseconds")]
    InvalidTimeout(String),
    /// `DATA_GRAPHQL_HOST` or `DATA_GRAPHQL_TIMEOUT_MS` is set without `DATA_GRAPHQL_URL`.
    #[error("{0} is set but DATA_GRAPHQL_URL is not")]
    WithoutUrl(&'static str),
    /// Forwarding accepts only true or false.
    #[error("DATA_GRAPHQL_FORWARD_AUTHORIZATION must be true or false")]
    InvalidForwardAuthorization,
    /// Plaintext bearer forwarding requires a separate explicit transport exception.
    #[error("Bearer forwarding requires HTTPS; trusted private HTTP needs DATA_GRAPHQL_ALLOW_INSECURE_AUTHORIZATION=true")]
    InsecureForwardAuthorization,
    /// The transport exception accepts only true or false.
    #[error("DATA_GRAPHQL_ALLOW_INSECURE_AUTHORIZATION must be true or false")]
    InvalidInsecureAuthorization,
}

/// `url` as it may be logged: `scheme://host[:port]/path`, without any user name, password,
/// query string or fragment. A value that doesn't parse as a URL is replaced entirely.
pub fn redacted_url(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(parsed) => {
            let host = parsed.host_str().unwrap_or_default();
            match parsed.port() {
                Some(port) => format!("{}://{host}:{port}{}", parsed.scheme(), parsed.path()),
                None => format!("{}://{host}{}", parsed.scheme(), parsed.path()),
            }
        }
        Err(_) => "<unparseable URL>".to_string(),
    }
}

/// The data plane's GraphQL settings from the environment, or `None` when `DATA_GRAPHQL_URL`
/// is unset. Empty values count as unset.
pub fn data_graphql_config() -> Result<Option<DataGraphqlConfig>, DataGraphqlConfigError> {
    data_graphql_config_from(|name| env::var(name).ok())
}

/// [`data_graphql_config`] with the environment read through `var`.
pub fn data_graphql_config_from(
    var: impl Fn(&str) -> Option<String>,
) -> Result<Option<DataGraphqlConfig>, DataGraphqlConfigError> {
    let forwarding = non_empty(&var, DATA_GRAPHQL_FORWARD_AUTHORIZATION);
    let forward_authorization = match forwarding.as_deref().map(str::trim) {
        None | Some("false") => false,
        Some("true") => true,
        _ => return Err(DataGraphqlConfigError::InvalidForwardAuthorization),
    };
    let allow_insecure_authorization =
        match non_empty(&var, DATA_GRAPHQL_ALLOW_INSECURE_AUTHORIZATION)
            .as_deref()
            .map(str::trim)
        {
            None | Some("false") => false,
            Some("true") => true,
            _ => return Err(DataGraphqlConfigError::InvalidInsecureAuthorization),
        };
    let host = non_empty(&var, DATA_GRAPHQL_HOST).map(|h| h.trim().to_string());
    let timeout = non_empty(&var, DATA_GRAPHQL_TIMEOUT_MS);
    let Some(url) = non_empty(&var, DATA_GRAPHQL_URL).map(|u| u.trim().to_string()) else {
        if forward_authorization {
            return Err(DataGraphqlConfigError::WithoutUrl(
                DATA_GRAPHQL_FORWARD_AUTHORIZATION,
            ));
        }
        if host.is_some() {
            return Err(DataGraphqlConfigError::WithoutUrl(DATA_GRAPHQL_HOST));
        }
        if timeout.is_some() {
            return Err(DataGraphqlConfigError::WithoutUrl(DATA_GRAPHQL_TIMEOUT_MS));
        }
        return Ok(None);
    };
    let parsed = reqwest::Url::parse(&url).map_err(|_| DataGraphqlConfigError::InvalidUrl)?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(DataGraphqlConfigError::InvalidUrl);
    }
    if forward_authorization && !allow_insecure_authorization && parsed.scheme() != "https" {
        return Err(DataGraphqlConfigError::InsecureForwardAuthorization);
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(DataGraphqlConfigError::UrlWithCredentials);
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(DataGraphqlConfigError::UrlWithQueryOrFragment);
    }
    if let Some(host) = &host {
        let valid = host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'));
        if !valid {
            return Err(DataGraphqlConfigError::InvalidHost(host.clone()));
        }
    }
    let timeout = match timeout {
        None => DEFAULT_DATA_GRAPHQL_TIMEOUT,
        Some(value) => match value.trim().parse::<u64>() {
            Ok(ms) if ms > 0 => std::time::Duration::from_millis(ms),
            _ => return Err(DataGraphqlConfigError::InvalidTimeout(value)),
        },
    };
    Ok(Some(DataGraphqlConfig {
        url,
        host,
        timeout,
        forward_authorization,
        allow_insecure_authorization,
    }))
}

fn non_empty(var: &impl Fn(&str) -> Option<String>, name: &str) -> Option<String> {
    var(name).filter(|v| !v.trim().is_empty())
}

fn required_url(
    var: &impl Fn(&str) -> Option<String>,
    name: &'static str,
) -> Result<String, DatabaseUrlError> {
    match non_empty(var, name) {
        Some(url) => Ok(url),
        None if non_empty(var, LEGACY_DATABASE_URL).is_some() => {
            Err(DatabaseUrlError::LegacyDatabaseUrl)
        }
        None => Err(DatabaseUrlError::Missing(name)),
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: env::var("BACKEND_PORT")
                    .unwrap_or_else(|_| "9876".to_string())
                    .parse()
                    .unwrap_or(9876),
            },
            cors: CorsConfig {
                allowed_origins: vec![format!(
                    "http://localhost:{}",
                    env::var("FRONTEND_PORT").unwrap_or_else(|_| "3000".to_string())
                )],
            },
            crawler: CrawlerConfig {
                fred_api_key: None,
                bls_api_key: None,
                max_concurrent_jobs: 5,
                queue_poll_interval_seconds: 10,
            },
            rate_limits: RateLimitConfig {
                fred_rate_limit_per_minute: 120,
                bls_rate_limit_per_minute: 500,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run the real environment-reading entry points in a fresh process. Command's
    /// environment belongs only to the child, so parallel tests never mutate the
    /// current process's environment (which is unsafe in Rust 2024).
    fn in_environment(test: &str, vars: &[(&str, &str)], check: impl FnOnce()) {
        const CHILD_TEST: &str = "ECON_GRAPH_CONFIG_CHILD_TEST";
        if env::var(CHILD_TEST).as_deref() == Ok(test) {
            check();
            return;
        }

        let directory = tempfile::tempdir().unwrap();
        // Stop dotenv from loading developer settings from a parent directory.
        std::fs::write(directory.path().join(".env"), "").unwrap();
        let output = std::process::Command::new(env::current_exe().unwrap())
            .args(["--exact", &format!("config::tests::{test}"), "--nocapture"])
            .current_dir(directory.path())
            .env_clear()
            .env(CHILD_TEST, test)
            .envs(vars.iter().copied())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{test} failed: {stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn test_config_default() {
        in_environment("test_config_default", &[], || {
            let config = Config::default();
            assert_eq!(config.server.host, "127.0.0.1");
            assert_eq!(config.server.port, 9876);
            assert_eq!(config.cors.allowed_origins, vec!["http://localhost:3000"]);
        });
    }

    #[test]
    fn test_config_from_env() {
        in_environment(
            "test_config_from_env",
            &[("BACKEND_PORT", "9000"), ("FRONTEND_PORT", "4000")],
            || {
                let config = Config::from_env().unwrap();
                assert_eq!(config.server.port, 9000);
                assert_eq!(config.cors.allowed_origins, vec!["http://localhost:4000"]);
            },
        );
    }

    /// Pool sizes default to 10 each, read positive whole numbers, and refuse anything else.
    #[test]
    fn backend_pool_sizes_default_and_parse() {
        assert_eq!(
            backend_pool_sizes_from(env_of(&[])),
            Ok(BackendPoolSizes { app: 10, data: 10 })
        );
        assert_eq!(
            backend_pool_sizes_from(env_of(&[
                ("APP_DB_POOL_SIZE", " 4 "),
                ("DATA_DB_POOL_SIZE", ""),
            ])),
            Ok(BackendPoolSizes { app: 4, data: 10 })
        );
        for bad in ["0", "-1", "ten", "2.5"] {
            assert_eq!(
                backend_pool_sizes_from(env_of(&[("DATA_DB_POOL_SIZE", bad)])),
                Err(PoolSizeError {
                    name: "DATA_DB_POOL_SIZE",
                    value: bad.to_string()
                })
            );
        }
    }

    /// An environment of exactly `vars`.
    fn env_of<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            vars.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_string())
        }
    }

    /// Every binary's database URLs: the backend's (app required, data optional) and the
    /// data-only binaries' (data required), including the startup error when only `DATABASE_URL`
    /// is set.
    #[test]
    fn database_urls_come_only_from_the_app_and_data_variables() {
        let legacy =
            "DATABASE_URL is no longer read; set APP_DATABASE_URL and/or DATA_DATABASE_URL";
        let only_legacy = [("DATABASE_URL", "postgres://db/econ_graph")];

        // Both set, to different databases or to the same one.
        let both = [
            ("APP_DATABASE_URL", "postgres://app/econ_graph_app"),
            ("DATA_DATABASE_URL", "postgres://data/econ_graph"),
            ("DATABASE_URL", "postgres://db/ignored"),
        ];
        assert_eq!(
            backend_database_urls_from(env_of(&both)),
            Ok(BackendDatabaseUrls {
                app: "postgres://app/econ_graph_app".into(),
                data: Some("postgres://data/econ_graph".into()),
            })
        );
        assert_eq!(
            data_database_url_from(env_of(&both)).as_deref(),
            Ok("postgres://data/econ_graph")
        );
        let same = [
            ("APP_DATABASE_URL", "postgres://db/econ_graph"),
            ("DATA_DATABASE_URL", "postgres://db/econ_graph"),
        ];
        let urls = backend_database_urls_from(env_of(&same)).unwrap();
        assert_eq!(urls.data.as_deref(), Some(urls.app.as_str()));

        // The backend runs without a data database; the data-only binaries do not.
        let app_only = [("APP_DATABASE_URL", "postgres://app/econ_graph_app")];
        assert_eq!(
            backend_database_urls_from(env_of(&app_only)),
            Ok(BackendDatabaseUrls {
                app: "postgres://app/econ_graph_app".into(),
                data: None,
            })
        );
        assert_eq!(
            data_database_url_from(env_of(&app_only)),
            Err(DatabaseUrlError::Missing("DATA_DATABASE_URL"))
        );

        // The backend needs an app database even when it has a data one.
        let data_only = [("DATA_DATABASE_URL", "postgres://data/econ_graph")];
        assert_eq!(
            backend_database_urls_from(env_of(&data_only)),
            Err(DatabaseUrlError::Missing("APP_DATABASE_URL"))
        );

        // DATABASE_URL alone is never used, and says what to set instead.
        for err in [
            backend_database_urls_from(env_of(&only_legacy)).unwrap_err(),
            data_database_url_from(env_of(&only_legacy)).unwrap_err(),
        ] {
            assert_eq!(err, DatabaseUrlError::LegacyDatabaseUrl);
            assert_eq!(err.to_string(), legacy);
        }
        let data_with_legacy = [
            ("DATA_DATABASE_URL", "postgres://data/econ_graph"),
            ("DATABASE_URL", "postgres://db/econ_graph"),
        ];
        assert_eq!(
            backend_database_urls_from(env_of(&data_with_legacy)),
            Err(DatabaseUrlError::LegacyDatabaseUrl)
        );

        // Nothing set at all; empty values count as unset.
        assert_eq!(
            data_database_url_from(env_of(&[])).unwrap_err().to_string(),
            "DATA_DATABASE_URL is not set"
        );
        assert_eq!(
            backend_database_urls_from(env_of(&[]))
                .unwrap_err()
                .to_string(),
            "APP_DATABASE_URL is not set"
        );
        let empty = [
            ("APP_DATABASE_URL", "postgres://app/econ_graph_app"),
            ("DATA_DATABASE_URL", ""),
            ("DATABASE_URL", "postgres://db/econ_graph"),
        ];
        assert_eq!(
            backend_database_urls_from(env_of(&empty)).unwrap().data,
            None
        );
        assert_eq!(
            data_database_url_from(env_of(&empty)),
            Err(DatabaseUrlError::LegacyDatabaseUrl)
        );
    }

    /// The process-environment entry points read the app and data URL variables.
    #[test]
    fn database_urls_read_the_process_environment() {
        in_environment(
            "database_urls_read_the_process_environment",
            &[
                (APP_DATABASE_URL, "postgres://app/econ_graph_app"),
                (DATA_DATABASE_URL, "postgres://data/econ_graph"),
            ],
            || {
                assert_eq!(
                    backend_database_urls().map(|u| (u.app, u.data)),
                    Ok((
                        "postgres://app/econ_graph_app".to_string(),
                        Some("postgres://data/econ_graph".to_string())
                    ))
                );
                assert_eq!(
                    data_database_url().as_deref(),
                    Ok("postgres://data/econ_graph")
                );
            },
        );
    }

    /// `var` for [`data_graphql_config_from`], reading `pairs`.
    fn vars<'a>(pairs: &'a [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn data_graphql_config_reads_url_host_and_timeout() {
        assert_eq!(data_graphql_config_from(vars(&[])), Ok(None));
        assert_eq!(
            data_graphql_config_from(vars(&[(DATA_GRAPHQL_URL, "  ")])),
            Ok(None)
        );
        assert_eq!(
            data_graphql_config_from(vars(&[
                (DATA_GRAPHQL_URL, "http://gw.example/data/graphql"),
                (DATA_GRAPHQL_HOST, "br-1.localhost"),
            ])),
            Ok(Some(DataGraphqlConfig {
                forward_authorization: false,
                allow_insecure_authorization: false,
                url: "http://gw.example/data/graphql".to_string(),
                host: Some("br-1.localhost".to_string()),
                timeout: DEFAULT_DATA_GRAPHQL_TIMEOUT,
            }))
        );
        assert_eq!(
            data_graphql_config_from(vars(&[
                (DATA_GRAPHQL_URL, "http://gw.example/data/graphql"),
                (DATA_GRAPHQL_TIMEOUT_MS, "250"),
            ]))
            .map(|c| c.map(|c| (c.host, c.timeout))),
            Ok(Some((None, std::time::Duration::from_millis(250))))
        );
    }

    #[test]
    fn data_graphql_config_refuses_unusable_settings() {
        let cases: [(&[(&str, &str)], DataGraphqlConfigError); 12] = [
            (
                &[(DATA_GRAPHQL_URL, "gw.example/data/graphql")],
                DataGraphqlConfigError::InvalidUrl,
            ),
            (
                &[(DATA_GRAPHQL_URL, "ftp://gw.example/")],
                DataGraphqlConfigError::InvalidUrl,
            ),
            (
                &[(DATA_GRAPHQL_URL, "http://user:s3cret@gw/data/graphql")],
                DataGraphqlConfigError::UrlWithCredentials,
            ),
            (
                &[(DATA_GRAPHQL_URL, "http://user@gw/data/graphql")],
                DataGraphqlConfigError::UrlWithCredentials,
            ),
            (
                &[(DATA_GRAPHQL_URL, "http://:s3cret@gw/data/graphql")],
                DataGraphqlConfigError::UrlWithCredentials,
            ),
            (
                &[(DATA_GRAPHQL_URL, "https://gw/data/graphql?api_key=s3cret")],
                DataGraphqlConfigError::UrlWithQueryOrFragment,
            ),
            (
                &[(DATA_GRAPHQL_URL, "https://gw/data/graphql?")],
                DataGraphqlConfigError::UrlWithQueryOrFragment,
            ),
            (
                &[(DATA_GRAPHQL_URL, "https://gw/data/graphql#s3cret")],
                DataGraphqlConfigError::UrlWithQueryOrFragment,
            ),
            (
                &[(DATA_GRAPHQL_URL, "http://gw/"), (DATA_GRAPHQL_HOST, "a b")],
                DataGraphqlConfigError::InvalidHost("a b".to_string()),
            ),
            (
                &[
                    (DATA_GRAPHQL_URL, "http://gw/"),
                    (DATA_GRAPHQL_TIMEOUT_MS, "0"),
                ],
                DataGraphqlConfigError::InvalidTimeout("0".to_string()),
            ),
            (
                &[(DATA_GRAPHQL_HOST, "localhost")],
                DataGraphqlConfigError::WithoutUrl(DATA_GRAPHQL_HOST),
            ),
            (
                &[(DATA_GRAPHQL_TIMEOUT_MS, "500")],
                DataGraphqlConfigError::WithoutUrl(DATA_GRAPHQL_TIMEOUT_MS),
            ),
        ];
        for (pairs, expected) in cases {
            let refused = data_graphql_config_from(vars(pairs));
            assert_eq!(refused, Err(expected));
            // The startup error never repeats a secret from the URL.
            assert!(!refused.unwrap_err().to_string().contains("s3cret"));
        }
    }

    #[test]
    fn redacted_url_keeps_only_scheme_host_port_and_path() {
        for (url, redacted) in [
            (
                "http://econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local/data/graphql",
                "http://econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local/data/graphql",
            ),
            (
                "https://user:s3cret@gw:8443/data/graphql?api_key=s3cret#s3cret",
                "https://gw:8443/data/graphql",
            ),
            ("http://127.0.0.1:9/graphql?", "http://127.0.0.1:9/graphql"),
            ("not a url s3cret", "<unparseable URL>"),
        ] {
            assert_eq!(redacted_url(url), redacted, "{url}");
        }
    }
}

#[cfg(test)]
mod data_plane_trust_tests {
    use super::*;

    #[test]
    fn data_plane_forwarding_requires_explicit_boolean_and_a_target() {
        let configured = |key: &str| match key {
            DATA_GRAPHQL_URL => Some("https://data.test/graphql".into()),
            DATA_GRAPHQL_FORWARD_AUTHORIZATION => Some("true".into()),
            _ => None,
        };
        assert!(
            data_graphql_config_from(configured)
                .unwrap()
                .unwrap()
                .forward_authorization
        );
        let plaintext = |key: &str| match key {
            DATA_GRAPHQL_URL => Some("http://private.test/graphql".into()),
            DATA_GRAPHQL_FORWARD_AUTHORIZATION => Some("true".into()),
            _ => None,
        };
        assert_eq!(
            data_graphql_config_from(plaintext),
            Err(DataGraphqlConfigError::InsecureForwardAuthorization)
        );
        let private_exception = |key: &str| {
            if key == DATA_GRAPHQL_ALLOW_INSECURE_AUTHORIZATION {
                Some("true".into())
            } else {
                plaintext(key)
            }
        };
        assert!(
            data_graphql_config_from(private_exception)
                .unwrap()
                .unwrap()
                .allow_insecure_authorization
        );
        assert_eq!(
            data_graphql_config_from(|key| if key == DATA_GRAPHQL_FORWARD_AUTHORIZATION {
                Some("maybe".into())
            } else {
                None
            }),
            Err(DataGraphqlConfigError::InvalidForwardAuthorization)
        );
        assert_eq!(
            data_graphql_config_from(|key| if key == DATA_GRAPHQL_FORWARD_AUTHORIZATION {
                Some("true".into())
            } else {
                None
            }),
            Err(DataGraphqlConfigError::WithoutUrl(
                DATA_GRAPHQL_FORWARD_AUTHORIZATION
            ))
        );
    }
}
