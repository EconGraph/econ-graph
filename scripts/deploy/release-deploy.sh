#!/usr/bin/env bash
#
# Deploy a release to the kind cluster econ-graph: the one entry point for a
# production window. It runs, in order, stopping at the first failure:
#
#   1. backup   scripts/deploy/backup-postgres-now.sh (the nightly backup Job, now)
#   2. load     scripts/deploy/load-release-images.sh --tag TAG
#   3. keycloak scripts/deploy/backup-keycloak-db.sh (a verified dump of Keycloak's database)
#   4. precheck scripts/deploy/precheck-db-migration.sh --stop-crawler
#   5. deploy   scripts/deploy/deploy.sh, with PUBLIC_HOST from the host's deploy config
#   6. verify   scripts/deploy/verify-release-images.sh --tag TAG
#
# The backup comes before the load: once the load has put the new images on
# the nodes under the names the manifests run, any pod that restarts runs the
# new image against the old schema, so the time between the load and the
# deploy is kept short. The Keycloak dump stops Keycloak, so it comes after the
# load (a load that refuses leaves sign-in running), just before the precheck.
#
# Usage:
#   scripts/deploy/release-deploy.sh --tag vX.Y.Z-rc.N [--cluster NAME] [--dry-run] [--yes]
#       [--confirm-production UID-PREFIX] [--change-public-host] [--report FILE]
#   scripts/deploy/release-deploy.sh --tag vX.Y.Z ...
#
#   --tag TAG      the release candidate to deploy. A final tag vX.Y.Z deploys
#                  the release candidate tag on the same commit (the highest
#                  vX.Y.Z-rc.N there): release.yml publishes the final tag on
#                  that candidate's images, and the load records go by it.
#                  The checkout must be that commit, with no changes, no
#                  untracked files, no files git is told to overlook
#                  (skip-worktree, assume-unchanged) and no ignored files where
#                  deploy.sh reads: deploy.sh applies k8s/manifests from it. It
#                  is checked again just before the deploy step.
#   --cluster NAME the kind cluster (default: KIND_CLUSTER, else econ-graph).
#                  Every step gets it: --cluster for the load and the verify,
#                  --context kind-NAME for the backups, KIND_CLUSTER for the
#                  precheck and deploy.sh. A cluster other than econ-graph is a
#                  throwaway one and needs ECONGRAPH_SCRATCH_STORAGE
#                  (lib/kind-env.sh).
#   --dry-run      run every step's own dry run (backup, load, keycloak,
#                  precheck and deploy.sh --dry-run) and change nothing. verify
#                  can't be dry-run before a load and deploy have happened: it
#                  says what it would run.
#   --yes          don't ask before the first step (needed without a terminal);
#                  on a cluster marked production it is not enough:
#   --confirm-production UID-PREFIX
#                  the first 8 characters of the cluster's kube-system UID
#                  (shown with the cluster's identity): what a cluster marked
#                  production needs instead of a typed answer. One that doesn't
#                  match the cluster is refused on any cluster.
#   --change-public-host
#                  deploy a PUBLIC_HOST other than the one the production
#                  cluster serves now (or one where it serves none)
#   --report FILE  append one line per step (step=NAME status=ok|failed|dry-run,
#                  and unchecked=N on a verify that left N things unchecked),
#                  the backups taken (backup= and keycloak_backup=), on a
#                  failure the backups to restore from (restore_backup= and
#                  restore_keycloak_backup=), and a final result= line to FILE
#                  (result=interrupted step=NAME signal=SIG after Ctrl-C or
#                  SIGTERM), for a caller that reads which step failed
#
# Before any step, with nothing changed yet:
#   - The checkout is the tag's commit and clean (see --tag).
#   - It pins the cluster itself: the rootful Docker daemon the kind cluster
#     runs on (lib/kind-env.sh: unix:///var/run/docker.sock, or
#     ECONGRAPH_DOCKER_HOST, never the shell's DOCKER_HOST), and the kind
#     cluster's own kubectl and kubeconfig (ensure_kind_kubectl), exported to
#     every step, on context kind-<cluster>. On the production host plain
#     `kubectl` is the MicroK8s snap wrapper.
#   - PUBLIC_HOST comes from the host's deploy config,
#     ${XDG_CONFIG_HOME:-~/.config}/econgraph/deploy.env (ECONGRAPH_DEPLOY_ENV
#     names another file), a line PUBLIC_HOST=<host name> (the last one, when
#     there are several); the file is read, not sourced, and the name is
#     lowercased. On the production cluster it is econgraph.com. A cluster
#     marked production (scripts/deploy/mark-production-cluster.sh) with no
#     PUBLIC_HOST configured is refused, so the public site is never deployed
#     without its host (and its tunnel connector stopped); so is one whose
#     configured PUBLIC_HOST differs from the host it serves now, without
#     --change-public-host; a cluster not marked production with one configured
#     (deploy.sh would refuse it after the load and backups), a cluster whose
#     marker can't be read, and a PUBLIC_HOST in the environment that differs
#     from the config.
#   - It shows the cluster's identity and the plan, and asks once (--yes
#     answers, except on a cluster marked production, which needs the UID
#     prefix typed or --confirm-production; a dry run doesn't ask).
#
# verify-release-images.sh passes with things it could not check (a
# crawler-migrate Job already deleted, a pod that never started); they are
# listed again at the end, so the run isn't read as fully checked.
#
# On a failure it prints which steps ran, which failed, what that leaves
# changed, and the commands to go back: scripts/deploy/rollback-release-images.sh
# --to pre-TAG for the images; the commands that start Keycloak and the crawler
# worker at their earlier replicas; and the database restores, from the backups
# taken before the tag's first deploy attempt.
#
# Each run appends what it found and took to
# ${ECONGRAPH_RELEASES_DIR:-~/econgraph-releases}/<tag>/release-deploy-<cluster>.txt,
# each line with the run's id, the cluster's kube-system UID and the time:
# Keycloak's and the crawler worker's replicas before the run, the replicas the
# precheck scaled the crawler down from, and the backups, recorded only when
# their step succeeded (backup_complete=1 marks a complete database backup). A
# later run of the same tag on the same cluster reads them, since by then
# Keycloak and the crawler may already be at 0 and the databases may already be
# migrated; lines from another cluster (another kube-system UID, such as one
# recreated under the same name) are ignored. A record that can't be written
# stops the run.
#
# Ctrl-C or SIGTERM during a step stops that step (and what it started), and
# prints the same guidance as a failure of that step.
#
# Idempotent: every step is safe to run again with the same tag (the load keeps
# its first record's rollback copies, the backups add new ones, precheck only
# scales down a running worker, deploy.sh applies, verify restarts and checks),
# so after a failure, fix the cause and run the same command again.
#
# Environment: ECONGRAPH_DEPLOY_ENV, ECONGRAPH_DOCKER_HOST, ECONGRAPH_RELEASES_DIR
# (the load records and this script's), ECONGRAPH_BACKUP_ROOT.

