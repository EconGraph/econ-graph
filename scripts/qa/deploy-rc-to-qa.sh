#!/usr/bin/env bash
# Stand up a QA environment for a release candidate: a scratch kind cluster running the
# candidate's release images on a copy of production's data, with QA accounts. Idempotent:
# rerun it to finish or refresh one; --teardown removes it.
#
# Usage:
#   scripts/qa/deploy-rc-to-qa.sh --tag vX.Y.Z-rc.N --dump-dir DIR [--name NAME] [--dry-run]
#   scripts/qa/deploy-rc-to-qa.sh --tag vX.Y.Z-rc.N --teardown [--name NAME] [--dry-run]
#
#   --tag TAG       the release candidate, as release.yml built and published it
#   --dump-dir DIR  a verified backup directory (backup-kind-databases.sh: econ_graph.dump,
#                   keycloak.dump, econ_graph_app.dump when there is one, counts.env,
#                   secrets/). Read only; never written. Must not be inside the QA root.
#   --name NAME     the QA environment and its kind cluster (default qa-<tag without dots>,
#                   e.g. qa-v410-rc1): qa- followed by lowercase letters, digits and
#                   inner hyphens, at most 32 characters in all
#   --dry-run       run the read-only checks and print the plan; create or change nothing
#   --teardown      delete that cluster and its QA directory (never the dump)
#
# Everything lives in ${ECONGRAPH_QA_ROOT:-~/econgraph-qa}/NAME/:
#   src/        a detached git worktree of TAG: the candidate's manifests and deploy scripts
#               run, not this checkout's
#   storage/    btrfs subvolumes data-plane and app-prod (the scratch host storage,
#               docs/deployment/HOST_STORAGE.md, "Testing on a scratch cluster")
#   home/       HOME for the candidate's scripts, so no default path of theirs
#               (~/econgraph-backups, ~/econgraph-releases, ~/.config/econgraph, ~/.kube)
#               is the operator's; cache/ (XDG_CACHE_HOME: the cluster's kubectl and
#               kubeconfig), backups/ (ECONGRAPH_BACKUP_ROOT), releases/
#               (ECONGRAPH_RELEASES_DIR), deploy.env (ECONGRAPH_DEPLOY_ENV)
#   kind-config.yaml  the candidate's terraform/k8s/kind-config.yaml with its port
#               mappings replaced by one: 127.0.0.1:PORT to the gateway's port 80
#   state.env   the environment's id, port, tag and dump directory
#   accounts.env, accounts/*.password  the QA accounts (mode 600; never printed)
#
# What it does, each step skipped or converged when already done:
#   1. Refuses, before changing anything: a name that isn't qa-*, the production cluster
#      names econ-graph and econgraph-train1-qa, a cluster with a production marker
#      (ConfigMap kube-system/econgraph-cluster-role), an existing cluster this script
#      did not create (no kube-system/econgraph-qa-owner naming this environment's id), a
#      dump directory inside the QA root (or containing it), and a tag without the scripts
#      it runs (scripts/deploy/restore-kind-databases.sh and the others, from v4.1).
#   2. Fetches tags and makes the worktree src/ at TAG.
#   3. Creates the btrfs subvolumes and the kind config, then the cluster with the
#      candidate's recreate-kind-cluster.sh --if-missing, and marks it as this
#      environment's (econgraph-qa-owner). Every kubectl call checks first that the
#      kubeconfig's cluster is the one the control-plane container reports (its
#      kube-system UID).
#   4. Loads TAG's release images with the candidate's load-release-images.sh
#      (--cluster NAME, which verifies the GHCR digests and revision labels); a fresh
#      cluster has no previous images to keep, so --allow-missing-nodes.
#   5. Restores the dump's databases and saved Secrets with the candidate's
#      restore-kind-databases.sh (APP_DATABASE=shared when the dump has no app
#      database), then deletes the restored crawler-api-keys Secret: production's
#      upstream API keys stay in production.
#   6. Deploys with the candidate's deploy.sh, PUBLIC_HOST empty and no API keys in its
#      environment; cloudflared stays at 0 replicas (the cluster is not marked production).
#   7. Moves sign-in to http://localhost:PORT: Keycloak's KC_HOSTNAME and web origins, the
#      realm import's redirect URIs, the backend's OIDC_ISSUER and CORS_ORIGIN. Without
#      this, sign-in would send the browser to http://localhost, port 80: production's
#      gateway on this host.
#   8. Scales crawler-worker to 0, so the restored crawl queue reaches no upstream API
#      (kubectl ... scale deployment/crawler-worker --replicas=1 to crawl, keyless sources
#      only).
#   9. Creates or resets the QA users qa-a, qa-b and qa-admin (realm role admin) with
#      random passwords, passed to Keycloak on stdin, never in arguments or output.
#  10. Checks the dump directory is unchanged and prints the URL, the accounts file and
#      the values for the Linear QA prompt.
#
# Needs: git, python3 with PyYAML, btrfs-progs, the rootful Docker daemon
# (ECONGRAPH_DOCKER_HOST, default unix:///var/run/docker.sock) and `docker login ghcr.io`.
# Exit status: 0 done; 1 a step failed; 2 usage; 3 refused.
set -euo pipefail

