#!/bin/bash

# Check the crawler's curated series lists and catalog selection rules against each source's
# live upstream catalog, as a one-off Kubernetes Job in one namespace.
#
# Usage:
#   scripts/deploy/check-selection.sh --namespace NAMESPACE [--source FRED[,BEA,...]] [--json]
#       [--image IMAGE] [--timeout SECONDS] [--rate-percent PCT]
#
# The Job runs `crawler check-selection` (docs/data-sources/README.md) with the API keys from
# the namespace's Secret crawler-api-keys, read through the same optional secretKeyRefs as
# crawler-worker (k8s/manifests/crawler-worker.yaml), so no key is ever on a command line or in
# this script's output. A source whose key the namespace doesn't have is reported as skipped.
# The check is read-only and needs no database: the Job gets no database URL. Its requests go
# through the crawler's own per-source rate limits, one source at a time.
#
# Rate: the check runs at --rate-percent (default 25) of each source's policy rate, under its own
# rate limiter. With crawler-worker running on the same keys the two together send up to
# (100 + PCT)% of each policy rate; at the default, FRED gets up to 150 requests a minute against
# its 120-a-minute limit (the fetcher backs off on 429s), and BEA, which also caps data volume
# per minute and locks a key out for an hour, gets more traffic than the worker alone. The
# script warns, naming the combined rate, when the namespace's crawler-worker has replicas. It
# never scales the worker; to stay within the limits, run the check while the worker is idle or
# scaled down.
#
# --namespace is required and has no default: name production (econ-graph) or a branch
# namespace (econ-graph-br-...) explicitly. The image is the one the namespace's crawler-worker
# Deployment runs (with its pull policy); --image names one instead, for a namespace without a
# crawler-worker. The Job's pod is labelled app=crawler-check-selection, and a NetworkPolicy of
# the same name lets that pod reach public HTTPS (a branch namespace otherwise allows it only
# to crawler-worker); both are deleted afterwards.
#
# In a branch namespace:
#   - Only a namespace deployed with local data has a crawler-worker. For any other, pass
#     --image (a crawler-worker image the cluster has loaded).
#   - Branch namespaces have no crawler-api-keys Secret, so FRED, BEA and Census are reported
#     as skipped (exit 3 when nothing else is wrong); BLS, World Bank and FHFA need no key.
#
# One check at a time per namespace: if a crawler-check-selection Job is still running there
# (another operator's), it refuses to start (exit 5). A finished one left by an earlier run is
# deleted first (with its pod), and the exit status is read from the new Job's own pod, by UID.
#
# Idempotent: it deletes a Job and NetworkPolicy left by an earlier run, creates them, waits for
# the Job to finish (--timeout seconds, default 1800), prints the Job's log, deletes both and
# exits with the check's exit status: 0 clean, 1 problems, 2 a source's check could not finish,
# 3 a source was skipped for a missing API key (and nothing worse). 4: the Job did not finish
# in time or its exit status could not be read. They are deleted on every exit, including a
# failure partway and an interrupt (Ctrl-C, SIGTERM). 64: the check's command line was refused
# (an unknown or unsupported source), and also this script's own usage errors, before anything
# runs. With --json the log carries only the JSON report and errors.
#
# Every kubectl call goes to the kind cluster's own kubectl and kubeconfig
# (scripts/deploy/lib/kind-env.sh) on context kind-econ-graph, through kc
# (scripts/deploy/lib/cluster-role.sh), without changing the current context. The cluster's
# context, kube-system UID and production marker are printed before anything is created. It may
# first install the kind cluster's kubectl and kubeconfig under ~/.cache/econgraph.
#
# Environment: KIND_CLUSTER (default econ-graph), POLL_SECONDS (default 5), GRACE_SECONDS
# (default 60: how long past --timeout to wait for the Job's status).

set -euo pipefail

