#!/usr/bin/env bash
# The stubs are written as quoted heredocs and run on their own.
# shellcheck disable=SC2016
# Tests for scripts/deploy/load-release-images.sh, verify-release-images.sh and
# rollback-release-images.sh against stub git, docker (with two kind nodes'
# containerd behind `docker exec`), kind and kubectl, in a scratch HOME and
# TMPDIR. No registry, Docker daemon or cluster is touched.
#   - the refs are release.yml's: ghcr.io/econgraph/econ-graph-<name>:sha-<full
#     commit> and :<rc tag>, pulled by digest and loaded as the manifests'
#     econ-graph-<name>:<tag>, the tag read from k8s/manifests (here and in the
#     scripts; it is written nowhere else);
#   - every docker and kind call runs on the rootful daemon's DOCKER_HOST (or
#     ECONGRAPH_DOCKER_HOST), never the caller's; every kind call runs the
#     pinned kind, and load stops before changing anything without it;
#   - the crawler-migrate Job: its manifest must name the crawler-worker image
#     exactly as deployment/crawler-worker's does, or load and rollback refuse
#     before changing anything; the load and the rollback move the name the
#     Job runs on every node; verify checks the current Job's pods (by its
#     controller-uid, never an earlier run's) that started (succeeded, failed
#     or running) against the loaded image, fails on one that ran another
#     image or when the Job or its pods can't be read, records one whose
#     container never started or that is terminating as unchecked and lists it
#     in the summary, and reports no Job, or a Job with no pod, as unchecked
#     rather than failing;
#   - rollback refuses when the manifests now name another tag than the one
#     the load moved, and so does a second load of the same tag, before
#     anything changes; both read local= from v4.0-format records too, where
#     only the image= lines carry it;
#   - --cluster: an exported KIND_CLUSTER or ECONGRAPH_SCRATCH_STORAGE does not
#     stop load or rollback;
#   - --dry-run pulls, tags, loads, restarts and writes nothing;
#   - load refuses before anything reaches the nodes on: a revision label that
#     isn't the commit or is missing, a pulled image without the registry's
#     digest, an rc tag on another digest, an unreachable or rootless daemon,
#     a daemon on Docker's containerd image store (or whose store can't be
#     read), a pulled image whose ID is the registry digest,
#     an unknown tag, pods of one Deployment on different images, a pre-<tag>
#     copy of another image, a node without the image to keep (unless
#     --allow-missing-nodes, which exits 3), no node with it, or a node it
#     can't read;
#   - load keeps the images the pods run now (not just what the name points
#     at) as pre-<tag>, pinned and reported pinned, on every node, and records
#     them; a run after verify or after a partial load keeps the first record's
#     choice without warning; a copy that lost its pin is pinned again;
#   - load refuses when tagging or kind load leaves a node on another image,
#     and records status=partial; verify refuses a partial load;
#   - verify restarts the three Deployments, waits, passes when every running
#     pod runs the loaded image, skips terminating pods, and fails on a pod on
#     another image or a Deployment with no running pod;
#   - rollback points the names back at pre-<tag> on every node, restarts, and
#     checks the pods; it refuses on copies that differ between nodes or from
#     the record, on a node that has the name but no copy (unless
#     --allow-missing-nodes, which exits 3), and without copies; it fails on
#     a retag that doesn't take and on pods on another image, and records
#     status=partial when it stops half way;
#   - every kubectl call carries --context kind-econ-graph, also with another
#     CLUSTER_ROLE_CONTEXT inherited from the environment.
#
# Usage: scripts/deploy/test-release-images.sh (from anywhere)
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
load="$repo_root/scripts/deploy/load-release-images.sh"
verify="$repo_root/scripts/deploy/verify-release-images.sh"
rollback="$repo_root/scripts/deploy/rollback-release-images.sh"
failures=0
fail() {
  echo "FAIL $1"
  failures=$((failures + 1))
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin"
export STUB_LOG="$tmp/log" STUB_STATE="$tmp/state"
export STUB_COMMIT="d00f596618276a4bf6cd556be628a8d9064a2571"
TAG="v4.0.0-rc.3"
# The tag the manifests run the three images under, as the scripts read it.
STUB_LOCAL_TAG="$(grep -m1 -oE 'image: econ-graph-backend:[^[:space:]]+' "$repo_root/k8s/manifests/backend-deployment.yaml")"
export STUB_LOCAL_TAG="${STUB_LOCAL_TAG##*:}"
[ -n "$STUB_LOCAL_TAG" ] || { echo "no econ-graph-backend tag in k8s/manifests/backend-deployment.yaml"; exit 1; }
# The crawler-migrate Job's manifest, and the image it names.
job_manifest="$repo_root/k8s/manifests/crawler-migrate-job.yaml"
job_image="$(grep -m1 -oE 'image: econ-graph-crawler-worker:[^[:space:]]+' "$job_manifest")"
job_image="${job_image#image: }"
[ -n "$job_image" ] || { echo "no econ-graph-crawler-worker image in $job_manifest"; exit 1; }

# New images: digest a/b/c and image ID 1/2/3 for backend/crawler-worker/frontend.
# What production runs now: backend 9, crawler-worker 7 (under another tag; its
# local name points at 8), frontend 6.
cat >"$tmp/bin/stub-common" <<'STUB'
rep() { local s=""; for _ in $(seq 64); do s+="$1"; done; printf '%s' "$s"; }
image_name() { local r="${1##*/}"; r="${r%%[:@]*}"; printf '%s' "${r#econ-graph-}"; }
digest_of() { case "$1" in backend) echo "sha256:$(rep a)" ;; crawler-worker) echo "sha256:$(rep b)" ;; frontend) echo "sha256:$(rep c)" ;; esac; }
id_of() { case "$1" in backend) echo "sha256:$(rep 1)" ;; crawler-worker) echo "sha256:$(rep 2)" ;; frontend) echo "sha256:$(rep 3)" ;; esac; }
NODES="econ-graph-control-plane econ-graph-worker"
# A node's containerd: lines "ref id" in $STUB_STATE/node-<node>; labels as
# lines "node ref key=value" in $STUB_STATE/labels.
node_get() {
  local f="$STUB_STATE/node-$1"
  case "$2" in
    sha256:*) awk -v id="$2" '$2 == id { print id; exit }' "$f" ;;
    *) awk -v ref="$2" '$1 == ref { print $2; exit }' "$f" ;;
  esac
}
node_set() {
  local f="$STUB_STATE/node-$1"
  awk -v ref="$2" '$1 != ref' "$f" >"$f.new"; mv "$f.new" "$f"
  echo "$2 $3" >>"$f"
  grep -qxF "$3 $3" "$f" || echo "$3 $3" >>"$f"
}
node_del() {
  local f="$STUB_STATE/node-$1"
  awk -v ref="$2" '$1 != ref' "$f" >"$f.new"; mv "$f.new" "$f"
}
node_tags() { awk -v id="$2" '$2 == id && $1 !~ /^sha256:/ { printf "%s\"%s\"", (n++ ? "," : ""), $1 }' "$STUB_STATE/node-$1"; }
# The CRI reports an image pinned when any of its refs carries the pinned label.
node_pinned() {
  [ "${STUB_NO_PIN:-0}" != 1 ] || { echo false; return; }
  local ref
  for ref in $(awk -v id="$2" '$2 == id { print $1 }' "$STUB_STATE/node-$1"); do
    if grep -qxF "$1 $ref io.cri-containerd.pinned=pinned" "$STUB_STATE/labels" 2>/dev/null; then echo true; return; fi
  done
  echo false
}
STUB

cat >"$tmp/bin/git" <<'STUB'
#!/usr/bin/env bash
echo "git $*" >>"$STUB_LOG"
[ "$1" = -C ] && shift 2
case "$*" in
  "fetch --tags origin") ;;
  "rev-parse --verify --quiet refs/tags/v4.0.0-rc.3^{commit}") echo "$STUB_COMMIT" ;;
  "rev-parse "*) exit 1 ;;
  *) echo "stub git: unexpected call: $*" >&2; exit 1 ;;
esac
STUB