set -euo pipefail

SCRIPT="release-deploy"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEPLOY_DIR="$PROJECT_ROOT/scripts/deploy"
DEFAULT_CLUSTER="econ-graph"

TAG=""
DRY_RUN=false
ASSUME_YES=false
REPORT=""
CONFIRM_PRODUCTION=""
CHANGE_PUBLIC_HOST=false
CLUSTER="${KIND_CLUSTER:-$DEFAULT_CLUSTER}"
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
        --confirm-production)
            if [ $# -lt 2 ] || [ -z "$2" ]; then
                echo "${SCRIPT}: error: --confirm-production needs the first 8 characters of the kube-system UID" >&2
                exit 2
            fi
            CONFIRM_PRODUCTION="$2"; shift 2 ;;
        --change-public-host) CHANGE_PUBLIC_HOST=true; shift ;;
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
# public_host_in_cluster: the public host the cluster serves now.
# shellcheck source=scripts/deploy/lib/public-host.sh
source "$DEPLOY_DIR/lib/public-host.sh"

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

# --- the checkout: the tag's commit, unchanged ---------------------------------------
# deploy.sh applies k8s/manifests (and runs the scripts) from this working tree,
# so it must be exactly what the tag names. git is asked with the settings that
# could hide a change turned off: every untracked file listed
# (--untracked-files=all, whatever status.showUntrackedFiles says), no fsmonitor
# or untracked cache, files marked skip-worktree or assume-unchanged refused,
# and ignored files (.gitignore, .git/info/exclude) refused where deploy.sh
# reads. git status compares the index with HEAD as well as the working tree.
tag_commit="$(git -C "$PROJECT_ROOT" rev-parse --verify --quiet "refs/tags/${RC_TAG}^{commit}")" ||
    die "tag ${RC_TAG} is not in this checkout (git fetch --tags origin)"