SCRIPT_REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
QA_ROOT="${ECONGRAPH_QA_ROOT:-$HOME/econgraph-qa}"
REAL_HOME="$HOME"
REAL_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
export DOCKER_HOST="${ECONGRAPH_DOCKER_HOST:-unix:///var/run/docker.sock}"
unset DOCKER_CONTEXT
# Production's cluster names; a QA environment never takes or touches them.
PRODUCTION_CLUSTERS=(econ-graph econgraph-train1-qa)
QA_USERS=(qa-a qa-b qa-admin)
REALM=econ-graph
NAMESPACE=econ-graph
EXIT_FAILED=1 EXIT_USAGE=2 EXIT_REFUSED=3

TAG="" DUMP_DIR="" NAME="" DRY_RUN=false TEARDOWN=false
usage() { sed -n '4,5p' "$0" | sed 's/^# \{0,1\}//'; }
die() { echo "error: $*" >&2; exit "$EXIT_FAILED"; }
refuse() { echo "refused: $*" >&2; exit "$EXIT_REFUSED"; }
log() { echo "==> $*"; }
while [ $# -gt 0 ]; do
    case "$1" in
        --tag|--dump-dir|--name)
            [ $# -ge 2 ] && [ -n "$2" ] || { echo "$1 needs a value" >&2; exit "$EXIT_USAGE"; }
            case "$1" in
                --tag) TAG="$2" ;;
                --dump-dir) DUMP_DIR="$2" ;;
                --name) NAME="$2" ;;
            esac
            shift 2 ;;
        --dry-run) DRY_RUN=true; shift ;;
        --teardown) TEARDOWN=true; shift ;;
        -h|--help) sed -n '2,/^set -euo/p' "$0" | sed '$d' | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) usage >&2; echo "unknown argument: $1" >&2; exit "$EXIT_USAGE" ;;
    esac
