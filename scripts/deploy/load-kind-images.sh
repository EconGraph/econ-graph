#!/bin/bash

# Load locally built images (econ-graph-*, imagePullPolicy: Never) into the
# kind cluster, from whichever Docker daemon holds them. The cluster runs on
# the rootful daemon (lib/kind-env.sh); images built before the move to it
# exist only in the rootless daemon, so they are exported from there with
# `docker save` and imported with `kind load image-archive`: nothing is
# rebuilt, and the rootful daemon's image store is not touched.
#
# Idempotent: an image every node already holds with the same ID is skipped.
# After loading, each node's image ID is checked against the source's.
#
# Usage:
#   scripts/deploy/load-kind-images.sh [--dry-run]
#       [--from-docker-host URL]  daemon holding the images (default:
#                                 ECONGRAPH_IMAGE_DOCKER_HOST, else the
#                                 cluster's daemon)
#       (--list FILE | IMAGE...)  e.g. ~/econgraph-backups/<ts>/local-images.txt

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/kind-env.sh"

SOURCE_DOCKER_HOST="${ECONGRAPH_IMAGE_DOCKER_HOST:-${DOCKER_HOST}}"
IMAGES=()
usage="usage: $0 [--dry-run] [--from-docker-host URL] (--list FILE | IMAGE...)"
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run|-n) DRY_RUN="true"; shift ;;
        --from-docker-host) SOURCE_DOCKER_HOST="$2"; shift 2 ;;
        --list)
            [ -s "$2" ] || die "$2 is missing or empty"
            mapfile -t -O "${#IMAGES[@]}" IMAGES < <(grep -vE '^[[:space:]]*(#|$)' "$2")
            shift 2
            ;;
        -h|--help) echo "$usage"; exit 0 ;;
        -*) echo "$usage" >&2; exit 2 ;;
        *) IMAGES+=("$1"); shift ;;
    esac
done
[ "${#IMAGES[@]}" -gt 0 ] || die "no images given. $usage"
[ "${DRY_RUN}" = "true" ] && log "DRY RUN: checks run for real; nothing is changed"

log "Docker daemons"
check_docker_daemon
echo "  images from: ${SOURCE_DOCKER_HOST}"
src_docker() { docker -H "${SOURCE_DOCKER_HOST}" "$@"; }
src_docker version --format '{{.Server.Version}}' >/dev/null 2>&1 ||
    die "cannot reach the image source daemon ${SOURCE_DOCKER_HOST}"

cluster="present"
if ! kind_cluster_exists; then
    [ "${DRY_RUN}" = "true" ] || die "kind cluster '${KIND_CLUSTER}' not found on ${DOCKER_HOST}; run recreate-kind-cluster.sh first"
    cluster="absent"
    echo "  kind cluster '${KIND_CLUSTER}' not found (dry run: every image would be loaded)"
fi
nodes=()
if [ "$cluster" = "present" ]; then
    mapfile -t nodes < <(kind get nodes --name "${KIND_CLUSTER}")
fi

# node_image_id NODE IMAGE: the image's ID in the node's containerd, or "".
node_image_id() {
    docker exec "$1" crictl inspecti -o go-template --template '{{.status.id}}' "docker.io/library/$2" 2>/dev/null || true
}

# Scratch space for one image archive at a time (removed after each load).
tmp="${TMPDIR:-/tmp}/econgraph-images.<random>"
if [ "${DRY_RUN}" != "true" ]; then
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/econgraph-images.XXXXXX")"
    trap 'rm -rf "$tmp"' EXIT
fi

failed=false
for image in "${IMAGES[@]}"; do
    src_id="$(src_docker image inspect --format '{{.Id}}' "$image" 2>/dev/null)" ||
        die "$image is not in ${SOURCE_DOCKER_HOST}; nothing more was loaded"
    missing=()
    for node in "${nodes[@]}"; do
        [ "$(node_image_id "$node" "$image")" = "$src_id" ] || missing+=("$node")
    done
    if [ "$cluster" = "present" ] && [ "${#missing[@]}" -eq 0 ]; then
        echo "  = $image ${src_id:7:12} already on every node"
        continue
    fi
    echo "  $image ${src_id:7:12} -> ${missing[*]:-all nodes}"
    archive="$tmp/$(tr '/:' '__' <<<"$image").tar"
    if [ "${DRY_RUN}" = "true" ]; then
        echo "+ docker -H ${SOURCE_DOCKER_HOST} save $image -o $archive"
        echo "+ kind load image-archive $archive --name ${KIND_CLUSTER}"
        continue
    fi
    src_docker save "$image" -o "$archive"
    kind load image-archive "$archive" --name "${KIND_CLUSTER}"
    rm -f "$archive"
    for node in "${nodes[@]}"; do
        got="$(node_image_id "$node" "$image")"
        if [ "$got" != "$src_id" ]; then
            echo "error: $image on $node is ${got:-missing}, expected $src_id" >&2
            failed=true
        fi
    done
done
[ "$failed" = false ] || die "some images did not load correctly"
if [ "${DRY_RUN}" = "true" ]; then
    log "Dry run complete; nothing was changed"
else
    log "All ${#IMAGES[@]} images are on every node of ${KIND_CLUSTER}"
fi
