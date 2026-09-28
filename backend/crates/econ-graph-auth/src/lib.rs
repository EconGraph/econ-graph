// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! # EconGraph Auth
//!
//! Authentication and authorization for the `EconGraph` system: verifying Keycloak access
//! tokens and enforcing fine-grained roles. There is no in-house sign-in; every caller is
//! either anonymous or authenticated by the identity provider's token.
//!
//! ## Features
//!
//! - **Fine-grained roles**: the [`Role`] catalog, [`Principal`] and [`authorize`]
//! - **Identity-provider tokens**: [`oidc::OidcVerifier`] checks Keycloak access tokens, and
//!   [`bearer::authenticate`] turns a request's bearer token into the signed-in [`Caller`]

pub mod bearer;
pub mod oidc;
pub mod roles;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

// Re-export commonly used auth types
pub use bearer::{authenticate, BearerError, Caller};
pub use oidc::{OidcConfig, OidcConfigError, OidcVerifier, VerifiedToken, VerifyError};
pub use roles::{authorize, role_list, Forbidden, Principal, Role, UnknownRole};