usage() {
    sed -n '3,59p' "$0" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

JOB=crawler-check-selection
target_ns=""
sources=""
json=false
image=""
timeout=1800
rate_percent=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --namespace)
            [[ $# -ge 2 ]] || usage 64
            target_ns="$2"; shift ;;
        --source)
            [[ $# -ge 2 ]] || usage 64
            sources="$2"; shift ;;
        --json) json=true ;;
        --image)
            [[ $# -ge 2 ]] || usage 64
            image="$2"; shift ;;
        --timeout)
            [[ $# -ge 2 ]] || usage 64
            timeout="$2"; shift ;;
        --rate-percent)
            [[ $# -ge 2 ]] || usage 64
            rate_percent="$2"; shift ;;
        -h | --help) usage 0 ;;
        *) echo "unknown argument: $1" >&2; usage 64 ;;
    esac
    shift
done
if [[ -z "$target_ns" ]]; then
    echo "--namespace is required (econ-graph for production, or a branch namespace)" >&2
    usage 64
fi
# Everything below lands in a manifest, so only plain names reach it.
[[ "$target_ns" =~ ^[a-z0-9]([-a-z0-9]{0,61}[a-z0-9])?$ ]] ||
    { echo "not a namespace name: $target_ns" >&2; exit 64; }
[[ -z "$sources" || "$sources" =~ ^[A-Z_]+(,[A-Z_]+)*$ ]] ||
    { echo "--source takes comma-separated source names, e.g. FRED,BEA: $sources" >&2; exit 64; }
[[ -z "$image" || "$image" =~ ^[A-Za-z0-9][A-Za-z0-9./:@_-]*$ ]] ||
    { echo "not an image reference: $image" >&2; exit 64; }
[[ "$timeout" =~ ^[1-9][0-9]*$ ]] || { echo "--timeout takes whole seconds: $timeout" >&2; exit 64; }
[[ -z "$rate_percent" || "$rate_percent" =~ ^([1-9][0-9]?|100)$ ]] ||
    { echo "--rate-percent takes 1 to 100: $rate_percent" >&2; exit 64; }

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "$PROJECT_ROOT/scripts/deploy/lib/kind-env.sh"
check_docker_daemon >&2
kind_cluster_exists || die "kind cluster '${KIND_CLUSTER}' not found on ${DOCKER_HOST}"
ensure_kind_kubectl
KUBECTL="${ECONGRAPH_KUBE_DIR:-${ECONGRAPH_CACHE}}/bin/kubectl"
[ -x "$KUBECTL" ] || die "the kind cluster's kubectl is not at ${KUBECTL}"
command "$KUBECTL" config get-contexts -o name | grep -qxF -- "kind-${KIND_CLUSTER}" ||
    die "$KUBECTL has no context kind-${KIND_CLUSTER}"
CLUSTER_ROLE_CONTEXT="kind-${KIND_CLUSTER}"
export KUBECTL CLUSTER_ROLE_CONTEXT

# Say which cluster this is before creating anything in it.
role_status=0
cluster_is_marked_production || role_status=$?
case "$role_status" in
    0) cluster_role="marked production" ;;
    1) cluster_role="not marked production${CLUSTER_ROLE_REASON:+: ${CLUSTER_ROLE_REASON}}" ;;
    *) cluster_role="production marker unknown: ${CLUSTER_ROLE_REASON}" ;;
esac
cluster_uid="$(kube_system_uid)" || cluster_uid="unknown"
echo "Cluster: context ${CLUSTER_ROLE_CONTEXT}, kube-system UID ${cluster_uid}, ${cluster_role}" \
    "(kubectl ${KUBECTL}, KUBECONFIG=${KUBECONFIG:-default})" >&2

kc get namespace "$target_ns" -o name >/dev/null 2>&1 ||
    die "namespace ${target_ns} not found on ${CLUSTER_ROLE_CONTEXT}"

