#!/bin/bash
#
# Check that teardown.sh (and protect-postgres-pv.sh, which it runs) never call
# plain `kubectl`, and that every call they make goes to the kind cluster's own
# kubectl with --context kind-econ-graph:
#
#   - teardown.sh --dry-run, and with --include-data-plane --delete-cluster;
#   - the refusals that stop before anything is deleted (no terminal; a
#     current context other than kind-econ-graph);
#   - the backup comes before anything is patched or deleted: a teardown
#     whose PVs need patching to Retain writes the dumps and counts.env to
#     ECONGRAPH_BACKUP_ROOT before the first patch or delete, and a
#     failed dump, or a postgresql-0 that isn't running without --no-backup,
#     stops it with nothing patched or deleted.
#
# Runs offline against stubs, never a cluster: a plain `kubectl` first on PATH
# that records any call and fails; `docker` and `kind` stubs that pretend a kind
# cluster econ-graph exists and hand out a fake "kind kubectl"; and that fake,
# which records every call and answers the few reads teardown makes.
#
# Usage: scripts/deploy/test-teardown-kubectl.sh

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/bin" "$WORK/home" "$WORK/tmp"
PLAIN="$WORK/plain-kubectl.log"
KIND_CALLS="$WORK/kind-kubectl.log"
OTHER="$WORK/other.log"

# Plain kubectl: must never run.
cat >"$WORK/bin/kubectl" <<EOF
#!/bin/sh
echo "\$*" >>"$PLAIN"
echo "plain kubectl stub: refusing" >&2
exit 1
EOF

# The fake kind kubectl that docker cp hands out. FAKE_CURRENT_CONTEXT, when
# set, is what it reports as the current context after use-context;
# FAKE_PRODUCTION=true makes the cluster carry a production marker for its own
# kube-system UID; FAKE_PHASE is every pod's phase (default Running);
# FAKE_DUMP_FAILS set makes pg_dump fail; FAKE_RECLAIM is every PV's reclaim
# policy (default Retain, so protect-postgres-pv.sh patches nothing; Delete makes
# it patch). pg_dump prints a stub archive and psql a count, so a backup can
# complete.
CLUSTER_UID="11111111-2222-3333-4444-555555555555"
cat >"$WORK/kind-kubectl" <<EOF
#!/bin/sh
# One line per call (the Secret go-template argument spans several lines).
echo "\$*" | tr '\\n' ' ' >>"$KIND_CALLS"; echo >>"$KIND_CALLS"
case " \$* " in
    *" version "*) exit 0 ;;
    *" config use-context "*) exit 0 ;;
    *" config current-context"*) echo "\${FAKE_CURRENT_CONTEXT:-kind-econ-graph}"; exit 0 ;;
    *" config view "*) echo "https://127.0.0.1:6443"; exit 0 ;;
    *" get namespace kube-system "*) echo "$CLUSTER_UID"; exit 0 ;;
    *" get configmap econgraph-cluster-role "*)
        # The marker (lib/cluster-role.sh): exists, then each value with a trailing "#".
        [ "\${FAKE_PRODUCTION:-false}" = true ] || exit 0
        case " \$* " in
            *" -o name "*) echo configmap/econgraph-cluster-role ;;
            *"{.data.role}"*) printf 'production#' ;;
            *) printf '%s#' "$CLUSTER_UID" ;;
        esac
        exit 0 ;;
    *"{.status.phase}"*) echo "\${FAKE_PHASE:-Running}"; exit 0 ;;
    *" pg_dump "*) [ -z "\${FAKE_DUMP_FAILS:-}" ] || exit 1; echo "PGDMP stub archive"; exit 0 ;;
    *"to_regclass"*) echo t; exit 0 ;;
    *" psql "*) echo 7; exit 0 ;;
    *" -o name"*) echo "persistentvolumeclaim/stub"; exit 0 ;;
    *"{.spec.volumeName}"*) echo pv-stub; exit 0 ;;
    *" patch "*) exit 0 ;;
    *"persistentVolumeReclaimPolicy"*) echo "\${FAKE_RECLAIM:-Retain}"; exit 0 ;;
esac
exit 0
EOF

# docker exec on the control-plane node reports FAKE_NODE_UID (default: the
# same cluster's UID) for kube-system.
cat >"$WORK/bin/docker" <<EOF
#!/bin/sh
echo "docker \$*" >>"$OTHER"
case "\$1" in
    info) echo "28.0.0 /var/lib/docker [name=seccomp]" ;;
    cp) for last; do :; done; cp "$WORK/kind-kubectl" "\$last" ;;
    exec)
        case " \$* " in
            *" get namespace kube-system "*) echo "\${FAKE_NODE_UID:-$CLUSTER_UID}" ;;
        esac ;;
