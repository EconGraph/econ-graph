#!/usr/bin/env bash
# Tests for scripts/deploy/refetch-full-history.sh against stubs, never a cluster or a
# Docker daemon:
#   - every kubectl call goes to the kind cluster's own kubectl (copied out of the node by
#     lib/kind-env.sh) with --context kind-econ-graph, never the kubectl on PATH nor a
#     KUBECTL or CLUSTER_ROLE_CONTEXT from the environment, and the current context is not
#     changed; without that context, or without the kind cluster, it stops before running
#     anything;
#   - the cluster's context, kube-system UID and production marker are printed before the
#     crawler runs;
#   - --dry-run passes --dry-run to the crawler and prints no progress or resume hint;
#   - the Resume hint repeats --include-inactive and --priority N, and the source selection;
#   - --status runs refetch-status.
#
# The stubs: a plain `kubectl` first on PATH that records any call and fails; `docker` and
# `kind` stubs that pretend a kind cluster econ-graph exists and hand out a fake "kind
# kubectl"; and that fake, which records every call and answers the reads the script makes.
#
# Usage: scripts/deploy/test-refetch-full-history.sh (from anywhere)
set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
script="$repo_root/scripts/deploy/refetch-full-history.sh"
failures=0
fail() {
  echo "FAIL $1"
  failures=$((failures + 1))
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/home" "$work/tmp"
log="$work/kind-kubectl.log"
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

# The fake kind kubectl that `docker cp` hands out. Every call is appended to the log;
# nothing is changed.
#   STUB_NO_KIND=1         its kubeconfig has no kind-econ-graph context
#   STUB_PRODUCTION=false  the cluster carries no production marker
#   STUB_MARKER_FAILS=1    reading the production marker fails
cat >"$work/kind-kubectl" <<EOF
#!/bin/sh
echo "kubectl \$*" >>"$log"
case " \$* " in
  *" version "*) exit 0 ;;
  *" config get-contexts -o name "*)
    [ "\${STUB_NO_KIND:-0}" = 1 ] || echo kind-econ-graph
    echo other ;;
  *" config current-context "*) echo other ;;
  *" get namespace kube-system "*) echo "$cluster_uid" ;;
  *" get configmap econgraph-cluster-role "*)
    if [ "\${STUB_MARKER_FAILS:-0}" = 1 ]; then echo "connection refused" >&2; exit 1; fi
    [ "\${STUB_PRODUCTION:-true}" = true ] && printf 'production $cluster_uid' ;;
  *" exec deploy/crawler-worker -- /app/crawler "*) echo "crawler ran" ;;
esac
exit 0
EOF

#   STUB_NO_CLUSTER=1  kind lists no cluster econ-graph
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
  "get clusters") [ "\${STUB_NO_CLUSTER:-0}" = 1 ] || echo econ-graph ;;
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
kind_kubectl="$work/cache/econgraph/bin/kubectl"

# run [ENV=VALUE...] -- ARGS...: runs the script with only the stubs reachable, a KUBECTL
# and a CLUSTER_ROLE_CONTEXT in the environment that it must ignore, and stdin not a
# terminal; output in $out, exit status in $status.
run() {
  : >"$log"
  local envs=()
  while [ "$1" != -- ]; do envs+=("$1"); shift; done
  shift
  status=0
  env -u KUBECONFIG -u ECONGRAPH_KUBE_DIR -u KIND_CLUSTER -u NAMESPACE -u DEPLOYMENT \
    -u ECONGRAPH_DOCKER_HOST \
    KUBECTL="$work/bin/kubectl" CLUSTER_ROLE_CONTEXT=other \
    HOME="$work/home" XDG_DATA_HOME="$work/home/.local/share" XDG_CACHE_HOME="$work/cache" \
    TMPDIR="$work/tmp" PATH="$work/bin:/usr/local/bin:/usr/bin:/bin" \
    "${envs[@]}" "$script" "$@" </dev/null >"$out" 2>&1 || status=$?
  # The same run's stdout alone, for checks that stderr text stays out of it.
  env -u KUBECONFIG -u ECONGRAPH_KUBE_DIR -u KIND_CLUSTER -u NAMESPACE -u DEPLOYMENT \
    -u ECONGRAPH_DOCKER_HOST \
    KUBECTL="$work/bin/kubectl" CLUSTER_ROLE_CONTEXT=other \
    HOME="$work/home" XDG_DATA_HOME="$work/home/.local/share" XDG_CACHE_HOME="$work/cache" \
    TMPDIR="$work/tmp" PATH="$work/bin:/usr/local/bin:/usr/bin:/bin" \
    "${envs[@]}" "$script" "$@" </dev/null >"$out.stdout" 2>/dev/null || true
}
called() { grep -qF -- "$1" "$log"; }
# Every call but kind-env.sh's reachability check and listing the contexts is on
# --context kind-econ-graph, and none changes the current context.
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

