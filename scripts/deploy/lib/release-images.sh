# shellcheck shell=bash
#
# Shared by load-release-images.sh, verify-release-images.sh and
# rollback-release-images.sh: which images a release candidate has, where
# release.yml pushes them, what the manifests call them, how to read and tag
# images in the kind nodes' containerd, and where the records live. Source it
# after lib/kind-env.sh, which points DOCKER_HOST at the kind cluster's daemon,
# sources lib/cluster-role.sh and runs `kind` as the pinned copy
# (scripts/deploy/install-kind.sh); verify-release-images.sh, which needs only
# kubectl, sources lib/cluster-role.sh alone. Don't run it.

# The variables are read by the scripts that source this.
# shellcheck disable=SC2034

# release.yml: ${REGISTRY}/<github.repository_owner, lowercased>/econ-graph-<name>:sha-<full commit>,
# plus the rc tag itself on the same digest.
RI_REGISTRY="ghcr.io"
RI_GITHUB_OWNER="EconGraph"
RI_OWNER="$(printf '%s' "$RI_GITHUB_OWNER" | tr '[:upper:]' '[:lower:]')"

RI_NAMES=(backend crawler-worker frontend)
RI_NAMESPACE="econ-graph"

# Deployment (also its pods' app label), its container, and its manifest, per image.
# Every other workload in k8s/manifests that runs one of these images must name
# it exactly as the Deployment's manifest does (ri_local_image checks), so the
# load and the rollback, which move that one name on the nodes, cover it too.
declare -A RI_DEPLOYMENT=(
    [backend]=econ-graph-backend
    [crawler-worker]=crawler-worker
    [frontend]=econ-graph-frontend
)
declare -A RI_CONTAINER=(
    [backend]=backend
    [crawler-worker]=crawler-worker
    [frontend]=frontend
)
declare -A RI_MANIFEST=(
    [backend]=k8s/manifests/backend-deployment.yaml
    [crawler-worker]=k8s/manifests/crawler-worker.yaml
    [frontend]=k8s/manifests/frontend-deployment.yaml
)

# Jobs that run one of the images, per image: the Job's pods' app label and its
# container. The crawler-migrate Job (k8s/manifests/crawler-migrate-job.yaml)
# runs the crawler-worker image under the same name as deployment/crawler-worker.
# deploy.sh deletes and reruns it on every deploy, so a deploy after a load runs
# the loaded image; nothing here starts it. verify-release-images.sh checks the
# pods the current Job left (ri_check_job_pods).
declare -A RI_JOB=(
    [crawler-worker]=crawler-migrate
)
declare -A RI_JOB_CONTAINER=(
    [crawler-worker]=crawler-migrate
)

# The kind nodes' containerd namespace for Kubernetes images, and the label that
# makes the CRI report an image as pinned, which kubelet's image garbage
# collection skips.
RI_CTR_NAMESPACE="k8s.io"
RI_PINNED_LABEL="io.cri-containerd.pinned=pinned"

# One directory per rc tag; files there are only ever appended to.
RI_RECORD_ROOT="${ECONGRAPH_RELEASES_DIR:-${HOME}/econgraph-releases}"

ri_die() {
    echo "${RI_SCRIPT}: error: $*" >&2
    exit 1
}

ri_check_tag() {
    [[ "$1" =~ ^v[0-9]+\.[0-9]+\.[0-9]+-rc\.[0-9]+$ ]] ||
        ri_die "'$1' is not a release candidate tag (vX.Y.Z-rc.N)"
}

ri_need_jq() {
    command -v jq >/dev/null 2>&1 || ri_die "jq is not installed"
}

