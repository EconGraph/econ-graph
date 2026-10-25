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

    #[serial_test::serial]
    #[test]
    fn test_config_default() {
        // REQUIREMENT: The application should have sensible defaults for development
        // PURPOSE: Verify that default configuration values are set correctly for local development
        // This ensures developers can run the app without extensive configuration setup

        // Store original values to restore later
        let original_backend_port = env::var("BACKEND_PORT").ok();
        let original_frontend_port = env::var("FRONTEND_PORT").ok();
        let original_cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS").ok();

        // Clear environment variables to test true defaults
        env::remove_var("BACKEND_PORT");
        env::remove_var("FRONTEND_PORT");
        env::remove_var("CORS_ALLOWED_ORIGINS");

        let config = Config::default();

        // Verify default server binds to localhost - safe for development
        assert_eq!(config.server.host, "127.0.0.1");
        // Verify default port 9876 - non-traditional port to avoid conflicts
        assert_eq!(config.server.port, 9876);
        // Verify CORS allows React dev server - required for frontend development
        assert_eq!(config.cors.allowed_origins, vec!["http://localhost:3000"]);

        // Restore original environment variables
        if let Some(val) = original_backend_port {
            env::set_var("BACKEND_PORT", val);
        } else {
            env::remove_var("BACKEND_PORT");
        }
        if let Some(val) = original_frontend_port {
            env::set_var("FRONTEND_PORT", val);
        } else {
            env::remove_var("FRONTEND_PORT");
        }
        if let Some(val) = original_cors_allowed_origins {
            env::set_var("CORS_ALLOWED_ORIGINS", val);
        } else {
            env::remove_var("CORS_ALLOWED_ORIGINS");
        }
    }

    #[serial_test::serial]
    #[test]
    fn test_config_from_env() {
        // REQUIREMENT: The application should be configurable via environment variables for deployment
        // PURPOSE: Verify that configuration can be overridden using environment variables
        // This is essential for containerized deployments and different environments

        // Store original values to restore later
        let original_backend_port = env::var("BACKEND_PORT").ok();
        let original_frontend_port = env::var("FRONTEND_PORT").ok();

        // Clear any existing environment variables first to ensure clean test state
        env::remove_var("BACKEND_PORT");
        env::remove_var("FRONTEND_PORT");

        // Set test environment variables
        env::set_var("BACKEND_PORT", "9000");
        env::set_var("FRONTEND_PORT", "4000");

        let config = Config::from_env().unwrap();

        // Verify environment variables override defaults - required for deployment flexibility
        assert_eq!(config.server.port, 9000);
        assert_eq!(config.cors.allowed_origins, vec!["http://localhost:4000"]);

        // Restore original environment variables
        if let Some(val) = original_backend_port {
            env::set_var("BACKEND_PORT", val);
        } else {
            env::remove_var("BACKEND_PORT");
        }
        if let Some(val) = original_frontend_port {
            env::set_var("FRONTEND_PORT", val);
        } else {
            env::remove_var("FRONTEND_PORT");
        }
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

    /// The process-environment entry points read the same variables. `DATABASE_URL` is left
    /// alone: other tests in this binary connect through it.
    #[serial_test::serial]
    #[test]
    fn database_urls_read_the_process_environment() {
        let saved: Vec<_> = [APP_DATABASE_URL, DATA_DATABASE_URL]
            .into_iter()
            .map(|k| (k, env::var(k).ok()))
            .collect();

        env::set_var(APP_DATABASE_URL, "postgres://app/econ_graph_app");
        env::set_var(DATA_DATABASE_URL, "postgres://data/econ_graph");
        let set = (backend_database_urls(), data_database_url());

        for (k, v) in saved {
            match v {
                Some(v) => env::set_var(k, v),
                None => env::remove_var(k),
            }
        }

        assert_eq!(
            set.0.map(|u| (u.app, u.data)),
            Ok((
                "postgres://app/econ_graph_app".to_string(),
                Some("postgres://data/econ_graph".to_string())
            ))
        );
        assert_eq!(set.1.as_deref(), Ok("postgres://data/econ_graph"));
    }
}
