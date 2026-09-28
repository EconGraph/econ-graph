//! # Request authentication
//!
//! [`authenticate`] turns a request's `Authorization` header into the signed-in caller: their
//! `users` row (created on first sight) and their [`Principal`]. Every API route that accepts
//! a token (GraphQL and `/mcp`) goes through it, so all of them check the token and the
//! account's `is_active` flag the same way.

use econ_graph_core::models::User;
use econ_graph_core::DatabasePool;
use uuid::Uuid;

use crate::oidc::{OidcVerifier, VerifyError};
use crate::roles::Principal;

/// A signed-in caller: their `users` row and the roles their token grants.
#[derive(Debug, Clone)]
pub struct Caller {
    /// The caller's `users` row, keyed by the token's `sub`.
    pub user: User,
    /// The caller's id and fine-grained roles, from the token.
    pub principal: Principal,
}

/// Why a request that sent a bearer token is not signed in.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BearerError {
    /// The token is not acceptable; the client should sign in again.
    #[error("invalid token: {0}")]
    InvalidToken(String),
    /// The identity provider's keys could not be fetched, so the token could not be checked.
    #[error("identity provider unavailable: {0}")]
    Unavailable(String),
    /// The token is valid but the account is suspended (`users.is_active = false`).
    #[error("account {0} is suspended")]
    Inactive(Uuid),
    /// The caller's `users` row could not be read or created.
    #[error("could not load the caller's account: {0}")]
    Database(String),
}

/// The token in an `Authorization: Bearer <token>` header value.
///
/// `Ok(None)` for no header or another scheme (the request is anonymous); an error for a
/// `Bearer` header without a token. The scheme name is matched case-insensitively
/// (RFC 7235, section 2.1).
pub fn bearer_token(authorization: Option<&str>) -> Result<Option<&str>, BearerError> {
    let Some(value) = authorization else {
        return Ok(None);
    };
    let (scheme, rest) = value.split_once(' ').unwrap_or((value, ""));
    if !scheme.eq_ignore_ascii_case("bearer") {
        return Ok(None);
    }
    match rest.trim() {
        "" => Err(BearerError::InvalidToken("empty bearer token".into())),
        token => Ok(Some(token)),
    }
}

/// The signed-in caller for a request with this `Authorization` header value.
///
/// - No header, or a scheme other than `Bearer`: `Ok(None)`, an anonymous request.
/// - Sign-in disabled (`verifier` is `None`, `OIDC_ISSUER` unset): `Ok(None)` whatever the
///   header says; no token is trusted.
/// - Otherwise the token must verify, and the account must be active. The caller's `users`
///   row is created the first time their `sub` is seen.
///
/// Nothing here contacts the identity provider for a request without a token, so an outage
/// never affects anonymous requests.
pub async fn authenticate(
    pool: &DatabasePool,
    verifier: Option<&OidcVerifier>,
    authorization: Option<&str>,
) -> Result<Option<Caller>, BearerError> {
    let Some(verifier) = verifier else {
        return Ok(None);
    };
    let Some(token) = bearer_token(authorization)? else {
        return Ok(None);
    };
    let verified = verifier.verify(token).await.map_err(|err| match err {
        VerifyError::Invalid(why) => BearerError::InvalidToken(why),
        VerifyError::Unavailable(why) => BearerError::Unavailable(why),
    })?;

    let user = User::get_or_create_for_subject(
        pool,
        verified.subject,
        verified.email.as_deref(),
        verified.email_verified,
        verified.name.as_deref(),
    )
    .await
    .map_err(|e| BearerError::Database(e.to_string()))?;
    let principal = verified.principal();
    check_active(user, principal)
}

/// Refuse a suspended account; otherwise the signed-in caller.
pub fn check_active(user: User, principal: Principal) -> Result<Option<Caller>, BearerError> {
    if !user.is_active {
        return Err(BearerError::Inactive(user.id));
    }
    Ok(Some(Caller { user, principal }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_token_reads_only_the_bearer_scheme() {
        assert_eq!(bearer_token(None), Ok(None));
        assert_eq!(bearer_token(Some("Basic dXNlcjpwYXNz")), Ok(None));
        assert_eq!(bearer_token(Some("Bearerabc")), Ok(None));
        assert_eq!(bearer_token(Some("Bearer abc")), Ok(Some("abc")));
        assert_eq!(bearer_token(Some("bearer  abc ")), Ok(Some("abc")));
        assert_eq!(bearer_token(Some("BEARER abc")), Ok(Some("abc")));
        for empty in ["Bearer", "Bearer ", "Bearer    "] {
            assert!(
                matches!(bearer_token(Some(empty)), Err(BearerError::InvalidToken(_))),
                "{empty:?}"
            );
        }
    }
}