# The image name the manifests run for image $1 (e.g. econ-graph-backend:v4.0.0),
# read from the manifests so it always matches what deploy.sh applies; the tag
# is written nowhere in these scripts. Every manifest under k8s/manifests that
# runs econ-graph-$1 (the Deployment's, and a Job's such as crawler-migrate's)
# must name the same tag, or this stops: the load and the rollback move one
# name per image, and a workload on another name would be left on another image.
ri_local_image() {
    local dir="$PROJECT_ROOT/k8s/manifests" pattern refs files
    pattern="^[[:space:]-]*image:[[:space:]]*[\"']?econ-graph-$1:[^[:space:]\"'#]+"
    refs="$(grep -rhoE --include='*.yaml' --include='*.yml' "$pattern" "$dir" |
        sed -E "s/^[[:space:]-]*image:[[:space:]]*[\"']?//" | sort -u)" || true
    [ -n "$refs" ] || ri_die "no manifest under k8s/manifests runs econ-graph-$1"
    if [ "$(grep -c . <<<"$refs")" -ne 1 ]; then
        files="$(grep -rlE --include='*.yaml' --include='*.yml' "$pattern" "$dir" | sed "s|^$PROJECT_ROOT/||" | paste -sd, -)"
        ri_die "the manifests name econ-graph-$1 with different tags ($(paste -sd, <<<"$refs"), in ${files}); every workload that runs it must name the same one"
    fi
    grep -qE "^[[:space:]-]*image:[[:space:]]*[\"']?${refs}([\"'[:space:]#]|$)" "$PROJECT_ROOT/${RI_MANIFEST[$1]}" ||
        ri_die "${RI_MANIFEST[$1]} does not run ${refs}"
    printf '%s\n' "$refs"
}

# The same name as the nodes' containerd stores it.
ri_node_ref() {
    printf 'docker.io/library/%s\n' "$1"
}

# The node ref of the rollback copy of image $1 for rc tag $2:
# docker.io/library/econ-graph-<name>:pre-<rc tag>.
ri_rollback_ref() {
    printf 'docker.io/library/econ-graph-%s:pre-%s\n' "$1" "$2"
}

ri_remote_repo() {
    printf '%s/%s/econ-graph-%s\n' "$RI_REGISTRY" "$RI_OWNER" "$1"
}

# Use the daemon the kind cluster runs on, as lib/kind-env.sh chose it (the
# rootful unix:///var/run/docker.sock unless ECONGRAPH_DOCKER_HOST says
# otherwise; an operator shell's own DOCKER_HOST, often the rootless daemon, is
# never followed), so nothing is pulled into, `kind load`ed from or `docker
# exec`ed into the wrong daemon. Refuse when it can't be reached or is rootless.
ri_use_rootful_docker() {
    export DOCKER_HOST="$ECONGRAPH_DOCKER_HOST"
    unset DOCKER_CONTEXT
    local security
    if ! docker info --format '{{.ServerVersion}}' >/dev/null 2>&1; then
        ri_die "the rootful Docker daemon (DOCKER_HOST=${DOCKER_HOST}) is not reachable"
    fi
    security="$(docker info --format '{{json .SecurityOptions}}' 2>/dev/null || true)"
    case "$security" in
        *rootless*) ri_die "DOCKER_HOST=${DOCKER_HOST} is a rootless daemon; the kind cluster runs on the rootful one" ;;
    esac
}

# Refuse, before anything changes, when the daemon keeps images in Docker's
# containerd image store (the default for new installs since Docker 29) rather
# than the classic one. With that store `docker pull` of a multi-platform image
# keeps the whole index but only this platform's content, so `kind load
# docker-image` can fail on a node part way through, and `docker image inspect
# .Id` is the manifest or index digest rather than the config digest the nodes
# report, so every node check after the load would fail. Either way the load
# would stop after the nodes had changed. A daemon whose store can't be read is
# refused too.
ri_require_classic_image_store() {
    local driver
    driver="$(docker info --format '{{json .DriverStatus}}' 2>/dev/null)" ||
        ri_die "could not read the image store of DOCKER_HOST=${DOCKER_HOST} (docker info .DriverStatus)"
    case "$driver" in
        *io.containerd.snapshotter*)
            ri_die "DOCKER_HOST=${DOCKER_HOST} keeps images in Docker's containerd image store; kind load docker-image and the image ID checks need the classic store (\"features\": {\"containerd-snapshotter\": false} in daemon.json). Refusing before anything changes" ;;
    esac
}

# Fill RI_NODES with the nodes of kind cluster $1; fail when it has none.
ri_kind_nodes() {
    local out
    out="$(kind get nodes --name "$1")" || ri_die "kind get nodes --name $1 failed"
    mapfile -t RI_NODES < <(grep -v '^[[:space:]]*$' <<<"$out")
    [ "${#RI_NODES[@]}" -gt 0 ] || ri_die "kind cluster $1 has no nodes on DOCKER_HOST=${DOCKER_HOST}"
}

