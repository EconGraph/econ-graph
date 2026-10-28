#!/usr/bin/env bash
#
# Deploy a release to the kind cluster econ-graph: the one entry point for a
# production window, and for the upgrade rehearsal in CI. It runs, in order,
# stopping at the first failure:
#
#   1. load     scripts/deploy/load-release-images.sh --tag TAG
#   2. backup   scripts/deploy/backup-postgres-now.sh (the nightly backup Job, now)
#   3. keycloak scripts/deploy/backup-keycloak-db.sh (a verified dump of Keycloak's database)
#   4. precheck scripts/deploy/precheck-db-migration.sh --stop-crawler
#   5. deploy   scripts/deploy/deploy.sh, with PUBLIC_HOST from the host's deploy config
#   6. verify   scripts/deploy/verify-release-images.sh --tag TAG
#
# Usage:
#   scripts/deploy/release-deploy.sh --tag vX.Y.Z-rc.N [--cluster NAME] [--dry-run] [--yes] [--report FILE]
#   scripts/deploy/release-deploy.sh --tag vX.Y.Z ...
#
#   --tag TAG      the release candidate to deploy. A final tag vX.Y.Z deploys
#                  the release candidate tag on the same commit (the highest
#                  vX.Y.Z-rc.N there): release.yml publishes the final tag on
#                  that candidate's images, and the load records go by it.
#   --cluster NAME the kind cluster (default: KIND_CLUSTER, else econ-graph).
#                  Every step gets it: --cluster for the load and the verify,
#                  --context kind-NAME for the backups, KIND_CLUSTER for the
#                  precheck and deploy.sh. A cluster other than econ-graph is a
#                  throwaway one and needs ECONGRAPH_SCRATCH_STORAGE
#                  (lib/kind-env.sh), as for the rehearsal.
#   --dry-run      run every step's own dry run (load, both backups, precheck
#                  and deploy.sh --dry-run) and change nothing. verify can't be
#                  dry-run before a load and deploy have happened: it says what
#                  it would run.
#   --yes          don't ask before the first step (needed without a terminal)
#   --report FILE  append one line per step (step=NAME status=ok|failed|dry-run,
#                  and unchecked=N on a verify that left N things unchecked)
#                  and a final result= line to FILE, for automation such as the
#                  upgrade rehearsal, which reports the step that failed
#
# Before any step, with nothing changed yet:
#   - It pins the cluster itself: the rootful Docker daemon the kind cluster
#     runs on (lib/kind-env.sh: unix:///var/run/docker.sock, or
#     ECONGRAPH_DOCKER_HOST, never the shell's DOCKER_HOST), and the kind
#     cluster's own kubectl and kubeconfig (ensure_kind_kubectl), exported to
#     every step, on context kind-<cluster>. On the production host plain
#     `kubectl` is the MicroK8s snap wrapper.
#   - PUBLIC_HOST comes from the host's deploy config,
#     ${XDG_CONFIG_HOME:-~/.config}/econgraph/deploy.env (ECONGRAPH_DEPLOY_ENV
#     names another file), a line PUBLIC_HOST=<host name>; the file is read,
#     not sourced. On the production cluster it is econgraph.com. A cluster
#     marked production (scripts/deploy/mark-production-cluster.sh) with no
#     PUBLIC_HOST configured is refused, so the public site is never deployed
#     without its host (and its tunnel connector stopped); so is a cluster not
#     marked production with one configured (deploy.sh would refuse it after the
#     load and backups), a cluster whose marker can't be read, and a PUBLIC_HOST
#     in the environment that differs from the config.
#   - It shows the cluster's identity and the plan, and asks once (--yes
#     answers; a dry run doesn't ask).
#
# verify-release-images.sh passes with things it could not check (a
# crawler-migrate Job already deleted, a pod that never started); they are
# listed again at the end, so the run isn't read as fully checked.
#
# On a failure it prints which steps ran, which failed, what that leaves
# changed, and the commands to go back (scripts/deploy/rollback-release-images.sh
# --to pre-TAG for the images; the database restores for a failed deploy).
#
# Idempotent: every step is safe to run again with the same tag (the load keeps
# its first record's rollback copies, the backups add new ones, precheck only
# scales down a running worker, deploy.sh applies, verify restarts and checks),
# so after a failure, fix the cause and run the same command again.
#
# Environment: ECONGRAPH_DEPLOY_ENV, ECONGRAPH_DOCKER_HOST, ECONGRAPH_RELEASES_DIR
# (the load records), ECONGRAPH_BACKUP_ROOT.