# docker:
#   STUB_DOCKER_DOWN=1                the daemon can't be reached
#   STUB_ROOTLESS=1                   the daemon is rootless
#   STUB_RC_DIGEST_DIFFERS=1          the rc tag points at another digest than the sha tag
#   STUB_BAD_REVISION=<name>          that image's revision label is another commit
#   STUB_NO_REVISION=<name>           that image has no revision label
#   STUB_PULLED_DIGEST_DIFFERS=<name> the pulled image doesn't carry the registry's digest
#   STUB_CRICTL_BROKEN=1              crictl in the nodes fails (not "no such image")
#   STUB_CTR_TAG_WRONG=1              ctr images tag points the new ref at another image
#   STUB_CTR_TAG_FAIL=<name>          ctr images tag of econ-graph-<name> refs fails
#   STUB_NO_PIN=1                     the CRI never reports an image pinned
#   STUB_CONTAINERD_STORE=1           the daemon keeps images in Docker's containerd image store
#   STUB_DRIVER_UNREADABLE=1          docker info can't report the image store
#   STUB_ID_IS_DIGEST=<name>          that pulled image's .Id is the registry digest (containerd store)
cat >"$tmp/bin/docker" <<'STUB'
#!/usr/bin/env bash
echo "docker DOCKER_HOST=${DOCKER_HOST:-} DOCKER_CONTEXT=${DOCKER_CONTEXT:-} $*" >>"$STUB_LOG"
source "$(dirname "$0")/stub-common"
case "$1" in
  info)
    [ "${STUB_DOCKER_DOWN:-0}" != 1 ] || { echo "Cannot connect to the Docker daemon" >&2; exit 1; }
    case "$*" in
      *SecurityOptions*) if [ "${STUB_ROOTLESS:-0}" = 1 ]; then echo '["name=seccomp","name=rootless"]'; else echo '["name=seccomp"]'; fi ;;
      *DriverStatus*)
        [ "${STUB_DRIVER_UNREADABLE:-0}" != 1 ] || { echo "template: error" >&2; exit 1; }
        if [ "${STUB_CONTAINERD_STORE:-0}" = 1 ]; then echo '[["driver-type","io.containerd.snapshotter.v1"]]'
        else echo '[["Backing Filesystem","extfs"],["Supports d_type","true"],["Using metacopy","false"],["Native Overlay Diff","true"],["userxattr","false"]]'; fi ;;
      *) echo 28.4.0 ;;
    esac ;;
  buildx)
    [ "$2 $3 $5 $6" = "imagetools inspect --format {{json .Manifest}}" ] || { echo "stub docker: unexpected: $*" >&2; exit 1; }
    ref="$4"; name="$(image_name "$ref")"; d="$(digest_of "$name")"
    case "$ref" in
      ghcr.io/econgraph/econ-graph-*:sha-"$STUB_COMMIT") ;;
      ghcr.io/econgraph/econ-graph-*:v4.0.0-rc.3) [ "${STUB_RC_DIGEST_DIFFERS:-0}" != 1 ] || d="sha256:$(rep d)" ;;
      *) echo "ERROR: $ref: not found" >&2; exit 1 ;;
    esac
    printf '{"size":856,"digest":"%s","mediaType":"application/vnd.oci.image.index.v1+json"}\n' "$d" ;;
  pull)
    name="$(image_name "$2")"
    [ "$2" = "ghcr.io/econgraph/econ-graph-$name@$(digest_of "$name")" ] || { echo "not found: $2" >&2; exit 1; }
    touch "$STUB_STATE/pulled-$name" ;;
  image)
    [ "$2 $3" = "inspect --format" ] || { echo "stub docker: unexpected: $*" >&2; exit 1; }
    fmt="$4"; ref="$5"; name="$(image_name "$ref")"
    case "$ref" in
      ghcr.io/*@sha256:*)
        [ -e "$STUB_STATE/pulled-$name" ] || { echo "No such image: $ref" >&2; exit 1; }
        digests="$ref"
        [ "${STUB_PULLED_DIGEST_DIFFERS:-}" != "$name" ] || digests="ghcr.io/econgraph/econ-graph-$name@sha256:$(rep e)"
        case "$fmt" in
          *RepoDigests*) echo "$digests" ;;
          *revision*)
            if [ "${STUB_BAD_REVISION:-}" = "$name" ]; then echo 89ebfa64e3da9efd7db5cade91fe69ae1d57179f
            elif [ "${STUB_NO_REVISION:-}" = "$name" ]; then echo
            else echo "$STUB_COMMIT"; fi ;;
          "{{.Id}}") if [ "${STUB_ID_IS_DIGEST:-}" = "$name" ]; then digest_of "$name"; else id_of "$name"; fi ;;
          *) exit 1 ;;
        esac ;;
      econ-graph-*:"$STUB_LOCAL_TAG")
        [ -e "$STUB_STATE/tagged-$name" ] || { echo "No such image: $ref" >&2; exit 1; }
        id_of "$name" ;;
      *) exit 1 ;;
    esac ;;
  tag)
    [ "$2" = "$(id_of "$(image_name "$3")")" ] || { echo "stub docker: tag of $2, not the pulled image ID" >&2; exit 1; }
    touch "$STUB_STATE/tagged-$(image_name "$3")" ;;
  exec)
    node="$2"; shift 2
    [ -f "$STUB_STATE/node-$node" ] || { echo "No such container: $node" >&2; exit 1; }
    case "$*" in
      "crictl inspecti -o json "*)
        [ "${STUB_CRICTL_BROKEN:-0}" != 1 ] || { echo 'FATA[0000] connect: connection refused' >&2; exit 1; }
        id="$(node_get "$node" "$5")"
        [ -n "$id" ] || { echo "FATA[0000] no such image \"$5\" present" >&2; exit 1; }
        printf '{"status":{"id":"%s","repoTags":[%s],"repoDigests":[],"pinned":%s}}\n' \
          "$id" "$(node_tags "$node" "$id")" "$(node_pinned "$node" "$id")" ;;
      "ctr -n k8s.io images tag "*)
        shift 5; force=0
        [ "$1" != --force ] || { force=1; shift; }
        [ -z "${STUB_CTR_TAG_FAIL:-}" ] || [ "$(image_name "$2")" != "${STUB_CTR_TAG_FAIL}" ] || { echo "ctr: failed" >&2; exit 1; }
        id="$(node_get "$node" "$1")"
        [ -n "$id" ] || { echo "ctr: image $1 not found" >&2; exit 1; }
        if [ -n "$(node_get "$node" "$2")" ] && [ "$force" = 0 ]; then echo "ctr: image $2 already exists" >&2; exit 1; fi
        [ "${STUB_CTR_TAG_WRONG:-0}" != 1 ] || id="sha256:$(rep f)"
        node_set "$node" "$2" "$id"
        # ctr copies the source's labels onto the new ref.
        awk -v node="$node" -v src="$1" -v dst="$2" '$1 == node && $2 == src { print node, dst, $3 }' \
          "$STUB_STATE/labels" 2>/dev/null >>"$STUB_STATE/labels.new" || true
        cat "$STUB_STATE/labels.new" >>"$STUB_STATE/labels" 2>/dev/null || true
        rm -f "$STUB_STATE/labels.new"
        echo "$2" ;;
      "ctr -n k8s.io images label "*)
        [ -n "$(node_get "$node" "$6")" ] || { echo "ctr: image $6 not found" >&2; exit 1; }
        echo "$node $6 $7" >>"$STUB_STATE/labels" ;;
      *) echo "stub docker exec: unexpected: $*" >&2; exit 1 ;;
    esac ;;
  *) echo "stub docker: unexpected call: $*" >&2; exit 1 ;;
esac
STUB

#   STUB_LOAD_FAILS=<name>     kind load of that image fails
#   STUB_KIND_LOAD_SKIPS=<node> kind load leaves that node as it was
cat >"$tmp/bin/kind" <<'STUB'
#!/usr/bin/env bash
echo "kind($0) DOCKER_HOST=${DOCKER_HOST:-} $*" >>"$STUB_LOG"
source "$(dirname "$0")/stub-common"
case "$*" in
  version) echo "kind v0.33.0 go1.25.3 linux/amd64" ;;
  "get nodes --name econ-graph") printf '%s\n' $NODES ;;
  "load docker-image econ-graph-"*":${STUB_LOCAL_TAG} --name econ-graph")
    name="$(image_name "$3")"
    [ "${STUB_LOAD_FAILS:-}" != "$name" ] || { echo "ERROR: failed to load image" >&2; exit 1; }
    for node in $NODES; do
      [ "${STUB_KIND_LOAD_SKIPS:-}" != "$node" ] || continue
      node_set "$node" "docker.io/library/$3" "$(id_of "$name")"
    done ;;
  *) echo "stub kind: unexpected call: $*" >&2; exit 1 ;;
esac
STUB

# kubectl: pods run $STUB_STATE/running-<deployment>; a restart switches them to
# what their node's local name points at, and leaves a terminating pod on the
# previous image.
#   STUB_POD_NODE=<node>                restarted pods land there (default econ-graph-worker)
#   STUB_WRONG_DEPLOYMENT=<deployment>  one new pod stays on the previous image
#   STUB_NO_PODS=<deployment>           its only pod is Pending
#   STUB_MIXED=<deployment>             before any restart, its pods run two images
#   STUB_JOB_LIST_FAILS=1               listing the crawler-migrate pods fails
#   STUB_JOB_GET_FAILS=1                reading job/crawler-migrate fails
# job/crawler-migrate exists when $STUB_STATE/job-uid does, with that uid. Its
# pods (batch.kubernetes.io/controller-uid=<uid>) are the lines of
# $STUB_STATE/job-pods (pod|phase|deletionTimestamp|imageID). Lines of
# $STUB_STATE/job-pods-stale are pods an earlier run of the Job left: they
# carry app=crawler-migrate but another controller-uid.
cat >"$tmp/bin/kubectl" <<'STUB'
#!/usr/bin/env bash
echo "kubectl $*" >>"$STUB_LOG"
source "$(dirname "$0")/stub-common"
args=("$@")
[ "${args[0]}" = --context ] && ctx="${args[1]}" && args=("${args[@]:2}")
[ "${args[0]}" = -n ] && args=("${args[@]:2}")
set -- "${args[@]}"
name_of() { case "$1" in econ-graph-backend) echo backend ;; crawler-worker) echo crawler-worker ;; econ-graph-frontend) echo frontend ;; esac; }
[ "${ctx:-}" = kind-econ-graph ] || { echo "error: context ${ctx:-(current)} not found" >&2; exit 1; }
case "$*" in
  "config view --minify -o jsonpath="*) echo "https://127.0.0.1:6443" ;;
  "get nodes -o jsonpath="*) echo "$NODES" ;;
  "get namespace kube-system -o jsonpath="*) echo "11111111-2222-3333-4444-555555555555" ;;
  "get configmap econgraph-cluster-role --ignore-not-found -o jsonpath="*) ;;
  "get job crawler-migrate --ignore-not-found -o jsonpath={.metadata.uid}")
    [ "${STUB_JOB_GET_FAILS:-0}" != 1 ] || { echo "error: the server is unavailable" >&2; exit 1; }
    [ ! -f "$STUB_STATE/job-uid" ] || cat "$STUB_STATE/job-uid" ;;
  "get pods -l batch.kubernetes.io/controller-uid="*" -o jsonpath="*)
    [ "${STUB_JOB_LIST_FAILS:-0}" != 1 ] || { echo "error: the server is unavailable" >&2; exit 1; }
    uid="${4#batch.kubernetes.io/controller-uid=}"
    if [ -f "$STUB_STATE/job-uid" ] && [ "$uid" = "$(cat "$STUB_STATE/job-uid")" ] && [ -f "$STUB_STATE/job-pods" ]; then
      cat "$STUB_STATE/job-pods"
    fi ;;
  "get pods -l app=crawler-migrate -o jsonpath="*)
    [ "${STUB_JOB_LIST_FAILS:-0}" != 1 ] || { echo "error: the server is unavailable" >&2; exit 1; }
    cat "$STUB_STATE/job-pods" "$STUB_STATE/job-pods-stale" 2>/dev/null || true ;;
  "rollout restart deployment/"*)
    d="${3#deployment/}"
    cp "$STUB_STATE/running-$d" "$STUB_STATE/previous-$d"
    node_get "${STUB_POD_NODE:-econ-graph-worker}" "docker.io/library/econ-graph-$(name_of "$d"):${STUB_LOCAL_TAG}" >"$STUB_STATE/running-$d"
    touch "$STUB_STATE/restarted-$d" ;;
  "rollout status deployment/"*" --timeout=600s") ;;
  "get pods -l app="*" -o jsonpath="*)
    d="${4#app=}"; id="$(cat "$STUB_STATE/running-$d")"
    if [ "${STUB_NO_PODS:-}" = "$d" ]; then echo "$d-p|Pending||"; exit 0; fi
    if [ -e "$STUB_STATE/restarted-$d" ]; then
      prev="$(cat "$STUB_STATE/previous-$d")"
      echo "$d-new1|Running||docker.io/library/econ-graph-$(name_of "$d")@$id"
      if [ "${STUB_WRONG_DEPLOYMENT:-}" = "$d" ]; then echo "$d-new2|Running||$prev"; else echo "$d-new2|Running||$id"; fi
      echo "$d-new3|Pending||"
      echo "$d-old|Running|2026-10-25T00:00:00Z|$prev"
    else
      echo "$d-a|Running||$id"
      if [ "${STUB_MIXED:-}" = "$d" ]; then echo "$d-b|Running||sha256:$(rep 5)"; else echo "$d-b|Running||$id"; fi
    fi ;;
  *) echo "stub kubectl: unexpected call: $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$tmp/bin/git" "$tmp/bin/docker" "$tmp/bin/kind" "$tmp/bin/kubectl"
# shellcheck source=/dev/null
source "$tmp/bin/stub-common"

# A scratch HOME and TMPDIR, and only the stubs for the four tools.
export PATH="$tmp/bin:$PATH"
unset KUBECTL KUBECONFIG CLUSTER_ROLE_CONTEXT XDG_CACHE_HOME XDG_DATA_HOME ECONGRAPH_RELEASES_DIR DOCKER_CONTEXT \
  ECONGRAPH_DOCKER_HOST ECONGRAPH_KUBE_DIR ECONGRAPH_SCRATCH_STORAGE KIND_CLUSTER NAMESPACE DRY_RUN
export DOCKER_HOST="unix:///run/user/1000/docker.sock"
# The stub kubectl, chosen explicitly: without KUBECTL the scripts take the kind
# cluster's copies from ~/.cache/econgraph and never plain kubectl from PATH.
export KUBECTL="$tmp/bin/kubectl"

old_backend="sha256:$(rep 9)"
old_crawler="sha256:$(rep 7)"
old_crawler_named="sha256:$(rep 8)"
old_frontend="sha256:$(rep 6)"
records="$tmp/home/econgraph-releases/$TAG"
kind_version="$(sed -nE 's/^KIND_VERSION="([^"]+)"$/\1/p' "$repo_root/scripts/deploy/lib/kind-version.sh")"
pinned_kind="$tmp/home/.local/share/econgraph/bin/kind-$kind_version"
names=(backend crawler-worker frontend)
nodes=(econ-graph-control-plane econ-graph-worker)

# Every call of every case also goes to $tmp/all-calls, for the context check at the end.
clear_log() {
  [ ! -f "$STUB_LOG" ] || cat "$STUB_LOG" >>"$tmp/all-calls"
  : >"$STUB_LOG"
}
reset() {
  clear_log
  rm -rf "${tmp:?}/home" "${tmp:?}/tmpdir" "${STUB_STATE:?}"
  mkdir -p "$tmp/home" "$tmp/tmpdir" "$STUB_STATE"
  for node in "${nodes[@]}"; do
    : >"$STUB_STATE/node-$node"
    node_set "$node" docker.io/library/econ-graph-backend:"${STUB_LOCAL_TAG}" "$old_backend"
    node_set "$node" docker.io/library/econ-graph-crawler-worker:"${STUB_LOCAL_TAG}" "$old_crawler_named"
    node_set "$node" docker.io/library/econ-graph-crawler-worker:v4.0.0-bea-fix-525c2132 "$old_crawler"
    node_set "$node" docker.io/library/econ-graph-frontend:"${STUB_LOCAL_TAG}" "$old_frontend"
  done
  : >"$STUB_STATE/labels"
  echo "$old_backend" >"$STUB_STATE/running-econ-graph-backend"
  echo "$old_crawler" >"$STUB_STATE/running-crawler-worker"
  echo "$old_frontend" >"$STUB_STATE/running-econ-graph-frontend"
  # The pinned kind, where install-kind.sh puts it under the scratch HOME.
  mkdir -p "$(dirname "$pinned_kind")"
  cp "$tmp/bin/kind" "$pinned_kind"
  cp "$tmp/bin/stub-common" "$(dirname "$pinned_kind")/"
}
# What deploy.sh leaves of the crawler-migrate Job: one pod that failed and one
# that succeeded, both on what the nodes' crawler-worker name points at now, and
# one whose container never started.
job_uid="0a1b2c3d-0000-4000-8000-000000000001"
job_exists() { echo "$job_uid" >"$STUB_STATE/job-uid"; }
job_ran() {
  local id
  id="$(node_get econ-graph-worker "$(local_ref crawler-worker)")"
  job_exists
  {
    echo "crawler-migrate-a|Failed||docker.io/library/econ-graph-crawler-worker@$id"
    echo "crawler-migrate-b|Succeeded||$id"
    echo "crawler-migrate-c|Pending||"
  } >"$STUB_STATE/job-pods"
}
# A copy of the scripts and k8s/manifests in $tmp/repo, for manifests changed by a case.
make_repo() {
  rm -rf "${tmp:?}/repo"
  mkdir -p "$tmp/repo/scripts/deploy" "$tmp/repo/k8s"
  cp -r "$repo_root/scripts/deploy/lib" "$tmp/repo/scripts/deploy/"
  cp "$load" "$verify" "$rollback" "$tmp/repo/scripts/deploy/"
  cp -r "$repo_root/k8s/manifests" "$tmp/repo/k8s/"
}
run() {
  HOME="$tmp/home" TMPDIR="$tmp/tmpdir" "$@" >"$tmp/out" 2>&1
}
pre() { echo "docker.io/library/econ-graph-$1:pre-$TAG"; }
local_ref() { echo "docker.io/library/econ-graph-$1:${STUB_LOCAL_TAG}"; }
node_changes() { grep -E 'load docker-image|ctr -n k8s.io images (tag|label)|rollout restart' "$STUB_LOG"; }
nothing_changed() { # $1: what for
  grep -qE '^docker .* (pull|tag) ' "$STUB_LOG" && fail "$1: pulled or tagged"
  node_changes >/dev/null && fail "$1: changed a node or restarted a Deployment"
  return 0
}
# Run $2... and expect a non-zero exit whose output contains $1.
expect_fail() {
  local want="$1"
  shift
  if run "$@"; then fail "ran despite: $want"; return 0; fi
  grep -qF -- "$want" "$tmp/out" || fail "did not say '$want': $(cat "$tmp/out")"
}
load_ok() { run "$load" --tag "$TAG" || fail "load failed: $(cat "$tmp/out")"; }

# === load --dry-run
reset
if ! run "$load" --tag "$TAG" --dry-run; then fail "load --dry-run exited non-zero: $(cat "$tmp/out")"; fi
nothing_changed "load --dry-run"
[ -e "$tmp/home/econgraph-releases" ] && fail "load --dry-run wrote a record"
for n in "${names[@]}"; do
  grep -qF "docker DOCKER_HOST=unix:///var/run/docker.sock DOCKER_CONTEXT= buildx imagetools inspect ghcr.io/econgraph/econ-graph-$n:sha-$STUB_COMMIT --format" "$STUB_LOG" ||
    fail "load --dry-run did not resolve ghcr.io/econgraph/econ-graph-$n:sha-$STUB_COMMIT"
  grep -qF "imagetools inspect ghcr.io/econgraph/econ-graph-$n:$TAG --format" "$STUB_LOG" ||
    fail "load --dry-run did not resolve ghcr.io/econgraph/econ-graph-$n:$TAG"
  grep -qF "econ-graph-$n:${STUB_LOCAL_TAG}" "$tmp/out" || fail "load --dry-run did not name econ-graph-$n:${STUB_LOCAL_TAG}"
done
grep -qF "keep $old_crawler as $(pre crawler-worker)" "$tmp/out" ||
  fail "load --dry-run would not keep the crawler-worker image that runs: $(cat "$tmp/out")"

# === load
reset
if ! run "$load" --tag "$TAG"; then fail "load exited non-zero: $(cat "$tmp/out")"; fi
grep -E '^(docker|kind)' "$STUB_LOG" | grep -vqF 'DOCKER_HOST=unix:///var/run/docker.sock' &&
  fail "a docker or kind call did not run on the rootful daemon"
grep -q '^warning' "$tmp/out" && fail "a clean load warned: $(grep '^warning' "$tmp/out")"
grep '^kind(' "$STUB_LOG" | grep -vqF "kind($pinned_kind)" && fail "a kind call did not run the pinned kind: $(grep '^kind(' "$STUB_LOG" | grep -vF "kind($pinned_kind)" | head -1)"
grep -qF "kind($pinned_kind) DOCKER_HOST=unix:///var/run/docker.sock load docker-image" "$STUB_LOG" || fail "load did not kind load with the pinned kind"
[ "$(grep -c '^docker .* pull ' "$STUB_LOG")" = 3 ] || fail "load did not pull exactly three images"
# The crawler-migrate Job's image is the name the load moved.
for node in "${nodes[@]}"; do
  [ "$(node_get "$node" "docker.io/library/$job_image")" = "$(id_of crawler-worker)" ] ||
    fail "$job_image, which the crawler-migrate Job runs, on $node is not the loaded crawler-worker image"
done
for n in "${names[@]}"; do
  grep -qxF "docker DOCKER_HOST=unix:///var/run/docker.sock DOCKER_CONTEXT= pull ghcr.io/econgraph/econ-graph-$n@$(digest_of "$n")" "$STUB_LOG" ||
    fail "load did not pull ghcr.io/econgraph/econ-graph-$n by digest"
  grep -qF "tag $(id_of "$n") econ-graph-$n:${STUB_LOCAL_TAG}" "$STUB_LOG" || fail "load did not tag econ-graph-$n:${STUB_LOCAL_TAG} by image ID"
  grep -qF "load docker-image econ-graph-$n:${STUB_LOCAL_TAG} --name econ-graph" "$STUB_LOG" ||
    fail "load did not kind load econ-graph-$n:${STUB_LOCAL_TAG}"
  for node in "${nodes[@]}"; do
    [ "$(node_get "$node" "$(local_ref "$n")")" = "$(id_of "$n")" ] || fail "econ-graph-$n:${STUB_LOCAL_TAG} on $node is not the loaded image"
    grep -qxF "$node $(pre "$n") io.cri-containerd.pinned=pinned" "$STUB_STATE/labels" || fail "$(pre "$n") is not pinned on $node"
  done
done
for node in "${nodes[@]}"; do
  [ "$(node_get "$node" "$(pre backend)")" = "$old_backend" ] || fail "$(pre backend) on $node is not the old backend"
  [ "$(node_get "$node" "$(pre crawler-worker)")" = "$old_crawler" ] ||
    fail "$(pre crawler-worker) on $node is not the image the crawler-worker pods ran"
  [ "$(node_get "$node" "$(pre frontend)")" = "$old_frontend" ] || fail "$(pre frontend) on $node is not the old frontend"
done
# Pulls and checks, then the rollback copies, then the loads.
last_pull="$(grep -n ' pull ' "$STUB_LOG" | tail -1 | cut -d: -f1)"
first_pin="$(grep -n 'ctr -n k8s.io images tag' "$STUB_LOG" | head -1 | cut -d: -f1)"
last_pin="$(grep -n 'ctr -n k8s.io images label' "$STUB_LOG" | tail -1 | cut -d: -f1)"
first_load="$(grep -nE '^docker .* tag sha256|load docker-image' "$STUB_LOG" | head -1 | cut -d: -f1)"
[ "$last_pull" -lt "$first_pin" ] || fail "load changed a node before every image was pulled and checked"
[ "$last_pin" -lt "$first_load" ] || fail "load loaded an image before keeping every running one"
r="$records/images.txt"
grep -qx 'status=complete' "$r" || fail "the record is not status=complete"
grep -qxF "commit=$STUB_COMMIT" "$r" || fail "the record has no commit"
grep -qxF "tag=$TAG" "$r" || fail "the record has no tag"
grep -qE '^loaded_at=[0-9]{4}-[0-9]{2}-[0-9]{2}T' "$r" || fail "the record has no time"
grep -qF "before=crawler-worker local=econ-graph-crawler-worker:${STUB_LOCAL_TAG} running=$old_crawler keep=$old_crawler econ-graph-control-plane=$old_crawler_named econ-graph-worker=$old_crawler_named" "$r" ||
  fail "the record does not say what crawler-worker ran before: $(cat "$r")"
grep -qF "pinned=backend node=econ-graph-worker ref=$(pre backend) id=$old_backend create" "$r" || fail "the record has no rollback copy"
grep -qF "image=crawler-worker ref=ghcr.io/econgraph/econ-graph-crawler-worker@$(digest_of crawler-worker) digest=$(digest_of crawler-worker) id=$(id_of crawler-worker)" "$r" ||
  fail "the record has no crawler-worker image"

# A second load moves nothing, keeps pre-<tag>, adds a block, and re-pins a copy that lost its pin.
clear_log
: >"$STUB_STATE/labels"
if ! run "$load" --tag "$TAG"; then fail "second load exited non-zero: $(cat "$tmp/out")"; fi
grep -qE '^docker .* tag |ctr -n k8s.io images tag' "$STUB_LOG" && fail "second load retagged something"
[ "$(grep -c '^loaded_at=' "$r")" = 2 ] || fail "second load did not append a second record block"
grep -qF "pinned=backend node=econ-graph-worker ref=$(pre backend) id=$old_backend kept" "$r" || fail "second load did not record the kept copy"
for node in "${nodes[@]}"; do
  grep -qxF "$node $(pre frontend) io.cri-containerd.pinned=pinned" "$STUB_STATE/labels" || fail "second load did not pin the kept copy again on $node"
done

# === verify, then load again: the first record's choice, no warning
clear_log
job_ran
if ! run "$verify" --tag "$TAG"; then fail "verify failed on matching pods: $(cat "$tmp/out")"; fi
grep -qF "ok job/crawler-migrate crawler-migrate-a Failed docker.io/library/econ-graph-crawler-worker@$(id_of crawler-worker)" "$tmp/out" ||
  fail "verify did not check the failed crawler-migrate pod: $(cat "$tmp/out")"
grep -qF "ok job/crawler-migrate crawler-migrate-b Succeeded $(id_of crawler-worker)" "$records/verified.txt" ||
  fail "verify did not record the succeeded crawler-migrate pod"
grep -qF "unchecked job/crawler-migrate crawler-migrate-c Pending: its container never started" "$records/verified.txt" ||
  fail "verify did not record the crawler-migrate pod that never started as unchecked: $(cat "$records/verified.txt")"
sed -n '/^Not checked:$/,$p' "$tmp/out" | grep -qF "unchecked job/crawler-migrate crawler-migrate-c Pending: its container never started" ||
  fail "verify's summary did not list the crawler-migrate pod that never started: $(cat "$tmp/out")"
grep -q 'rollout restart.*crawler-migrate\|delete job' "$STUB_LOG" && fail "verify restarted or deleted the crawler-migrate Job"
for d in econ-graph-backend crawler-worker econ-graph-frontend; do
  grep -qxF "kubectl --context kind-econ-graph -n econ-graph rollout restart deployment/$d" "$STUB_LOG" || fail "verify did not restart deployment/$d"
  grep -qxF "kubectl --context kind-econ-graph -n econ-graph rollout status deployment/$d --timeout=600s" "$STUB_LOG" ||
    fail "verify did not wait for deployment/$d"
done
last_restart="$(grep -n 'rollout restart' "$STUB_LOG" | tail -1 | cut -d: -f1)"
first_get="$(grep -n 'get pods' "$STUB_LOG" | head -1 | cut -d: -f1)"
[ "$last_restart" -lt "$first_get" ] || fail "verify checked pods before restarting"
grep -q 'old: terminating, not checked' "$tmp/out" || fail "verify did not skip the terminating pod"
grep -qx 'result=ok' "$records/verified.txt" || fail "verify did not record its result"
clear_log
if ! run "$load" --tag "$TAG"; then fail "load after verify failed: $(cat "$tmp/out")"; fi
grep -qi 'warning' "$tmp/out" && fail "load after verify warned: $(cat "$tmp/out")"
grep -q 'ctr -n k8s.io images tag' "$STUB_LOG" && fail "load after verify retagged a copy"
grep -qF "before=backend local=econ-graph-backend:${STUB_LOCAL_TAG} running=$(id_of backend) keep=$old_backend" "$r" || fail "load after verify did not keep the first record's choice"

# === verify --dry-run
reset
load_ok
clear_log
if run "$verify" --tag "$TAG" --dry-run; then fail "verify --dry-run passed on pods still on the old image"; fi
grep -q 'rollout restart' "$STUB_LOG" && fail "verify --dry-run restarted a Deployment"
[ -e "$records/verified.txt" ] && fail "verify --dry-run wrote a record"
grep -q 'MISMATCH econ-graph-backend' "$tmp/out" || fail "verify --dry-run did not report the old backend pod"

# === verify and the crawler-migrate Job
# No Job (skipped, or deleted a day after it finished): reported, not a failure.
reset
load_ok
clear_log
if ! run "$verify" --tag "$TAG"; then fail "verify failed with no crawler-migrate Job: $(cat "$tmp/out")"; fi
grep -qF "unchecked job/crawler-migrate: no Job" "$tmp/out" || fail "verify did not say the Job went unchecked: $(cat "$tmp/out")"
grep -qF "unchecked job/crawler-migrate: no Job" "$records/verified.txt" || fail "verify did not record the unchecked Job"
grep -q 'get pods -l batch.kubernetes.io/controller-uid=' "$STUB_LOG" && fail "verify listed Job pods without a Job"
# A Job with no pod: reported, not a failure; the pods are read by the Job's controller-uid.
reset
load_ok
job_exists
clear_log
if ! run "$verify" --tag "$TAG"; then fail "verify failed with a crawler-migrate Job without pods: $(cat "$tmp/out")"; fi
grep -qF "unchecked job/crawler-migrate: the Job (uid $job_uid) has no pod" "$records/verified.txt" ||
  fail "verify did not record the Job without pods as unchecked: $(cat "$tmp/out")"
grep -qxF "kubectl --context kind-econ-graph -n econ-graph get pods -l batch.kubernetes.io/controller-uid=$job_uid -o jsonpath={range .items[*]}{.metadata.name}{\"|\"}{.status.phase}{\"|\"}{.metadata.deletionTimestamp}{\"|\"}{range .status.containerStatuses[?(@.name==\"crawler-migrate\")]}{.imageID}{end}{\"\\n\"}{end}" "$STUB_LOG" ||
  fail "verify did not read the crawler-migrate container's imageID by the Job's controller-uid: $(grep crawler-migrate "$STUB_LOG")"
# A Job pod that ran the image production ran before (deploy.sh not run after the load): fails.
reset
load_ok
job_exists
echo "crawler-migrate-x|Succeeded||docker.io/library/econ-graph-crawler-worker@$old_crawler" >"$STUB_STATE/job-pods"
expect_fail "MISMATCH job/crawler-migrate crawler-migrate-x Succeeded ran" "$verify" --tag "$TAG"
grep -qx 'result=FAILED' "$records/verified.txt" || fail "verify did not record the crawler-migrate pod on another image"
# A pod an earlier run of the Job left (same app label, another controller-uid) is not this run's.
reset
load_ok
job_ran
echo "crawler-migrate-stale|Failed||docker.io/library/econ-graph-crawler-worker@$old_crawler" >"$STUB_STATE/job-pods-stale"
run "$verify" --tag "$TAG" || fail "verify checked a pod of an earlier crawler-migrate Job: $(cat "$tmp/out")"
grep -q 'crawler-migrate-stale' "$tmp/out" && fail "verify listed a pod of an earlier crawler-migrate Job"
# A terminating Job pod on another image is recorded as unchecked, and shown in the summary.
reset
load_ok
job_ran
echo "crawler-migrate-old|Succeeded|2026-10-25T00:00:00Z|$old_crawler" >>"$STUB_STATE/job-pods"
run "$verify" --tag "$TAG" || fail "verify failed on a terminating crawler-migrate pod: $(cat "$tmp/out")"
grep -qF "unchecked job/crawler-migrate crawler-migrate-old Succeeded: terminating" "$records/verified.txt" ||
  fail "verify did not record the terminating crawler-migrate pod as unchecked"
sed -n '/^Not checked:$/,$p' "$tmp/out" | grep -qF "unchecked job/crawler-migrate crawler-migrate-old Succeeded: terminating" ||
  fail "verify's summary did not list the terminating crawler-migrate pod: $(cat "$tmp/out")"
# Only pods that never started (ErrImageNeverPull): passes, but each is recorded and shown.
reset
load_ok
job_exists
printf '%s\n' "crawler-migrate-p|Pending||" "crawler-migrate-q|Pending||" >"$STUB_STATE/job-pods"
run "$verify" --tag "$TAG" || fail "verify failed on crawler-migrate pods that never started: $(cat "$tmp/out")"
[ "$(grep -c '^unchecked job/crawler-migrate crawler-migrate-[pq] Pending: its container never started$' "$records/verified.txt")" = 2 ] ||
  fail "verify did not record both crawler-migrate pods that never started: $(cat "$records/verified.txt")"
[ "$(sed -n '/^Not checked:$/,$p' "$tmp/out" | grep -c 'crawler-migrate-[pq] Pending: its container never started')" = 2 ] ||
  fail "verify's summary did not list both crawler-migrate pods that never started: $(cat "$tmp/out")"
# The unchecked pods are shown when another check fails, too.
reset
load_ok
job_ran
expect_fail "MISMATCH crawler-worker crawler-worker-new2" env STUB_WRONG_DEPLOYMENT=crawler-worker "$verify" --tag "$TAG"
sed -n '/^Not checked:$/,$p' "$tmp/out" | grep -qF "crawler-migrate-c Pending: its container never started" ||
  fail "a failing verify did not list what it left unchecked: $(cat "$tmp/out")"
# The Job, or its pods, can't be read: fails.
reset
load_ok
job_ran
expect_fail "MISMATCH job/crawler-migrate: could not list its pods" env STUB_JOB_LIST_FAILS=1 "$verify" --tag "$TAG"
reset
load_ok
job_ran
expect_fail "MISMATCH job/crawler-migrate: could not read the Job" env STUB_JOB_GET_FAILS=1 "$verify" --tag "$TAG"

# === the pinned kind
# Without it, load stops before anything changes; the kind on PATH is never used to load.
reset
rm -f "$pinned_kind"
clear_log
expect_fail "install-kind.sh" "$load" --tag "$TAG"
nothing_changed "load without the pinned kind"
grep -q "load docker-image" "$STUB_LOG" && fail "load ran kind load without the pinned kind"
[ -e "$records" ] && fail "load without the pinned kind wrote a record"

# === the daemon: ECONGRAPH_DOCKER_HOST, never the caller's DOCKER_HOST
reset
clear_log
run env ECONGRAPH_DOCKER_HOST=unix:///home/ops/other.sock "$load" --tag "$TAG" --dry-run ||
  fail "load --dry-run with ECONGRAPH_DOCKER_HOST failed: $(cat "$tmp/out")"
grep -E '^(docker|kind)' "$STUB_LOG" | grep -vqF 'DOCKER_HOST=unix:///home/ops/other.sock' &&
  fail "a docker or kind call did not follow ECONGRAPH_DOCKER_HOST"
grep -qF 'DOCKER_HOST=unix:///run/user/1000/docker.sock' "$STUB_LOG" && fail "a call followed the caller's DOCKER_HOST"

# === an inherited CLUSTER_ROLE_CONTEXT is not followed
reset
clear_log
run env CLUSTER_ROLE_CONTEXT=kind-other "$load" --tag "$TAG" --dry-run ||
  fail "load --dry-run with an inherited CLUSTER_ROLE_CONTEXT failed: $(cat "$tmp/out")"
grep '^kubectl' "$STUB_LOG" | grep -vq '^kubectl --context kind-econ-graph ' && fail "load followed an inherited CLUSTER_ROLE_CONTEXT"
grep -qF "context         kind-econ-graph" "$tmp/out" || fail "load did not show the cluster's identity: $(cat "$tmp/out")"

# === manifests that name the crawler-worker image differently for the Job
# The load moves one name per image; a Job on another name would be left on another image.
make_repo
sed -i -E "s/image: econ-graph-crawler-worker:[^[:space:]]+/image: econ-graph-crawler-worker:${STUB_LOCAL_TAG}-other/" "$tmp/repo/k8s/manifests/crawler-migrate-job.yaml"
reset
clear_log
expect_fail "name econ-graph-crawler-worker with different tags" "$tmp/repo/scripts/deploy/load-release-images.sh" --tag "$TAG" --dry-run
grep -qF "k8s/manifests/crawler-migrate-job.yaml" "$tmp/out" || fail "load did not name the Job's manifest: $(cat "$tmp/out")"
expect_fail "name econ-graph-crawler-worker with different tags" "$tmp/repo/scripts/deploy/load-release-images.sh" --tag "$TAG"
nothing_changed "load with a Job on another tag"
[ -e "$records" ] && fail "load with a Job on another tag wrote a record"
reset
load_ok
clear_log
expect_fail "name econ-graph-crawler-worker with different tags" "$tmp/repo/scripts/deploy/rollback-release-images.sh" --to "pre-$TAG"
node_changes >/dev/null && fail "rollback changed something with a Job on another tag"
[ -e "$records/rollback.txt" ] && fail "rollback with a Job on another tag wrote a record"
# The Deployment's own manifest no longer runs the image.
make_repo
sed -i -E "/image: econ-graph-crawler-worker:/d" "$tmp/repo/k8s/manifests/crawler-worker.yaml"
reset
expect_fail "k8s/manifests/crawler-worker.yaml does not run econ-graph-crawler-worker:${STUB_LOCAL_TAG}" "$tmp/repo/scripts/deploy/load-release-images.sh" --tag "$TAG" --dry-run

# === rollback after the manifests moved to another tag: pointing the old name back would change nothing
make_repo
sed -i -E "s/image: econ-graph-backend:[^[:space:]]+/image: econ-graph-backend:${STUB_LOCAL_TAG}-next/" "$tmp/repo/k8s/manifests/backend-deployment.yaml"
reset
load_ok
clear_log
expect_fail "but the manifests now run econ-graph-backend:${STUB_LOCAL_TAG}-next" "$tmp/repo/scripts/deploy/rollback-release-images.sh" --to "pre-$TAG"
node_changes >/dev/null && fail "rollback changed something after the manifests moved"
[ -e "$records/rollback.txt" ] && fail "rollback after the manifests moved wrote a record"

# === a second load of the tag after the manifests moved to another tag: refused in step 3,
# before anything changes (it would move a second name, and rollback would then refuse).
moved_load="$tmp/repo/scripts/deploy/load-release-images.sh"
moved_rollback="$tmp/repo/scripts/deploy/rollback-release-images.sh"
reset
load_ok
clear_log
expect_fail "moved econ-graph-backend:${STUB_LOCAL_TAG} for backend, but the manifests now run econ-graph-backend:${STUB_LOCAL_TAG}-next" "$moved_load" --tag "$TAG"
nothing_changed "a load of the tag after the manifests moved"
[ "$(grep -c '^loaded_at=' "$records/images.txt")" = 1 ] || fail "a refused load after the manifests moved appended a record"
for node in "${nodes[@]}"; do
  [ -z "$(node_get "$node" "docker.io/library/econ-graph-backend:${STUB_LOCAL_TAG}-next")" ] ||
    fail "a refused load after the manifests moved put the new name on $node"
done
expect_fail "but the manifests now run econ-graph-backend:${STUB_LOCAL_TAG}-next" "$moved_load" --tag "$TAG" --dry-run

# === a load record in the v4.0 format: local= only on the image= lines, not on before=
legacy_record() {
  sed -i -E '/^before=/ s/ local=[^ ]*//' "$records/images.txt"
  grep -q '^before=.* local=' "$records/images.txt" && fail "the legacy record still has local= on a before= line"
  grep -q '^image=backend .* local=' "$records/images.txt" || fail "the legacy record has no local= on its image= lines"
}
reset
load_ok
legacy_record
clear_log
run "$rollback" --to "pre-$TAG" --dry-run || fail "rollback --dry-run with a v4.0 record failed: $(cat "$tmp/out")"
grep -qF "$(pre backend) = $old_backend -> $(local_ref backend)" "$tmp/out" || fail "rollback with a v4.0 record did not plan the backend: $(cat "$tmp/out")"
run "$load" --tag "$TAG" --dry-run || fail "load --dry-run of a tag with a v4.0 record failed: $(cat "$tmp/out")"
clear_log
expect_fail "the load of ${TAG} moved econ-graph-backend:${STUB_LOCAL_TAG}, but the manifests now run econ-graph-backend:${STUB_LOCAL_TAG}-next" "$moved_rollback" --to "pre-$TAG"
node_changes >/dev/null && fail "rollback changed something with a v4.0 record after the manifests moved"
[ -e "$records/rollback.txt" ] && fail "rollback with a v4.0 record after the manifests moved wrote a record"
expect_fail "but the manifests now run econ-graph-backend:${STUB_LOCAL_TAG}-next" "$moved_load" --tag "$TAG"
nothing_changed "a load with a v4.0 record after the manifests moved"

