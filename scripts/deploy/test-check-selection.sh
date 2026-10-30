#!/usr/bin/env bash
# Tests for scripts/deploy/check-selection.sh against stubs, never a cluster or a Docker daemon:
#   - --namespace is required (NAMESPACE from the environment doesn't count), and bad
#     namespace, source, image and timeout values stop it before any kubectl call;
#   - every kubectl call goes to the kind cluster's own kubectl with --context kind-econ-graph,
#     never the kubectl on PATH, and the cluster identity is printed before anything is created;
#   - the Job uses the namespace's crawler-worker image and pull policy (or --image), reads the
#     keys through optional secretKeyRefs to crawler-api-keys, gets no database URL, and passes
#     --source and --json to `crawler check-selection`;
#   - a leftover Job and NetworkPolicy are deleted before the new ones are applied, and both are
#     deleted afterwards, also when the Job fails or times out, the script fails partway or it
#     is interrupted with SIGTERM or SIGINT;
#   - the Job's log is printed, and the script exits with the check's exit status (4 when the
#     Job doesn't finish or its status can't be read), reading the log and status from the new
#     Job's pod by its UID, never by the job-name label;
#   - a crawler-check-selection Job still running (another operator's) makes it refuse (5)
#     without touching it; a finished one is deleted in the foreground first;
#   - a running crawler-worker gets a warning naming the combined rate, nothing is scaled, and
#     --rate-percent reaches the crawler.
#
# The stubs: a plain `kubectl` first on PATH that records any call and fails; `docker` and
# `kind` stubs that pretend a kind cluster econ-graph exists and hand out a fake "kind
# kubectl"; and that fake, which records every call (and every manifest applied) and answers
# the reads the script makes. HOME, XDG_* and TMPDIR point into a temporary directory.
#
# Usage: scripts/deploy/test-check-selection.sh (from anywhere)
set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
script="$repo_root/scripts/deploy/check-selection.sh"
failures=0
fail() {
  echo "FAIL $1"
  failures=$((failures + 1))
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/home" "$work/tmp"
log="$work/kind-kubectl.log"
applied="$work/applied.yaml"
path_log="$work/path-kubectl.log"
other_log="$work/other.log"
out="$work/out"
cluster_uid="11111111-2222-3333-4444-555555555555"
new_uid="0000aaaa-1111-2222-3333-444455556666"

# The kubectl on PATH (the microk8s wrapper on production): must never run.
cat >"$work/bin/kubectl" <<EOF
#!/bin/sh
echo "\$*" >>"$path_log"
echo "plain kubectl stub: refusing" >&2
exit 1
EOF

# The fake kind kubectl that `docker cp` hands out.
#   STUB_NO_DEPLOYMENT=1  the namespace has no crawler-worker Deployment
#   STUB_REPLICAS=N       crawler-worker's replicas (default 0)
#   STUB_NO_NAMESPACE=1   the namespace doesn't exist
#   STUB_EXISTING=none|running|finished  a crawler-check-selection Job before the run
#   STUB_JOB=done|failed|running  the new Job's state (default done)
#   STUB_JOB_READ_FAILS=1 reading the new Job's state fails
#   STUB_JOB_REPLACED=1   the Job's UID changes while waiting (another operator replaced it)
#   STUB_EXIT=N           the check's exit status (default 0); "none" for no status
# The new Job's UID is $new_uid; a pod read by any other selector answers exit status 9.
cat >"$work/kind-kubectl" <<EOF
#!/bin/sh
echo "kubectl \$*" >>"$log"
case " \$* " in
  *" version "*) exit 0 ;;
  *" config get-contexts -o name "*) echo kind-econ-graph; echo other ;;
  *" config current-context "*) echo other ;;
  *" get namespace kube-system "*) echo "$cluster_uid" ;;
  *" get configmap econgraph-cluster-role "*) printf 'production $cluster_uid' ;;
  *" get namespace "*)
    [ "\${STUB_NO_NAMESPACE:-0}" = 1 ] && { echo "NotFound" >&2; exit 1; }
    echo namespace/x ;;
  *" get deployment crawler-worker "*)
    if [ "\${STUB_NO_DEPLOYMENT:-0}" = 1 ]; then
      case " \$* " in *" --ignore-not-found "*) exit 0 ;; esac
      echo "NotFound" >&2; exit 1
    fi
    case " \$* " in
      *spec.replicas*) printf '%s' "\${STUB_REPLICAS:-0}" ;;
      *imagePullPolicy*) echo Never ;;
      *) echo econ-graph-crawler-worker:v4.1.0 ;;
    esac ;;
  *" apply -f - "*) cat >>"$applied"; echo "---" >>"$applied" ;;
  *" get job crawler-check-selection --ignore-not-found "*)
    case "\${STUB_EXISTING:-none}" in
      running) echo "old-uid / 2026-10-30T10:00:00Z" ;;
      finished) echo "old-uid 1/ 2026-10-30T10:00:00Z" ;;
    esac ;;
  *" get job crawler-check-selection "*status.succeeded*)
    [ "\${STUB_JOB_READ_FAILS:-0}" = 1 ] && { echo "connection refused" >&2; exit 1; }
    uid="$new_uid"
    [ "\${STUB_JOB_REPLACED:-0}" = 1 ] && uid=0000ffff-0000-0000-0000-000000000000
    case "\${STUB_JOB:-done}" in
      done) echo "\$uid 1/" ;;
      failed) echo "\$uid /1" ;;
      *) echo "\$uid /" ;;
    esac ;;
  *" get job crawler-check-selection -o jsonpath={.metadata.uid} "*) echo "$new_uid" ;;
  *" logs -l batch.kubernetes.io/controller-uid=$new_uid --tail=-1 "*) echo "FRED: ok" ;;
  *" logs "*) echo "STALE LOG" ;;
  *" get pods -l batch.kubernetes.io/controller-uid=$new_uid "*)
    [ "\${STUB_EXIT:-0}" = none ] || printf '%s' "\${STUB_EXIT:-0}" ;;
  *" get pods "*) printf 9 ;;