esac
exit 0
EOF

cat >"$WORK/bin/kind" <<EOF
#!/bin/sh
echo "kind \$*" >>"$OTHER"
case "\$1 \$2" in
    "get clusters") echo econ-graph ;;
    "get kubeconfig") echo "apiVersion: v1" ;;
    "version "*) echo "kind v0.33.0 go1.26.7 linux/amd64" ;;
esac
exit 0
EOF
chmod +x "$WORK/bin/"* "$WORK/kind-kubectl"
# The scripts run kind from its pinned path (lib/kind-version.sh); the stub stands in there too.
mkdir -p "$WORK/home/.local/share/econgraph/bin"
cp "$WORK/bin/kind" "$WORK/home/.local/share/econgraph/bin/kind-v0.33.0"

# The environment teardown.sh runs in: the stubs first on PATH, no kube config,
# and none of the caller's settings that would point it at another cluster,
# Docker daemon or namespace, or relax the backup.
TEARDOWN_ENV=(
    -u KUBECONFIG -u KUBECTL -u KIND -u KIND_CLUSTER -u CLUSTER_ROLE_CONTEXT -u DOCKER_CONTEXT
    -u KIND_EXPERIMENTAL_PROVIDER -u ECONGRAPH_DOCKER_HOST -u ECONGRAPH_AGENT_SESSION
    -u ECONGRAPH_KUBE_DIR -u ECONGRAPH_SCRATCH_STORAGE -u DRY_RUN -u ASSUME_YES -u NAMESPACE
    -u ALLOW_MISSING_APP_DUMP -u ALLOW_MISSING_KEYCLOAK_DUMP
    HOME="$WORK/home" XDG_DATA_HOME="$WORK/home/.local/share" XDG_CACHE_HOME="$WORK/cache" TMPDIR="$WORK/tmp"
    ECONGRAPH_BACKUP_ROOT="$WORK/backups" PATH="$WORK/bin:/usr/local/bin:/usr/bin:/bin"
)

# teardown CMD-ARGS...: teardown.sh in TEARDOWN_ENV, stdin not a terminal.
# Prints its output; returns its exit status.
teardown() {
    env "${TEARDOWN_ENV[@]}" "$PROJECT_ROOT/scripts/deploy/teardown.sh" "$@" </dev/null 2>&1
}

failed=0
fail() { echo "FAIL $*"; failed=1; }

# check NAME: no plain kubectl, and every fake kind kubectl call (other than
# kind-env.sh's reachability check, `version`) carries --context kind-econ-graph.
check() {
    local name="$1" bad
    if [ -s "$PLAIN" ]; then
        fail "$name called plain kubectl:"
        sed 's/^/     /' "$PLAIN"
    fi
    bad="$(grep -v -- '--context kind-econ-graph' "$KIND_CALLS" | grep -vE '^(version|config use-context kind-econ-graph)( |$)' || true)"
    if [ -n "$bad" ]; then
        fail "$name made kind kubectl calls without --context kind-econ-graph:"
        printf '     %s\n' "$bad"
    fi
}
reset() { : >"$PLAIN"; : >"$KIND_CALLS"; : >"$OTHER"; }

reset
out="$(teardown --dry-run)"; rc=$?
if [ "$rc" -eq 0 ] && grep -q "Dry run complete" <<<"$out" &&
    grep -q "Cluster: context kind-econ-graph, API server https://127.0.0.1:6443" <<<"$out"; then
    echo "ok   teardown.sh --dry-run completes and names the cluster"
else
    fail "teardown.sh --dry-run (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --dry-run"
grep -q "kube-system UID $CLUSTER_UID" <<<"$out" || fail "teardown.sh --dry-run did not print the kube-system UID"
if grep -q "use-context" "$KIND_CALLS"; then
    fail "teardown.sh --dry-run ran kubectl config use-context"
else
    echo "ok   teardown.sh --dry-run never runs config use-context"
fi
grep -q -- '--context kind-econ-graph get pvc postgresql-data-postgresql-0' "$KIND_CALLS" ||
    fail "protect-postgres-pv.sh --dry-run did not read the PVC through the kind kubectl"
[ -s "$PLAIN" ] || echo "ok   no plain kubectl in teardown.sh --dry-run (with protect-postgres-pv.sh)"