# === --cluster: an exported KIND_CLUSTER or ECONGRAPH_SCRATCH_STORAGE (for the host-storage
# scripts) does not stop load or rollback; without --cluster, lib/kind-env.sh's check stands.
reset
run env KIND_CLUSTER=econ-graph-scratch "$load" --tag "$TAG" --cluster econ-graph --dry-run ||
  fail "load --cluster stopped on an exported KIND_CLUSTER: $(cat "$tmp/out")"
run env ECONGRAPH_SCRATCH_STORAGE="$tmp/scratch-storage" "$load" --tag "$TAG" --cluster econ-graph --dry-run ||
  fail "load --cluster stopped on an exported ECONGRAPH_SCRATCH_STORAGE: $(cat "$tmp/out")"
expect_fail "needs ECONGRAPH_SCRATCH_STORAGE" env KIND_CLUSTER=econ-graph-scratch "$load" --tag "$TAG" --dry-run
load_ok
run env KIND_CLUSTER=econ-graph-scratch "$rollback" --to "pre-$TAG" --cluster econ-graph --dry-run ||
  fail "rollback --cluster stopped on an exported KIND_CLUSTER: $(cat "$tmp/out")"
run env ECONGRAPH_SCRATCH_STORAGE="$tmp/scratch-storage" "$rollback" --to "pre-$TAG" --cluster econ-graph --dry-run ||
  fail "rollback --cluster stopped on an exported ECONGRAPH_SCRATCH_STORAGE: $(cat "$tmp/out")"