esac
exit 0
EOF

cat >"$work/bin/docker" <<EOF
#!/bin/sh
echo "docker \$*" >>"$other_log"
case "\$1" in
  info) echo "28.0.0 /var/lib/docker [name=seccomp]" ;;
  cp) for last; do :; done; cp "$work/kind-kubectl" "\$last" ;;
esac
exit 0
EOF
cat >"$work/bin/kind" <<EOF
#!/bin/sh
echo "kind \$*" >>"$other_log"
case "\$1 \${2:-}" in
  "get clusters") echo econ-graph ;;
  "get kubeconfig") echo "apiVersion: v1" ;;
  "version "*) echo "kind v0.33.0 go1.26.7 linux/amd64" ;;
esac
exit 0
EOF
chmod +x "$work/bin/"* "$work/kind-kubectl"
# The script runs kind from its pinned path (lib/kind-version.sh); the stub stands in there.
kind_pinned="$(HOME="$work/home" XDG_DATA_HOME="$work/home/.local/share" bash -c \
  'source "$1/scripts/deploy/lib/kind-version.sh" && echo "$KIND_PINNED_BIN"' _ "$repo_root")"
mkdir -p "$(dirname "$kind_pinned")"
cp "$work/bin/kind" "$kind_pinned"

# run [ENV=VALUE...] -- ARGS...: runs the script with only the stubs reachable, a KUBECTL,
# CLUSTER_ROLE_CONTEXT and NAMESPACE in the environment that it must ignore, and stdin not a
# terminal; output in $out, exit status in $status.
run() {
  : >"$log"
  : >"$applied"
  local envs=()
  while [ "$1" != -- ]; do envs+=("$1"); shift; done
  shift
  status=0
  env -u KUBECONFIG -u ECONGRAPH_KUBE_DIR -u KIND_CLUSTER -u ECONGRAPH_DOCKER_HOST \
    KUBECTL="$work/bin/kubectl" CLUSTER_ROLE_CONTEXT=other NAMESPACE=econ-graph POLL_SECONDS=0 \
    HOME="$work/home" XDG_DATA_HOME="$work/home/.local/share" XDG_CACHE_HOME="$work/cache" \
    TMPDIR="$work/tmp" PATH="$work/bin:/usr/local/bin:/usr/bin:/bin" \
    "${envs[@]}" "$script" "$@" </dev/null >"$out" 2>&1 || status=$?
}
called() { grep -qF -- "$1" "$log"; }
line_of() { grep -nF -- "$1" "$log" | head -1 | cut -d: -f1; }
all_pinned() {
  local line
  while IFS= read -r line; do
    case "$line" in
      "kubectl --context kind-econ-graph version "* | "kubectl version "*) ;;
      "kubectl config get-contexts -o name") ;;
      "kubectl --context kind-econ-graph "*) ;;
      *) return 1 ;;
    esac
  done <"$log"
  ! grep -q "use-context" "$log"
}

