#!/usr/bin/env bash
#
# Put a release candidate's images on the kind cluster's nodes, under the names
# the manifests run (econ-graph-<name>:<the manifests' tag>, imagePullPolicy:
# Never), and keep the images production runs now on every node as the way back.
#
# Usage:
#   scripts/deploy/load-release-images.sh --tag vX.Y.Z-rc.N [--dry-run] [--cluster econ-graph] [--allow-missing-nodes]
#
#   --tag TAG        the release candidate tag release.yml built
#   --dry-run        resolve the tag, the registry digests and the cluster's
#                    current images, run every check, and print what would
#                    happen; pull, tag, load and write nothing (it still runs
#                    git fetch --tags)
#   --cluster NAME   the kind cluster (default econ-graph); kubectl runs on
#                    context kind-NAME, the one deploy.sh deploys to
#   --allow-missing-nodes
#                    load even though some node lacks the image to keep (so
#                    rollback could not restore that node); the load is
#                    recorded, and the script exits 3
#
# The local names come from k8s/manifests, never from this script: for each
# image, every workload there that runs it (deployment/econ-graph-backend;
# deployment/crawler-worker and the crawler-migrate Job;
# deployment/econ-graph-frontend) must name the same tag, or the script stops
# before changing anything. Moving that one name on the nodes therefore covers
# the Job too: the next deploy.sh runs it on the loaded image.
#
# Steps:
#   1. git fetch --tags origin (an existing tag is never moved), and resolve
#      the tag to its commit.
#   2. The refs release.yml pushes: ghcr.io/econgraph/econ-graph-{backend,
#      crawler-worker,frontend}:sha-<full commit>, and :<tag> on the same
#      digest. Both must resolve to one digest on GHCR.
#   3. Decide what to keep for each image: the image its Deployment's running
#      pods use now (with none running, the one the manifests' name points at on
#      the nodes). When an earlier load of the same tag is on record, its first
#      record's choice wins, so a run after verify (or after a partial load)
#      keeps the same image. Stop before changing anything if the pods of one
#      Deployment run different images, if any node already has a pre-<tag>
#      copy of another image, or if any node has neither the copy nor the image
#      to copy (unless --allow-missing-nodes).
#   4. docker pull each image by that digest on the kind cluster's daemon
#      (lib/kind-env.sh: unix:///var/run/docker.sock, the rootful one, unless
#      ECONGRAPH_DOCKER_HOST says otherwise; the script stops if it isn't
#      reachable or is rootless). The packages are private: pulling uses the
#      operator's existing `docker login ghcr.io`. Each pulled image must carry
#      the digest and an org.opencontainers.image.revision label (release.yml
#      always sets one) equal to the commit; otherwise the script stops before
#      anything reaches the nodes.
#   5. On every node, tag the image to keep as econ-graph-<name>:pre-<tag> in
#      containerd (an existing copy is never moved), label it pinned, and
#      require the CRI to report it pinned, so kubelet's image garbage
#      collection keeps it. scripts/deploy/rollback-release-images.sh --to
#      pre-<tag> goes back to these. Pinned copies are never released
#      automatically; docs/deployment/RELEASE_IMAGES.md says how to remove an
#      old one.
#   6. docker tag each pulled image (by image ID) as the manifests' name, then
#      kind load docker-image into every node. This moves that name on the
#      nodes: the image it named before keeps only its pre-<tag> name. Then
#      check that every node now has the pulled image under that name.
#   7. Append the record to ~/econgraph-releases/<tag>/images.txt
#      (ECONGRAPH_RELEASES_DIR overrides the root): tag, commit, digests, image
#      IDs, the manifests' names, what ran before and the pre-<tag> copies, and
#      status=complete. A run that fails once it has started changing the nodes
#      appends status=partial with what it did (a failed pull changes no node
#      and records nothing). Earlier records are kept.
#
# Pods only pick the new images up when they restart: after scripts/deploy/deploy.sh,
# run scripts/deploy/verify-release-images.sh --tag <tag>. Until then any pod
# that restarts for another reason already starts the new image, and after a
# partial load some nodes have it and some don't.
#
# Running it again with the same tag is safe: an image that already carries the
# name is not retagged, pre-<tag> is kept as it is, and kind skips nodes that
# already have the image. Needs jq, and the pinned kind that
# scripts/deploy/install-kind.sh installs (`kind load` stops without it). Other
# environment: ECONGRAPH_DOCKER_HOST, KUBECTL, KUBECONFIG (default: the kind
# cluster's copies in ~/.cache/econgraph, as lib/cluster-role.sh picks them).