DEPLOY_READS=(k8s config scripts grafana-dashboards ports.env)
# check_checkout: fails, saying why, unless the checkout is the tag's commit and clean.
check_checkout() {
    local g=(git -C "$PROJECT_ROOT" -c core.fsmonitor=false -c core.untrackedCache=false) head changes flagged ignored
    head="$("${g[@]}" rev-parse --verify HEAD)" || { echo "could not read the commit of ${PROJECT_ROOT}"; return 1; }
    if [ "$head" != "$tag_commit" ]; then
        echo "${PROJECT_ROOT} is at ${head}, not ${RC_TAG} (${tag_commit}): deploy.sh applies the manifests in this checkout. Check out the tag (git checkout ${RC_TAG}) and run this again"
        return 1
    fi
    "${g[@]}" update-index -q --really-refresh >/dev/null 2>&1 || true
    changes="$("${g[@]}" status --porcelain --untracked-files=all)" || { echo "could not read git status of ${PROJECT_ROOT}"; return 1; }
    if [ -n "$changes" ]; then
        echo "${PROJECT_ROOT} has changes or untracked files, which deploy.sh would apply with ${RC_TAG}; commit, stash or remove them first:"
        head -20 <<<"$changes"
        return 1
    fi
    # git ls-files -v: S is skip-worktree, a lowercase tag assume-unchanged;
    # git status does not show changes to either.
    flagged="$("${g[@]}" ls-files -v | grep -v '^H ' || true)"
    if [ -n "$flagged" ]; then
        echo "${PROJECT_ROOT} has files git is told to overlook (skip-worktree or assume-unchanged), whose changes git status does not show; clear the flags (git update-index --no-skip-worktree --no-assume-unchanged FILE) and check them:"
        head -20 <<<"$flagged"
        return 1
    fi
    ignored="$("${g[@]}" status --porcelain --untracked-files=all --ignored=matching -- "${DEPLOY_READS[@]}" | grep '^!! ' || true)"
    if [ -n "$ignored" ]; then
        echo "${PROJECT_ROOT} has ignored files where deploy.sh reads (${DEPLOY_READS[*]}), which it would apply or source with ${RC_TAG}; remove them first:"
        head -20 <<<"$ignored"
        return 1
    fi
}
if ! why="$(check_checkout)"; then
    die "$why"
fi
echo "Checkout: ${PROJECT_ROOT} at ${RC_TAG} (${tag_commit}), clean"

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
CLUSTER_UID="$(kube_system_uid)" || die "could not read the kube-system UID of ${CONTEXT}"
UID_PREFIX="${CLUSTER_UID:0:8}"

# --- PUBLIC_HOST, from the host's deploy config ----------------------------------------
DEPLOY_ENV="${ECONGRAPH_DEPLOY_ENV:-${XDG_CONFIG_HOME:-$HOME/.config}/econgraph/deploy.env}"
configured=false
PUBLIC_HOST=""
if [ -f "$DEPLOY_ENV" ]; then
    # The last line wins, as it would if the file were sourced.
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
        # Host names are case-insensitive; the routes, the certificate and the
        # comparison below use the lowercase form.
        if [ "$PUBLIC_HOST" != "${PUBLIC_HOST,,}" ]; then
            echo "  PUBLIC_HOST     ${PUBLIC_HOST} in ${DEPLOY_ENV} is used as ${PUBLIC_HOST,,}"
            PUBLIC_HOST="${PUBLIC_HOST,,}"
        fi
        [ -z "$PUBLIC_HOST" ] || configured=true
    fi
fi
# A malformed host is refused here, before any step, not by deploy.sh after the
# load, the backups and the precheck (the same check, render-public-host.py).
if [ -n "$PUBLIC_HOST" ] && ! "$DEPLOY_DIR/render-public-host.py" "$PUBLIC_HOST" /dev/null >/dev/null; then
    die "${DEPLOY_ENV} sets PUBLIC_HOST='${PUBLIC_HOST}', which is not a public DNS name"
fi
if [ "$ENV_PUBLIC_HOST_SET" = set ] && [ "${ENV_PUBLIC_HOST,,}" != "$PUBLIC_HOST" ]; then
    die "PUBLIC_HOST='${ENV_PUBLIC_HOST}' is set in the environment, but ${DEPLOY_ENV} says '${PUBLIC_HOST}'; the config decides: unset PUBLIC_HOST, or change the config"
fi
marker=0
cluster_is_marked_production || marker=$?
case "$marker" in
    0)
        [ "$configured" = true ] ||
            die "context ${CONTEXT} is the production cluster (marked by mark-production-cluster.sh), but no PUBLIC_HOST is configured in ${DEPLOY_ENV}: deploying without it would take the public site down. Add PUBLIC_HOST=econgraph.com there."
        # A typo in the config (econ-graph.com) would move the public routes,
        # the certificate and the tunnel's host to a name nobody visits.
        served="$(public_host_in_cluster)" ||
            die "could not read which public host ${CONTEXT} serves now, to compare it with PUBLIC_HOST=${PUBLIC_HOST}"
        served="${served,,}"
        if [ "$served" != "$PUBLIC_HOST" ]; then
            [ "$CHANGE_PUBLIC_HOST" = true ] ||
                die "${CONTEXT} serves the public host '${served:-(none)}' now, but ${DEPLOY_ENV} says PUBLIC_HOST=${PUBLIC_HOST}. If ${PUBLIC_HOST} is right, pass --change-public-host; otherwise correct the config."
            echo "  PUBLIC_HOST     changes from ${served:-(none)} (served now) to ${PUBLIC_HOST} (--change-public-host)"
        fi ;;
    1)
        [ "$configured" = false ] ||
            die "${DEPLOY_ENV} sets PUBLIC_HOST=${PUBLIC_HOST}, but ${CLUSTER_ROLE_REASON}; only the production cluster serves the public host" ;;
    *) die "${CLUSTER_ROLE_REASON}" ;;