# Pin every kc call to kind-<cluster $1>, the context deploy.sh deploys to,
# through the kind cluster's own kubectl and kubeconfig when copies are kept in
# ~/.cache/econgraph, and print that cluster's identity
# (cluster_role_operator_context in lib/cluster-role.sh). The current context is
# not changed and a CLUSTER_ROLE_CONTEXT inherited from the environment is never
# followed. The checks go through kc too, so even they name the context.
ri_pin_context() {
    cluster_role_operator_context "kind-$1" ||
        ri_die "could not use context kind-$1, or read that cluster's identity"
}

# `crictl inspecti -o json` of ref $2 in node $1: the JSON, or nothing when the
# node has no such image. Any other failure (node not running, crictl or
# containerd down) exits the script: "can't tell" must not read as "not there".
# Call it as a plain assignment (x="$(...)"), so that exit stops the caller too.
ri_node_inspect() {
    local json err
    err="$(mktemp)"
    if json="$(docker exec "$1" crictl inspecti -o json "$2" 2>"$err")"; then
        rm -f "$err"
        printf '%s\n' "$json"
        return 0
    fi
    if grep -qi 'no such image' "$err"; then
        rm -f "$err"
        return 0
    fi
    echo "${RI_SCRIPT}: error: could not read image $2 on node $1: $(tr '\n' ' ' <"$err")" >&2
    rm -f "$err"
    exit 1
}

# The image ID (sha256:<config digest>) that ref $2 names in node $1's
# containerd, or nothing when the node doesn't have it. $2 may also be an image
# ID. Exits on any other failure; call it as a plain assignment.
ri_node_image_id() {
    local json
    json="$(ri_node_inspect "$1" "$2")" || exit 1
    [ -n "$json" ] || return 0
    jq -r '.status.id // empty' <<<"$json"
}

# "true" when the CRI reports the image ref $2 names in node $1 as pinned.
ri_node_image_pinned() {
    local json
    json="$(ri_node_inspect "$1" "$2")" || exit 1
    [ -n "$json" ] || { echo false; return 0; }
    jq -r '.status.pinned // false' <<<"$json"
}

# A ref in node $1 that names image ID $2: its first tag, else the ID itself
# (the CRI keeps an image record under its ID).
ri_node_source_ref() {
    local json tag
    json="$(ri_node_inspect "$1" "$2")" || exit 1
    tag="$(jq -r '.status.repoTags[0] // empty' <<<"$json")"
    printf '%s\n' "${tag:-$2}"
}

# Point ref $3 at ref $2 in node $1's containerd. With --force as $4, an
# existing $3 is moved. ctr copies $2's labels, so a pinned source makes a
# pinned $3.
ri_ctr_tag() {
    local force=()
    [ "${4:-}" != --force ] || force=(--force)
    docker exec "$1" ctr -n "$RI_CTR_NAMESPACE" images tag "${force[@]}" "$2" "$3"
}

# Label ref $2 in node $1 pinned, then require the CRI to report it pinned.
ri_ctr_pin() {
    local pinned
    docker exec "$1" ctr -n "$RI_CTR_NAMESPACE" images label "$2" "$RI_PINNED_LABEL" >/dev/null
    pinned="$(ri_node_image_pinned "$1" "$2")" || exit 1
    [ "$pinned" = true ] || ri_die "${2} on ${1} is labelled ${RI_PINNED_LABEL}, but the CRI does not report it pinned"
}

# Print "pod|phase|deletionTimestamp|imageID" for each pod of the Deployment of
# image $1 (imageID of its app container). Pipes, not tabs: an empty field
# between tabs would vanish when read.
ri_pod_lines() {
    local jsonpath
    jsonpath='{range .items[*]}{.metadata.name}{"|"}{.status.phase}{"|"}{.metadata.deletionTimestamp}{"|"}{range .status.containerStatuses[?(@.name=="'"${RI_CONTAINER[$1]}"'")]}{.imageID}{end}{"\n"}{end}'
    kc -n "$RI_NAMESPACE" get pods -l "app=${RI_DEPLOYMENT[$1]}" -o "jsonpath=${jsonpath}" ||
        ri_die "could not list the pods of deployment/${RI_DEPLOYMENT[$1]}"
}

# The trailing sha256:<hex> of a pod's imageID. On the kind nodes the CRI
# reports the image ID, sha256:<config digest> (also seen as
# repo@sha256:<digest> or docker-pullable://repo@sha256:<digest> elsewhere).
ri_image_digest() {
    grep -oE 'sha256:[0-9a-f]{64}$' <<<"$1" || true
}

