#!/usr/bin/env bash
# Isolated Train 3 baseline. Never accepts an existing data/app database URL.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo"
candidate="$(git rev-parse HEAD)"
if [[ "${CI:-}" == true && -n "$(git status --porcelain --untracked-files=normal)" ]]; then
  echo "CI compatibility candidate must have a clean checkout" >&2
  exit 1
fi
for command in docker cargo python3; do command -v "$command" >/dev/null || { echo "missing $command" >&2; exit 1; }; done
# Pin the local socket: a saved context must never select a production Docker daemon.
docker_local() {
  env -u DOCKER_HOST -u DOCKER_CONTEXT -u DOCKER_TLS -u DOCKER_TLS_VERIFY -u DOCKER_CERT_PATH \
    docker --host=unix:///var/run/docker.sock "$@"
}
work="$(mktemp -d)"
identity="econgraph-train3-$(basename "$work" | tr '[:upper:].' '[:lower:]-')"
container=""
cleanup() {
  status=$?
  resource="${container:-$identity}"
  if [[ -n "$resource" ]]; then
    owner="$(docker_local inspect --format '{{index .Config.Labels "econgraph.train3.owner"}}' "$resource" 2>/dev/null || true)"
    if [[ "$owner" == "$identity" ]]; then docker_local rm --force "$resource" >/dev/null; fi
  fi
  rm -rf "$work"
  exit "$status"
}
trap cleanup EXIT
# A bounded disposable copy; Docker selects an unused host port and only loopback is exposed.
container="$(docker_local run --detach --name "$identity" --label "econgraph.train3.owner=$identity" \
  --publish 127.0.0.1::5432 --tmpfs /var/lib/postgresql:rw,size=512m \
  --env POSTGRES_USER=postgres --env POSTGRES_PASSWORD=password --env POSTGRES_DB=train3 \
  postgres:18)"
ready=0
for ((attempt=0; attempt<60; attempt++)); do
  if docker_local exec "$container" pg_isready -U postgres >/dev/null 2>&1; then ready=1; break; fi
  sleep 1
done
[[ "$ready" == 1 ]] || { docker_local logs "$container"; exit 1; }
port="$(docker_local port "$container" 5432/tcp | sed -n 's/^127\.0\.0\.1://p')"
[[ "$port" =~ ^[0-9]+$ ]] || { echo 'missing loopback Postgres port' >&2; exit 1; }
export DATABASE_URL="postgres://postgres:password@127.0.0.1:$port/train3"
# Existing DB environment values are deliberately overridden. Suites create their own scratch
# databases from this admin URL, never a dev/primary database, including the combined case.
export DATA_DATABASE_URL="$DATABASE_URL" APP_DATABASE_URL="$DATABASE_URL"
export TRAIN3_TOOL_DATABASE_URL=postgres://postgres:password@127.0.0.1:5432/train3
export TRAIN3_BACKUP_CHECK=1
for tool in pg_dump psql; do
  cat > "$work/$tool" <<WRAPPER
#!/usr/bin/env bash
set -euo pipefail
exec env -u DOCKER_HOST -u DOCKER_CONTEXT -u DOCKER_TLS -u DOCKER_TLS_VERIFY -u DOCKER_CERT_PATH docker --host=unix:///var/run/docker.sock exec -i '$container' '$tool' "\$@"
WRAPPER
  chmod +x "$work/$tool"