set -euo pipefail

RI_SCRIPT="load-release-images"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "$PROJECT_ROOT/scripts/deploy/lib/kind-env.sh"
# shellcheck source=scripts/deploy/lib/release-images.sh
source "$PROJECT_ROOT/scripts/deploy/lib/release-images.sh"

TAG=""
DRY_RUN=false
ALLOW_MISSING=false
CLUSTER="econ-graph"
while [ $# -gt 0 ]; do
    case "$1" in
        --tag)
            [ $# -ge 2 ] || { echo "${RI_SCRIPT}: error: --tag needs a value" >&2; exit 2; }
            TAG="$2"; shift 2 ;;
        --cluster)
            [ $# -ge 2 ] || { echo "${RI_SCRIPT}: error: --cluster needs a value" >&2; exit 2; }
            CLUSTER="$2"; shift 2 ;;
        --dry-run) DRY_RUN=true; shift ;;
        --allow-missing-nodes) ALLOW_MISSING=true; shift ;;
        -h | --help) awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"; exit 0 ;;
        *) echo "${RI_SCRIPT}: error: unknown argument: $1" >&2; exit 2 ;;
    esac
done
[ -n "$TAG" ] || { echo "${RI_SCRIPT}: error: --tag is required" >&2; exit 2; }
ri_check_tag "$TAG"
ri_need_jq

# 1. The commit.
git -C "$PROJECT_ROOT" fetch --tags origin ||
    ri_die "git fetch --tags origin failed (a tag that moved on origin is not updated here; check it by hand)"
COMMIT="$(git -C "$PROJECT_ROOT" rev-parse --verify --quiet "refs/tags/${TAG}^{commit}")" ||
    ri_die "tag ${TAG} not found"
[[ "$COMMIT" =~ ^[0-9a-f]{40}$ ]] || ri_die "tag ${TAG} resolved to '${COMMIT}', not a full commit SHA"
echo "${TAG} is commit ${COMMIT}"

ri_use_rootful_docker
# `kind load` needs the pinned kind (scripts/deploy/install-kind.sh): an older
# one cannot load into the nodes. Checked before anything changes, and in a dry run.
require_kind_version
ri_kind_nodes "$CLUSTER"
ri_pin_context "$CLUSTER"

# The digest GHCR reports for ref $1, read the way release.yml reads it.
registry_digest() {
    local digest
    digest="$(docker buildx imagetools inspect "$1" --format '{{json .Manifest}}' | jq -r '.digest // empty')" || return 1
    [[ "$digest" =~ ^sha256:[0-9a-f]{64}$ ]] || return 1
    printf '%s\n' "$digest"
}

# 2. Refs and registry digests.
declare -A SHA_REF PIN_REF LOCAL DIGEST
for name in "${RI_NAMES[@]}"; do
    repo="$(ri_remote_repo "$name")"
    SHA_REF[$name]="${repo}:sha-${COMMIT}"
    LOCAL[$name]="$(ri_local_image "$name")"
    sha_digest="$(registry_digest "${SHA_REF[$name]}")" ||
        ri_die "could not resolve ${SHA_REF[$name]} on ${RI_REGISTRY} (not built yet, or no docker login ghcr.io with read:packages?)"
    rc_digest="$(registry_digest "${repo}:${TAG}")" ||
        ri_die "could not resolve ${repo}:${TAG} on ${RI_REGISTRY}"
    [ "$sha_digest" = "$rc_digest" ] ||
        ri_die "${repo}:${TAG} is ${rc_digest} but ${SHA_REF[$name]} is ${sha_digest}; refusing"
    DIGEST[$name]="$sha_digest"
    PIN_REF[$name]="${repo}@${sha_digest}"
    echo "  ${SHA_REF[$name]}"
    echo "    = ${repo}:${TAG} = ${PIN_REF[$name]} -> ${LOCAL[$name]}"
done