set -euo pipefail

SCRIPT="release-deploy"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEPLOY_DIR="$PROJECT_ROOT/scripts/deploy"

TAG=""
DRY_RUN=false
ASSUME_YES=false
REPORT=""
CLUSTER="${KIND_CLUSTER:-econ-graph}"
while [ $# -gt 0 ]; do
    case "$1" in
        --tag)
            [ $# -ge 2 ] || { echo "${SCRIPT}: error: --tag needs a value" >&2; exit 2; }
            TAG="$2"; shift 2 ;;
        --report)
            [ $# -ge 2 ] || { echo "${SCRIPT}: error: --report needs a file" >&2; exit 2; }
            REPORT="$2"; shift 2 ;;
        --cluster)
            if [ $# -lt 2 ] || [ -z "$2" ]; then
                echo "${SCRIPT}: error: --cluster needs a value" >&2
                exit 2
            fi
            CLUSTER="$2"; shift 2 ;;
        --dry-run) DRY_RUN=true; shift ;;
        --yes | -y) ASSUME_YES=true; shift ;;
        -h | --help) awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"; exit 0 ;;
        *) echo "${SCRIPT}: error: unknown argument: $1" >&2; exit 2 ;;
    esac
done
[ -n "$TAG" ] || { echo "${SCRIPT}: error: --tag is required" >&2; exit 2; }
[[ "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-rc\.[0-9]+)?$ ]] ||
    { echo "${SCRIPT}: error: '${TAG}' is not vX.Y.Z or vX.Y.Z-rc.N" >&2; exit 2; }

# The PUBLIC_HOST the environment had, before kind-env.sh and the libraries.
ENV_PUBLIC_HOST_SET="${PUBLIC_HOST+set}"
ENV_PUBLIC_HOST="${PUBLIC_HOST-}"
unset PUBLIC_HOST

# Read by kind-env.sh: under a dry run nothing is written to the kubectl cache.
# The cluster is set before it is sourced, so its checks (a throwaway cluster
# needs ECONGRAPH_SCRATCH_STORAGE) and every step see the same one.
export DRY_RUN ASSUME_YES
export KIND_CLUSTER="$CLUSTER"
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "$DEPLOY_DIR/lib/kind-env.sh"

die() {
    echo "${SCRIPT}: error: $*" >&2
    report "result=refused"
    exit 1
}
# Append a line to the report. A write that fails is said, never fatal: it must
# not cut the run, or its failure guidance, short.
report() {
    [ -n "$REPORT" ] || return 0
    printf '%s\n' "$*" 2>/dev/null >>"$REPORT" ||
        echo "${SCRIPT}: warning: could not append '$*' to ${REPORT}" >&2
}
# Checked before anything else, so an unwritable report stops the run before
# it changes anything.
if [ -n "$REPORT" ] && ! { : >>"$REPORT"; } 2>/dev/null; then
    echo "${SCRIPT}: error: cannot write the report ${REPORT}" >&2
    exit 2
fi

# --- the tag ------------------------------------------------------------------------
RC_TAG="$TAG"
if [[ "$TAG" != *-rc.* ]]; then
    git -C "$PROJECT_ROOT" fetch --tags origin >/dev/null 2>&1 ||
        warn "git fetch --tags origin failed; using the tags this checkout has"
    commit="$(git -C "$PROJECT_ROOT" rev-parse --verify --quiet "refs/tags/${TAG}^{commit}")" ||
        die "tag ${TAG} not found"
    RC_TAG="$(git -C "$PROJECT_ROOT" tag --points-at "$commit" | grep -E "^${TAG//./\\.}-rc\.[0-9]+$" |
        sort -t. -k4,4n | tail -1)" || true
    [ -n "$RC_TAG" ] ||
        die "no release candidate tag ${TAG}-rc.N is on ${TAG}'s commit ${commit}; deploy a release candidate"
    echo "${TAG} is ${RC_TAG} (commit ${commit})"
fi

# --- the cluster, pinned ---------------------------------------------------------------
echo "Cluster:"
check_docker_daemon
kind_cluster_exists || die "kind cluster '${KIND_CLUSTER}' not found on ${DOCKER_HOST}"
ensure_kind_kubectl
KUBECTL="${ECONGRAPH_KUBE_DIR:-${ECONGRAPH_CACHE}}/bin/kubectl"
export KUBECTL KUBECONFIG
CONTEXT="kind-${KIND_CLUSTER}"
CLUSTER_ROLE_CONTEXT="$CONTEXT"
kc config get-contexts -o name "$CONTEXT" >/dev/null 2>&1 || die "${KUBECTL} has no context ${CONTEXT}"
cluster_role_show_identity || die "could not read the identity of context ${CONTEXT}"

# --- PUBLIC_HOST, from the host's deploy config ----------------------------------------
DEPLOY_ENV="${ECONGRAPH_DEPLOY_ENV:-${XDG_CONFIG_HOME:-$HOME/.config}/econgraph/deploy.env}"
configured=false
PUBLIC_HOST=""
if [ -f "$DEPLOY_ENV" ]; then
    line="$(grep -E '^[[:space:]]*PUBLIC_HOST=' "$DEPLOY_ENV" | tail -1)" || line=""
    if [ -n "$line" ]; then
        PUBLIC_HOST="${line#*=}"
        # A CRLF file would otherwise pass silently (the renderer strips a
        # trailing CR): refuse it.
        [[ "$PUBLIC_HOST" != *$'\r'* ]] ||
            die "${DEPLOY_ENV} has a carriage return in its PUBLIC_HOST line (CRLF line endings?); save it with LF endings"
        # Surrounding blanks and one pair of matching quotes; nothing else is
        # rewritten, so a comment or a typo reaches the check below.
        PUBLIC_HOST="${PUBLIC_HOST#"${PUBLIC_HOST%%[![:space:]]*}"}"
        PUBLIC_HOST="${PUBLIC_HOST%"${PUBLIC_HOST##*[![:space:]]}"}"
        if [[ "$PUBLIC_HOST" =~ ^\"(.*)\"$ || "$PUBLIC_HOST" =~ ^\'(.*)\'$ ]]; then
            PUBLIC_HOST="${BASH_REMATCH[1]}"
        fi
        # Blanks inside (or inside the quotes): the renderer would strip them,
        # but deploy.sh's gateway step would not. Refuse, rather than guess.
        [[ "$PUBLIC_HOST" != *[[:space:]]* ]] ||
            die "${DEPLOY_ENV} sets PUBLIC_HOST='${PUBLIC_HOST}', with blanks in it; write the host name alone"
        [ -z "$PUBLIC_HOST" ] || configured=true
    fi
fi
# A malformed host is refused here, before any step, not by deploy.sh after the
# load, the backups and the precheck (the same check, render-public-host.py).
if [ -n "$PUBLIC_HOST" ] && ! "$DEPLOY_DIR/render-public-host.py" "$PUBLIC_HOST" /dev/null >/dev/null; then
    die "${DEPLOY_ENV} sets PUBLIC_HOST='${PUBLIC_HOST}', which is not a public DNS name"
fi
if [ "$ENV_PUBLIC_HOST_SET" = set ] && [ "$ENV_PUBLIC_HOST" != "$PUBLIC_HOST" ]; then
    die "PUBLIC_HOST='${ENV_PUBLIC_HOST}' is set in the environment, but ${DEPLOY_ENV} says '${PUBLIC_HOST}'; the config decides: unset PUBLIC_HOST, or change the config"
fi
marker=0
cluster_is_marked_production || marker=$?
case "$marker" in
    0)
        [ "$configured" = true ] ||
            die "context ${CONTEXT} is the production cluster (marked by mark-production-cluster.sh), but no PUBLIC_HOST is configured in ${DEPLOY_ENV}: deploying without it would take the public site down. Add PUBLIC_HOST=econgraph.com there." ;;
    1)
        [ "$configured" = false ] ||
            die "${DEPLOY_ENV} sets PUBLIC_HOST=${PUBLIC_HOST}, but ${CLUSTER_ROLE_REASON}; only the production cluster serves the public host" ;;
    *) die "${CLUSTER_ROLE_REASON}" ;;
esac
# Keycloak's replicas now: the keycloak step scales it to 0 and leaves it there
# until deploy.sh applies it again, so a failure in between says how to start it.
KEYCLOAK_REPLICAS="$(kc -n "$NAMESPACE" get deployment keycloak --ignore-not-found \
    -o 'jsonpath={.spec.replicas}')" || die "could not read deployment/keycloak on ${CONTEXT}"
unset CLUSTER_ROLE_CONTEXT
export PUBLIC_HOST
echo "  PUBLIC_HOST     ${PUBLIC_HOST:-(none: localhost only)} (from ${DEPLOY_ENV})"

# --- the steps ------------------------------------------------------------------------------
dry=()
[ "$DRY_RUN" = true ] && dry=(--dry-run)
STEPS=(load backup keycloak precheck deploy verify)
declare -A CMD=(
    [load]="$DEPLOY_DIR/load-release-images.sh --tag ${RC_TAG} --cluster ${KIND_CLUSTER}"
    [backup]="$DEPLOY_DIR/backup-postgres-now.sh --context ${CONTEXT}"
    [keycloak]="$DEPLOY_DIR/backup-keycloak-db.sh --context ${CONTEXT}"
    [precheck]="$DEPLOY_DIR/precheck-db-migration.sh --stop-crawler"
    [deploy]="$DEPLOY_DIR/deploy.sh"
    [verify]="$DEPLOY_DIR/verify-release-images.sh --tag ${RC_TAG} --cluster ${KIND_CLUSTER}"
)
declare -A STATE=()

echo
echo "Plan for ${RC_TAG}$([ "$DRY_RUN" = true ] && echo ' (dry run: each step dry-runs)'):"
for step in "${STEPS[@]}"; do
    echo "  ${step}: ${CMD[$step]#"$PROJECT_ROOT"/}${dry[*]:+ ${dry[*]}}"
done
confirm "Deploy ${RC_TAG} to ${CONTEXT}${PUBLIC_HOST:+ (PUBLIC_HOST=${PUBLIC_HOST})}?"

kubectl_hint="KUBECONFIG=${KUBECONFIG} ${KUBECTL} --context ${CONTEXT} -n ${NAMESPACE}"
STEP_LOG="$(mktemp "${TMPDIR:-/tmp}/release-deploy.XXXXXX")"
add_exit_cleanup "rm -f '${STEP_LOG}'"
UNCHECKED=""

# What a failure at step $1 leaves, and how to go back.
explain_failure() {
    local failed="$1" step
    echo
    echo "==> ${RC_TAG}: step ${failed} failed; nothing after it ran."
    for step in "${STEPS[@]}"; do
        printf '  %-9s %s\n' "$step" "${STATE[$step]:-not run}"
    done
    echo
    case "$failed" in
        load)
            echo "If the load refused (its last line says so), nothing was changed. Otherwise"
            echo "the images may be partly loaded: if the last record in"
            echo "  ${ECONGRAPH_RELEASES_DIR:-$HOME/econgraph-releases}/${RC_TAG}/images.txt"
            echo "says status=partial, some nodes run the new images when their pods restart."
            echo "Fix the cause and run this command again, or go back with:" ;;
        backup | keycloak)
            echo "The new images are loaded on the nodes under the names the manifests run;"
            echo "running pods keep the old ones until they restart. The databases are unchanged."
            echo "Fix the backup and run this command again, or go back with:" ;;
        precheck)
            echo "The new images are loaded, the backups are taken, and the crawler worker"
            echo "may be stopped. Start it again with:"
            echo "  ${kubectl_hint} scale deployment/crawler-worker --replicas=1"
            echo "Fix what the precheck reported and run this command again, or go back with:" ;;
        deploy)
            echo "deploy.sh stopped partway: some manifests are applied, and the data migrations"
            echo "(crawler-migrate Job) may have run; if they did, the crawler worker stays"
            echo "stopped. Pods that restarted run the new images. Go back with:" ;;
        verify)
            echo "The deploy finished, but not every pod runs ${RC_TAG}'s images (see above)."
            echo "Run the verify step again once rollouts settle, or go back with:" ;;
    esac
    echo "  scripts/deploy/rollback-release-images.sh --to pre-${RC_TAG} --dry-run   # then without --dry-run"
    case "$failed" in
        keycloak | precheck | deploy)
            if [ -n "$KEYCLOAK_REPLICAS" ] && [ "$KEYCLOAK_REPLICAS" != 0 ]; then
                echo "Keycloak (sign-in) may be stopped: the keycloak step scales it to 0, and only"
                echo "deploy.sh applies it again. It ran ${KEYCLOAK_REPLICAS} replica(s) before; start it with:"
                echo "  ${kubectl_hint} scale deployment/keycloak --replicas=${KEYCLOAK_REPLICAS}"
            fi ;;
    esac
    case "$failed" in
        deploy | verify)
            echo "and, if the migrations ran, restore the databases from the backups this run took:"
            echo "  the data databases: docs/deployment/POSTGRES_BACKUP_RESTORE.md (scripts/deploy/restore-postgres-backup.sh)"
            echo "  Keycloak: scripts/deploy/restore-keycloak-db.sh <the VERIFIED dump the keycloak step printed>" ;;
    esac
}

