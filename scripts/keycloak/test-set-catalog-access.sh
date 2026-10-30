#!/usr/bin/env bash
# Tests for scripts/keycloak/set-catalog-access.sh against stubs, never a cluster, a Docker
# daemon or a real Keycloak:
#   - --dry-run prints the plan and changes nothing in Keycloak;
#   - --admin-only removes the role from catalog-public and the anonymous scope and grants
#     it to admin; --withdraw does the same without the admin grant; both together in one
#     command, the production one;
#   - a second run changes nothing (idempotent);
#   - --make-public restores the original state exactly;
#   - it refuses, changing nothing, a role that isn't a catalog read, a role or composite
#     that doesn't exist, a missing or non-kind --context, and a typo in a later role;
#   - the Keycloak password never appears in its output, and the pod gets it from Secret
#     econ-graph-keycloak by reference;
#   - every kubectl call goes to the kind cluster's own kubectl on --context kind-econ-graph,
#     never the kubectl on PATH, and the cluster's identity is printed first.
#
# The stubs: a plain `kubectl` first on PATH that records any call and fails; `docker` and
# `kind` stubs that pretend a kind cluster econ-graph exists and hand out
# testdata/catalog-access/fake_kind_kubectl.py as its kubectl, which runs the pod's helper
# against testdata/catalog-access/fake_keycloak.py, a local HTTP server on 127.0.0.1.
#
# Usage: scripts/keycloak/test-set-catalog-access.sh (from anywhere)
set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
script="$repo_root/scripts/keycloak/set-catalog-access.sh"
fixtures="$repo_root/scripts/keycloak/testdata/catalog-access"
failures=0
fail() {
  echo "FAIL $1"
  failures=$((failures + 1))
}

work="$(mktemp -d)"
server_pid=""
cleanup() {
  [ -n "$server_pid" ] && kill "$server_pid" 2>/dev/null
  rm -rf "$work"
}
trap cleanup EXIT
mkdir -p "$work/bin" "$work/home" "$work/tmp"
log="$work/kind-kubectl.log"
path_log="$work/path-kubectl.log"
writes="$work/writes.log"
state="$work/state.json"
out="$work/out"

cat >"$work/bin/kubectl" <<EOF
#!/bin/sh
echo "\$*" >>"$path_log"
echo "plain kubectl stub: refusing" >&2
exit 1
EOF
cat >"$work/bin/docker" <<EOF
#!/bin/sh
case "\$1" in
  info) echo "28.0.0 /var/lib/docker [name=seccomp]" ;;
  cp) for last; do :; done; cp "$fixtures/fake_kind_kubectl.py" "\$last" ;;
esac
exit 0
EOF
cat >"$work/bin/kind" <<EOF
#!/bin/sh
case "\$1 \${2:-}" in
  "get clusters") echo econ-graph ;;
  "get kubeconfig") echo "apiVersion: v1" ;;
  "version "*) echo "kind v0.33.0 go1.26.7 linux/amd64" ;;
esac
exit 0
EOF
chmod +x "$work/bin/"*
kind_pinned="$(HOME="$work/home" XDG_DATA_HOME="$work/home/.local/share" bash -c \
  'source "$1/scripts/deploy/lib/kind-version.sh" && echo "$KIND_PINNED_BIN"' _ "$repo_root")"
mkdir -p "$(dirname "$kind_pinned")"
cp "$work/bin/kind" "$kind_pinned"

# The live state before any change: FRED and IMF public, as production was.
initial='{"realm": true, "anonymous": true,
  "roles": ["catalog.fred:read", "catalog.imf:read", "catalog.ecb:read", "catalog.bls:read", "annotation:create"],
  "realm_roles": ["catalog-public", "admin", "user"],
  "composites": {"catalog-public": ["catalog.bls:read", "catalog.ecb:read", "catalog.fred:read", "catalog.imf:read"],
                 "admin": ["admin.users:read"]},
  "scope": ["catalog.bls:read", "catalog.ecb:read", "catalog.fred:read", "catalog.imf:read"]}'
reset() { python3 -c 'import json,sys; json.dump(json.loads(sys.argv[1]), open(sys.argv[2], "w"))' "$initial" "$state"; : >"$writes"; }
# field EXPR: a JSON value from the fake Keycloak's state. EXPR is one of this file's own
# fixed expressions, evaluated over the test's own state file.
field() { python3 -c 'import json,sys; s=json.load(open(sys.argv[1])); print(json.dumps(eval(sys.argv[2], {}, s)))' "$state" "$1"; }

reset
python3 -I "$fixtures/fake_keycloak.py" "$state" "$work/port" "$writes" &
server_pid=$!
for _ in $(seq 50); do [ -s "$work/port" ] && break; sleep 0.1; done
[ -s "$work/port" ] || { echo "the fake Keycloak did not start"; exit 1; }
keycloak_url="http://127.0.0.1:$(cat "$work/port")"

