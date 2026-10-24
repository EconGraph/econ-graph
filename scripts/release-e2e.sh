#!/usr/bin/env bash
# Runs the release end-to-end suite (frontend/playwright.release.config.ts):
#   1. Postgres 18: a fresh docker-compose.release-e2e.yml container, unless
#      RELEASE_E2E_DATABASE_URL is set (CI sets it to its service container). A DATABASE_URL
#      you export for development is ignored, so the seed never writes to your dev database.
#   2. Keycloak, with the dev realm from config/keycloak/, always via docker-compose.release-e2e.yml
#      (CI has no equivalent service container for it: the realm and users files must be mounted
#      from the checkout under test).
#   3. Builds the backend and the seed tool in one cargo invocation.
#   4. Seeds the database from recorded fixtures through the crawler adapters, with any request
#      outside its local mock upstreams refused.
#   5. Runs Playwright, which starts the backend and a release build of the frontend.
# Extra arguments go to `playwright test`, e.g. `scripts/release-e2e.sh smoke.spec.ts --headed`.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Recreated every run, so each run starts from an empty realm.
docker compose -f "$repo/docker-compose.release-e2e.yml" up -d --wait --force-recreate keycloak

if [[ -z "${RELEASE_E2E_DATABASE_URL:-}" ]]; then
  # Recreated every run, so each run starts from an empty database.
  docker compose -f "$repo/docker-compose.release-e2e.yml" up -d --wait --force-recreate postgres
  export RELEASE_E2E_DATABASE_URL=postgres://postgres:password@localhost:5439/econ_graph_e2e
fi

# Cargo resolves a relative CARGO_TARGET_DIR against backend/, where it runs.
target="$(cd "$repo/backend" && realpath -m "${CARGO_TARGET_DIR:-target}")"
(
  cd "$repo/backend"
  cargo build -p econ-graph-backend --bin econ-graph-backend \
    -p econ-graph-crawler --features econ-graph-crawler/testkit --bin seed-fixtures
)
# The seed talks only to its own mock upstreams on 127.0.0.1. Every other request goes through
# a proxy where nothing listens, so one that would reach a real data source fails the seed here
# instead of quietly depending on that source being up (or reachable through your proxy).
no_network_proxy=http://127.0.0.1:9
env -u http_proxy -u https_proxy -u all_proxy -u no_proxy \
  HTTP_PROXY="$no_network_proxy" HTTPS_PROXY="$no_network_proxy" ALL_PROXY="$no_network_proxy" \
  NO_PROXY=127.0.0.1,localhost \
  DATABASE_URL="$RELEASE_E2E_DATABASE_URL" "$target/debug/seed-fixtures"
export RELEASE_BACKEND_BIN="${RELEASE_BACKEND_BIN:-$target/debug/econ-graph-backend}"

cd "$repo/frontend"
exec npx playwright test --config=playwright.release.config.ts "$@"
