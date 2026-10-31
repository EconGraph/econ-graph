#!/usr/bin/env bash
#
# Go back to the images production ran before load-release-images.sh loaded a
# release candidate: point the manifests' names (econ-graph-<name>:<the
# manifests' tag>) on every kind node back at the pre-<tag> copies that load
# kept, restart the Deployments, and check that their pods run those images.
#
# Usage:
#   scripts/deploy/rollback-release-images.sh --to pre-vX.Y.Z-rc.N [--dry-run] [--no-restart] [--cluster econ-graph] [--allow-missing-nodes]
#
#   --to pre-TAG     the rollback copies to go back to, as load-release-images.sh
#                    --tag TAG named them
#   --dry-run        run every check and print what would happen; retag,
#                    restart and write nothing
#   --no-restart     retag on the nodes only; pods switch when they next restart
#   --cluster NAME   the kind cluster (default econ-graph); kubectl runs on
#                    context kind-NAME
#   --allow-missing-nodes
#                    go ahead even though a node that has the manifests' name
#                    lacks the copy; that node is left as it is, and the
#                    script exits 3 even when every pod checks out
#
# Steps:
#   1. Checks, before anything changes: the first load record for TAG
#      (~/econgraph-releases/<tag>/images.txt) says which image each Deployment
#      ran before; on every node econ-graph-<name>:pre-<tag> must be that image
#      (one image across the nodes, and the recorded one), and every node that
#      has the manifests' name must have the copy (unless
#      --allow-missing-nodes). The name is the one the load moved (recorded as
#      local=), and the manifests must still run it: if they have moved to
#      another tag since, retagging the old name would change nothing, so the
#      script stops.
#   2. On each node, `ctr -n k8s.io images tag --force` the copy to the
#      manifests' name (docker exec on the rootful daemon), and check that the
#      name now points at it. pre-<tag> stays. ctr copies the copy's labels, so
#      the name is pinned too.
#   3. kubectl rollout restart and rollout status for the four Deployments,
#      then check that every running pod's imageID is the copy's image ID.
#   4. Append the result to ~/econgraph-releases/<tag>/rollback.txt; a run that
#      fails after it starts retagging records status=partial with what it did.
#
# The crawler-migrate Job runs the crawler-worker image under the same name as
# deployment/crawler-worker (ri_local_image refuses manifests where they
# differ), so the retag in step 2 moves the Job's image as well: it is never
# left on the release candidate while the Deployment goes back. The Job is not
# rerun here; the next deploy.sh runs it on the restored image. The data
# migrations the release candidate's Job applied stay applied: a rollback of
# images does not undo them.
#
# A pod spec that names another tag is not changed by this. The rootful
# daemon's own copy of the manifests' name still names the release candidate;
# the nodes are what pods run from. Running it again is safe. Needs jq. Other
# environment: ECONGRAPH_DOCKER_HOST, KUBECTL, KUBECONFIG,
# ECONGRAPH_RELEASES_DIR, as in load-release-images.sh.

set -euo pipefail

RI_SCRIPT="rollback-release-images"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

TO=""
DRY_RUN=false
RESTART=true
ALLOW_MISSING=false
CLUSTER="econ-graph"
CLUSTER_GIVEN=false
while [ $# -gt 0 ]; do
    case "$1" in
        --to)
            [ $# -ge 2 ] || { echo "${RI_SCRIPT}: error: --to needs a value" >&2; exit 2; }
            TO="$2"; shift 2 ;;
        --cluster)
            [ $# -ge 2 ] || { echo "${RI_SCRIPT}: error: --cluster needs a value" >&2; exit 2; }
            CLUSTER="$2"; CLUSTER_GIVEN=true; shift 2 ;;
        --no-restart) RESTART=false; shift ;;
        --dry-run) DRY_RUN=true; shift ;;
        --allow-missing-nodes) ALLOW_MISSING=true; shift ;;
        -h | --help) awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"; exit 0 ;;
        *) echo "${RI_SCRIPT}: error: unknown argument: $1" >&2; exit 2 ;;
    esac