pull_policy=IfNotPresent
if [[ -z "$image" ]]; then
    container='{.spec.template.spec.containers[?(@.name=="crawler-worker")]'
    image="$(kc -n "$target_ns" get deployment crawler-worker -o "jsonpath=${container}.image}" 2>/dev/null)" ||
        image=""
    [[ -n "$image" ]] ||
        die "namespace ${target_ns} has no crawler-worker Deployment to take the image from; pass --image"
    pull_policy="$(kc -n "$target_ns" get deployment crawler-worker \
        -o "jsonpath=${container}.imagePullPolicy}")"
    [[ "$pull_policy" =~ ^(Always|IfNotPresent|Never)$ ]] || pull_policy=IfNotPresent
fi
echo "Image: ${image} (pull policy ${pull_policy}), namespace ${target_ns}" >&2

# The check has its own rate limiter: beside a running worker the two add up.
replicas="$(kc -n "$target_ns" get deployment crawler-worker --ignore-not-found \
    -o 'jsonpath={.spec.replicas}' 2>/dev/null)" || replicas=""
if [[ "$replicas" =~ ^[1-9][0-9]*$ ]]; then
    share="${rate_percent:-25}"
    warn "crawler-worker is running (${replicas} replica(s)) on the same API keys. The check adds" \
        "${share}% of each source's policy rate to the worker's: up to $((100 + share))% combined," \
        "e.g. FRED $((120 * (100 + share) / 100))/min against its 120/min limit and BEA" \
        "$((30 * (100 + share) / 100))/min. For no overlap, run it while the worker is idle."
fi

# One check per namespace at a time; a finished leftover is deleted below.
existing="$(kc -n "$target_ns" get job "$JOB" --ignore-not-found \
    -o 'jsonpath={.metadata.uid} {.status.succeeded}/{.status.failed} {.metadata.creationTimestamp}')" ||
    die "could not read job/${JOB} in ${target_ns}"
if [[ -n "$existing" ]]; then
    read -r existing_uid existing_state existing_created <<<"$existing"
    case "$existing_state" in
        [1-9]*/* | */[1-9]*) ;;
        *)
            echo "error: job/${JOB} (UID ${existing_uid}, created ${existing_created}) is still" \
                "running in ${target_ns}: another check is in progress. Wait for it, or delete it:" \
                "kubectl -n ${target_ns} delete job ${JOB}" >&2
            exit 5 ;;
    esac
fi

args=('"check-selection"')
if [[ -n "$sources" ]]; then
    args+=('"--source"' "\"${sources}\"")
fi
log_level=warn
if $json; then
    args+=('"--json"')
    log_level=error
fi
if [[ -n "$rate_percent" ]]; then
    args+=('"--rate-percent"' "\"${rate_percent}\"")
fi
command_yaml='["/app/crawler"'
for arg in "${args[@]}"; do
    command_yaml+=", ${arg}"
done
command_yaml+="]"

key_env() {
    local var="$1" key="$2"
    cat <<EOF
        - name: ${var}
          valueFrom:
            secretKeyRef:
              name: crawler-api-keys
              key: ${key}
              optional: true
EOF
}

# shellcheck disable=SC2317,SC2329 # run by the EXIT trap
cleanup() {
    kc -n "$target_ns" delete job "$JOB" --ignore-not-found --wait=false >/dev/null 2>&1 || true
    kc -n "$target_ns" delete networkpolicy "$JOB" --ignore-not-found >/dev/null 2>&1 || true
}

# A finished Job (and its pod) or a NetworkPolicy from an earlier run goes first.
kc -n "$target_ns" delete job "$JOB" --ignore-not-found --cascade=foreground --wait=true >&2
kc -n "$target_ns" delete networkpolicy "$JOB" --ignore-not-found >&2
trap cleanup EXIT
# An interrupt exits through the EXIT trap too.
trap 'exit 130' INT
trap 'exit 143' TERM