# The distinct image IDs the running (not terminating) pods of image $1 run, one per line.
ri_running_image_ids() {
    local pod phase deleting image_id lines
    lines="$(ri_pod_lines "$1")" || return 1
    while IFS="|" read -r pod phase deleting image_id; do
        if [ -z "$pod" ] || [ -n "$deleting" ] || [ "$phase" != Running ]; then
            continue
        fi
        ri_image_digest "$image_id"
    done <<<"$lines" | sort -u
}

# Check that every running pod of image $1 runs image ID or digest $2 (or $3).
# Prints a line per pod and appends the checked ones to RI_REPORT. Returns 1 on
# any other image, or when no pod is running, and 2 when the pods can't be listed.
ri_check_pods() {
    local name="$1" want="$2" want_alt="${3:-}" pod phase deleting image_id got line running=0 status=0
    local deployment="${RI_DEPLOYMENT[$name]}" lines
    lines="$(ri_pod_lines "$name")" || return 2
    while IFS="|" read -r pod phase deleting image_id; do
        [ -n "$pod" ] || continue
        if [ -n "$deleting" ]; then
            echo "  ${pod}: terminating, not checked"
            continue
        fi
        if [ "$phase" != Running ]; then
            echo "  ${pod}: ${phase}, not checked"
            continue
        fi
        running=$((running + 1))
        got="$(ri_image_digest "$image_id")"
        if [ -n "$got" ] && { [ "$got" = "$want" ] || [ "$got" = "$want_alt" ]; }; then
            line="ok ${deployment} ${pod} ${image_id}"
        else
            line="MISMATCH ${deployment} ${pod} runs '${image_id}', expected ${want}"
            status=1
        fi
        echo "  ${line}"
        RI_REPORT+="${line}"$'\n'
    done <<<"$lines"
    if [ "$running" -eq 0 ]; then
        echo "  MISMATCH ${deployment}: no running pod"
        RI_REPORT+="MISMATCH ${deployment} no running pod"$'\n'
        status=1
    fi
    return "$status"
}

# Check the pods the current Job of image $1 (RI_JOB) left, if the image has a
# Job, against image ID or digest $2 (or $3). They ran for the last deploy.sh: a
# pod whose container started (running, or finished either way) must have run
# that image. The pods are selected by the Job's own
# batch.kubernetes.io/controller-uid, so a pod that an earlier run of the Job
# left behind is never taken for this run's. A pod whose container never
# started (Pending, ErrImageNeverPull, still in an init container), or that is
# being deleted, is recorded as unchecked. No Job, or a Job with no pod, is
# recorded as unchecked too, not as a failure: deploy.sh skips the Job in a
# namespace without a data database, and Kubernetes deletes a finished Job,
# with its pods, a day later (ttlSecondsAfterFinished). Prints a line per pod
# and appends the result to RI_REPORT. Returns 1 on any other image, 2 when the
# Job or its pods can't be read.
ri_check_job_pods() {
    local name="$1" want="$2" want_alt="${3:-}" job="${RI_JOB[$1]:-}"
    local jsonpath lines uid pod phase deleting image_id got line seen=0 status=0
    [ -n "$job" ] || return 0
    uid="$(kc -n "$RI_NAMESPACE" get job "$job" --ignore-not-found -o 'jsonpath={.metadata.uid}')" || {
        echo "  MISMATCH job/${job}: could not read the Job"
        RI_REPORT+="MISMATCH job/${job} could not read the Job"$'\n'
        return 2
    }
    if [ -z "$uid" ]; then
        line="unchecked job/${job}: no Job (deploy.sh skips it without a data database, and Kubernetes deletes it a day after it finishes)"
        echo "  ${line}"
        RI_REPORT+="${line}"$'\n'
        return 0
    fi
    jsonpath='{range .items[*]}{.metadata.name}{"|"}{.status.phase}{"|"}{.metadata.deletionTimestamp}{"|"}{range .status.containerStatuses[?(@.name=="'"${RI_JOB_CONTAINER[$name]}"'")]}{.imageID}{end}{"\n"}{end}'
    lines="$(kc -n "$RI_NAMESPACE" get pods -l "batch.kubernetes.io/controller-uid=${uid}" -o "jsonpath=${jsonpath}")" || {
        echo "  MISMATCH job/${job}: could not list its pods"
        RI_REPORT+="MISMATCH job/${job} could not list its pods"$'\n'
        return 2
    }
    while IFS="|" read -r pod phase deleting image_id; do
        [ -n "$pod" ] || continue
        seen=$((seen + 1))
        if [ -n "$deleting" ]; then
            line="unchecked job/${job} ${pod} ${phase}: terminating"
        elif [ -z "$image_id" ]; then
            line="unchecked job/${job} ${pod} ${phase}: its container never started"
        else
            got="$(ri_image_digest "$image_id")"
            if [ -n "$got" ] && { [ "$got" = "$want" ] || [ "$got" = "$want_alt" ]; }; then
                line="ok job/${job} ${pod} ${phase} ${image_id}"
            else
                line="MISMATCH job/${job} ${pod} ${phase} ran '${image_id}', expected ${want}"
                status=1
            fi
        fi
        echo "  ${line}"
        RI_REPORT+="${line}"$'\n'
    done <<<"$lines"
    if [ "$seen" -eq 0 ]; then
        line="unchecked job/${job}: the Job (uid ${uid}) has no pod"
        echo "  ${line}"
        RI_REPORT+="${line}"$'\n'
    fi
    return "$status"
}