esac
# --confirm-production names one cluster: refuse it for any other, marked or not.
if [ -n "$CONFIRM_PRODUCTION" ] && [ "$CONFIRM_PRODUCTION" != "$UID_PREFIX" ]; then
    die "--confirm-production ${CONFIRM_PRODUCTION} does not match ${CONTEXT}'s kube-system UID; nothing was changed"
fi
if [ "$marker" = 0 ] && [ -z "$CONFIRM_PRODUCTION" ] && [ "$DRY_RUN" != true ] && [ ! -t 0 ]; then
    die "deploying to the production cluster needs --confirm-production with the first 8 characters of its kube-system UID, or a terminal to type them on; --yes alone does not answer it. Read the UID from the cluster's identity above, on the terminal you deploy from"
fi
export PUBLIC_HOST
echo "  PUBLIC_HOST     ${PUBLIC_HOST:-(none: localhost only)} (from ${DEPLOY_ENV})"

# --- what earlier runs of this tag recorded -------------------------------------------
RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)-$$"
RECORD="${ECONGRAPH_RELEASES_DIR:-$HOME/econgraph-releases}/${RC_TAG}/release-deploy-${KIND_CLUSTER}.txt"
if [ "$DRY_RUN" != true ] &&
    ! { mkdir -p "$(dirname "$RECORD")" && : >>"$RECORD" && [ -w "$RECORD" ]; } 2>/dev/null; then
    die "cannot write ${RECORD}, where this run records Keycloak's and the crawler's replicas and its backups"
fi
RECORD_FAILED=""
# record KEY VALUE: append "run=ID cluster_uid=UID date=TIME KEY=VALUE" to this
# tag's record (not in a dry run). VALUE is the rest of the line, so it may hold
# blanks. A write that fails sets RECORD_FAILED, which stops the run.
record() {
    [ "$DRY_RUN" != true ] || return 0
    if ! printf 'run=%s cluster_uid=%s date=%s %s=%s\n' "$RUN_ID" "$CLUSTER_UID" \
        "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$1" "$2" 2>/dev/null >>"$RECORD"; then
        echo "${SCRIPT}: error: could not append $1=$2 to ${RECORD}" >&2
        RECORD_FAILED="$1"
        return 1
    fi
}
# record_entries [KEY]: this cluster's record lines (its kube-system UID), as
# RUN<tab>DATE<tab>KEY<tab>VALUE, in the order they were written.
record_entries() {
    [ -f "$RECORD" ] || return 0
    sed -n "s/^run=\([^ ]*\) cluster_uid=${CLUSTER_UID} date=\([^ ]*\) \([a-z_]*\)=\(.*\)\$/\1\t\2\t\3\t\4/p" "$RECORD" |
        awk -F'\t' -v k="${1:-}" 'k == "" || $3 == k'
}
# first_recorded KEY: the value of KEY in this cluster's earliest line with one.
first_recorded() {
    record_entries "$1" | head -1 | cut -f4-
}
# first_run_with KEY: the id and date (RUN<tab>DATE) of the earliest run on this
# cluster that recorded KEY.
first_run_with() {
    record_entries "$1" | head -1 | cut -f1,2
}
# recorded_in_run RUN KEY: the values of KEY that run RUN recorded, one per line.
recorded_in_run() {
    record_entries "$2" | awk -F'\t' -v r="$1" '$1 == r { print $4 }'
}
# manifest_replicas FILE: spec.replicas of the Deployment in FILE.
manifest_replicas() {
    awk '/^spec:/ { s = 1; next } s && /^  replicas:/ { print $2; exit }' "$PROJECT_ROOT/$1"
}
if [ -f "$RECORD" ] && stale="$(grep -vc " cluster_uid=${CLUSTER_UID} " "$RECORD")" && [ "$stale" != 0 ]; then
    echo "  record          ${RECORD}: ${stale} line(s) from another cluster (another kube-system UID) ignored"
fi

# Keycloak's and the crawler worker's replicas now: the keycloak step and the
# precheck scale them to 0, and only deploy.sh starts them again, so a failure
# in between says how to start them. Recorded (once the run is confirmed) when
# they run; a later run that finds them at 0 reads the earliest record.
keycloak_now="$(kc -n "$NAMESPACE" get deployment keycloak --ignore-not-found \
    -o 'jsonpath={.spec.replicas}')" || die "could not read deployment/keycloak on ${CONTEXT}"
crawler_now="$(kc -n "$NAMESPACE" get deployment crawler-worker --ignore-not-found \
    -o 'jsonpath={.spec.replicas}')" || die "could not read deployment/crawler-worker on ${CONTEXT}"
unset CLUSTER_ROLE_CONTEXT