expect_fail "needs ECONGRAPH_SCRATCH_STORAGE" env KIND_CLUSTER=econ-graph-scratch "$rollback" --to "pre-$TAG" --dry-run

# === load refusals: nothing reaches the nodes, nothing is recorded
refuses() {
  local what="$1" want="$2"
  shift 2
  reset
  [ -z "${SETUP:-}" ] || "$SETUP"
  clear_log
  if run env "$@" "$load" --tag "${REFUSE_TAG:-$TAG}"; then fail "load ran despite $what"; fi
  grep -qF -- "$want" "$tmp/out" || fail "load did not say why ($what): $(cat "$tmp/out")"
  grep -qE '^docker .* tag ' "$STUB_LOG" && fail "load tagged an image despite $what"
  node_changes >/dev/null && fail "load changed the nodes despite $what"
  [ -e "$records" ] && fail "load wrote a record despite $what"
  return 0
}
refuses "a revision mismatch" "has org.opencontainers.image.revision 89ebfa64e3da9efd7db5cade91fe69ae1d57179f, not $STUB_COMMIT" STUB_BAD_REVISION=crawler-worker
refuses "a missing revision label" "has no org.opencontainers.image.revision label" STUB_NO_REVISION=frontend
refuses "a pulled image without the registry's digest" "does not carry digest $(digest_of backend)" STUB_PULLED_DIGEST_DIFFERS=backend
refuses "an rc tag on another digest" "refusing" STUB_RC_DIGEST_DIFFERS=1
refuses "an unreachable daemon" "is not reachable" STUB_DOCKER_DOWN=1
refuses "a rootless daemon" "is a rootless daemon" STUB_ROOTLESS=1
# Docker's containerd image store: refused before anything is pulled, and in a dry run.
refuses "the containerd image store" "keeps images in Docker's containerd image store" STUB_CONTAINERD_STORE=1
grep -qE '^docker .* pull ' "$STUB_LOG" && fail "load pulled despite the containerd image store"
reset
expect_fail "keeps images in Docker's containerd image store" env STUB_CONTAINERD_STORE=1 "$load" --tag "$TAG" --dry-run
refuses "an image store it can't read" "could not read the image store" STUB_DRIVER_UNREADABLE=1
# A pulled image whose ID is the registry digest (what the containerd store reports):
# refused after the pulls, before any node changes.
refuses "an image ID that is the registry digest" "the registry digest rather than the config digest" STUB_ID_IS_DIGEST=frontend
refuses "pods on different images" "use different images" STUB_MIXED=econ-graph-backend
refuses "a node it can't read" "could not read image" STUB_CRICTL_BROKEN=1
REFUSE_TAG=v4.0.0-rc.9 refuses "an unknown tag" "tag v4.0.0-rc.9 not found"
REFUSE_TAG=v4.0.0 refuses "a final tag" "is not a release candidate tag"
grep -q ' pull ' "$STUB_LOG" && fail "load pulled despite a non-rc tag"