# 3. What to keep, and every check, before anything changes.
FIRST_RECORD="$(ri_first_record "$TAG")"
declare -A KEEP PLAN
before_lines=""
missing_nodes=""
for name in "${RI_NAMES[@]}"; do
    pin="$(ri_rollback_ref "$name" "$TAG")"
    running="$(ri_running_image_ids "$name")" || ri_die "could not list the pods of deployment/${RI_DEPLOYMENT[$name]}"
    if [ "$(grep -c . <<<"$running")" -gt 1 ]; then
        ri_die "the running pods of deployment/${RI_DEPLOYMENT[$name]} use different images ($(paste -sd, <<<"$running")); let that rollout finish first"
    fi
    node_line=""
    named=""
    for node in "${RI_NODES[@]}"; do
        id="$(ri_node_image_id "$node" "$(ri_node_ref "${LOCAL[$name]}")")" || exit 1
        node_line+=" ${node}=${id:-none}"
        [ -z "$id" ] || named+="${id}"$'\n'
    done
    keep="$running"
    if [ -z "$keep" ]; then
        keep="$(grep . <<<"$named" | sort -u || true)"
        [ "$(grep -c . <<<"$keep")" -le 1 ] || keep=""
    fi
    recorded="$(ri_before_field "$name" keep "$FIRST_RECORD")"
    if [ -n "$recorded" ] && [ "$recorded" != none ] && [ "$recorded" != "$keep" ]; then
        echo "${name}: keeping ${recorded}, which ran when ${TAG} was first loaded (runs now: ${running:-nothing})"
        keep="$recorded"
    fi
    KEEP[$name]="$keep"
    before_lines+="before=${name} local=${LOCAL[$name]} running=${running:-none} keep=${keep:-none}${node_line}"$'\n'
    [ -n "$keep" ] || { echo "${name}: nothing runs or is named ${LOCAL[$name]}; nothing to keep"; continue; }

    nodes_with_copy=0
    for node in "${RI_NODES[@]}"; do
        existing="$(ri_node_image_id "$node" "$pin")" || exit 1
        if [ -n "$existing" ]; then
            [ "$existing" = "$keep" ] ||
                ri_die "${node} already has ${pin} as ${existing}, but the image to keep is ${keep}; refusing to load over a copy of another image"
            PLAN[$name,$node]=kept
            nodes_with_copy=$((nodes_with_copy + 1))
            continue
        fi
        source_id="$(ri_node_image_id "$node" "$keep")" || exit 1
        if [ -n "$source_id" ]; then
            PLAN[$name,$node]=create
            nodes_with_copy=$((nodes_with_copy + 1))
        else
            PLAN[$name,$node]=missing
            missing_nodes+=" ${node}:${name}"
        fi
    done
    [ "$nodes_with_copy" -gt 0 ] ||
        ri_die "no node has ${keep}, the image to keep for deployment/${RI_DEPLOYMENT[$name]}; there would be no way back"
done
if [ -n "$missing_nodes" ] && [ "$ALLOW_MISSING" = false ]; then
    ri_die "these nodes lack the image to keep, so rollback could not restore them:${missing_nodes}; refusing (--allow-missing-nodes loads anyway)"
fi
printf '%s' "$before_lines"

if [ "$DRY_RUN" = true ]; then
    echo "${RI_SCRIPT}: dry run, nothing pulled, tagged, loaded or written. Would:"
    for name in "${RI_NAMES[@]}"; do
        [ -z "${KEEP[$name]}" ] ||
            echo "  keep ${KEEP[$name]} as $(ri_rollback_ref "$name" "$TAG") (pinned) on ${RI_NODES[*]}"
        echo "  docker pull ${PIN_REF[$name]} (DOCKER_HOST=${DOCKER_HOST}), check its digest and revision label"
        echo "  docker tag it ${LOCAL[$name]}; $(kind_bin load) load docker-image ${LOCAL[$name]} --name ${CLUSTER}"
    done
    [ -z "$missing_nodes" ] || echo "  and exit 3: no copy on${missing_nodes}"
    echo "  append the record to $(ri_record_file "$TAG")"
    exit 0
fi