reset
out="$(teardown --dry-run --include-data-plane --delete-cluster)"; rc=$?
if [ "$rc" -eq 0 ] && grep -q "+ kind delete cluster" <<<"$out"; then
    echo "ok   teardown.sh --dry-run --include-data-plane --delete-cluster completes"
else
    fail "teardown.sh --dry-run --include-data-plane --delete-cluster (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --dry-run --include-data-plane --delete-cluster"
grep -q "^kind delete" "$OTHER" && fail "a dry run ran kind delete"

reset
out="$(FAKE_CURRENT_CONTEXT=microk8s teardown --yes)"; rc=$?
if [ "$rc" -ne 0 ] && grep -q "context is microk8s, not kind-econ-graph; nothing was changed" <<<"$out"; then
    echo "ok   teardown.sh refuses when the current context is not kind-econ-graph"
else
    fail "teardown.sh with context microk8s (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh with the wrong context"
grep -qE -- ' (delete|patch) ' "$KIND_CALLS" && fail "teardown.sh deleted or patched before refusing the context"

reset
out="$(teardown)"; rc=$?
if [ "$rc" -ne 0 ] && grep -q "no terminal to confirm on" <<<"$out"; then
    echo "ok   teardown.sh refuses without a terminal or --yes"
else
    fail "teardown.sh without a terminal (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh without a terminal"
[ -s "$KIND_CALLS" ] && fail "teardown.sh without a terminal called kubectl before refusing"

reset
out="$(teardown --include-data-plane --yes)"; rc=$?
if [ "$rc" -ne 0 ] && grep -q -- "--yes doesn't answer it" <<<"$out"; then
    echo "ok   teardown.sh --include-data-plane --yes refuses without a terminal"
else
    fail "teardown.sh --include-data-plane --yes (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --include-data-plane --yes"

reset
out="$(FAKE_NODE_UID=99999999-0000-0000-0000-000000000000 teardown --yes)"; rc=$?
if [ "$rc" -ne 0 ] && grep -q "kubectl and kind would act on different clusters; nothing was changed" <<<"$out"; then
    echo "ok   teardown.sh refuses when kubectl and the kind node report different kube-system UIDs"
else
    fail "teardown.sh with a UID mismatch (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh with a UID mismatch"
grep -qE -- ' (delete|patch|exec) ' "$KIND_CALLS" && fail "teardown.sh deleted, patched or dumped before refusing the UID mismatch"

reset
out="$(FAKE_PRODUCTION=true teardown --dry-run)"; rc=$?
if [ "$rc" -eq 0 ] && grep -q "This cluster is marked production" <<<"$out" &&
    grep -q "would ask you to type '${CLUSTER_UID:0:8}'" <<<"$out"; then
    echo "ok   teardown.sh --dry-run on a production-marked cluster would ask for the UID prefix"
else
    fail "teardown.sh --dry-run on a production cluster (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --dry-run on a production cluster"

# On a terminal (script(1) gives it one): the namespace typed right, then a
# wrong UID prefix, must abort with nothing deleted. Pods report Pending, so
# no backup is attempted (--no-backup).
reset
out="$(printf 'econ-graph\nwrong\n' | env "${TEARDOWN_ENV[@]}" FAKE_PRODUCTION=true FAKE_PHASE=Pending \
    script -qec "$PROJECT_ROOT/scripts/deploy/teardown.sh --no-backup" /dev/null 2>&1)"; rc=$?
if [ "$rc" -ne 0 ] && grep -q "Type '${CLUSTER_UID:0:8}' to continue" <<<"$out" &&
    grep -q "aborted; nothing was deleted" <<<"$out"; then
    echo "ok   teardown.sh on a production-marked cluster asks for the UID prefix and aborts on a wrong one"
else
    fail "teardown.sh on a production cluster with a wrong UID prefix (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh on a production cluster"
grep -qE -- ' delete ' "$KIND_CALLS" && fail "teardown.sh deleted something after a wrong UID prefix"

# --confirm-production: a matching prefix lets a production teardown go ahead
# with --yes and no terminal; a wrong one is refused on any cluster; --yes alone
# is refused on production. Pods report Pending, so no backup is attempted.
reset
out="$(FAKE_PRODUCTION=true FAKE_PHASE=Pending teardown --yes --no-backup --confirm-production "${CLUSTER_UID:0:8}")"; rc=$?
if [ "$rc" -eq 0 ] && grep -q "confirm-production ${CLUSTER_UID:0:8} matches this cluster" <<<"$out" &&
    grep -q "App plane removed" <<<"$out"; then
    echo "ok   teardown.sh --yes --confirm-production <right prefix> tears down a production cluster"