kc -n "$target_ns" apply -f - >&2 <<EOF
# Public HTTPS for the check's pod only. DNS comes from the namespace's own policies.
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: ${JOB}
  labels:
    app: ${JOB}
    component: crawler
spec:
  podSelector:
    matchLabels:
      app: ${JOB}
  policyTypes:
  - Egress
  egress:
  - to:
    - ipBlock:
        cidr: 0.0.0.0/0
        except: [0.0.0.0/8, 10.0.0.0/8, 100.64.0.0/10, 127.0.0.0/8, 169.254.0.0/16,
                 172.16.0.0/12, 192.168.0.0/16, 224.0.0.0/4, 240.0.0.0/4]
    ports:
    - port: 443
      protocol: TCP
---
apiVersion: batch/v1
kind: Job
metadata:
  name: ${JOB}
  labels:
    app: ${JOB}
    component: crawler
spec:
  # A retry would repeat every upstream request; one run is the check.
  backoffLimit: 0
  activeDeadlineSeconds: ${timeout}
  # Normally deleted by the script; this cleans up after one that was killed.
  ttlSecondsAfterFinished: 3600
  template:
    metadata:
      labels:
        app: ${JOB}
        component: crawler
    spec:
      restartPolicy: Never
      automountServiceAccountToken: false
      containers:
      - name: check-selection
        image: ${image}
        imagePullPolicy: ${pull_policy}
        command: ${command_yaml}
        env:
        - name: RUST_LOG
          value: ${log_level}
        # Shared reference data (countries.csv) copied into the image; matches its ENV.
        - name: REFERENCE_DATA_DIR
          value: /app/reference
$(key_env FRED_API_KEY fred-api-key)
$(key_env BLS_API_KEY bls-api-key)
$(key_env BEA_API_KEY bea-api-key)
$(key_env CENSUS_API_KEY census-api-key)
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        securityContext:
          runAsNonRoot: true
          runAsUser: 1000
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop:
            - ALL
          seccompProfile:
            type: RuntimeDefault
      securityContext:
        fsGroup: 1000
        seccompProfile:
          type: RuntimeDefault
EOF

# This run's Job, by UID: its pod is selected by that, never by name, so a pod of an earlier
# Job of the same name is never read.
job_uid="$(kc -n "$target_ns" get job "$JOB" -o 'jsonpath={.metadata.uid}')" ||
    die "could not read job/${JOB}"
[[ "$job_uid" =~ ^[0-9a-f-]+$ ]] || die "job/${JOB} has no UID"
pods="batch.kubernetes.io/controller-uid=${job_uid}"

echo "Waiting for job/${JOB} (UID ${job_uid}, up to ${timeout}s)..." >&2
# The Job itself fails at activeDeadlineSeconds; the grace covers its status catching up.
deadline=$((SECONDS + timeout + ${GRACE_SECONDS:-60}))
finished=false
while [[ $SECONDS -lt $deadline ]]; do
    state="$(kc -n "$target_ns" get job "$JOB" -o 'jsonpath={.metadata.uid} {.status.succeeded}/{.status.failed}')" ||
        die "could not read job/${JOB}"
    [[ "${state%% *}" == "$job_uid" ]] || die "job/${JOB} was replaced while waiting (UID ${state%% *})"
    state="${state#* }"
    case "$state" in
        [1-9]*/* | */[1-9]*) finished=true; break ;;
    esac
    sleep "${POLL_SECONDS:-5}"
done

kc -n "$target_ns" logs -l "$pods" --tail=-1 || warn "could not read the log of job/${JOB}"

$finished || { warn "job/${JOB} did not finish within ${timeout}s"; exit 4; }
code="$(kc -n "$target_ns" get pods -l "$pods" \
    -o 'jsonpath={.items[0].status.containerStatuses[0].state.terminated.exitCode}')" || code=""
[[ "$code" =~ ^[0-9]+$ ]] || { warn "could not read the exit status of job/${JOB}"; exit 4; }
exit "$code"