stale_copy() { for node in "${nodes[@]}"; do node_set "$node" "$(pre backend)" "sha256:$(rep 4)"; done; }
SETUP=stale_copy refuses "a stale pre-<tag> copy" "already has $(pre backend) as sha256:$(rep 4)"
# Remove an image (every ref to it) from a node.
node_rmi() { awk -v id="$2" '$2 != id' "$STUB_STATE/node-$1" >"$STUB_STATE/node-$1.new"; mv "$STUB_STATE/node-$1.new" "$STUB_STATE/node-$1"; }
lacking_node() { node_rmi econ-graph-control-plane "$old_crawler"; }
SETUP=lacking_node refuses "a node without the image to keep" "econ-graph-control-plane:crawler-worker"
nowhere() { for node in "${nodes[@]}"; do node_rmi "$node" "$old_crawler"; done; }
SETUP=nowhere refuses "no node with the image to keep" "no node has $old_crawler"

# --allow-missing-nodes loads, records, and exits 3.
reset
lacking_node
run "$load" --tag "$TAG" --allow-missing-nodes && fail "load --allow-missing-nodes exited 0"
[ "$(HOME="$tmp/home" TMPDIR="$tmp/tmpdir" "$load" --tag "$TAG" --allow-missing-nodes >/dev/null 2>&1; echo $?)" = 3 ] ||
  fail "load --allow-missing-nodes did not exit 3"