# --namespace is required; NAMESPACE from the environment is not a default.
run -- --source FRED
[ "$status" = 64 ] || fail "no --namespace: status $status, want 64"
grep -q -- "--namespace is required" "$out" || fail "no --namespace is not explained: $(cat "$out")"
[ -s "$log" ] && fail "kubectl ran without --namespace: $(cat "$log")"

bad_args=("--namespace|Econ_Graph" "--source|fred" "--source|FRED;rm" "--image|a\"b"
  "--image|x
y" "--timeout|0" "--rate-percent|0" "--rate-percent|101")
for bad in "${bad_args[@]}"; do
  run -- --namespace econ-graph "${bad%%|*}" "${bad#*|}"
  [ "$status" = 64 ] || fail "'$bad': status $status, want 64"
  [ -s "$log" ] && fail "'$bad': kubectl ran: $(cat "$log")"
done

# A full run: identity first, cleanup before apply, the manifest, the log, the exit status.
run -- --namespace econ-graph-br-x-0123456789 --source FRED,BEA --json
[ "$status" = 0 ] || fail "clean run: status $status: $(cat "$out")"
all_pinned || fail "a kubectl call was not pinned to kind-econ-graph: $(cat "$log")"
grep -qF "Cluster: context kind-econ-graph, kube-system UID $cluster_uid, marked production" "$out" ||
  fail "the cluster identity is not shown: $(cat "$out")"
grep -qF "FRED: ok" "$out" || fail "the Job's log is not printed: $(cat "$out")"
called "get pods -l batch.kubernetes.io/controller-uid=$new_uid" ||
  fail "the pod is not selected by the new Job's UID: $(cat "$log")"
called "job-name=" && fail "a pod was selected by the job-name label: $(cat "$log")"
grep -q "STALE LOG" "$out" && fail "a log was read by a selector other than the new Job's UID"
grep -q "crawler-worker is running" "$out" && fail "warned about a worker with 0 replicas"
ns="-n econ-graph-br-x-0123456789"
delete_job="$(line_of "$ns delete job crawler-check-selection --ignore-not-found --cascade=foreground --wait=true")"
delete_policy="$(line_of "$ns delete networkpolicy crawler-check-selection")"
apply="$(line_of "$ns apply -f -")"
if ! { [ -n "$delete_job" ] && [ -n "$delete_policy" ] && [ -n "$apply" ] &&
  [ "$delete_job" -lt "$apply" ] && [ "$delete_policy" -lt "$apply" ]; }; then
  fail "a leftover Job or NetworkPolicy is not deleted before apply: $(cat "$log")"
fi
[ "$(grep -cF -- "$ns delete job crawler-check-selection" "$log")" = 2 ] ||
  fail "the Job is not deleted afterwards: $(cat "$log")"
[ "$(grep -cF -- "$ns delete networkpolicy crawler-check-selection" "$log")" = 2 ] ||
  fail "the NetworkPolicy is not deleted afterwards: $(cat "$log")"
grep -qF 'command: ["/app/crawler", "check-selection", "--source", "FRED,BEA", "--json"]' "$applied" ||
  fail "the crawler command is wrong: $(grep command "$applied")"
grep -qF "image: econ-graph-crawler-worker:v4.1.0" "$applied" ||
  fail "the Job does not use the crawler-worker image: $(cat "$applied")"
grep -qF "imagePullPolicy: Never" "$applied" || fail "the pull policy is not the Deployment's"
for key in fred-api-key bls-api-key bea-api-key census-api-key; do
  grep -A3 "key: $key" "$applied" | grep -qF "optional: true" ||
    fail "$key is not an optional secretKeyRef: $(cat "$applied")"
done
[ "$(grep -c "name: crawler-api-keys" "$applied")" = 4 ] || fail "keys don't come from crawler-api-keys"
grep -q "DATABASE_URL" "$applied" && fail "the Job gets a database URL"
grep -qF "value: error" "$applied" || fail "--json does not quiet the log"
grep -qF "app: crawler-check-selection" "$applied" || fail "the pod is not labelled for its policy"
# The manifest is valid YAML: a NetworkPolicy for the pod's label, and the Job.
if python3 -c 'import yaml' 2>/dev/null; then
  python3 - "$applied" <<'PY' || fail "the applied manifest is not what it should be: $(cat "$applied")"
import sys, yaml
docs = [d for d in yaml.safe_load_all(open(sys.argv[1])) if d]
kinds = {d["kind"]: d for d in docs}
assert sorted(kinds) == ["Job", "NetworkPolicy"], kinds.keys()
policy, job = kinds["NetworkPolicy"], kinds["Job"]
pod = job["spec"]["template"]
assert policy["spec"]["podSelector"]["matchLabels"] == {"app": pod["metadata"]["labels"]["app"]}
container = pod["spec"]["containers"][0]
assert container["command"] == ["/app/crawler", "check-selection", "--source", "FRED,BEA", "--json"]
keys = {e["name"]: e["valueFrom"]["secretKeyRef"] for e in container["env"] if "valueFrom" in e}
assert sorted(keys) == ["BEA_API_KEY", "BLS_API_KEY", "CENSUS_API_KEY", "FRED_API_KEY"], keys
assert all(k["name"] == "crawler-api-keys" and k["optional"] is True for k in keys.values())
assert job["spec"]["backoffLimit"] == 0
PY
else
  echo "python3 with PyYAML not found; skipping the manifest parse check"
fi
grep -q "app: crawler-worker" "$applied" && fail "the Job's pod claims to be crawler-worker"

# The check's exit status is the script's; the Job is still cleaned up.
for code in 1 2 3; do
  run STUB_EXIT=$code STUB_JOB=failed -- --namespace econ-graph
  [ "$status" = "$code" ] || fail "check exit $code: script status $status: $(cat "$out")"
  grep -qF "FRED: ok" "$out" || fail "a failed Job's log is not printed"
  [ "$(grep -cF "delete job crawler-check-selection" "$log")" = 2 ] ||
    fail "a failed Job is not deleted: $(cat "$log")"
done
grep -qF 'command: ["/app/crawler", "check-selection"]' "$applied" ||
  fail "a run without --source/--json passes extra arguments: $(grep command "$applied")"
grep -qF "value: warn" "$applied" || fail "the default log level is not warn"

# A Job that never finishes: 4, log printed, cleaned up.
run STUB_JOB=running POLL_SECONDS=0.2 GRACE_SECONDS=0 -- --namespace econ-graph --timeout 1
[ "$status" = 4 ] || fail "a Job that never finishes: status $status, want 4"
grep -q "did not finish" "$out" || fail "the timeout is not explained: $(cat "$out")"
[ "$(grep -cF "delete job crawler-check-selection" "$log")" = 2 ] || fail "a timed-out Job is not deleted"

# No readable exit status: 4.
run STUB_EXIT=none -- --namespace econ-graph
[ "$status" = 4 ] || fail "no exit status: status $status, want 4"

# Without a crawler-worker, --image is needed and used with IfNotPresent.
run STUB_NO_DEPLOYMENT=1 -- --namespace econ-graph-br-x
[ "$status" = 1 ] || fail "no crawler-worker and no --image: status $status, want 1"
grep -q "pass --image" "$out" || fail "the missing image is not explained: $(cat "$out")"
called "apply" && fail "applied without an image"
run STUB_NO_DEPLOYMENT=1 -- --namespace econ-graph-br-x --image econ-graph-crawler-worker:v4.1.0
[ "$status" = 0 ] || fail "--image run: status $status: $(cat "$out")"
grep -qF "imagePullPolicy: IfNotPresent" "$applied" || fail "--image does not use IfNotPresent"
called ".image}" && fail "--image still read the Deployment's image"

# Cleaned up after: deleted twice (before apply, and on exit).
cleaned_up() {
  [ "$(grep -cF "delete job crawler-check-selection" "$log")" = 2 ] &&
    [ "$(grep -cF "delete networkpolicy crawler-check-selection" "$log")" = 2 ]
}

# A failure partway (the Job's state can't be read) still deletes both.
run STUB_JOB_READ_FAILS=1 -- --namespace econ-graph
[ "$status" = 1 ] || fail "unreadable Job state: status $status, want 1"
cleaned_up || fail "a failure partway left the Job or NetworkPolicy: $(cat "$log")"

# An interrupt while waiting still deletes both.
for signal in TERM INT; do
  : >"$log"
  : >"$applied"
  # A background job starts with SIGINT ignored; --default-signal restores it, as at a terminal.
  env --default-signal=INT -u KUBECONFIG -u ECONGRAPH_KUBE_DIR -u KIND_CLUSTER \
    -u ECONGRAPH_DOCKER_HOST \
    KUBECTL="$work/bin/kubectl" CLUSTER_ROLE_CONTEXT=other POLL_SECONDS=1 STUB_JOB=running \
    HOME="$work/home" XDG_DATA_HOME="$work/home/.local/share" XDG_CACHE_HOME="$work/cache" \
    TMPDIR="$work/tmp" PATH="$work/bin:/usr/local/bin:/usr/bin:/bin" \
    "$script" --namespace econ-graph --timeout 5 </dev/null >"$out" 2>&1 &
  pid=$!
  for _ in $(seq 100); do
    grep -qF "status.succeeded" "$log" && break
    sleep 0.1
  done
  kill -s "$signal" "$pid"
  status=0
  wait "$pid" || status=$?
  want=143
  [ "$signal" = INT ] && want=130
  [ "$status" = "$want" ] || fail "SIG$signal: status $status, want $want: $(cat "$out")"
  cleaned_up || fail "SIG$signal left the Job or NetworkPolicy: $(cat "$log")"
done

# Another operator's check still running: refuse (5), and touch nothing of theirs.
run STUB_EXISTING=running -- --namespace econ-graph
[ "$status" = 5 ] || fail "a running Job: status $status, want 5: $(cat "$out")"
grep -q "another check is in progress" "$out" || fail "the running Job is not explained: $(cat "$out")"
called "delete" && fail "deleted while another check runs: $(cat "$log")"
called "apply" && fail "applied while another check runs"

# A finished leftover is deleted (with its pod) and the run goes on.
run STUB_EXISTING=finished -- --namespace econ-graph
[ "$status" = 0 ] || fail "a finished leftover: status $status: $(cat "$out")"
called "delete job crawler-check-selection --ignore-not-found --cascade=foreground --wait=true" ||
  fail "a finished leftover is not deleted in the foreground: $(cat "$log")"

# The Job replaced by someone else while waiting: stop, and don't read their pod.
run STUB_JOB_REPLACED=1 -- --namespace econ-graph
[ "$status" = 1 ] || fail "a replaced Job: status $status, want 1"
grep -q "was replaced while waiting" "$out" || fail "the replaced Job is not explained: $(cat "$out")"
called "get pods" && fail "read a pod of a replaced Job"

# A running worker: a warning naming the combined rate; --rate-percent goes to the crawler.
run STUB_REPLICAS=1 -- --namespace econ-graph
[ "$status" = 0 ] || fail "with a running worker: status $status: $(cat "$out")"
grep -q "crawler-worker is running (1 replica(s)).*up to 125% combined.*FRED 150/min" "$out" ||
  fail "no combined-rate warning: $(cat "$out")"
called "scale" && fail "the script scaled something: $(cat "$log")"
run STUB_REPLICAS=2 -- --namespace econ-graph --rate-percent 50
grep -q "up to 150% combined.*FRED 180/min" "$out" || fail "the warning ignores --rate-percent: $(cat "$out")"
grep -qF '"--rate-percent", "50"]' "$applied" || fail "--rate-percent is not passed on: $(grep command "$applied")"

# A namespace that doesn't exist stops it before anything is created.
run STUB_NO_NAMESPACE=1 -- --namespace econ-graph-br-gone
[ "$status" = 1 ] || fail "missing namespace: status $status, want 1"
grep -q "namespace econ-graph-br-gone not found" "$out" || fail "missing namespace not named: $(cat "$out")"
called "apply" && fail "applied into a missing namespace"

# The kubectl on PATH, which KUBECTL also named, is never called.
[ -s "$path_log" ] && fail "the kubectl on PATH was called: $(cat "$path_log")"

if [ "$failures" -ne 0 ]; then
  echo "$failures failure(s)"
  exit 1
fi
echo "check-selection: all checks passed"
