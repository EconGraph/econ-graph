# EconGraph Auth

The authentication and authorization layer for EconGraph verifies Keycloak access tokens and enforces fine-grained roles. Keycloak handles sign-in and upstream identity providers, and supplies the roles in each access token. See the [local Keycloak setup](../../../config/keycloak/README.md#local-stack) to configure sign-in.

## Features

- **JWT verification**: `OidcVerifier` checks RS256 signatures against the provider's JWKS, validates issuer, audience and token times, and accepts access tokens only. It caches signing keys and refreshes them to handle key rotation.
- **Bearer authentication**: `bearer::authenticate` verifies the request's bearer token, loads or creates the app plane user keyed by the token's `sub`, and rejects suspended accounts before returning a `Caller`.
- **Role-Based Access Control**: `Principal` takes recognized roles from the token's top-level `roles` claim. `authorize` checks whether the caller holds the required role; names outside the `Role` catalog are ignored.

## Testing

The crate tests cover bearer-header parsing, the role catalog and authorization decisions, and OIDC configuration. Token-verification tests exercise accepted and rejected tokens, discovery, signing-key caching and rotation, and provider failures.

The verification tests use `testkit::TestIssuer`, which serves OpenID Connect discovery and JWKS endpoints on a local port and signs test tokens. These tests run without a database or a live identity provider. The helper is available automatically during this crate's tests; other crates can enable the optional `testkit` feature to reuse it.

### Running Tests

Run these commands from the repository's `backend/` directory:

```bash
# Run the crate's library tests
cargo test -p econ-graph-auth --lib

# Run individual test groups
cargo test -p econ-graph-auth --lib bearer::tests
cargo test -p econ-graph-auth --lib roles::tests
cargo test -p econ-graph-auth --lib oidc::tests
cargo test -p econ-graph-auth --lib oidc::verify_tests
```

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the [license terms](../../LICENSE) for complete terms and conditions.