# run [ENV=VALUE...] -- ARGS...: the script with only the stubs reachable and stdin not a
# terminal; output in $out, exit status in $status.
run() {
  : >"$log"
  local envs=()
  while [ "$1" != -- ]; do envs+=("$1"); shift; done
  shift
  status=0
  env -u KUBECONFIG -u ECONGRAPH_KUBE_DIR -u KIND_CLUSTER -u NAMESPACE -u ECONGRAPH_DOCKER_HOST \
    -u CLUSTER_ROLE_CONTEXT -u KUBECTL \
    STUB_LOG="$log" STUB_KEYCLOAK_URL="$keycloak_url" \
    HOME="$work/home" XDG_DATA_HOME="$work/home/.local/share" XDG_CACHE_HOME="$work/cache" \
    TMPDIR="$work/tmp" PATH="$work/bin:/usr/local/bin:/usr/bin:/bin" \
    "${envs[@]}" "$script" "$@" </dev/null >"$out" 2>&1 || status=$?
}
unchanged() { [ ! -s "$writes" ] && [ "$(field 'composites')" = "$(python3 -c 'import json,sys; print(json.dumps(json.loads(sys.argv[1])["composites"]))' "$initial")" ]; }
# shellcheck disable=SC2054 # the comma joins two roles in one --withdraw
production=(--admin-only catalog.fred:read --withdraw catalog.imf:read,catalog.ecb:read)

# --dry-run: the plan, and nothing changed.
reset
run -- --context kind-econ-graph --dry-run "${production[@]}"
[ "$status" = 0 ] || fail "dry run failed: $(cat "$out")"
for line in "would remove catalog.fred:read from catalog-public" "would add catalog.fred:read to admin" \
    "would remove catalog.fred:read from the econgraph-anonymous scope" \
    "would remove catalog.imf:read from catalog-public" \
    "would remove catalog.imf:read from the econgraph-anonymous scope" \
    "would remove catalog.ecb:read from catalog-public" \
    "would remove catalog.ecb:read from the econgraph-anonymous scope" "dry run: nothing changed"; do
  grep -qxF "$line" "$out" || fail "dry run does not say '$line': $(cat "$out")"
done
if grep -qE "catalog.(imf|ecb):read to admin" "$out"; then fail "--withdraw would grant IMF to admin: $(cat "$out")"; fi
unchanged || fail "dry run changed Keycloak: $(cat "$writes")"
if grep -q "rollout restart" "$out"; then fail "dry run printed the restart hint"; fi

# The production command.
reset
run -- --context kind-econ-graph "${production[@]}"
[ "$status" = 0 ] || fail "apply failed: $(cat "$out")"
[ "$(field 'composites["catalog-public"]')" = '["catalog.bls:read"]' ] || fail "catalog-public is $(field 'composites["catalog-public"]')"
[ "$(field 'composites["admin"]')" = '["admin.users:read", "catalog.fred:read"]' ] || fail "admin is $(field 'composites["admin"]')"
[ "$(field 'scope')" = '["catalog.bls:read"]' ] || fail "the anonymous scope is $(field 'scope')"
grep -qxF "catalog.fred:read is now admin-only" "$out" || fail "FRED not reported admin-only: $(cat "$out")"
grep -qxF "catalog.imf:read is now not public" "$out" || fail "IMF not reported withdrawn: $(cat "$out")"
grep -qxF "catalog.ecb:read is now not public" "$out" || fail "ECB not reported withdrawn: $(cat "$out")"
grep -q "rollout restart deployment/econ-graph-backend" "$out" || fail "no restart hint: $(cat "$out")"
# Cluster identity before the change, every call on the kind context, never the PATH kubectl.
grep -qF "context         kind-econ-graph" "$out" || fail "the cluster identity is not shown: $(cat "$out")"
identity_line="$(grep -n "kube-system UID" "$out" | head -1 | cut -d: -f1)"
change_line="$(grep -n "^remove catalog.fred:read" "$out" | head -1 | cut -d: -f1)"
[ "${identity_line:-999}" -lt "${change_line:-0}" ] || fail "the identity is not shown before the change: $(cat "$out")"
grep -q "^kubectl --context kind-econ-graph -n econ-graph run keycloak-catalog-access-" "$log" ||
  fail "the pod did not run on kind-econ-graph: $(cat "$log")"
while IFS= read -r line; do
  case "$line" in
    "kubectl --context kind-econ-graph "* | "kubectl version "* | "kubectl config get-contexts -o name") ;;
    *) fail "a kubectl call was not pinned to kind-econ-graph: $line" ;;
  esac