# Restart the three Deployments and wait for each rollout.
ri_restart_deployments() {
    local name
    for name in "${RI_NAMES[@]}"; do
        kc -n "$RI_NAMESPACE" rollout restart "deployment/${RI_DEPLOYMENT[$name]}"
    done
    for name in "${RI_NAMES[@]}"; do
        kc -n "$RI_NAMESPACE" rollout status "deployment/${RI_DEPLOYMENT[$name]}" --timeout=600s ||
            ri_die "deployment/${RI_DEPLOYMENT[$name]} did not finish rolling out"
    done
}

ri_record_file() {
    printf '%s/%s/images.txt\n' "$RI_RECORD_ROOT" "$1"
}

# Append text $2 to file $1 as a new block (a blank line between blocks).
ri_append_block() {
    local separator=""
    mkdir -p "$(dirname "$1")"
    [ ! -s "$1" ] || separator=$'\n'
    printf '%s%s' "$separator" "$2" >>"$1"
}

# The last block of the load record for tag $1 (one load per block).
ri_last_record() {
    local file
    file="$(ri_record_file "$1")"
    [ -s "$file" ] || ri_die "no load record ${file}; run scripts/deploy/load-release-images.sh --tag $1 first"
    awk '/^loaded_at=/ { block = "" } { block = block $0 "\n" } END { printf "%s", block }' "$file"
}

# The first block of the load record for tag $1, or nothing when there is no
# record. Its before= lines say what ran before the tag was first loaded.
ri_first_record() {
    local file
    file="$(ri_record_file "$1")"
    [ -s "$file" ] || return 0
    awk '/^loaded_at=/ { n++ } n == 1' "$file"
}

# Field $2 (key=value) of the "before=<name>" line for image $1 in record text $3.
ri_before_field() {
    awk -v name="$1" -v key="$2" '
        $1 == "before=" name { for (i = 2; i <= NF; i++) if (index($i, key "=") == 1) { print substr($i, length(key) + 2); exit } }
    ' <<<"$3"
}

# Field $2 (key=value) of the "image=<name>" line for image $1 in record text $3.
ri_record_field() {
    awk -v name="$1" -v key="$2" '
        $1 == "image=" name { for (i = 2; i <= NF; i++) if (index($i, key "=") == 1) { print substr($i, length(key) + 2); exit } }
    ' <<<"$3"
}

# The distinct local names (local=) that the loads of tag $2 recorded for image
# $1, one per line, or nothing when no record names one.
ri_recorded_locals() {
    local file
    file="$(ri_record_file "$2")"
    [ -s "$file" ] || return 0
    awk -v name="$1" '
        $1 == "before=" name || $1 == "image=" name {
            for (i = 2; i <= NF; i++) if (index($i, "local=") == 1) print substr($i, 7)
        }
    ' "$file" | sort -u
}

# Value of top-level key $1 in record text $2.
ri_record_value() {
    awk -F= -v key="$1" '$1 == key { print substr($0, length(key) + 2); exit }' <<<"$2"
}
