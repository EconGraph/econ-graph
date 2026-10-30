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
#     deleted afterwards, also when the Job fails or times out;
#   - the Job's log is printed, and the script exits with the check's exit status (4 when the
#     Job doesn't finish or its status can't be read).
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

# The kubectl on PATH (the microk8s wrapper on production): must never run.
cat >"$work/bin/kubectl" <<EOF
#!/bin/sh
echo "\$*" >>"$path_log"
echo "plain kubectl stub: refusing" >&2
exit 1
EOF

# The fake kind kubectl that `docker cp` hands out.
#   STUB_NO_DEPLOYMENT=1  the namespace has no crawler-worker Deployment
#   STUB_NO_NAMESPACE=1   the namespace doesn't exist
#   STUB_JOB=done|failed|running  the Job's state (default done)
#   STUB_EXIT=N           the check's exit status (default 0); "none" for no status
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
    [ "\${STUB_NO_DEPLOYMENT:-0}" = 1 ] && { echo "NotFound" >&2; exit 1; }
    case " \$* " in
      *imagePullPolicy*) echo Never ;;
      *) echo econ-graph-crawler-worker:v4.1.0 ;;
    esac ;;
  *" apply -f - "*) cat >>"$applied"; echo "---" >>"$applied" ;;
  *" get job crawler-check-selection "*)
    case "\${STUB_JOB:-done}" in
      done) echo "1/" ;;
      failed) echo "/1" ;;
      *) echo "/" ;;
    esac ;;
  *" logs job/crawler-check-selection "*) echo "FRED: ok" ;;
  *" get pods -l job-name=crawler-check-selection "*)
    [ "\${STUB_EXIT:-0}" = none ] || printf '%s' "\${STUB_EXIT:-0}" ;;
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
[ "$status" = 2 ] || fail "no --namespace: status $status, want 2"
grep -q -- "--namespace is required" "$out" || fail "no --namespace is not explained: $(cat "$out")"
[ -s "$log" ] && fail "kubectl ran without --namespace: $(cat "$log")"

bad_args=("--namespace|Econ_Graph" "--source|fred" "--source|FRED;rm" "--image|a\"b"
  "--image|x
y" "--timeout|0")
for bad in "${bad_args[@]}"; do
  run -- --namespace econ-graph "${bad%%|*}" "${bad#*|}"
  [ "$status" = 2 ] || fail "'$bad': status $status, want 2"
  [ -s "$log" ] && fail "'$bad': kubectl ran: $(cat "$log")"
done

# A full run: identity first, cleanup before apply, the manifest, the log, the exit status.
run -- --namespace econ-graph-br-x-0123456789 --source FRED,BEA --json
[ "$status" = 0 ] || fail "clean run: status $status: $(cat "$out")"
all_pinned || fail "a kubectl call was not pinned to kind-econ-graph: $(cat "$log")"
grep -qF "Cluster: context kind-econ-graph, kube-system UID $cluster_uid, marked production" "$out" ||
  fail "the cluster identity is not shown: $(cat "$out")"
grep -qF "FRED: ok" "$out" || fail "the Job's log is not printed: $(cat "$out")"
ns="-n econ-graph-br-x-0123456789"
delete_job="$(line_of "$ns delete job crawler-check-selection --ignore-not-found --wait=true")"
delete_policy="$(line_of "$ns delete networkpolicy crawler-check-selection")"
apply="$(line_of "$ns apply -f -")"
[ -n "$delete_job" ] && [ -n "$delete_policy" ] && [ -n "$apply" ] &&
  [ "$delete_job" -lt "$apply" ] && [ "$delete_policy" -lt "$apply" ] ||
  fail "a leftover Job or NetworkPolicy is not deleted before apply: $(cat "$log")"
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
run STUB_JOB=running -- --namespace econ-graph --timeout 1
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
called "get deployment" && fail "--image still read the Deployment"

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