done
[ -n "$TAG" ] || { usage >&2; exit "$EXIT_USAGE"; }
[[ "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+-rc\.[0-9]+$ ]] ||
    { echo "--tag must be vX.Y.Z-rc.N, not ${TAG}" >&2; exit "$EXIT_USAGE"; }
if [ "$TEARDOWN" = false ] && [ -z "$DUMP_DIR" ]; then
    usage >&2; echo "--dump-dir is required" >&2; exit "$EXIT_USAGE"
fi
NAME="${NAME:-qa-${TAG//./}}"
NAME="${NAME,,}"

# --- Refusals that need nothing but the arguments ------------------------------------------
for production in "${PRODUCTION_CLUSTERS[@]}"; do
    [ "$NAME" != "$production" ] || refuse "${NAME} is a production cluster"
done
[[ "$NAME" =~ ^qa-[a-z0-9]([a-z0-9-]{0,27}[a-z0-9])?$ ]] ||
    refuse "--name must be qa- then lowercase letters, digits and inner hyphens (at most 32 characters), not ${NAME}"
CONTEXT="kind-${NAME}"
CONTROL_PLANE="${NAME}-control-plane"

QA_ROOT="$(realpath -m "$QA_ROOT")"
case "$QA_ROOT" in
    /tmp|/tmp/*|/dev/shm|/dev/shm/*) refuse "the QA root ${QA_ROOT} is on a RAM filesystem; set ECONGRAPH_QA_ROOT under /home" ;;
    /srv/econgraph|/srv/econgraph/*) refuse "the QA root ${QA_ROOT} is production's host storage" ;;
esac
QA_DIR="${QA_ROOT}/${NAME}"
SRC="${QA_DIR}/src"
STORAGE="${QA_DIR}/storage"
STATE="${QA_DIR}/state.env"
QA_KUBECTL="${QA_DIR}/cache/econgraph/bin/kubectl"
QA_KUBECONFIG="${QA_DIR}/cache/econgraph/kubeconfig-${NAME}"
KIND_BIN="${REAL_DATA_HOME}/econgraph/bin/kind-v0.33.0"

# state_get KEY: a value from state.env, or nothing.
state_get() {
    [ -f "$STATE" ] || return 0
    sed -n "s/^$1=//p" "$STATE" | tail -1
}
# state_set KEY VALUE: record KEY in state.env (mode 600), replacing an earlier value.
state_set() {
    python3 - "$STATE" "$1" "$2" <<'PY'
import os, sys, tempfile
path, key, value = sys.argv[1:]
lines = []
if os.path.exists(path):
    lines = [l for l in open(path).read().splitlines() if not l.startswith(key + '=')]
lines.append(f'{key}={value}')
fd, tmp = tempfile.mkstemp(dir=os.path.dirname(path), prefix='.state.')
with os.fdopen(fd, 'w') as out:
    out.write('\n'.join(lines) + '\n')
os.chmod(tmp, 0o600)
os.replace(tmp, path)
PY
}

# The cluster's own view, read in its control-plane container (no kubeconfig involved).
cluster_exists() {
    [ -n "$(docker ps -a --filter "label=io.x-k8s.kind.cluster=${NAME}" --format '{{.Names}}')" ]
}
node_kubectl() {
    docker exec "$CONTROL_PLANE" kubectl --kubeconfig /etc/kubernetes/admin.conf "$@"
}
node_uid() { node_kubectl get namespace kube-system -o 'jsonpath={.metadata.uid}'; }
# The production marker: any ConfigMap kube-system/econgraph-cluster-role, valid or not.
# A marker that can't be read counts as one.
refuse_production_marker() {
    local marker
    marker="$(node_kubectl -n kube-system get configmap econgraph-cluster-role --ignore-not-found -o name)" ||
        refuse "could not read ${NAME}'s production marker"
    [ -z "$marker" ] || refuse "${NAME} has a production marker (kube-system/econgraph-cluster-role)"
}
owner_id() {
    node_kubectl -n kube-system get configmap econgraph-qa-owner --ignore-not-found -o 'jsonpath={.data.id}'
}
# qk ARGS: the cluster's kubectl on its own kubeconfig and context, after checking that
# they reach the cluster whose control-plane container this environment owns.
QK_CHECKED=false
qk() {
    if [ "$QK_CHECKED" = false ]; then
        # The cluster's own kubectl and a kubeconfig from kind, kept in the QA directory.
        mkdir -p "$(dirname "$QA_KUBECTL")"
        if [ ! -x "$QA_KUBECTL" ]; then
            if ! { docker cp -q -L "${CONTROL_PLANE}:/usr/bin/kubectl" "${QA_KUBECTL}.new" &&
                chmod 0755 "${QA_KUBECTL}.new" && mv -f "${QA_KUBECTL}.new" "$QA_KUBECTL"; }; then
                die "could not copy kubectl from ${CONTROL_PLANE}"
            fi
        fi
        if [ ! -s "$QA_KUBECONFIG" ]; then
            if ! { (umask 077; "$KIND_BIN" get kubeconfig --name "$NAME" >"${QA_KUBECONFIG}.new") &&
                mv -f "${QA_KUBECONFIG}.new" "$QA_KUBECONFIG"; }; then
                die "could not get ${NAME}'s kubeconfig"
            fi
        fi
        local via_kubeconfig via_node
        via_kubeconfig="$("$QA_KUBECTL" --kubeconfig "$QA_KUBECONFIG" --context "$CONTEXT" \
            get namespace kube-system -o 'jsonpath={.metadata.uid}')" || die "${CONTEXT} does not answer"
        via_node="$(node_uid)" || die "could not read ${NAME}'s kube-system UID"
        [ -n "$via_node" ] && [ "$via_kubeconfig" = "$via_node" ] ||
            refuse "${QA_KUBECONFIG} context ${CONTEXT} reaches another cluster than ${CONTROL_PLANE}"
        QK_CHECKED=true
    fi
    "$QA_KUBECTL" --kubeconfig "$QA_KUBECONFIG" --context "$CONTEXT" "$@"
}

# rc ARGS: run one of the candidate's scripts from src/, with every default path of theirs
# inside the QA directory and no upstream API key or production kubeconfig in reach.
rc() {
    (
        cd "$SRC"
        env -u KUBECONFIG -u KUBECTL -u CLUSTER_ROLE_CONTEXT -u DOCKER_CONTEXT -u ECONGRAPH_KUBE_DIR \
            -u FRED_API_KEY -u BLS_API_KEY -u BEA_API_KEY -u CENSUS_API_KEY -u WTO_API_KEY \
            -u DATABASE_URL -u DATA_DATABASE_URL -u MAIN_HOST -u MAIN_HOST_PERMANENT \
            HOME="${QA_DIR}/home" DOCKER_CONFIG="${DOCKER_CONFIG:-${REAL_HOME}/.docker}" \
            XDG_DATA_HOME="$REAL_DATA_HOME" XDG_CACHE_HOME="${QA_DIR}/cache" \
            XDG_CONFIG_HOME="${QA_DIR}/home/.config" XDG_STATE_HOME="${QA_DIR}/home/.local/state" \
            KIND_CLUSTER="$NAME" ECONGRAPH_SCRATCH_STORAGE="$STORAGE" \
            ECONGRAPH_BACKUP_ROOT="${QA_DIR}/backups" ECONGRAPH_RELEASES_DIR="${QA_DIR}/releases" \
            ECONGRAPH_DEPLOY_ENV="${QA_DIR}/deploy.env" ECONGRAPH_CONFIG_DIR="${QA_DIR}/home/.config/econgraph" \
            TMPDIR="${QA_DIR}/tmp" PUBLIC_HOST= \
            "$@"
    )
}

# The dump directory: every file's size and modification time, to show it unchanged.
dump_fingerprint() {
    python3 - "$1" <<'PY'
import os, sys
root = sys.argv[1]
for dirpath, dirnames, filenames in sorted(os.walk(root)):
    dirnames.sort()
    for name in sorted(filenames):
        st = os.lstat(os.path.join(dirpath, name))
        print(os.path.relpath(os.path.join(dirpath, name), root), st.st_size, st.st_mtime_ns)
PY
}

# --- Teardown ------------------------------------------------------------------------------
if [ "$TEARDOWN" = true ]; then
    [ -d "$QA_DIR" ] || cluster_exists || { echo "nothing to tear down for ${NAME}"; exit 0; }
    if cluster_exists; then
        refuse_production_marker
        expected="$(state_get QA_ID)"
        actual="$(owner_id)" || refuse "could not read ${NAME}'s owner marker"
        [ -n "$expected" ] && [ "$actual" = "$expected" ] ||
            refuse "${NAME} is not this QA environment's cluster (kube-system/econgraph-qa-owner '${actual}', ${STATE} '${expected}')"
    fi
    dump="$(state_get DUMP_DIR)"
    if [ -n "$dump" ]; then
        case "$(realpath -m "$dump")/" in
            "$QA_DIR"/*) refuse "the recorded dump ${dump} is inside ${QA_DIR}" ;;
        esac
    fi
    if [ "$DRY_RUN" = true ]; then
        cluster_exists && echo "[dry-run] delete kind cluster ${NAME}"
        echo "[dry-run] remove the Postgres files in ${STORAGE}, its subvolumes, the worktree ${SRC} and ${QA_DIR}"
        exit 0
    fi
    if cluster_exists; then
        log "Deleting kind cluster ${NAME}"
        mkdir -p "${QA_DIR}/cache/econgraph"
        "$KIND_BIN" delete cluster --name "$NAME" --kubeconfig "${QA_DIR}/cache/econgraph/kind-delete.kubeconfig"
    fi
    for volume in data-plane app-prod; do
        [ -d "${STORAGE}/${volume}" ] || continue
        # Postgres leaves files owned by UID 999; remove them through Docker.
        docker run --rm -v "${STORAGE}/${volume}:/s" postgres:18 \
            find /s -mindepth 1 -maxdepth 1 -exec rm -rf {} +
        btrfs subvolume delete "${STORAGE}/${volume}" >/dev/null
    done
    if [ -e "$SRC" ]; then
        git -C "$SCRIPT_REPO" worktree remove --force "$SRC"
    fi
    git -C "$SCRIPT_REPO" worktree prune
    rm -rf -- "$QA_DIR"
    echo "Removed QA environment ${NAME}. The dump directory was not touched."
    exit 0
fi

# --- Checks before any change --------------------------------------------------------------
DUMP_DIR="$(realpath -e "$DUMP_DIR" 2>/dev/null)" || refuse "--dump-dir does not exist"
[ -d "$DUMP_DIR" ] || refuse "--dump-dir ${DUMP_DIR} is not a directory"
case "$DUMP_DIR/" in
    "$QA_ROOT"/*) refuse "the dump ${DUMP_DIR} is inside the QA root ${QA_ROOT}" ;;
esac
case "$QA_ROOT/" in
    "$DUMP_DIR"/*) refuse "the QA root ${QA_ROOT} is inside the dump ${DUMP_DIR}" ;;
esac
[ -s "${DUMP_DIR}/counts.env" ] || refuse "${DUMP_DIR} has no counts.env: not a verified backup directory"
[ -s "${DUMP_DIR}/econ_graph.dump" ] || refuse "${DUMP_DIR} has no econ_graph.dump"
[ -s "${DUMP_DIR}/keycloak.dump" ] || refuse "${DUMP_DIR} has no keycloak.dump"
recorded_dump="$(state_get DUMP_DIR)"
[ -z "$recorded_dump" ] || [ "$recorded_dump" = "$DUMP_DIR" ] ||
    refuse "${NAME} was restored from ${recorded_dump}; tear it down to use another dump"
recorded_tag="$(state_get TAG)"
[ -z "$recorded_tag" ] || [ "$recorded_tag" = "$TAG" ] ||
    refuse "${NAME} runs ${recorded_tag}; tear it down or choose another --name for ${TAG}"
DUMP_BEFORE="$(dump_fingerprint "$DUMP_DIR")"

git -C "$SCRIPT_REPO" fetch --quiet --tags origin || die "git fetch --tags failed"
COMMIT="$(git -C "$SCRIPT_REPO" rev-parse --verify --quiet "refs/tags/${TAG}^{commit}")" ||
    refuse "no tag ${TAG}"
for required in scripts/deploy/recreate-kind-cluster.sh scripts/deploy/load-release-images.sh \
    scripts/deploy/restore-kind-databases.sh scripts/deploy/deploy.sh \
    terraform/k8s/kind-config.yaml k8s/manifests/keycloak/realm-import-job.yaml; do
    git -C "$SCRIPT_REPO" cat-file -e "${COMMIT}:${required}" 2>/dev/null ||
        refuse "${TAG} has no ${required}; this script deploys release candidates from v4.1"
done

if cluster_exists; then
    refuse_production_marker
    actual="$(owner_id)" || refuse "could not read ${NAME}'s owner marker"
    expected="$(state_get QA_ID)"
    if [ -n "$actual" ]; then
        [ -n "$expected" ] && [ "$actual" = "$expected" ] ||
            refuse "cluster ${NAME} belongs to another QA environment (kube-system/econgraph-qa-owner)"
    elif [ "$(state_get CLUSTER)" != creating ]; then
        refuse "cluster ${NAME} exists but was not created by this script (no kube-system/econgraph-qa-owner)"
    fi
fi

if [ "$DRY_RUN" = true ]; then
    echo "[dry-run] QA environment ${NAME} in ${QA_DIR}, tag ${TAG} (${COMMIT}), dump ${DUMP_DIR}"
    echo "[dry-run] worktree ${SRC} at ${TAG}; btrfs subvolumes ${STORAGE}/{data-plane,app-prod}"
    echo "[dry-run] kind cluster ${NAME} (context ${CONTEXT}), gateway on 127.0.0.1 only"
    echo "[dry-run] ${TAG}'s load-release-images.sh, restore-kind-databases.sh, deploy.sh (PUBLIC_HOST empty)"
    echo "[dry-run] sign-in moved to http://localhost:PORT; crawler-worker at 0; QA users ${QA_USERS[*]}"
    exit 0
fi

# --- 2. The candidate's checkout -----------------------------------------------------------
mkdir -p "$QA_DIR"
chmod 700 "$QA_DIR"
mkdir -p "${QA_DIR}/home" "${QA_DIR}/cache" "${QA_DIR}/backups" "${QA_DIR}/releases" "${QA_DIR}/tmp"
[ -n "$(state_get QA_ID)" ] || state_set QA_ID "$(python3 -c 'import secrets; print(secrets.token_hex(16))')"
state_set TAG "$TAG"
state_set DUMP_DIR "$DUMP_DIR"
QA_ID="$(state_get QA_ID)"
if [ -e "$SRC" ]; then
    [ "$(git -C "$SRC" rev-parse HEAD)" = "$COMMIT" ] || refuse "${SRC} is not at ${TAG}"
else
    log "Worktree ${SRC} at ${TAG}"
    git -C "$SCRIPT_REPO" worktree add --detach "$SRC" "$COMMIT" >/dev/null
fi

# --- 3. Storage, kind config and cluster ---------------------------------------------------
mkdir -p "$STORAGE"
for volume in data-plane app-prod; do
    [ -d "${STORAGE}/${volume}" ] || btrfs subvolume create "${STORAGE}/${volume}" >/dev/null
done
PORT="$(state_get PORT)"
if [ -z "$PORT" ]; then
    PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')"
    state_set PORT "$PORT"
fi
python3 - "${SRC}/terraform/k8s/kind-config.yaml" "${QA_DIR}/kind-config.yaml" "$STORAGE" "$PORT" <<'PY'
import sys, yaml
source, target, storage, port = sys.argv[1:]
config = yaml.safe_load(open(source))
mounts = {'/srv/econgraph/data-plane': storage + '/data-plane', '/srv/econgraph/app-prod': storage + '/app-prod'}
found = set()
for node in config['nodes']:
    node.pop('extraPortMappings', None)
    if node.get('role') == 'control-plane':
        node['extraPortMappings'] = [{'containerPort': 80, 'hostPort': int(port),
                                      'listenAddress': '127.0.0.1', 'protocol': 'TCP'}]
    for mount in node.get('extraMounts', []):
        if mount['containerPath'] not in mounts:
            sys.exit(f'unexpected mount {mount["containerPath"]} in {source}')
        mount['hostPath'] = mounts[mount['containerPath']]
        found.add(mount['containerPath'])
if found != set(mounts):
    sys.exit(f'{source} does not mount {sorted(set(mounts) - found)}')
config.setdefault('networking', {})['apiServerAddress'] = '127.0.0.1'
with open(target, 'w') as out:
    yaml.safe_dump(config, out, sort_keys=False)
PY
if ! cluster_exists; then
    state_set CLUSTER creating
fi
log "Cluster ${NAME}"
rc scripts/deploy/recreate-kind-cluster.sh --if-missing --config "${QA_DIR}/kind-config.yaml"
refuse_production_marker
if [ -z "$(owner_id)" ]; then
    node_kubectl -n kube-system create configmap econgraph-qa-owner \
        --from-literal="id=${QA_ID}" --from-literal="name=${NAME}" >/dev/null
fi
[ "$(owner_id)" = "$QA_ID" ] || refuse "cluster ${NAME} is not this QA environment's"
state_set CLUSTER ready
actual_port="$(docker port "$CONTROL_PLANE" 80/tcp | head -1)"
[ "$actual_port" = "127.0.0.1:${PORT}" ] ||
    die "${CONTROL_PLANE} publishes port 80 as '${actual_port}', not 127.0.0.1:${PORT}"

# --- 4. Images -----------------------------------------------------------------------------
log "Release images of ${TAG}"
status=0
rc scripts/deploy/load-release-images.sh --tag "$TAG" --cluster "$NAME" --allow-missing-nodes || status=$?
# 3: loaded, but a fresh cluster had no previous images to keep.
[ "$status" = 0 ] || [ "$status" = 3 ] || die "load-release-images.sh failed (exit ${status})"

# --- 5. Data -------------------------------------------------------------------------------
if [ -s "${DUMP_DIR}/econ_graph_app.dump" ]; then
    app_database=()
else
    app_database=(APP_DATABASE=shared)
fi
log "Restoring ${DUMP_DIR}"
rc env "${app_database[@]}" scripts/deploy/restore-kind-databases.sh --from "$DUMP_DIR"
qk -n "$NAMESPACE" delete secret crawler-api-keys --ignore-not-found >/dev/null

# --- 6. Deploy -----------------------------------------------------------------------------
log "Deploying ${TAG} (PUBLIC_HOST empty)"
rc env "${app_database[@]}" scripts/deploy/deploy.sh

# --- 7. Sign-in on http://localhost:PORT --------------------------------------------------
ORIGIN="http://localhost:${PORT}"
log "Sign-in on ${ORIGIN}"
qk -n "$NAMESPACE" set env deployment/keycloak \
    "KC_HOSTNAME=${ORIGIN}/idp" "KC_WEB_BASE_URL=${ORIGIN}" "KC_WEB_E2E_BASE_URL=${ORIGIN}" >/dev/null
qk -n "$NAMESPACE" rollout status deployment/keycloak --timeout=900s
python3 - "${SRC}/k8s/manifests/keycloak/realm-import-job.yaml" "$ORIGIN" >"${QA_DIR}/realm-import.yaml" <<'PY'
import sys, yaml
source, origin = sys.argv[1:]
docs = [d for d in yaml.safe_load_all(open(source)) if d]
changed = 0
for doc in docs:
    if doc.get('kind') != 'Job':
        continue
    spec = doc['spec']['template']['spec']
    for container in spec.get('initContainers', []) + spec.get('containers', []):
        for env in container.get('env', []):
            if env['name'] in ('KC_WEB_BASE_URL', 'KC_WEB_E2E_BASE_URL'):
                env['value'] = origin
                changed += 1
if not changed:
    sys.exit(f'{source} sets no KC_WEB_BASE_URL')
yaml.safe_dump_all(docs, sys.stdout, sort_keys=False)
PY
qk -n "$NAMESPACE" delete job keycloak-realm-import --ignore-not-found >/dev/null
qk apply -f "${QA_DIR}/realm-import.yaml" >/dev/null
qk -n "$NAMESPACE" wait --for=condition=complete job/keycloak-realm-import --timeout=900s >/dev/null
qk -n "$NAMESPACE" patch configmap econ-graph-config --type merge \
    -p "{\"data\":{\"CORS_ORIGIN\":\"${ORIGIN},http://localhost\"}}" >/dev/null
qk -n "$NAMESPACE" set env deployment/econ-graph-backend "OIDC_ISSUER=${ORIGIN}/idp/realms/${REALM}" >/dev/null
# The backend and frontend read econ-graph-config only at start.
qk -n "$NAMESPACE" rollout restart deployment/econ-graph-backend deployment/econ-graph-frontend >/dev/null
for deployment in econ-graph-backend econ-graph-frontend; do
    qk -n "$NAMESPACE" rollout status "deployment/${deployment}" --timeout=900s
done

# --- 8. No crawling ------------------------------------------------------------------------
qk -n "$NAMESPACE" scale deployment/crawler-worker --replicas=0 >/dev/null

# --- 9. QA accounts ------------------------------------------------------------------------
ACCOUNTS="${QA_DIR}/accounts.env"
mkdir -p "${QA_DIR}/accounts"
chmod 700 "${QA_DIR}/accounts"
# Passwords are generated once and kept, so a rerun sets the same ones again.
python3 - "$ACCOUNTS" "${QA_DIR}/accounts" "${QA_USERS[@]}" <<'PY'
import os, secrets, sys
accounts, directory, users = sys.argv[1], sys.argv[2], sys.argv[3:]
os.umask(0o077)
for user in users:
    path = os.path.join(directory, user + '.password')
    if not os.path.exists(path):
        with open(path, 'w') as out:
            out.write(secrets.token_urlsafe(24) + '\n')
lines = []
for user in users:
    key = user.upper().replace('-', '_')
    password = open(os.path.join(directory, user + '.password')).read().strip()
    lines += [f'{key}_USERNAME={user}', f'{key}_PASSWORD={password}',
              f'{key}_PASSWORD_FILE={os.path.join(directory, user + ".password")}']
tmp = accounts + '.tmp'
with open(tmp, 'w') as out:
    out.write('\n'.join(lines) + '\n')
os.chmod(tmp, 0o600)
os.replace(tmp, accounts)
PY
# kcadm ARGS: Keycloak's admin CLI in its pod, authenticated there with the bootstrap admin
# the pod already has; stdin is passed through.
kcadm() {
    # shellcheck disable=SC2016 # expanded in the Keycloak pod
    qk -n "$NAMESPACE" exec -i deployment/keycloak -- /bin/bash -ceu '
        config=$(mktemp)
        trap '\''rm -f "$config"'\'' EXIT
        cli=/opt/keycloak/bin/kcadm.sh
        "$cli" config credentials --server http://localhost:8080/idp --realm master \
            --user "$KC_BOOTSTRAP_ADMIN_USERNAME" --password "$KC_BOOTSTRAP_ADMIN_PASSWORD" \
            --config "$config" >/dev/null
        "$cli" "$@" --config "$config"
    ' -- "$@"
}
for user in "${QA_USERS[@]}"; do
    id="$(kcadm get users -r "$REALM" -q "username=${user}" -q exact=true --fields id --format csv --noquotes </dev/null | head -1)"
    if [ -z "$id" ]; then
        kcadm create users -r "$REALM" -s "username=${user}" -s enabled=true -s emailVerified=true \
            -s "email=${user}@qa.invalid" -s firstName=QA -s "lastName=${user}" </dev/null >/dev/null
        id="$(kcadm get users -r "$REALM" -q "username=${user}" -q exact=true --fields id --format csv --noquotes </dev/null | head -1)"
    fi
    [ -n "$id" ] || die "could not create Keycloak user ${user}"
    kcadm update "users/${id}" -r "$REALM" -s enabled=true -s emailVerified=true -s 'requiredActions=[]' </dev/null >/dev/null
    python3 -c 'import json, sys; print(json.dumps({"type": "password", "temporary": False, "value": open(sys.argv[1]).read().strip()}))' \
        "${QA_DIR}/accounts/${user}.password" | kcadm update "users/${id}/reset-password" -r "$REALM" -f - >/dev/null
    kcadm add-roles -r "$REALM" --uid "$id" --rolename user </dev/null >/dev/null
    if [ "$user" = qa-admin ]; then
        kcadm add-roles -r "$REALM" --uid "$id" --rolename admin </dev/null >/dev/null
    fi
done

# --- 10. Done ------------------------------------------------------------------------------
[ "$(dump_fingerprint "$DUMP_DIR")" = "$DUMP_BEFORE" ] || die "the dump directory ${DUMP_DIR} changed during the run"
cat <<EOF

QA environment ${NAME} is ready: ${TAG} on a copy of ${DUMP_DIR}.

  URL:       ${ORIGIN}   (this host only; 127.0.0.1)
  Accounts:  ${ACCOUNTS}   (qa-a, qa-b, qa-admin; mode 600)
  kubectl:   ${QA_KUBECTL} --kubeconfig ${QA_KUBECONFIG} --context ${CONTEXT}

For the Linear QA prompt:
  TARGET_URL=${ORIGIN}
  TARGET_HOST=localhost

QA scenarios: QA_LAB_TARGET_OK=localhost scripts/qa/run-scenarios.sh --target ${ORIGIN} --profile lab
crawler-worker is at 0 replicas and has no API keys.
Tear down: $0 --tag ${TAG} --name ${NAME} --teardown
EOF