RECORD_STATUS=""
STEP=""
done_lines=""
# Pulling changes nothing on the nodes, so a failed pull leaves no record and
# an earlier complete load stays the last one.
write_record() {
    local exit_status=$?
    [ -n "$RECORD_STATUS" ] || RECORD_STATUS="partial (exit ${exit_status} during: ${STEP})"
    ri_append_block "$(ri_record_file "$TAG")" "loaded_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
status=${RECORD_STATUS}
tag=${TAG}
commit=${COMMIT}
cluster=${CLUSTER}
context=${CLUSTER_ROLE_CONTEXT}
nodes=${RI_NODES[*]}
missing=${missing_nodes# }
${before_lines}${done_lines}"
    if [ "${RECORD_STATUS}" != complete ]; then
        echo "${RI_SCRIPT}: recorded the partial load in $(ri_record_file "$TAG")" >&2
    fi
}

# 4. Pull and check all three before anything reaches the nodes.
declare -A IMAGE_ID
for name in "${RI_NAMES[@]}"; do
    ref="${PIN_REF[$name]}"
    docker pull "$ref"
    pulled="$(docker image inspect --format '{{range .RepoDigests}}{{println .}}{{end}}' "$ref" |
        grep -xF -- "$ref" || true)"
    [ "$pulled" = "$ref" ] ||
        ri_die "pulled ${ref} does not carry digest ${DIGEST[$name]} (its digests: $(docker image inspect --format '{{join .RepoDigests ","}}' "$ref")); refusing"
    revision="$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.revision"}}' "$ref")"
    if [ -z "$revision" ] || [ "$revision" = "<no value>" ]; then
        ri_die "${ref} has no org.opencontainers.image.revision label; refusing"
    fi
    [ "$revision" = "$COMMIT" ] ||
        ri_die "${ref} has org.opencontainers.image.revision ${revision}, not ${COMMIT}; refusing"
    IMAGE_ID[$name]="$(docker image inspect --format '{{.Id}}' "$ref")"
    echo "${ref}: revision ${revision}, image ID ${IMAGE_ID[$name]}"
done

# 5. Keep what runs now. From here on the nodes change, so a failure leaves a
# status=partial record of what was done.
trap write_record EXIT
for name in "${RI_NAMES[@]}"; do
    STEP="keeping ${name} as pre-${TAG}"
    keep="${KEEP[$name]}"
    [ -n "$keep" ] || continue
    pin="$(ri_rollback_ref "$name" "$TAG")"
    for node in "${RI_NODES[@]}"; do
        case "${PLAN[$name,$node]}" in
            missing)
                echo "warning: ${node} does not have ${keep}; no ${pin} there" >&2
                done_lines+="pinned=${name} node=${node} ref=${pin} id=${keep} missing"$'\n'
                continue ;;
            create)
                src="$(ri_node_source_ref "$node" "$keep")" || exit 1
                ri_ctr_tag "$node" "$src" "$pin" >/dev/null ;;
        esac
        ri_ctr_pin "$node" "$pin"
        id="$(ri_node_image_id "$node" "$pin")" || exit 1
        [ "$id" = "$keep" ] || ri_die "${pin} on ${node} is '${id}', not ${keep}, after tagging it"
        done_lines+="pinned=${name} node=${node} ref=${pin} id=${keep} ${PLAN[$name,$node]}"$'\n'
        echo "${node}: ${pin} = ${keep} (pinned, ${PLAN[$name,$node]})"
    done
done

# 6. Name and load. The record's image ID is the one the nodes report, which is
# what pods' imageIDs are compared with.
for name in "${RI_NAMES[@]}"; do
    STEP="loading ${name}"
    current="$(docker image inspect --format '{{.Id}}' "${LOCAL[$name]}" 2>/dev/null || true)"
    if [ "$current" = "${IMAGE_ID[$name]}" ]; then
        echo "${LOCAL[$name]} is already ${IMAGE_ID[$name]}"
    else
        docker tag "${IMAGE_ID[$name]}" "${LOCAL[$name]}"
    fi
    kind load docker-image "${LOCAL[$name]}" --name "$CLUSTER"
    for node in "${RI_NODES[@]}"; do
        id="$(ri_node_image_id "$node" "$(ri_node_ref "${LOCAL[$name]}")")" || exit 1
        # The node's ID is the config digest; Docker's .Id is too, except on
        # Docker's containerd image store, which this check would refuse.
        [ "$id" = "${IMAGE_ID[$name]}" ] ||
            ri_die "after kind load, ${LOCAL[$name]} on ${node} is '${id}', not ${IMAGE_ID[$name]}"
    done
    done_lines+="image=${name} ref=${PIN_REF[$name]} digest=${DIGEST[$name]} id=${IMAGE_ID[$name]} local=${LOCAL[$name]} rollback=$(ri_rollback_ref "$name" "$TAG")"$'\n'
done

RECORD_STATUS=complete
echo "Loaded ${TAG} (${COMMIT}) into kind cluster ${CLUSTER}; recorded in $(ri_record_file "$TAG")."
echo "Next: scripts/deploy/deploy.sh, then scripts/deploy/verify-release-images.sh --tag ${TAG}"
echo "Way back: scripts/deploy/rollback-release-images.sh --to pre-${TAG}"
if [ -n "$missing_nodes" ]; then
    echo "${RI_SCRIPT}: loaded with --allow-missing-nodes; no rollback copy on:${missing_nodes}" >&2
    exit 3
fi
