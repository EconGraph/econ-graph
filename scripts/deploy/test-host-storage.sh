#!/usr/bin/env bash
# The stubs are written with single quotes on purpose: they expand when they run.
# shellcheck disable=SC2016
# Tests for the Postgres host storage (docs/deployment/HOST_STORAGE.md), with no
# cluster, no Docker and nothing written outside a scratch directory:
#   - the StorageClass, PVs and StatefulSets in the manifests equal what production
#     runs (scripts/deploy/check-host-storage-fixture.py against the committed
#     fixture), and a StatefulSet without the storage class fails that check;
#   - apply-host-storage.sh applies host-storage.yaml only when the host subvolumes
#     are mounted, the data directories exist, the node is labelled and bind-mounts
#     them, and every existing database still has its Secret; it creates a missing
#     data directory only with --create-dirs (not under --dry-run), re-arms a
#     Released PV, changes nothing under --dry-run, never deletes anything and never
#     changes ownership;
#   - deploy.sh runs it, on the pinned context, after the namespace and before
#     create-secrets.sh and both Postgres StatefulSets.
#
# Needs python3 with PyYAML. Usage: scripts/deploy/test-host-storage.sh (from anywhere)
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
failures=0
fail() {
  echo "FAIL $1"
  failures=$((failures + 1))
}

scratch="$(mktemp -d)"
trap '/bin/rm -rf "$scratch"' EXIT
# Nothing may reach the operator's home, cache, backups or /srv/econgraph.
export HOME="$scratch/home" XDG_CACHE_HOME="$scratch/cache"
export ECONGRAPH_HOST_ROOT="$scratch/srv" ECONGRAPH_BACKUP_ROOT="$scratch/backups"
unset KUBECONFIG CLUSTER_ROLE_CONTEXT DOCKER_HOST DOCKER_CONTEXT
mkdir -p "$HOME" "$XDG_CACHE_HOME"

# --- manifests against the production fixture ---------------------------------

if ! python3 "$repo_root/scripts/deploy/check-host-storage-fixture.py" >"$scratch/fixture.out" 2>&1; then
  fail "the manifests differ from production's objects:"
  cat "$scratch/fixture.out"
fi

# The same check on a copy whose postgresql StatefulSet has no storage class (as
# before host storage) must fail.
copy="$scratch/copy"
mkdir -p "$copy/scripts/deploy/testdata" "$copy/k8s"
cp "$repo_root/scripts/deploy/check-host-storage-fixture.py" "$repo_root/scripts/deploy/render-public-host.py" \
  "$copy/scripts/deploy/"
cp "$repo_root/scripts/deploy/testdata/production-host-storage.json" "$copy/scripts/deploy/testdata/"
cp -r "$repo_root/k8s/manifests" "$copy/k8s/"
sed -i '/storageClassName: econgraph-host/d' "$copy/k8s/manifests/postgres-deployment.yaml"
if python3 "$copy/scripts/deploy/check-host-storage-fixture.py" >"$scratch/fixture-neg.out" 2>&1; then
  fail "a StatefulSet without storageClassName passed the fixture check"
fi
grep -q "StatefulSet/postgresql.*immutable fields differ" "$scratch/fixture-neg.out" ||
  fail "the fixture check did not name the postgresql StatefulSet's immutable fields"

# --- stubs ---------------------------------------------------------------------

stub="$scratch/bin"
mkdir -p "$stub"
log="$scratch/calls.log"
export STUB_LOG="$log"