# --- the steps ------------------------------------------------------------------------------
dry=()
[ "$DRY_RUN" = true ] && dry=(--dry-run)
STEPS=(backup load keycloak precheck deploy verify)
declare -A CMD=(
    [backup]="$DEPLOY_DIR/backup-postgres-now.sh --context ${CONTEXT}"
    [load]="$DEPLOY_DIR/load-release-images.sh --tag ${RC_TAG} --cluster ${KIND_CLUSTER}"
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
question="Deploy ${RC_TAG} to ${CONTEXT}${PUBLIC_HOST:+ (PUBLIC_HOST=${PUBLIC_HOST})}?"
if [ "$marker" = 0 ] && [ "$DRY_RUN" != true ]; then
    # The production cluster: the UID prefix, from --confirm-production
    # (checked above) or typed; --yes does not answer it.
    if [ -n "$CONFIRM_PRODUCTION" ]; then
        echo "${question} (--confirm-production ${CONFIRM_PRODUCTION} matches this cluster)"
    else
        read -r -p "${question} This is the PRODUCTION cluster: type the first 8 characters of its kube-system UID (shown above) to continue: " answer ||
            die "aborted; nothing was changed"
        [ "$answer" = "$UID_PREFIX" ] || die "aborted; nothing was changed"
    fi
else
    confirm "$question"
fi

kubectl_hint="KUBECONFIG=${KUBECONFIG} ${KUBECTL} --context ${CONTEXT} -n ${NAMESPACE}"
cluster_arg=""
[ "$KIND_CLUSTER" = "$DEFAULT_CLUSTER" ] || cluster_arg=" --cluster ${KIND_CLUSTER}"
# Each step's output, also kept for what a step reports (verify's unchecked
# items, the backups' paths), in a file of its own per step, so a tee still
# writing for one step can't mix into the next. Not in /tmp, which may be
# small and shared.
STEP_LOG_PARENT="${XDG_RUNTIME_DIR:-${XDG_STATE_HOME:-$HOME/.local/state}/econgraph}"
if ! { mkdir -p "$STEP_LOG_PARENT" && STEP_LOG_DIR="$(mktemp -d "${STEP_LOG_PARENT}/release-deploy.XXXXXX")"; }; then
    die "cannot create a step log in ${STEP_LOG_PARENT}; nothing was changed. Free space there or set XDG_RUNTIME_DIR to a writable directory, then run this again"
fi
add_exit_cleanup "rm -rf '${STEP_LOG_DIR}' 2>/dev/null || true"
STEP_LOG=""
[ -z "$keycloak_now" ] || [ "$keycloak_now" = 0 ] || record keycloak_replicas "$keycloak_now" ||
    die "cannot write ${RECORD}; nothing was changed"
[ -z "$crawler_now" ] || [ "$crawler_now" = 0 ] || record crawler_replicas "$crawler_now" ||
    die "cannot write ${RECORD}; nothing was changed"
UNCHECKED=""
BACKUP_STILL_RUNNING=""
BACKUPS=()
KEYCLOAK_BACKUP=""
NOT_STARTED=""

# replicas_advice NAME NOW KEY MANIFEST: set REPLICAS and REPLICAS_FROM for
# deployment NAME: the earliest record on this cluster (the replicas before the
# tag's first run, read then or scaled down from by a precheck), else, when it
# ran before this run, what it ran, else the manifest's, saying it was at 0.
replicas_advice() {
    local name="$1" now="$2" key="$3" manifest="$4" first
    REPLICAS="$(first_recorded "$key")"
    if [ -n "$REPLICAS" ]; then
        first="$(first_run_with "$key")"
        REPLICAS_FROM="it ran ${REPLICAS} replica(s) before ${RC_TAG}'s first run on this cluster (${first#*$'\t'}), as recorded in ${RECORD}"
    elif [ -n "$now" ] && [ "$now" != 0 ]; then
        REPLICAS="$now"
        REPLICAS_FROM="it ran ${REPLICAS} replica(s) before this run"
    elif [ -n "$now" ]; then
        REPLICAS="$(manifest_replicas "$manifest")"
        REPLICAS_FROM="it was at 0 replicas before this run, and no run of ${RC_TAG} on this cluster recorded it running; ${manifest} runs ${REPLICAS}"
    else
        REPLICAS=""
    fi
}

# The database restores: from the backups of the first run of the tag on this
# cluster whose backup step completed, which come before any deploy of the tag
# and so hold the state before its migrations. This run's own are listed too
# when they differ.
explain_restore() {
    local first first_id first_date path kc_first kc_path
    first="$(first_run_with backup_complete)"
    first_id="${first%%$'\t'*}"
    first_date="${first#*$'\t'}"
    echo
    echo "If the data migrations (the crawler-migrate Job) ran, or Keycloak started on a new"
    echo "version, the databases are migrated: restore them from the backups taken before"
    echo "${RC_TAG} was first deployed on this cluster${first:+ (run ${first_id}, ${first_date})}:"
    if [ -n "$first" ]; then
        while IFS= read -r path; do
            [ -n "$path" ] || continue
            echo "  ${path}"
            report "restore_backup=${path}"
            case "${path##*/}" in
                econ_graph-*) echo "    scripts/deploy/restore-postgres-backup.sh --context ${CONTEXT} --file $(printf '%q' "${path##*/}")" ;;
            esac
        done < <(recorded_in_run "$first_id" backup)
        echo "    (the others: docs/deployment/POSTGRES_BACKUP_RESTORE.md)"
    else
        echo "  (no complete backup of the data databases is recorded for ${RC_TAG} on this cluster in ${RECORD})"
    fi
    kc_first="$(first_run_with keycloak_backup)"
    if [ -n "$kc_first" ]; then
        kc_path="$(first_recorded keycloak_backup)"
        echo "  Keycloak (run ${kc_first%%$'\t'*}, ${kc_first#*$'\t'}): scripts/deploy/restore-keycloak-db.sh $(printf '%q' "$kc_path") --context ${CONTEXT}"
        report "restore_keycloak_backup=${kc_path}"
    else
        echo "  Keycloak: no verified dump is recorded for ${RC_TAG} on this cluster; scripts/deploy/restore-keycloak-db.sh --list"
    fi
    local later=()
    [ "${#BACKUPS[@]}" -eq 0 ] || [ "$first_id" = "$RUN_ID" ] || later+=("${BACKUPS[@]}")
    [ -z "$KEYCLOAK_BACKUP" ] || [ "$KEYCLOAK_BACKUP" = "${kc_path:-}" ] || later+=("$KEYCLOAK_BACKUP")
    if [ "${#later[@]}" -gt 0 ]; then
        echo "This run's backups below were taken after an earlier run of ${RC_TAG}, which may have"
        echo "migrated the databases already: use the ones above, not these:"
        printf '  %s\n' "${later[@]}"
    fi
}

# What a failure at step $1 leaves, and how to go back. HOW says what happened
# to it (failed, or was interrupted).
explain_failure() {
    local failed="$1" how="${2:-failed}" step
    echo
    echo "==> ${RC_TAG}: step ${failed} ${how}; nothing after it ran."
    for step in "${STEPS[@]}"; do
        printf '  %-9s %s\n' "$step" "${STATE[$step]:-not run}"
    done
    if [ "${#BACKUPS[@]}" -gt 0 ] || [ -n "$KEYCLOAK_BACKUP" ]; then
        echo "  backups this run took:"
        printf '    %s\n' "${BACKUPS[@]}" ${KEYCLOAK_BACKUP:+"$KEYCLOAK_BACKUP"}
    fi
    echo
    case "$failed" in
        backup)
            if [ -n "$BACKUP_STILL_RUNNING" ]; then
                echo "The backup Job ${BACKUP_STILL_RUNNING} is still running: it timed out here, but was"
                echo "not stopped. Wait for it to finish:"
                echo "  ${kubectl_hint} wait --for=condition=complete ${BACKUP_STILL_RUNNING} --timeout=3600s"
                echo "and only then run this command again: a run starts another backup Job, which"
                echo "would run beside this one. Nothing else was changed."
            else
                echo "No image was loaded and the databases are unchanged. Fix the backup and run this"
                echo "command again."
            fi
            echo "If an earlier run loaded ${RC_TAG}, its images are still on the nodes; go back from them with:" ;;
        load)
            echo "If the load refused (its last line says so), nothing was changed. Otherwise"
            echo "the images may be partly loaded: if the last record in"
            echo "  ${ECONGRAPH_RELEASES_DIR:-$HOME/econgraph-releases}/${RC_TAG}/images.txt"
            echo "says status=partial, some nodes run the new images when their pods restart."
            echo "Fix the cause and run this command again, or go back with:" ;;
        keycloak)
            echo "The new images are loaded on the nodes under the names the manifests run;"
            echo "running pods keep the old ones until they restart. The databases are unchanged."
            echo "Fix the Keycloak backup and run this command again, or go back with:" ;;
        precheck)
            echo "The new images are loaded, the backups are taken, and the crawler worker"
            echo "may be stopped (see below)."
            echo "Fix what the precheck reported and run this command again, or go back with:" ;;
        deploy)
            if [ -n "$NOT_STARTED" ]; then
                echo "deploy.sh was not started: ${NOT_STARTED}. The new images are loaded, the backups"
                echo "are taken, and Keycloak and the crawler worker may be stopped (see below)."
                echo "Restore the checkout and run this command again, or go back with:"
            else
                echo "deploy.sh stopped partway: some manifests are applied, and the data migrations"
                echo "(crawler-migrate Job) may have run. Pods that restarted run the new images."
                echo "Go back with:"
            fi ;;
        verify)
            echo "The deploy finished, but not every pod runs ${RC_TAG}'s images (see above)."
            echo "Run the verify step again once rollouts settle, or go back with:" ;;
    esac
    echo "  scripts/deploy/rollback-release-images.sh --to pre-${RC_TAG}${cluster_arg} --dry-run   # then without --dry-run"
    case "$failed" in
        deploy | verify) [ -n "$NOT_STARTED" ] || explain_restore ;;
    esac
    case "$failed" in
        keycloak | precheck | deploy)
            replicas_advice keycloak "$keycloak_now" keycloak_replicas k8s/manifests/keycloak/deployment.yaml
            if [ -n "$REPLICAS" ] && [ "$REPLICAS" != 0 ]; then
                echo
                echo "Keycloak (sign-in) may be stopped: the keycloak step scales it to 0, and only"
                echo "deploy.sh applies it again; ${REPLICAS_FROM}."
                echo "If it is at 0, start it with:"
                echo "  ${kubectl_hint} scale deployment/keycloak --replicas=${REPLICAS}"
            fi ;;
    esac
    case "$failed" in
        precheck | deploy)
            replicas_advice crawler-worker "$crawler_now" crawler_replicas k8s/manifests/crawler-worker.yaml
            if [ -n "$REPLICAS" ] && [ "$REPLICAS" != 0 ]; then
                echo
                echo "The crawler worker may be stopped: the precheck scales it to 0, and only deploy.sh"
                echo "starts it again, when it applies crawler-worker.yaml after the crawler-migrate Job."
                echo "Any failure before that leaves it at 0. Here, ${REPLICAS_FROM}."
                echo "The nodes hold ${RC_TAG}'s crawler image under the name it runs, so start it only after"
                echo "the rollback above (or a restore), or once a deploy has applied the migrations:"
                echo "  ${kubectl_hint} scale deployment/crawler-worker --replicas=${REPLICAS}"
            fi ;;
    esac
}