done <"$log"
if grep -q "use-context" "$log"; then fail "the current context was changed"; fi
if grep -q "test-admin-password" "$out" "$log"; then fail "the password was printed or passed on the command line"; fi
grep -q '"secretKeyRef":{"name":"econ-graph-keycloak","key":"admin-password"}' "$log" ||
  fail "the pod does not take the password from the Secret: $(cat "$log")"

# Idempotent: the same command again changes nothing.
: >"$writes"
run -- --context kind-econ-graph "${production[@]}"
[ "$status" = 0 ] || fail "second apply failed: $(cat "$out")"
[ ! -s "$writes" ] || fail "the second run changed Keycloak: $(cat "$writes")"
grep -qxF "catalog.fred:read is already admin-only; nothing to change" "$out" || fail "second run output: $(cat "$out")"
grep -qxF "catalog.imf:read is already not public; nothing to change" "$out" || fail "second run output: $(cat "$out")"
grep -qxF "catalog.ecb:read is already not public; nothing to change" "$out" || fail "second run output: $(cat "$out")"

# The inverse puts back exactly what was there.
run -- --context kind-econ-graph --make-public catalog.fred:read,catalog.imf:read --make-public catalog.ecb:read
[ "$status" = 0 ] || fail "--make-public failed: $(cat "$out")"
[ "$(field 'sorted(composites["catalog-public"])')" = '["catalog.bls:read", "catalog.ecb:read", "catalog.fred:read", "catalog.imf:read"]' ] ||
  fail "--make-public left catalog-public $(field 'composites')"
[ "$(field 'composites["admin"]')" = '["admin.users:read"]' ] || fail "--make-public left admin $(field 'composites')"
[ "$(field 'sorted(scope)')" = '["catalog.bls:read", "catalog.ecb:read", "catalog.fred:read", "catalog.imf:read"]' ] ||
  fail "--make-public left the scope $(field 'scope')"
: >"$writes"
run -- --context kind-econ-graph --make-public catalog.fred:read
[ ! -s "$writes" ] || fail "a second --make-public changed Keycloak: $(cat "$writes")"

# Refusals change nothing.
reset
run -- --context kind-econ-graph --admin-only catalog.nosuch:read
[ "$status" != 0 ] || fail "a missing role was accepted"
grep -q "refused: econ-graph-api has no role catalog.nosuch:read; nothing changed" "$out" || fail "missing role: $(cat "$out")"
unchanged || fail "a missing role changed Keycloak"
# A typo in a later role stops the earlier one too.
run -- --context kind-econ-graph --admin-only catalog.fred:read --withdraw catalog.imf:read,catalog.imff:read
[ "$status" != 0 ] || fail "a later missing role was accepted"
unchanged || fail "a later missing role let the first change through: $(cat "$writes")"
run -- --context kind-econ-graph --admin-only annotation:create
[ "$status" = 2 ] || fail "a non-catalog role was not refused (status $status)"
grep -q "is not a catalog read role" "$out" || fail "non-catalog role: $(cat "$out")"
[ ! -s "$log" ] || fail "a non-catalog role reached kubectl"
python3 -c 'import json,sys; s=json.load(open(sys.argv[1])); s["realm_roles"].remove("admin"); json.dump(s, open(sys.argv[1], "w"))' "$state"
run -- --context kind-econ-graph --admin-only catalog.fred:read
grep -q "refused: realm role admin does not exist; nothing changed" "$out" || fail "missing admin: $(cat "$out")"
[ ! -s "$writes" ] || fail "a missing admin composite changed Keycloak"
reset
python3 -c 'import json,sys; s=json.load(open(sys.argv[1])); s["realm_roles"].remove("catalog-public"); json.dump(s, open(sys.argv[1], "w"))' "$state"
run -- --context kind-econ-graph --withdraw catalog.imf:read
grep -q "refused: realm role catalog-public does not exist; nothing changed" "$out" || fail "missing catalog-public: $(cat "$out")"
[ ! -s "$writes" ] || fail "a missing catalog-public changed Keycloak"
reset
run -- --admin-only catalog.fred:read
[ "$status" = 2 ] || fail "a missing --context was accepted (status $status)"
run -- --context econ-graph-microk8s --admin-only catalog.fred:read
[ "$status" = 2 ] || fail "a non-kind context was accepted (status $status)"
[ ! -s "$log" ] || fail "a bad --context reached kubectl"
run STUB_NO_KIND=1 -- --context kind-econ-graph --admin-only catalog.fred:read
[ "$status" != 0 ] || fail "a kubeconfig without kind-econ-graph was accepted"
if grep -q " run " "$log"; then fail "ran the pod without the kind context"; fi
unchanged || fail "a missing context changed Keycloak"

if [ -s "$path_log" ]; then fail "the kubectl on PATH was called: $(cat "$path_log")"; fi

if [ "$failures" -ne 0 ]; then
  echo "$failures failure(s)"
  exit 1
fi
echo "set-catalog-access: all checks passed"