# The stub cluster. Every call is logged; nothing is written anywhere else.
#   STUB_NO_LABEL=1        no node carries econgraph.io/host-storage=true
#   STUB_SECRETS           Secrets that exist (default: both database Secrets)
#   STUB_PV_PHASE          phase of both PVs (default Bound)
#   STUB_STOP_AT           a substring of the call at which to exit 99 (ends deploy.sh)
cat >"$stub/kubectl" <<'STUB'
#!/usr/bin/env bash
echo "kubectl $*" >>"$STUB_LOG"
if [ -n "${STUB_STOP_AT:-}" ] && [[ "$*" == *"$STUB_STOP_AT"* ]]; then exit 99; fi
case "$*" in
  *"config current-context"*) echo kind-econ-graph ;;
  *"config use-context"*) ;;
  *"get nodes -l econgraph.io/host-storage=true"*)
    [ "${STUB_NO_LABEL:-0}" = 1 ] || echo node/econ-graph-worker ;;
  *"get secret "*)
    for s in ${STUB_SECRETS-econ-graph-postgres econ-graph-keycloak}; do
      [[ "$*" == *"get secret $s "* ]] && echo "secret/$s"
    done ;;
  *"get pv "*"status.phase"*) echo "${STUB_PV_PHASE:-Bound}" ;;
  *"get pv "*"persistentVolumeReclaimPolicy"*) echo Retain ;;
  *"get pvc "*"-o name"*) echo persistentvolumeclaim/claim ;;
  *"get pvc "*"volumeName"*) echo pv-of-claim ;;
  *"create configmap"*) printf 'apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: keycloak-realm\n' ;;
  *"apply -f -"*) cat >/dev/null ;;
esac
exit 0
STUB
# kind: the cluster exists.
printf '#!/usr/bin/env bash\necho "kind $*" >>"$STUB_LOG"\necho econ-graph\n' >"$stub/kind"
# docker: only `docker exec econ-graph-worker mountpoint|test`.
#   STUB_NODE_UNMOUNTED=1  the node does not bind-mount the subvolumes
#   STUB_DB_PRESENT        "pg", "keycloak-pg", both or empty: which PGDATA exists
cat >"$stub/docker" <<'STUB'
#!/usr/bin/env bash
echo "docker $*" >>"$STUB_LOG"
case "$*" in
  "exec econ-graph-worker mountpoint -q /srv/econgraph/"*) [ "${STUB_NODE_UNMOUNTED:-0}" != 1 ] ;;
  "exec econ-graph-worker test -e /srv/econgraph/data-plane/pg/"*) [[ " ${STUB_DB_PRESENT-} " == *" pg "* ]] ;;
  "exec econ-graph-worker test -e /srv/econgraph/app-prod/keycloak-pg/"*) [[ " ${STUB_DB_PRESENT-} " == *" keycloak-pg "* ]] ;;
  *) exit 1 ;;
esac
STUB
# Host mounts: the scratch subvolumes count as mounted btrfs unless STUB_HOST_UNMOUNTED=1.
printf '#!/usr/bin/env bash\necho "mountpoint $*" >>"$STUB_LOG"\n[ "${STUB_HOST_UNMOUNTED:-0}" != 1 ]\n' >"$stub/mountpoint"
printf '#!/usr/bin/env bash\necho btrfs\n' >"$stub/findmnt"
# Anything that would change ownership or need root fails the test.
for cmd in sudo chown chmod rm rmdir; do
  printf '#!/usr/bin/env bash\necho "FORBIDDEN %s $*" >>"$STUB_LOG"\nexit 1\n' "$cmd" >"$stub/$cmd"