# What step $1 (exit status $2) reported in STEP_LOG, kept for the summary, the
# report and the record. Backups count only from a step that succeeded: a
# backup step that failed partway took no complete backup.
collect() {
    local path status="$2"
    case "$1" in
        backup)
            BACKUP_STILL_RUNNING="$(sed -n 's/^backup-postgres-now: still running: \(job\/[^ ]*\)$/\1/p' "$STEP_LOG" | tail -1)"
            [ "$status" = 0 ] || return 0
            # backup.sh's line per dump: "[name] Backup OK: N tables, SIZE in PATH".
            while IFS= read -r path; do
                [ -n "$path" ] || continue
                BACKUPS+=("$path")
                report "backup=${path}"
                record backup "$path" || return 1
            done < <(sed -n 's/.*Backup OK: [0-9]* tables, [^ ]* in \(.*\.dump\)$/\1/p' "$STEP_LOG")
            [ "${#BACKUPS[@]}" -eq 0 ] || record backup_complete 1 ;;
        keycloak)
            [ "$status" = 0 ] || return 0
            KEYCLOAK_BACKUP="$(sed -n 's/^backup-keycloak-db: verified dump in \(.*\)$/\1/p' "$STEP_LOG" | tail -1)"
            if [ -n "$KEYCLOAK_BACKUP" ]; then
                report "keycloak_backup=${KEYCLOAK_BACKUP}"
                record keycloak_backup "$KEYCLOAK_BACKUP"
            fi ;;
        precheck)
            # Recorded whether or not the precheck then passed: it did scale the worker down.
            path="$(sed -n 's/^precheck-db-migration: scaling deployment\/crawler-worker from \([0-9][0-9]*\) to 0$/\1/p' "$STEP_LOG" | tail -1)"
            [ -z "$path" ] || record crawler_replicas "$path" ;;
        verify)
            UNCHECKED="$(grep '^  unchecked ' "$STEP_LOG" || true)" ;;
    esac
}