done

# --cluster names the cluster. KIND_CLUSTER and ECONGRAPH_SCRATCH_STORAGE pick
# the cluster and host storage of the host-storage scripts, which this one does
# not use; kind-env.sh stops on some pairs of them, so they are dropped when
# --cluster is given. Without --cluster, kind-env.sh's checks still apply.
if [ "$CLUSTER_GIVEN" = true ]; then
    unset KIND_CLUSTER ECONGRAPH_SCRATCH_STORAGE
fi
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "$PROJECT_ROOT/scripts/deploy/lib/kind-env.sh"
# shellcheck source=scripts/deploy/lib/release-images.sh
source "$PROJECT_ROOT/scripts/deploy/lib/release-images.sh"
[ -n "$TO" ] || { echo "${RI_SCRIPT}: error: --to is required" >&2; exit 2; }
[[ "$TO" == pre-* ]] || ri_die "'${TO}' is not a rollback name (pre-vX.Y.Z-rc.N)"
TAG="${TO#pre-}"
ri_check_tag "$TAG"
ri_need_jq

FIRST_RECORD="$(ri_first_record "$TAG")"
[ -n "$FIRST_RECORD" ] ||
    ri_die "no load record $(ri_record_file "$TAG"); it says which images ran before, and the copies are checked against it"

ri_use_rootful_docker
ri_kind_nodes "$CLUSTER"
ri_pin_context "$CLUSTER"

# 1. Checks.
declare -A WANT LOCAL PLAN
missing=""
for name in "${RI_NAMES[@]}"; do
    pin="$(ri_rollback_ref "$name" "$TAG")"
    # The name to point back: the one every load of TAG moved, which must still
    # be what the manifests (Deployment and Jobs alike) run. Retagging another
    # name would change nothing the pods run.
    manifest_local="$(ri_local_image "$name")"
    recorded_local="$(ri_recorded_locals "$name" "$TAG")"
    if [ "$(grep -c . <<<"$recorded_local")" -gt 1 ]; then
        ri_die "the loads of ${TAG} moved different names for ${name} ($(paste -sd, <<<"$recorded_local")); refusing to guess which to point back"
    fi
    if [ -n "$recorded_local" ] && [ "$recorded_local" != "$manifest_local" ]; then
        ri_die "the load of ${TAG} moved ${recorded_local}, but the manifests now run ${manifest_local}; pointing ${recorded_local} back would not change what the pods run. Refusing"
    fi
    LOCAL[$name]="$(ri_node_ref "$manifest_local")"
    recorded="$(ri_before_field "$name" keep "$FIRST_RECORD")"
    if [ -z "$recorded" ] || [ "$recorded" = none ]; then
        ri_die "the first load record for ${TAG} keeps nothing for ${name}; there is nothing to go back to"
    fi
    ids=""
    for node in "${RI_NODES[@]}"; do
        id="$(ri_node_image_id "$node" "$pin")" || exit 1
        named="$(ri_node_image_id "$node" "${LOCAL[$name]}")" || exit 1
        if [ -n "$id" ]; then
            ids+="${id}"$'\n'
            if [ "$named" = "$id" ]; then PLAN[$name,$node]=already; else PLAN[$name,$node]=retag; fi
        elif [ -n "$named" ]; then
            PLAN[$name,$node]=missing
            missing+=" ${node}:${pin}"
        else
            PLAN[$name,$node]=absent
        fi
    done
    ids="$(grep . <<<"$ids" | sort -u || true)"
    [ -n "$ids" ] || ri_die "no node has ${pin}; nothing to go back to for ${name}"
    [ "$(grep -c . <<<"$ids")" -eq 1 ] ||
        ri_die "${pin} names different images on different nodes ($(paste -sd, <<<"$ids")); refusing"
    [ "$ids" = "$recorded" ] ||
        ri_die "${pin} is ${ids}, but the load record says ${recorded} ran before; refusing to restore another image"
    WANT[$name]="$ids"
    echo "${pin} = ${ids} -> ${LOCAL[$name]}"