else
    fail "teardown.sh --yes with the right --confirm-production (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --yes --confirm-production <right prefix>"

reset
out="$(FAKE_PHASE=Pending teardown --yes --no-backup --confirm-production deadbeef)"; rc=$?
if [ "$rc" -ne 0 ] &&
    grep -q "confirm-production deadbeef does not match this cluster's kube-system UID; nothing was changed" <<<"$out"; then
    echo "ok   teardown.sh refuses a --confirm-production that doesn't match, on a cluster not marked production"
else
    fail "teardown.sh with a wrong --confirm-production (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh with a wrong --confirm-production"
grep -qE -- ' (delete|patch|exec) ' "$KIND_CALLS" && fail "teardown.sh changed something before refusing a wrong --confirm-production"

reset
out="$(FAKE_PRODUCTION=true teardown --yes)"; rc=$?
if [ "$rc" -ne 0 ] &&
    grep -q "tearing down the production cluster needs --confirm-production with the first 8 characters" <<<"$out"; then
    echo "ok   teardown.sh --yes without --confirm-production refuses on a production cluster"
else
    fail "teardown.sh --yes on a production cluster (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh --yes on a production cluster"
grep -qE -- ' (delete|patch|exec) ' "$KIND_CALLS" && fail "teardown.sh changed or dumped something before refusing --yes on production"

# The backup comes first: a teardown dumps the databases into
# ECONGRAPH_BACKUP_ROOT before it patches or deletes anything. The PVs report
# reclaim policy Delete, so protect-postgres-pv.sh patches them, and the first
# patch has to come after the first dump too.
reset
rm -rf "$WORK/backups"
out="$(FAKE_RECLAIM=Delete teardown --yes)"; rc=$?
backup_dir="$(find "$WORK/backups" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | head -1)"
first_dump="$(grep -n ' pg_dump ' "$KIND_CALLS" | head -1 | cut -d: -f1)"
first_patch="$(grep -n ' patch ' "$KIND_CALLS" | head -1 | cut -d: -f1)"
first_change="$(grep -nE ' (delete|patch) ' "$KIND_CALLS" | head -1 | cut -d: -f1)"
if [ "$rc" -eq 0 ] && grep -q "App plane removed" <<<"$out" && [ -n "$backup_dir" ] &&
    [ -s "$backup_dir/econ_graph.dump" ] && grep -q '^SERIES_COUNT=7$' "$backup_dir/counts.env" 2>/dev/null &&
    [ -n "$first_dump" ] && [ -n "$first_patch" ] && [ -n "$first_change" ] &&
    [ "$first_dump" -lt "$first_patch" ] && [ "$first_dump" -lt "$first_change" ]; then
    echo "ok   teardown.sh --yes dumps the databases and counts.env before it patches a PV to Retain or deletes anything"
else
    fail "teardown.sh --yes did not back up first (exit $rc, first pg_dump at call ${first_dump:-none}, first patch at ${first_patch:-none}, first patch/delete at ${first_change:-none}):"
    printf '     %s\n' "$out"
fi
check "teardown.sh --yes with a backup"

reset
rm -rf "$WORK/backups"
out="$(FAKE_DUMP_FAILS=1 teardown --yes)"; rc=$?
if [ "$rc" -ne 0 ] && grep -q "pg_dump econ_graph from postgresql-0" <<<"$out" &&
    ! grep -qE -- ' (delete|patch) ' "$KIND_CALLS"; then
    echo "ok   teardown.sh stops, with nothing patched or deleted, when the dump fails"
else
    fail "teardown.sh with a failing pg_dump (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh with a failing pg_dump"

reset
out="$(FAKE_PHASE=Pending teardown --yes)"; rc=$?
if [ "$rc" -ne 0 ] && grep -q "no backup can be taken first; nothing was changed" <<<"$out" &&
    ! grep -qE -- ' (delete|patch) ' "$KIND_CALLS"; then
    echo "ok   teardown.sh refuses without --no-backup when postgresql-0 is not running"
else
    fail "teardown.sh with postgresql-0 not running (exit $rc):"; printf '     %s\n' "$out"
fi
check "teardown.sh with postgresql-0 not running"

if [ "$failed" -ne 0 ]; then
    echo "teardown kubectl checks failed"
    exit 1
fi
echo "teardown kubectl checks passed"