# Ctrl-C and SIGTERM: stop the running step and everything it started, then
# give the guidance for that step. A step runs in the background (which ignores
# SIGINT from the terminal), so it is sent SIGTERM whatever the signal was.
STEP_PID=""
INTERRUPTED=""
kill_tree() {
    local child
    for child in $(pgrep -P "$1" 2>/dev/null); do kill_tree "$child"; done
    kill -TERM "$1" 2>/dev/null || true
}
on_signal() {
    [ -n "$INTERRUPTED" ] || INTERRUPTED="$1"
    [ -z "$STEP_PID" ] || kill_tree "$STEP_PID"
}

# Run step $1 with its output on the terminal and in a step log of its own, and
# set STATUS to the step's own exit status. Its output goes through tee in a
# process substitution, not a pipe: a background process the step leaves behind
# keeps that open, and the run must not wait for it to close. tee gets a moment
# to finish writing the log; a tee that fails to write is not the step's
# failure. The step runs in the background with this run's stdin, so a signal
# reaches the trap above while it runs.
run_step() {
    local tee_fd tee_pid i
    STEP_LOG="${STEP_LOG_DIR}/${1}.log"
    : >"$STEP_LOG" 2>/dev/null || true
    exec {tee_fd}> >(tee "$STEP_LOG" || true)
    tee_pid=$!
    STATUS=0
    # shellcheck disable=SC2086 # CMD holds the command and its arguments, none with spaces
    (cd "$PROJECT_ROOT" && exec ${CMD[$1]} "${dry[@]}") <&0 >&"$tee_fd" 2>&1 &
    STEP_PID=$!
    # wait returns early when a trapped signal arrives; wait again until the
    # step itself has exited.
    while :; do
        STATUS=0
        wait "$STEP_PID" || STATUS=$?
        kill -0 "$STEP_PID" 2>/dev/null || break
    done
    STEP_PID=""
    exec {tee_fd}>&-
    for i in $(seq 1 30); do
        kill -0 "$tee_pid" 2>/dev/null || break
        sleep 0.1
    done
}