done
if [ -n "$missing" ] && [ "$ALLOW_MISSING" = false ]; then
    ri_die "these nodes have the manifests' names but no copy to go back to:${missing}; refusing (--allow-missing-nodes leaves them as they are)"
fi

if [ "$DRY_RUN" = true ]; then
    echo "${RI_SCRIPT}: dry run, nothing retagged, restarted or written. Would:"
    for name in "${RI_NAMES[@]}"; do
        for node in "${RI_NODES[@]}"; do
            case "${PLAN[$name,$node]}" in
                retag) echo "  ${node}: ctr -n ${RI_CTR_NAMESPACE} images tag --force $(ri_rollback_ref "$name" "$TAG") ${LOCAL[$name]}" ;;
                already) echo "  ${node}: ${LOCAL[$name]} is already ${WANT[$name]}" ;;
                missing) echo "  ${node}: leave ${LOCAL[$name]} as it is (no copy)" ;;
            esac
        done
    done
    [ "$RESTART" = false ] || echo "  rollout restart and wait for the four Deployments on ${CLUSTER_ROLE_CONTEXT}, then check their pods"
    [ -z "$missing" ] || echo "  and exit 3: no copy on${missing}"
    exit 0
fi

# From here on the nodes change; the record says how far it got.
RECORD_STATUS=""
STEP=""
done_lines=""
failed=false
RI_REPORT=""
write_record() {
    local exit_status=$?
    [ -n "$RECORD_STATUS" ] || RECORD_STATUS="partial (exit ${exit_status} during: ${STEP})"
    ri_append_block "$(dirname "$(ri_record_file "$TAG")")/rollback.txt" "rolled_back_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
status=${RECORD_STATUS}
to=${TO}
context=${CLUSTER_ROLE_CONTEXT}
nodes=${RI_NODES[*]}
missing=${missing# }
restarted=${RESTART}
result=$([ "$failed" = true ] && echo FAILED || echo ok)
${done_lines}${RI_REPORT}"
    if [ "${RECORD_STATUS}" != complete ]; then
        echo "${RI_SCRIPT}: recorded the partial rollback in $(dirname "$(ri_record_file "$TAG")")/rollback.txt" >&2
    fi
}
trap write_record EXIT

# 2. Retag.
for name in "${RI_NAMES[@]}"; do
    STEP="retagging ${name}"
    pin="$(ri_rollback_ref "$name" "$TAG")"
    for node in "${RI_NODES[@]}"; do
        case "${PLAN[$name,$node]}" in
            missing | absent) continue ;;
            retag) ri_ctr_tag "$node" "$pin" "${LOCAL[$name]}" --force >/dev/null ;;
        esac
        id="$(ri_node_image_id "$node" "${LOCAL[$name]}")" || exit 1
        [ "$id" = "${WANT[$name]}" ] ||
            ri_die "${LOCAL[$name]} on ${node} is '${id}', not ${WANT[$name]}, after retagging it"
        done_lines+="retagged=${name} node=${node} local=${LOCAL[$name]} id=${WANT[$name]} ${PLAN[$name,$node]}"$'\n'
        echo "${node}: ${LOCAL[$name]} = ${WANT[$name]}"
    done
done

# 3. Restart and check.
if [ "$RESTART" = true ]; then
    STEP="restarting the Deployments"
    ri_restart_deployments
    STEP="checking the pods"
    for name in "${RI_NAMES[@]}"; do
        ri_check_pods "$name" "${WANT[$name]}" || failed=true
    done
fi

# 4. Record (the EXIT trap writes it).
RECORD_STATUS=complete
if [ "$failed" = true ]; then
    ri_die "some pods do not run the ${TO} images"
fi
if [ -n "$missing" ]; then
    echo "${RI_SCRIPT}: rolled back with --allow-missing-nodes; these nodes still have the release candidate:${missing}" >&2
    exit 3
fi
if [ "$RESTART" = true ]; then
    echo "Every running backend, crawler-worker, frontend and chart-api pod runs the ${TO} images."
else
    echo "The nodes name the ${TO} images; pods switch when they restart."
fi