grep -qF "pinned=crawler-worker node=econ-graph-control-plane ref=$(pre crawler-worker) id=$old_crawler missing" "$records/images.txt" ||
  fail "load --allow-missing-nodes did not record the missing copy"
grep -qx 'status=complete' "$records/images.txt" || fail "load --allow-missing-nodes did not complete"

# === load failures after the nodes start changing: status=partial
partial() { # $1: what, $2: expected text, rest: env
  local what="$1" want="$2"
  shift 2
  reset
  expect_fail "$want" env "$@" "$load" --tag "$TAG"
  grep -q '^status=partial (exit 1 during: ' "$records/images.txt" 2>/dev/null ||
    fail "$what did not record status=partial: $(cat "$records/images.txt" 2>/dev/null)"
}
partial "a tag that points at another image" "is 'sha256:$(rep f)', not $old_backend, after tagging it" STUB_CTR_TAG_WRONG=1
partial "a copy the CRI does not report pinned" "does not report it pinned" STUB_NO_PIN=1
partial "kind load skipping a node" "after kind load, econ-graph-backend:${STUB_LOCAL_TAG} on econ-graph-control-plane is '$old_backend'" STUB_KIND_LOAD_SKIPS=econ-graph-control-plane
partial "a failing kind load" "failed to load image" STUB_LOAD_FAILS=crawler-worker
grep -qx 'status=partial (exit 1 during: loading crawler-worker)' "$records/images.txt" || fail "the partial record does not name the step"
grep -qF "image=backend " "$records/images.txt" || fail "the partial record does not say backend was loaded"
[ "$(node_get econ-graph-worker "$(pre crawler-worker)")" = "$old_crawler" ] || fail "a partial load lost the rollback copy"
expect_fail "not complete" "$verify" --tag "$TAG"
grep -q 'rollout restart' "$STUB_LOG" && fail "verify restarted after a partial load"
# A pod restarted onto the new backend, then a second run: it keeps the first record's copy.
clear_log
kubectl --context kind-econ-graph -n econ-graph rollout restart deployment/econ-graph-backend
if ! run "$load" --tag "$TAG"; then fail "load after a partial load failed: $(cat "$tmp/out")"; fi
grep -qi 'warning' "$tmp/out" && fail "load after a partial load warned: $(cat "$tmp/out")"
[ "$(node_get econ-graph-worker "$(pre backend)")" = "$old_backend" ] || fail "load after a partial load moved the backend copy"