for step in "${STEPS[@]}"; do
    echo
    echo "==> ${step}: ${CMD[$step]#"$PROJECT_ROOT"/}${dry[*]:+ ${dry[*]}}"
    if [ "$step" = verify ] && [ "$DRY_RUN" = true ]; then
        echo "dry run: verify checks the pods against the images the load put on the nodes, so"
        echo "it can only run after a real load and deploy; it would run:"
        echo "  ${CMD[verify]#"$PROJECT_ROOT"/}"
        STATE[$step]="dry run (not run)"
        report "step=${step} status=dry-run"
        continue
    fi
    # The step's output also goes to STEP_LOG, for verify's list of what it
    # could not check. pipefail: the status is the step's, not tee's.
    # shellcheck disable=SC2086 # CMD holds the command and its arguments, none with spaces
    if (cd "$PROJECT_ROOT" && ${CMD[$step]} "${dry[@]}") 2>&1 | tee "$STEP_LOG"; then
        STATE[$step]="$([ "$DRY_RUN" = true ] && echo "dry run ok" || echo "done")"
        extra=""
        if [ "$step" = verify ]; then
            UNCHECKED="$(grep '^  unchecked ' "$STEP_LOG" || true)"
            if [ -n "$UNCHECKED" ]; then
                STATE[$step]="done, $(grep -c . <<<"$UNCHECKED") unchecked"
                extra=" unchecked=$(grep -c . <<<"$UNCHECKED")"
            fi
        fi
        report "step=${step} status=$([ "$DRY_RUN" = true ] && echo dry-run || echo ok)${extra}"
    else
        STATE[$step]="FAILED"
        report "step=${step} status=failed"
        report "result=failed step=${step}"
        if [ "$DRY_RUN" = true ]; then
            echo
            echo "==> ${RC_TAG}: the dry run of step ${step} failed; nothing was changed."
        else
            explain_failure "$step"
        fi
        exit 1
    fi
done

report "result=$([ "$DRY_RUN" = true ] && echo dry-run || echo ok)"
echo
if [ "$DRY_RUN" = true ]; then
    echo "==> Dry run of ${RC_TAG} on ${CONTEXT} complete: nothing was changed."
else
    echo "==> ${RC_TAG} is deployed on ${CONTEXT} and verified."
    if [ -n "$UNCHECKED" ]; then
        echo "    Not checked (verify-release-images.sh could not; check these by hand):"
        while IFS= read -r line; do echo "      ${line#"${line%%[! ]*}"}"; done <<<"$UNCHECKED"
    fi
    echo "    Rollback copies of the images that ran before: pre-${RC_TAG} (scripts/deploy/rollback-release-images.sh)."
fi
