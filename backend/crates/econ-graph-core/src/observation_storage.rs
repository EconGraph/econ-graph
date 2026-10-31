//! Shared read/write selection for the observation migration. PostgreSQL remains the
//! rollback shadow when Iceberg is selected; a backend flag never authorizes dropping it.

use crate::{AppError, AppResult};
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Nullable, Text, Uuid as SqlUuid};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationBackend {
    Postgres,
    Iceberg,
}

impl ObservationBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Iceberg => "iceberg",
        }
    }
}

/// All writers and readers use the same selection and durable recovery target.
/// Mirroring while PostgreSQL is authoritative prepares a verified cutover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservationStorage {
    pub backend: ObservationBackend,
    pub target_id: Option<Uuid>,
    pub mirror_iceberg: bool,
}

impl ObservationStorage {
    pub fn from_env() -> AppResult<Self> {
        Self::from_lookup(|name| match std::env::var(name) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(_) => Err(AppError::ConfigError(format!(
                "{name} must be valid Unicode"
            ))),
        })
    }

    /// Pure parser so configuration tests never change process-global environment.
    pub fn from_lookup(
        mut lookup: impl FnMut(&str) -> AppResult<Option<String>>,
    ) -> AppResult<Self> {
        let backend = match lookup("TIMESERIES_BACKEND")?.as_deref() {
            None | Some("postgres") => ObservationBackend::Postgres,
            Some("iceberg") => ObservationBackend::Iceberg,
            _ => {
                return Err(AppError::ConfigError(
                    "TIMESERIES_BACKEND must be postgres or iceberg".into(),
                ))
            }
        };
        let target_id = lookup("TIMESERIES_TARGET_ID")?
            .map(|value| {
                Uuid::parse_str(&value).map_err(|_| {
                    AppError::ConfigError("TIMESERIES_TARGET_ID must be a UUID".into())
                })
            })
            .transpose()?;
        let mirror = match lookup("TIMESERIES_ICEBERG_MIRROR")?.as_deref() {
            None | Some("false") => false,
            Some("true") => true,
            _ => {
                return Err(AppError::ConfigError(
                    "TIMESERIES_ICEBERG_MIRROR must be true or false".into(),
                ))
            }
        };
        if (backend == ObservationBackend::Iceberg || mirror) && target_id.is_none() {
            return Err(AppError::ConfigError(
                "Iceberg requires an explicit TIMESERIES_TARGET_ID".into(),
            ));
        }
        Ok(Self {
            backend,
            target_id,
            mirror_iceberg: mirror || backend == ObservationBackend::Iceberg,
        })
    }

    /// Hold the recovery shared lock until the caller's transaction commits. A pause or
    /// backend transition takes the exclusive lock and therefore drains guarded writers.
    /// This function must be called inside the transaction owning all observation writes.
    pub async fn assert_write_allowed_conn(&self, conn: &mut AsyncPgConnection) -> AppResult<i64> {
        #[derive(QueryableByName)]
        struct Epoch {
            #[diesel(sql_type = BigInt)]
            epoch: i64,
        }
        let epoch = diesel::sql_query(
            "SELECT assert_timeseries_write_allowed($1::uuid, $2::text) AS epoch",
        )
        .bind::<Nullable<SqlUuid>, _>(self.target_id)
        .bind::<Text, _>(self.backend.as_str())
        .get_result::<Epoch>(conn)
        .await?;
        Ok(epoch.epoch)
    }

    /// Refuse stale process configuration after cutover. Reads remain available while
    /// writes are paused. Invoke within the store's snapshot where possible.
    pub async fn assert_read_selection_conn(&self, conn: &mut AsyncPgConnection) -> AppResult<()> {
        #[derive(QueryableByName)]
        struct State {
            #[diesel(sql_type = SqlUuid)]
            target_id: Uuid,
            #[diesel(sql_type = Text)]
            backend: String,
        }
        let state = diesel::sql_query(
            "SELECT target_id, backend FROM timeseries_recovery_state WHERE singleton",
        )
        .get_result::<State>(conn)
        .await
        .optional()?;
        match state {
            None if self.target_id.is_none()
                && self.backend == ObservationBackend::Postgres
                && !self.mirror_iceberg =>
            {
                Ok(())
            }
            Some(state)
                if self.target_id == Some(state.target_id)
                    && self.backend.as_str() == state.backend =>
            {
                Ok(())
            }
            _ => Err(AppError::ServiceUnavailable(
                "observation backend configuration does not match the durable recovery target"
                    .into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(values: &[(&str, &str)]) -> AppResult<ObservationStorage> {
        ObservationStorage::from_lookup(|name| {
            Ok(values
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.to_string()))
        })
    }

    #[test]
    fn default_remains_postgres_without_mirroring() {
        assert_eq!(
            parse(&[]).unwrap(),
            ObservationStorage {
                backend: ObservationBackend::Postgres,
                target_id: None,
                mirror_iceberg: false
            }
        );
    }

    #[test]
    fn iceberg_always_retains_the_postgres_shadow_and_requires_target() {
        assert!(parse(&[("TIMESERIES_BACKEND", "iceberg")]).is_err());
        let target = Uuid::new_v4().to_string();
        let config = parse(&[
            ("TIMESERIES_BACKEND", "iceberg"),
            ("TIMESERIES_TARGET_ID", &target),
            ("TIMESERIES_ICEBERG_MIRROR", "false"),
        ])
        .unwrap();
        assert_eq!(config.backend, ObservationBackend::Iceberg);
        assert!(config.mirror_iceberg);
    }

    #[test]
    fn shadow_mirroring_cannot_silently_initialize_an_unidentified_target() {
        assert!(parse(&[("TIMESERIES_ICEBERG_MIRROR", "true")]).is_err());
        let target = Uuid::new_v4().to_string();
        let config = parse(&[
            ("TIMESERIES_TARGET_ID", &target),
            ("TIMESERIES_ICEBERG_MIRROR", "true"),
        ])
        .unwrap();
        assert_eq!(config.backend, ObservationBackend::Postgres);
        assert!(config.mirror_iceberg);
    }

    #[test]
    fn invalid_or_empty_values_fail_closed_without_echoing_values() {
        for (key, value) in [
            ("TIMESERIES_BACKEND", ""),
            ("TIMESERIES_BACKEND", "secret://invalid"),
            ("TIMESERIES_TARGET_ID", "secret://invalid"),
            ("TIMESERIES_ICEBERG_MIRROR", "1"),
        ] {
            let error = parse(&[(key, value)]).unwrap_err().to_string();
            assert!(error.contains(key));
            assert!(!error.contains("secret://invalid"));
        }
    }
}