# === verify failures
reset
load_ok
expect_fail "MISMATCH crawler-worker crawler-worker-new2" env STUB_WRONG_DEPLOYMENT=crawler-worker "$verify" --tag "$TAG"
grep -qx 'result=FAILED' "$records/verified.txt" || fail "verify did not record the failure"
reset
load_ok
expect_fail "MISMATCH econ-graph-frontend: no running pod" env STUB_NO_PODS=econ-graph-frontend "$verify" --tag "$TAG"
expect_fail "no load record" "$verify" --tag v4.0.0-rc.2

# === rollback, after the release candidate runs
reset
load_ok
run "$verify" --tag "$TAG" || fail "verify before rollback failed: $(cat "$tmp/out")"
clear_log
if ! run "$rollback" --to "pre-$TAG" --dry-run; then fail "rollback --dry-run exited non-zero: $(cat "$tmp/out")"; fi
nothing_changed "rollback --dry-run"
[ -e "$records/rollback.txt" ] && fail "rollback --dry-run wrote a record"

clear_log
if ! run "$rollback" --to "pre-$TAG"; then fail "rollback failed: $(cat "$tmp/out")"; fi
for node in "${nodes[@]}"; do
  for n in "${names[@]}"; do
    grep -qF "exec $node ctr -n k8s.io images tag --force $(pre "$n") $(local_ref "$n")" "$STUB_LOG" ||
      fail "rollback did not retag econ-graph-$n:${STUB_LOCAL_TAG} on $node"
    grep -qxF "$node $(local_ref "$n") io.cri-containerd.pinned=pinned" "$STUB_STATE/labels" ||
      fail "the pin did not carry over to econ-graph-$n:${STUB_LOCAL_TAG} on $node"
  done
  [ "$(node_get "$node" "$(local_ref crawler-worker)")" = "$old_crawler" ] ||
    fail "rollback did not point econ-graph-crawler-worker:${STUB_LOCAL_TAG} on $node at the image it ran"
  # The crawler-migrate Job's image name is the Deployment's: the retag covers it.
  [ "$(node_get "$node" "docker.io/library/$job_image")" = "$old_crawler" ] ||
    fail "rollback left $job_image, which the crawler-migrate Job runs, on $node on another image than deployment/crawler-worker"
