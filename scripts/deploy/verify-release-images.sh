#!/usr/bin/env bash
#
# After scripts/deploy/deploy.sh: restart the backend, crawler-worker and
# frontend Deployments so their pods start from the images
# load-release-images.sh put on the nodes, then check that every running pod
# runs exactly those images.
#
# Usage:
#   scripts/deploy/verify-release-images.sh --tag vX.Y.Z-rc.N [--no-restart] [--dry-run] [--cluster econ-graph]
#
#   --tag TAG        the release candidate load-release-images.sh loaded; the
#                    last record in ~/econgraph-releases/<tag>/images.txt names
#                    the image IDs to expect, and must say status=complete
#   --no-restart     only check the running pods
#   --dry-run        restart nothing and write nothing; check the pods running now
#   --cluster NAME   the kind cluster (default econ-graph); kubectl runs on
#                    context kind-NAME, the one deploy.sh deploys to
#
# Steps:
#   1. kubectl rollout restart, then rollout status (up to 600s), for
#      deployment/econ-graph-backend, deployment/crawler-worker and
#      deployment/econ-graph-frontend in namespace econ-graph. deploy.sh only
#      restarts pods whose spec changed, and loading a new image under the same
#      name changes no spec.
#   2. For every running pod of each Deployment (pods already terminating, or
#      not running yet, are listed, not checked), the imageID of its app
#      container must be the image ID the nodes reported after the load. Any
#      other image, or no running pod, fails.
#   3. The crawler-migrate Job, which deploy.sh reran on the crawler-worker
#      image: every pod the current Job left (selected by the Job's
#      batch.kubernetes.io/controller-uid, so never a pod of an earlier run)
#      whose container started (running, succeeded or failed) must have run the
#      loaded crawler-worker image. It is not restarted. A pod whose container
#      never started (Pending, ErrImageNeverPull, in an init container), or that
#      is being deleted, is recorded as unchecked. So is a missing Job, or one
#      with no pod (one never run here, or one
#      Kubernetes deleted a day after it finished): unchecked, not a
#      failure. The summary lists everything unchecked; run this soon after
#      deploy.sh to have the Job checked.
#   4. Append the result to ~/econgraph-releases/<tag>/verified.txt.
#
# Every kubectl call goes through kc (lib/cluster-role.sh) on the pinned
# context; the current context is not changed. Other environment: KUBECTL,
# KUBECONFIG (default: the kind cluster's copies in ~/.cache/econgraph, as
# lib/cluster-role.sh picks them), ECONGRAPH_RELEASES_DIR.

set -euo pipefail

RI_SCRIPT="verify-release-images"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$PROJECT_ROOT/scripts/deploy/lib/cluster-role.sh"
# shellcheck source=scripts/deploy/lib/release-images.sh
source "$PROJECT_ROOT/scripts/deploy/lib/release-images.sh"

TAG=""
DRY_RUN=false
RESTART=true
CLUSTER="econ-graph"
while [ $# -gt 0 ]; do
    case "$1" in
        --tag)
            [ $# -ge 2 ] || { echo "${RI_SCRIPT}: error: --tag needs a value" >&2; exit 2; }
            TAG="$2"; shift 2 ;;
        --cluster)
            [ $# -ge 2 ] || { echo "${RI_SCRIPT}: error: --cluster needs a value" >&2; exit 2; }
            CLUSTER="$2"; shift 2 ;;
        --no-restart) RESTART=false; shift ;;
        --dry-run) DRY_RUN=true; shift ;;
        -h | --help) awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"; exit 0 ;;
        *) echo "${RI_SCRIPT}: error: unknown argument: $1" >&2; exit 2 ;;
    esac
done
[ -n "$TAG" ] || { echo "${RI_SCRIPT}: error: --tag is required" >&2; exit 2; }
ri_check_tag "$TAG"

record="$(ri_last_record "$TAG")"
status="$(ri_record_value status "$record")"
[ "$status" = complete ] ||
    ri_die "the last load of ${TAG} is '${status:-unknown}', not complete; run load-release-images.sh --tag ${TAG} again (or roll back) first"
recorded_cluster="$(ri_record_value cluster "$record")"
[ "$recorded_cluster" = "$CLUSTER" ] ||
    ri_die "the last load of ${TAG} went to kind cluster '${recorded_cluster}', not ${CLUSTER}"
commit="$(ri_record_value commit "$record")"
declare -A WANT_ID WANT_DIGEST
for name in "${RI_NAMES[@]}"; do
    WANT_ID[$name]="$(ri_record_field "$name" id "$record")"
    WANT_DIGEST[$name]="$(ri_record_field "$name" digest "$record")"
    [ -n "${WANT_ID[$name]}" ] || ri_die "the load record for ${TAG} has no image ID for ${name}"
done
echo "${TAG} (${commit}), loaded $(ri_record_value loaded_at "$record")"

ri_pin_context "$CLUSTER"

# 1. Restart.
if [ "$RESTART" = true ] && [ "$DRY_RUN" = true ]; then
    for name in "${RI_NAMES[@]}"; do
        echo "dry run: would rollout restart deployment/${RI_DEPLOYMENT[$name]} on ${CLUSTER_ROLE_CONTEXT} and wait for it"
    done
    echo "dry run: checking the pods running now"
elif [ "$RESTART" = true ]; then
    ri_restart_deployments
fi

# 2. Check.
failed=false
RI_REPORT=""
for name in "${RI_NAMES[@]}"; do
    ri_check_pods "$name" "${WANT_ID[$name]}" "${WANT_DIGEST[$name]}" || failed=true
done

# 3. The Jobs that run one of the images (crawler-migrate), as deploy.sh left them.
for name in "${RI_NAMES[@]}"; do
    [ -n "${RI_JOB[$name]:-}" ] || continue
    echo "job/${RI_JOB[$name]} (${name} image):"
    ri_check_job_pods "$name" "${WANT_ID[$name]}" "${WANT_DIGEST[$name]}" || failed=true
done

# 4. Record.
if [ "$DRY_RUN" = false ]; then
    ri_append_block "$(dirname "$(ri_record_file "$TAG")")/verified.txt" "verified_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
tag=${TAG}
commit=${commit}
context=${CLUSTER_ROLE_CONTEXT}
restarted=${RESTART}
result=$([ "$failed" = true ] && echo FAILED || echo ok)
${RI_REPORT}"
fi

# Everything recorded as unchecked, after the result either way.
print_unchecked() {
    grep -q '^unchecked ' <<<"$RI_REPORT" || return 0
    echo "Not checked:"
    grep '^unchecked ' <<<"$RI_REPORT" | awk '{ print "  " $0 }'
}
if [ "$failed" = true ]; then
    print_unchecked
    ri_die "some pods do not run the images loaded for ${TAG}"
fi
echo "Every running backend, crawler-worker and frontend pod, and every crawler-migrate pod that started, runs ${TAG} (${commit})."
print_unchecked