# Enqueue with the options a resume must keep.
run -- --source FRED,BLS --include-inactive --priority 7
[ "$status" = 0 ] || fail "enqueue failed: $(cat "$out")"
called "kubectl --context kind-econ-graph -n econ-graph exec deploy/crawler-worker -- /app/crawler refetch --source FRED,BLS --include-inactive --priority 7" ||
  fail "the crawler was not run on kind-econ-graph with the options: $(cat "$log")"
all_pinned || fail "a kubectl call was not pinned to kind-econ-graph: $(cat "$log")"
grep -qF "Cluster: context kind-econ-graph, kube-system UID $cluster_uid, marked production (kubectl $kind_kubectl," "$out" ||
  fail "the cluster identity is not shown: $(cat "$out")"
identity_line="$(grep -n "^Cluster:" "$out" | head -1 | cut -d: -f1)"
crawler_line="$(grep -n "^crawler ran" "$out" | head -1 | cut -d: -f1)"
[ "${identity_line:-999}" -lt "${crawler_line:-0}" ] ||
  fail "the cluster identity is not shown before the crawler runs: $(cat "$out")"
grep -qE "^Resume: +$script --source FRED,BLS --include-inactive --priority 7 --completed-since [0-9TZ:-]+$" "$out" ||
  fail "the Resume hint drops options: $(cat "$out")"
grep -qE "^Progress: +$script --status --source FRED,BLS --since [0-9TZ:-]+ --failures 50$" "$out" ||
  fail "the Progress hint is wrong: $(cat "$out")"

# The identity goes to stderr only, so stdout (e.g. --json output) stays clean.
if grep -q "^Cluster:" "$out.stdout"; then fail "the cluster identity went to stdout: $(cat "$out.stdout")"; fi

# A marker that can't be read is never shown as production, and says why.
run STUB_MARKER_FAILS=1 -- --all
grep -qF "Cluster: context kind-econ-graph, kube-system UID $cluster_uid, production marker unknown: could not read ConfigMap" "$out" ||
  fail "an unreadable marker is not reported as unknown: $(cat "$out")"
if grep -q "^Cluster:.*, marked production" "$out"; then fail "an unreadable marker is shown as production: $(cat "$out")"; fi

# An unmarked cluster is named as such.
run STUB_PRODUCTION=false -- --all
[ "$status" = 0 ] || fail "enqueue on an unmarked cluster failed: $(cat "$out")"
grep -qF "Cluster: context kind-econ-graph, kube-system UID $cluster_uid, not marked production" "$out" ||
  fail "an unmarked cluster is not named as such: $(cat "$out")"

# A resumed run keeps the original start and the options.
run -- --all --priority 3 --completed-since 2026-10-26T00:00:00Z
[ "$status" = 0 ] || fail "resume failed: $(cat "$out")"
grep -qxE "Resume: +$script --all --priority 3 --completed-since 2026-10-26T00:00:00Z" "$out" ||
  fail "the resumed Resume hint is wrong: $(cat "$out")"

# Without extra options the hint has none.
run -- --all
grep -qE "^Resume: +$script --all --completed-since [0-9TZ:-]+$" "$out" ||
  fail "the plain Resume hint is wrong: $(cat "$out")"

# --dry-run: the crawler's own dry run, no hints.
run -- --all --dry-run --include-inactive
[ "$status" = 0 ] || fail "dry run failed: $(cat "$out")"
called "/app/crawler refetch --all --dry-run --include-inactive" || fail "--dry-run was not passed on: $(cat "$log")"
if grep -qE "^(Resume|Progress):" "$out"; then fail "dry run printed hints: $(cat "$out")"; fi
all_pinned || fail "a dry-run kubectl call was not pinned: $(cat "$log")"

# --status runs refetch-status on the pinned context.
run -- --status --all --since 2026-10-26T00:00:00Z --failures 5
[ "$status" = 0 ] || fail "--status failed: $(cat "$out")"
called "kubectl --context kind-econ-graph -n econ-graph exec deploy/crawler-worker -- /app/crawler refetch-status --all --since 2026-10-26T00:00:00Z --failures 5" ||
  fail "refetch-status was not run: $(cat "$log")"
all_pinned || fail "a --status kubectl call was not pinned: $(cat "$log")"

# Without the kind context it stops before running anything.
run STUB_NO_KIND=1 -- --all
[ "$status" = 1 ] || fail "a missing kind-econ-graph context did not stop it (status $status)"
grep -q "has no context kind-econ-graph" "$out" || fail "the missing context is not named: $(cat "$out")"
if called "exec"; then fail "ran the crawler without the kind context"; fi

# Without the kind cluster it stops before running anything.
run STUB_NO_CLUSTER=1 -- --all
[ "$status" = 1 ] || fail "a missing kind cluster did not stop it (status $status)"
grep -q "kind cluster 'econ-graph' not found" "$out" || fail "the missing cluster is not named: $(cat "$out")"
if called "exec"; then fail "ran the crawler without the kind cluster"; fi

# The kubectl on PATH, which KUBECTL also named, is never called.
if [ -s "$path_log" ]; then fail "the kubectl on PATH was called: $(cat "$path_log")"; fi

if [ "$failures" -ne 0 ]; then
  echo "$failures failure(s)"
  exit 1
fi
echo "refetch-full-history: all checks passed"