done
export TRAIN3_PG_DUMP="$work/pg_dump" TRAIN3_PSQL="$work/psql"
# Production binary is built separately, before testkit unifies fixture-only dependencies.
cargo build --manifest-path backend/Cargo.toml --locked -p econ-graph-backend --bin econ-graph-backend -p econ-graph-crawler --bin crawler
cargo build --manifest-path backend/Cargo.toml --locked -p econ-graph-crawler --features testkit --bin seed-fixtures
target="$(realpath -m "${CARGO_TARGET_DIR:-$repo/backend/target}")"
# Existing recorded adapter fixtures, with every unexpected off-host request refused.
no_network_proxy=http://127.0.0.1:9
seed_database() {
  env -u http_proxy -u https_proxy -u all_proxy -u no_proxy \
    HTTP_PROXY="$no_network_proxy" HTTPS_PROXY="$no_network_proxy" ALL_PROXY="$no_network_proxy" \
    NO_PROXY=127.0.0.1,localhost DATA_DATABASE_URL="$1" "$target/debug/seed-fixtures" --deterministic-empty
}
snapshot() {
  docker_local exec -i "$container" psql --no-psqlrc --tuples-only --no-align --set ON_ERROR_STOP=1 -U postgres --dbname "$1" < scripts/train3/semantic-snapshot.sql
}
seed_database "$DATA_DATABASE_URL"
snapshot train3 > "$work/semantic.json"
docker_local exec "$container" createdb -U postgres train3_repeat
seed_database "postgres://postgres:password@127.0.0.1:$port/train3_repeat"
snapshot train3_repeat > "$work/semantic-repeat.json"
cmp --silent "$work/semantic.json" "$work/semantic-repeat.json" || { echo 'two fresh recorded seeds have different identities/content' >&2; exit 1; }
# A populated database must be refused before migrations or any identity writes.
if seed_database "$DATA_DATABASE_URL" > "$work/refusal.log" 2>&1; then
  echo 'deterministic-empty accepted a populated database' >&2; exit 1
fi
snapshot train3 > "$work/semantic-after-refusal.json"
cmp --silent "$work/semantic.json" "$work/semantic-after-refusal.json" || { echo 'refusing a populated database changed its contents' >&2; exit 1; }
export TRAIN3_SEMANTIC_SNAPSHOT="$work/semantic.json"
cargo test --manifest-path backend/Cargo.toml --locked -p econ-graph-crawler --features testkit --bin seed-fixtures deterministic_identity_tests
cargo test --manifest-path backend/Cargo.toml --locked -p econ-graph-services --test observation_contract -- --include-ignored --test-threads=1
cargo test --manifest-path backend/Cargo.toml --locked -p econ-graph-core --test train3_fixture_contract --test research_reference_contract -- --test-threads=1
cargo test --manifest-path backend/Cargo.toml --locked -p econ-graph-sec-crawler --test filing_identity_contract -- --test-threads=1
cargo test --manifest-path backend/Cargo.toml --locked -p econ-graph-graphql --test data_api_contract --test catalog_authorization -- --test-threads=1
# Existing migration boundaries and upgrade compatibility, not a second migration framework.
cargo test --manifest-path backend/Cargo.toml --locked -p econ-graph-core --lib database::tests:: -- --test-threads=1
migration_args=(--base-ref origin/main)
if [[ -n "${GITHUB_EVENT_PATH:-}" && -n "${GITHUB_EVENT_NAME:-}" ]]; then
  migration_args=(--event-path "$GITHUB_EVENT_PATH" --event-name "$GITHUB_EVENT_NAME")
fi
python3 backend/scripts/check_migration_order.py "${migration_args[@]}"
[[ "$(git rev-parse HEAD)" == "$candidate" ]] || { echo 'candidate changed during compatibility run' >&2; exit 1; }
if [[ "${CI:-}" == true && -n "$(git status --porcelain --untracked-files=normal)" ]]; then
  echo 'compatibility checks changed the candidate checkout' >&2
  exit 1
fi
python3 scripts/train3/fixture-manifest.py > "$work/evidence.json"
if [[ -n "${TRAIN3_EVIDENCE_PATH:-}" ]]; then cp "$work/evidence.json" "$TRAIN3_EVIDENCE_PATH"; fi
echo "Train 3 split/combined contracts and backup round trips passed for $(git rev-parse HEAD)."