done
chmod +x "$stub"/*
export PATH="$stub:$PATH" KUBECTL="$stub/kubectl"

srv="$ECONGRAPH_HOST_ROOT"
reset_host() {
  /bin/rm -rf "$srv"
  /bin/mkdir -p "$srv/data-plane/pg" "$srv/app-prod/keycloak-pg"
}

# run_apply NAME EXPECT(ok|fail) [ARGS...]: run apply-host-storage.sh with the stubs.
run_apply() {
  local name="$1" expect="$2" rc=0
  shift 2
  : >"$log"
  (cd "$scratch" && CLUSTER_ROLE_CONTEXT=kind-econ-graph \
    "$repo_root/scripts/deploy/apply-host-storage.sh" "$@") >"$scratch/out" 2>&1 || rc=$?
  if [ "$expect" = ok ] && [ "$rc" -ne 0 ]; then
    fail "$name: exited $rc"; sed 's/^/    /' "$scratch/out"
  elif [ "$expect" = fail ] && [ "$rc" -eq 0 ]; then
    fail "$name: succeeded"
  fi
  if grep -Eq 'FORBIDDEN|kubectl .*delete' "$log"; then
    fail "$name: deleted something or changed ownership:"; grep -E 'FORBIDDEN|delete' "$log"
  fi
  if grep -v -e '--context kind-econ-graph' "$log" | grep -q '^kubectl'; then
    fail "$name: a kubectl call left the pinned context"
  fi
}
applied() { grep -q "kubectl --context kind-econ-graph apply -f .*k8s/manifests/host-storage.yaml" "$log"; }

# --- apply-host-storage.sh -------------------------------------------------------

reset_host
run_apply "production layout" ok
applied || fail "production layout: host-storage.yaml not applied"
grep -q 'patch pv' "$log" && fail "production layout: a Bound PV was patched"

reset_host
STUB_DB_PRESENT="pg keycloak-pg" run_apply "existing databases with their Secrets" ok
applied || fail "existing databases with their Secrets: not applied"

STUB_HOST_UNMOUNTED=1 run_apply "host subvolumes not mounted" fail
applied && fail "host subvolumes not mounted: applied anyway"
grep -q 'setup-subvolumes.sh' "$scratch/out" || fail "host subvolumes not mounted: no pointer to setup-subvolumes.sh"

STUB_NO_LABEL=1 run_apply "no labelled node" fail
applied && fail "no labelled node: applied anyway"

STUB_NODE_UNMOUNTED=1 run_apply "node without the bind mounts" fail
applied && fail "node without the bind mounts: applied anyway"

STUB_DB_PRESENT="pg" STUB_SECRETS="econ-graph-keycloak" run_apply "database without its Secret" fail
applied && fail "database without its Secret: applied anyway"
grep -q 'econ-graph-postgres is missing' "$scratch/out" || fail "database without its Secret: not explained"

STUB_DB_PRESENT="keycloak-pg" STUB_SECRETS="econ-graph-postgres" run_apply "Keycloak database without its Secret" fail
applied && fail "Keycloak database without its Secret: applied anyway"

/bin/rm -rf "$srv/data-plane/pg"
run_apply "missing data directory" fail
applied && fail "missing data directory: applied anyway"
[ -e "$srv/data-plane/pg" ] && fail "missing data directory: created without --create-dirs"
grep -q -- '--create-dirs' "$scratch/out" || fail "missing data directory: no pointer to --create-dirs"

run_apply "--create-dirs --dry-run" ok --create-dirs --dry-run
[ -e "$srv/data-plane/pg" ] && fail "--create-dirs --dry-run: created the directory"
applied && fail "--create-dirs --dry-run: applied"
grep -q "kubectl --context kind-econ-graph diff -f .*host-storage.yaml" "$log" ||
  fail "--create-dirs --dry-run: no kubectl diff"

run_apply "--create-dirs" ok --create-dirs
[ -d "$srv/data-plane/pg" ] || fail "--create-dirs: pg/ not created"
[ "$(stat -c %a "$srv/data-plane/pg" 2>/dev/null)" = 700 ] || fail "--create-dirs: pg/ is not mode 0700"
applied || fail "--create-dirs: not applied"

STUB_PV_PHASE=Released run_apply "Released PVs" ok
[ "$(grep -c 'patch pv econgraph-.* --type merge -p {"spec":{"claimRef":{"uid":null,"resourceVersion":null}}}' "$log")" = 2 ] ||
  fail "Released PVs: not both re-armed"

STUB_PV_PHASE=Released run_apply "Released PVs, --dry-run" ok --dry-run
grep -q 'patch pv' "$log" && fail "Released PVs, --dry-run: patched"
grep -q 'apply -f' "$log" && fail "Released PVs, --dry-run: applied"

# --- deploy.sh order ---------------------------------------------------------------

# A copy of the repository pieces deploy.sh runs up to keycloak-postgres, with
# create-secrets.sh stubbed; the stub cluster stops it at Keycloak's rollout, after
# both StatefulSets are applied. No linter scripts are copied, so it skips them.
proj="$scratch/project"
mkdir -p "$proj/scripts/deploy" "$proj/scripts/keycloak" "$proj/k8s" "$proj/config"
cp -r "$repo_root/scripts/deploy/lib" "$proj/scripts/deploy/"
cp "$repo_root/scripts/deploy/"{deploy.sh,apply-host-storage.sh,protect-postgres-pv.sh,render-public-host.py} \
  "$proj/scripts/deploy/"
cp "$repo_root/scripts/keycloak/render-realm.sh" "$proj/scripts/keycloak/"
cp -r "$repo_root/k8s/manifests" "$proj/k8s/"
cp -r "$repo_root/config/keycloak" "$proj/config/"
printf '#!/usr/bin/env bash\necho "create-secrets.sh context=${CLUSTER_ROLE_CONTEXT:-}" >>"$STUB_LOG"\n' \
  >"$proj/scripts/deploy/create-secrets.sh"
/bin/chmod +x "$proj/scripts/deploy/create-secrets.sh"

reset_host
: >"$log"
rc=0
(cd "$scratch" && PUBLIC_HOST='' STUB_DB_PRESENT="pg keycloak-pg" \
  STUB_STOP_AT="rollout status statefulset/keycloak-postgres" \
  "$proj/scripts/deploy/deploy.sh") >"$scratch/deploy.out" 2>&1 || rc=$?
[ "$rc" = 99 ] || { fail "deploy.sh: expected to stop at Keycloak's rollout (99), exited $rc"; tail -20 "$scratch/deploy.out"; }

line_of() { grep -n -m1 -e "$1" "$log" | cut -d: -f1; }
ns="$(line_of 'apply -f k8s/manifests/namespace.yaml')"
storage="$(line_of 'apply -f .*k8s/manifests/host-storage.yaml')"
secrets="$(line_of 'create-secrets.sh')"
pg="$(line_of 'apply -f k8s/manifests/postgres-deployment.yaml')"
kpg="$(line_of 'apply -f k8s/manifests/keycloak/postgres.yaml')"
if [ -z "$ns" ] || [ -z "$storage" ] || [ -z "$secrets" ] || [ -z "$pg" ] || [ -z "$kpg" ]; then
  fail "deploy.sh: missing a step (namespace=$ns host-storage=$storage create-secrets=$secrets postgresql=$pg keycloak-postgres=$kpg)"
elif ! [ "$ns" -lt "$storage" ] || ! [ "$storage" -lt "$secrets" ] || ! [ "$storage" -lt "$pg" ] ||
  ! [ "$storage" -lt "$kpg" ]; then
  fail "deploy.sh: host-storage.yaml is not applied after the namespace and before create-secrets.sh and both StatefulSets"
fi
grep -q "create-secrets.sh context=kind-econ-graph" "$log" || fail "deploy.sh: create-secrets.sh not on the pinned context"
if grep -Eq 'FORBIDDEN|delete (pv|pvc|persistentvolume|storageclass|-f k8s/manifests/host-storage)' "$log"; then
  fail "deploy.sh: deleted storage or changed ownership"
fi
for d in "$srv/data-plane/pg" "$srv/app-prod/keycloak-pg"; do
  [ -d "$d" ] || fail "deploy.sh: $d is gone"
done

if [ "$failures" -gt 0 ]; then
  echo "$failures failure(s)"
  exit 1
fi
echo "ok: host storage manifests, apply-host-storage.sh and deploy.sh order"