# stop STEP STATUS-WORD RESULT: the step did not complete: report it, explain,
# and exit.
stop() {
    local step="$1" how="$2" result="$3"
    report "step=${step} status=${how%% *}"
    if [ "$DRY_RUN" = true ]; then
        echo
        echo "==> ${RC_TAG}: the dry run of step ${step} ${how}; nothing was changed."
    else
        explain_failure "$step" "$how" || echo "${SCRIPT}: warning: the failure summary above is incomplete" >&2
    fi
    report "result=${result}"
    case "$INTERRUPTED" in
        INT) exit 130 ;;
        TERM) exit 143 ;;
    esac
    exit 1
}

trap 'on_signal INT' INT
trap 'on_signal TERM' TERM
for step in "${STEPS[@]}"; do
    if [ -n "$INTERRUPTED" ]; then
        STATE[$step]="not run (interrupted before it started)"
        stop "$step" "interrupted (SIG${INTERRUPTED}) before it started" "interrupted step=${step} signal=${INTERRUPTED}"
    fi
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
    # deploy.sh applies what is in the checkout now: check it again.
    if [ "$step" = deploy ] && ! why="$(check_checkout)"; then
        echo "${SCRIPT}: error: ${why}" >&2
        NOT_STARTED="the checkout changed during the run"
        STATE[$step]="REFUSED (the checkout changed)"
        stop "$step" "refused" "failed step=${step}"
    fi
    run_step "$step"
    if [ -n "$INTERRUPTED" ]; then
        collect "$step" 1 || true
        STATE[$step]="INTERRUPTED (SIG${INTERRUPTED})"
        stop "$step" "interrupted (SIG${INTERRUPTED})" "interrupted step=${step} signal=${INTERRUPTED}"
    fi
    if ! collect "$step" "$STATUS"; then
        if [ -n "$RECORD_FAILED" ]; then
            STATE[$step]="FAILED (could not record its result in ${RECORD})"
            stop "$step" "failed" "failed step=${step}"
        fi
        echo "${SCRIPT}: warning: could not read what step ${step} reported from ${STEP_LOG}" >&2
    fi
    [ -z "$RECORD_FAILED" ] || {
        STATE[$step]="FAILED (could not record its result in ${RECORD})"
        stop "$step" "failed" "failed step=${step}"
    }
    if [ "$STATUS" = 0 ]; then
        STATE[$step]="$([ "$DRY_RUN" = true ] && echo "dry run ok" || echo "done")"
        extra=""
        if [ "$step" = verify ] && [ -n "$UNCHECKED" ]; then
            STATE[$step]="done, $(grep -c . <<<"$UNCHECKED") unchecked"
            extra=" unchecked=$(grep -c . <<<"$UNCHECKED")"
        fi
        report "step=${step} status=$([ "$DRY_RUN" = true ] && echo dry-run || echo ok)${extra}"
    else
        STATE[$step]="FAILED"
        stop "$step" "failed" "failed step=${step}"
    fi
done
trap - INT TERM

if [ "$DRY_RUN" = true ]; then
    report "result=dry-run"
else
    report "result=ok$([ -z "$UNCHECKED" ] || echo " unchecked=$(grep -c . <<<"$UNCHECKED")")"
fi
echo
if [ "$DRY_RUN" = true ]; then
    echo "==> Dry run of ${RC_TAG} on ${CONTEXT} complete: nothing was changed."
else
    echo "==> ${RC_TAG} is deployed on ${CONTEXT} and verified."
    if [ -n "$UNCHECKED" ]; then
        echo "    Not checked (verify-release-images.sh could not; check these by hand):"
        while IFS= read -r line; do echo "      ${line#"${line%%[! ]*}"}"; done <<<"$UNCHECKED"
    fi
    echo "    Rollback copies of the images that ran before: pre-${RC_TAG} (scripts/deploy/rollback-release-images.sh${cluster_arg})."
fi
