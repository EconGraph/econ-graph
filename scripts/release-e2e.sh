#!/usr/bin/env bash
# Runs the release end-to-end suite (frontend/playwright.release.config.ts):
#   1. Postgres 18: a fresh docker-compose.release-e2e.yml container, unless
#      RELEASE_E2E_DATABASE_URL is set (CI sets it to its service container). APP_DATABASE_URL
#      and DATA_DATABASE_URL are both set to it, and a DATABASE_URL, APP_DATABASE_URL or
#      DATA_DATABASE_URL you export for development is ignored, so the seed never writes to your
#      dev database.
#   2. Keycloak, with the dev realm from config/keycloak/, always via docker-compose.release-e2e.yml
#      (CI has no equivalent service container for it: the realm and users files must be mounted
#      from the checkout under test).
#   3. Builds the backend with --release --locked and the release flag profile
#      (config/flags/README.md). Builds the testkit-enabled seed in a separate invocation
#      so its mock dependencies cannot be unified into the backend under test.
#   4. Seeds the database from recorded fixtures through the crawler adapters, with any request
#      outside its local mock upstreams refused.
#   5. Runs Playwright, which starts the backend and a release build of the frontend.
# Extra arguments go to `playwright test`, e.g. `scripts/release-e2e.sh smoke.spec.ts --headed`.
# RELEASE_E2E_CONFIG picks another Playwright config on the same stack, relative to frontend/:
# playwright.qa-stack.config.ts runs the QA scenario suite (docs/release/qa-scenarios.md).
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Recreated every run, so each run starts from an empty realm.
docker compose -f "$repo/docker-compose.release-e2e.yml" up -d --wait --force-recreate keycloak

if [[ -z "${RELEASE_E2E_DATABASE_URL:-}" ]]; then
  # Recreated every run, so each run starts from an empty database.
  docker compose -f "$repo/docker-compose.release-e2e.yml" up -d --wait --force-recreate postgres
  export RELEASE_E2E_DATABASE_URL=postgres://postgres:password@localhost:5439/econ_graph_e2e
fi
# The app and data databases are this one database, for the seed and the backend alike.
export APP_DATABASE_URL="$RELEASE_E2E_DATABASE_URL"
export DATA_DATABASE_URL="$RELEASE_E2E_DATABASE_URL"

# Cargo resolves a relative CARGO_TARGET_DIR against backend/, where it runs.
target="$(cd "$repo/backend" && realpath -m "${CARGO_TARGET_DIR:-target}")"
(
  cd "$repo/backend"
  # backend/.cargo/config.toml defaults local builds to the dev flag profile, which compiles in
  # /mcp and the static catalogs; an exported FLAGS_PROFILE wins over it.
  started=$SECONDS
  FLAGS_PROFILE=release cargo build --release --locked -p econ-graph-backend --bin econ-graph-backend
  echo "Release backend build: $((SECONDS - started))s"
  # Separate cargo invocation: testkit/wiremock belongs only in the fixture loader. The seed
  # stays in the dev profile to avoid optimizing test-only dependencies in release CI.
  started=$SECONDS
  FLAGS_PROFILE=release cargo build --locked -p econ-graph-crawler \
    --features econ-graph-crawler/testkit --bin seed-fixtures
  echo "Fixture loader build: $((SECONDS - started))s"
)
# The seed talks only to its own mock upstreams on 127.0.0.1. Every other request goes through
# a proxy where nothing listens, so one that would reach a real data source fails the seed here
# instead of quietly depending on that source being up (or reachable through your proxy).
no_network_proxy=http://127.0.0.1:9
env -u http_proxy -u https_proxy -u all_proxy -u no_proxy \
  HTTP_PROXY="$no_network_proxy" HTTPS_PROXY="$no_network_proxy" ALL_PROXY="$no_network_proxy" \
  NO_PROXY=127.0.0.1,localhost \
  "$target/debug/seed-fixtures"
export RELEASE_BACKEND_BIN="${RELEASE_BACKEND_BIN:-$target/release/econ-graph-backend}"

cd "$repo/frontend"
exec npx playwright test --config="${RELEASE_E2E_CONFIG:-playwright.release.config.ts}" "$@"