done
grep -q 'crawler-migrate' "$STUB_LOG" && fail "rollback touched the crawler-migrate Job: $(grep crawler-migrate "$STUB_LOG" | head -1)"
grep -qF "ok crawler-worker crawler-worker-new1 docker.io/library/econ-graph-crawler-worker@$old_crawler" "$tmp/out" ||
  fail "rollback did not check the crawler-worker pods: $(cat "$tmp/out")"
grep -qx 'result=ok' "$records/rollback.txt" || fail "rollback did not record its result"
grep -qx 'status=complete' "$records/rollback.txt" || fail "rollback did not record status=complete"
clear_log
run "$rollback" --to "pre-$TAG" || fail "second rollback failed: $(cat "$tmp/out")"
grep -q 'ctr -n k8s.io images tag' "$STUB_LOG" && fail "second rollback retagged"

# Only the first load record says what ran before: a later block that names
# another image does not change what rollback restores.
reset
load_ok
printf '\nloaded_at=2026-10-26T00:00:00Z\nstatus=complete\ntag=%s\nbefore=backend local=econ-graph-backend:%s running=sha256:%s keep=sha256:%s\n' \
  "$TAG" "$STUB_LOCAL_TAG" "$(rep 4)" "$(rep 4)" >>"$records/images.txt"
run "$rollback" --to "pre-$TAG" --dry-run || fail "rollback followed a later record instead of the first: $(cat "$tmp/out")"
grep -qF "$(pre backend) = $old_backend" "$tmp/out" || fail "rollback did not check the copy against the first record"

# Rollback refusals: nothing changes.
rollback_refuses() { # $1: what, $2: setup function, $3: expected text, rest: env
  local what="$1" setup="$2" want="$3"
  shift 3
  reset
  load_ok
  "$setup"
  clear_log
  expect_fail "$want" env "$@" "$rollback" --to "pre-$TAG"
  node_changes >/dev/null && fail "rollback changed something despite $what"
  [ -e "$records/rollback.txt" ] && fail "rollback wrote a record despite $what"
  return 0
}
noop() { :; }
copies_differ() { node_set econ-graph-control-plane "$(pre backend)" "sha256:$(rep 4)"; }
copies_stale() { for node in "${nodes[@]}"; do node_set "$node" "$(pre frontend)" "sha256:$(rep 4)"; done; }
copy_missing() { node_del econ-graph-control-plane "$(pre frontend)"; }
no_copies() { for node in "${nodes[@]}"; do node_del "$node" "$(pre backend)"; done; }
rollback_refuses "copies that differ between nodes" copies_differ "names different images on different nodes"
rollback_refuses "copies that differ from the record" copies_stale "but the load record says $old_frontend ran before"
rollback_refuses "a node with the name but no copy" copy_missing "econ-graph-control-plane:$(pre frontend)"
rollback_refuses "no copies" no_copies "no node has $(pre backend)"
rollback_refuses "a node it can't read" noop "could not read image" STUB_CRICTL_BROKEN=1
reset
expect_fail "no load record" "$rollback" --to "pre-$TAG"

# --allow-missing-nodes goes ahead, skips that node, and exits 3; a pod there is caught.
reset
load_ok
copy_missing
clear_log
run "$rollback" --to "pre-$TAG" --allow-missing-nodes && fail "rollback --allow-missing-nodes exited 0"
grep -q "exec econ-graph-control-plane ctr -n k8s.io images tag --force $(pre frontend)" "$STUB_LOG" &&
  fail "rollback retagged frontend on the node without its copy"
grep -q "exec econ-graph-worker ctr -n k8s.io images tag --force $(pre frontend)" "$STUB_LOG" ||
  fail "rollback --allow-missing-nodes did not retag the other node"
grep -q 'these nodes still have the release candidate' "$tmp/out" || fail "rollback --allow-missing-nodes did not say what it skipped"
reset
load_ok
copy_missing
expect_fail "MISMATCH econ-graph-frontend econ-graph-frontend-new1" env STUB_POD_NODE=econ-graph-control-plane "$rollback" --to "pre-$TAG" --allow-missing-nodes
grep -qx 'result=FAILED' "$records/rollback.txt" || fail "rollback did not record the pod on the node without a copy"

# Rollback failures after it starts: the check after retagging, pods on the wrong image, partial records.
reset
load_ok
expect_fail "after retagging it" env STUB_CTR_TAG_WRONG=1 "$rollback" --to "pre-$TAG"
grep -qx 'status=partial (exit 1 during: retagging backend)' "$records/rollback.txt" || fail "rollback did not record its partial run: $(cat "$records/rollback.txt" 2>/dev/null)"
reset
load_ok
expect_fail "failed" env STUB_CTR_TAG_FAIL=frontend "$rollback" --to "pre-$TAG"
grep -qx 'status=partial (exit 1 during: retagging frontend)' "$records/rollback.txt" || fail "rollback did not record the failed step"
grep -qF "retagged=crawler-worker node=econ-graph-worker" "$records/rollback.txt" || fail "the partial rollback record does not say what was retagged"
reset
load_ok
run "$verify" --tag "$TAG" || fail "verify before rollback failed: $(cat "$tmp/out")"
expect_fail "MISMATCH crawler-worker crawler-worker-new2" env STUB_WRONG_DEPLOYMENT=crawler-worker "$rollback" --to "pre-$TAG"
grep -qx 'result=FAILED' "$records/rollback.txt" || fail "rollback did not record the pod on the wrong image"

# === every kubectl call, in every case above, names the pinned context
clear_log
grep '^kubectl' "$tmp/all-calls" | grep -vq '^kubectl --context kind-econ-graph ' &&
  fail "a kubectl call did not carry --context kind-econ-graph: $(grep '^kubectl' "$tmp/all-calls" | grep -v '^kubectl --context kind-econ-graph ' | head -1)"
grep -q '^kubectl' "$tmp/all-calls" || fail "no kubectl calls were logged"

if [ "$failures" -ne 0 ]; then
  echo "$failures failure(s)"
  exit 1
fi
echo "OK: load-release-images.sh, verify-release-images.sh and rollback-release-images.sh"
