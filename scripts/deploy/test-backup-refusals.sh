#!/bin/bash
#
# Check that teardown.sh, recreate-kind-cluster.sh and migrate-to-host-storage.sh
# (forward and --rollback) refuse before they dump, stop or delete anything when
# the encrypted backup cannot be made:
#
#   - no public age recipient (BACKUP_AGE_RECIPIENT unset, no backup.env);
#   - plaintext staging (BACKUP_PLAINTEXT_ROOT) that is not tmpfs;
#   - for the migration, which stops the writers before its backup: tmpfs
#     staging too small for the databases.
#
# Each refusal is paired with a control run that is set up correctly and gets
# past that point (it starts a dump, or stops the writers), so a refusal
# can't pass because the stubs stopped the script earlier.
#
# Runs offline against stubs, never a cluster or a Docker daemon: docker, kind,
# age and age-keygen stubs first on PATH, the fake kind kubectl that `docker cp`
# hands out (it records every call; `scale` and `delete` fail after they are
# recorded), and for the forward migration a scratch cluster whose "btrfs
# subvolumes" are plain directories a stat stub vouches for.
#
# Usage: scripts/deploy/test-backup-refusals.sh

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
# Private tmpfs staging for the runs that may start a (stubbed) backup.
RAM="$(mktemp -d /dev/shm/econgraph-refusals-test.XXXXXXXX)"
trap 'rm -rf "$WORK" "$RAM"' EXIT
mkdir -p "$WORK/bin" "$WORK/home" "$WORK/tmp" "$WORK/storage/data-plane" "$WORK/storage/app-prod" "$WORK/disk-staging"
KUBE_CALLS="$WORK/kubectl.log"
OTHER="$WORK/other.log"
CLUSTER_UID="11111111-2222-3333-4444-555555555555"
[ "$(stat -f -c %T "$WORK/disk-staging")" != tmpfs ] || {
    echo "run with TMPDIR on a disk filesystem, not tmpfs"
    exit 1
}

# Plain kubectl: never used.
cat >"$WORK/bin/kubectl" <<'EOF'
#!/bin/sh
echo "plain kubectl stub: refusing" >&2
exit 1
EOF

# The fake kind kubectl. FAKE_DB_SIZE is what pg_database_size reports.
cat >"$WORK/kind-kubectl" <<EOF
#!/bin/sh
echo "\$*" >>"$KUBE_CALLS"
case " \$* " in
    *" version "*) exit 0 ;;
    *" config use-context "*) exit 0 ;;
    *" config current-context"*) echo "kind-\${FAKE_CLUSTER:-econ-graph}"; exit 0 ;;
    *" config view "*) echo "https://127.0.0.1:6443"; exit 0 ;;
    *" get namespace kube-system "*) echo "$CLUSTER_UID"; exit 0 ;;
    *" get configmap econgraph-cluster-role "*) exit 0 ;;
    *" scale "*|*" delete "*) exit 1 ;;
    *"{.status.phase}"*) echo Running; exit 0 ;;
    *"to_regclass"*) echo t; exit 0 ;;
    *"pg_database_size"*) echo "\${FAKE_DB_SIZE:-1048576}"; exit 0 ;;
    *"count(*)"*) echo 1; exit 0 ;;
    *" get deploy,sts "*) echo "econ-graph-backend:test"; exit 0 ;;
    *" -o name"*) echo "deployment/stub"; exit 0 ;;
esac
exit 0
EOF

cat >"$WORK/bin/docker" <<EOF
#!/bin/sh
echo "docker \$*" >>"$OTHER"
case " \$* " in
    *" info "*) echo "28.0.0 /var/lib/docker [name=seccomp]" ;;
    *" cp "*) for last; do :; done; cp "$WORK/kind-kubectl" "\$last" ;;
    *" image inspect "*) echo "sha256:aaaa" ;;
    *" crictl inspecti "*) echo '  "id": "sha256:aaaa",' ;;
    *" mountpoint "*) exit 1 ;;
    *" get namespace kube-system "*) echo "$CLUSTER_UID" ;;
esac
exit 0
EOF

cat >"$WORK/bin/kind" <<EOF
#!/bin/sh
echo "kind \$*" >>"$OTHER"
case "\$1 \$2" in
    "get clusters") echo "\${FAKE_CLUSTER:-econ-graph}" ;;
    "get kubeconfig") echo "apiVersion: v1" ;;
    "version "*) echo "kind v0.33.0 go1.26.7 linux/amd64" ;;
esac
case "\$1" in delete|create) exit 1 ;; esac
exit 0
EOF

# Recipient parsing and the throwaway key are tested with real age in
# scripts/backup/test-encryption.sh; here only the refusals matter.
cat >"$WORK/bin/age" <<'EOF'
#!/bin/sh
cat >/dev/null
EOF
cat >"$WORK/bin/age-keygen" <<'EOF'
#!/bin/sh
exit 0
EOF

# The scratch storage directories stand in for btrfs subvolume roots.
cat >"$WORK/bin/stat" <<EOF
#!/bin/bash
for last; do :; done
case "\$last" in
    "$WORK/storage/"*)
        case "\$*" in
            "-c %i "*) echo 256; exit 0 ;;
            "-f -c %T "*) echo btrfs; exit 0 ;;
        esac ;;
esac
exec /usr/bin/stat "\$@"
EOF
for tool in yq mountpoint findmnt; do
    printf '#!/bin/sh\nexit 0\n' >"$WORK/bin/$tool"
done
chmod +x "$WORK/bin/"* "$WORK/kind-kubectl"
mkdir -p "$WORK/home/.local/share/econgraph/bin"
cp "$WORK/bin/kind" "$WORK/home/.local/share/econgraph/bin/kind-v0.33.0"
printf 'kind: Cluster\napiVersion: kind.x-k8s.io/v1alpha4\n' >"$WORK/kind-config.yaml"

# run_script SCRIPT ARGS...: SCRIPT with the stubs first on PATH, no kube config
# and stdin not a terminal. RECIPIENT, STAGING, CLUSTER and SCRATCH_STORAGE
# choose the setup. Prints the output; returns the exit status.
run_script() {
    local script="$1"
    shift
    env -u KUBECONFIG -u KUBECTL -u CLUSTER_ROLE_CONTEXT -u ECONGRAPH_KUBE_DIR -u BACKUP_AGE_RECIPIENT \
        -u ECONGRAPH_SCRATCH_STORAGE -u BACKUP_PLAINTEXT_ROOT \
        HOME="$WORK/home" XDG_DATA_HOME="$WORK/home/.local/share" XDG_CACHE_HOME="$WORK/cache" TMPDIR="$WORK/tmp" \
        ECONGRAPH_CONFIG_DIR="$WORK/no-config" ECONGRAPH_BACKUP_ROOT="$WORK/backups" \
        KIND_CLUSTER="${CLUSTER:-econ-graph}" FAKE_CLUSTER="${CLUSTER:-econ-graph}" \
        ${SCRATCH_STORAGE:+ECONGRAPH_SCRATCH_STORAGE="$SCRATCH_STORAGE"} \
        ${RECIPIENT:+BACKUP_AGE_RECIPIENT="$RECIPIENT"} \
        ${STAGING:+BACKUP_PLAINTEXT_ROOT="$STAGING"} \
        PATH="$WORK/bin:/usr/local/bin:/usr/bin:/bin" \
        "$PROJECT_ROOT/scripts/deploy/$script" "$@" </dev/null 2>&1
}

failed=0
fail() { echo "FAIL $*"; failed=1; }
reset() { : >"$KUBE_CALLS"; : >"$OTHER"; rm -rf "$WORK/backups" "$WORK/cache"; }
good_recipient="age1qyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqs3290gq"

# expect NAME MESSAGE FORBIDDEN-REGEX SCRIPT ARGS...: SCRIPT fails, says
# MESSAGE, and made no fake kubectl call matching FORBIDDEN-REGEX.
expect() {
    local name="$1" message="$2" forbidden="$3" out rc
    shift 3
    reset
    out="$(run_script "$@")"; rc=$?
    if [ "$rc" -ne 0 ] && grep -q -- "$message" <<<"$out"; then
        echo "ok   $name"
    else
        fail "$name (exit $rc):"; printf '     %s\n' "$out"
    fi
    if grep -qE -- "$forbidden" "$KUBE_CALLS"; then
        fail "$name: made a call it must not have made:"
        grep -E -- "$forbidden" "$KUBE_CALLS" | sed 's/^/     /'
    fi
    if grep -qE '^kind (delete|create)' "$OTHER"; then
        fail "$name: ran kind delete or create"
    fi
    if [ -e "$WORK/backups" ] && [ -n "$(ls -A "$WORK/backups")" ]; then
        fail "$name: wrote into the backup root: $(ls -A "$WORK/backups")"
    fi
}

# expect_reached NAME CALL-REGEX SCRIPT ARGS...: the control run, set up
# correctly, gets as far as a fake kubectl call matching CALL-REGEX.
expect_reached() {
    local name="$1" call="$2" out rc
    shift 2
    reset
    out="$(run_script "$@")"; rc=$?
    if grep -qE -- "$call" "$KUBE_CALLS"; then
        echo "ok   $name"
    else
        fail "$name never got to '$call' (exit $rc):"; printf '     %s\n' "$out"
    fi
}

no_dump=' exec .*(pg_dump|psql)'

# --- teardown.sh ---
RECIPIENT="$good_recipient" STAGING="$RAM" expect_reached "teardown.sh control run starts the backup" \
    "$no_dump" teardown.sh --yes
STAGING="$RAM" expect "teardown.sh refuses without a recipient before dumping" \
    'set BACKUP_AGE_RECIPIENT' "$no_dump|delete|patch" teardown.sh --yes
RECIPIENT="$good_recipient" STAGING="$WORK/disk-staging" expect "teardown.sh refuses disk staging before dumping" \
    'must be tmpfs' "$no_dump|delete|patch" teardown.sh --yes

# --- recreate-kind-cluster.sh ---
RECIPIENT="$good_recipient" STAGING="$RAM" expect_reached "recreate-kind-cluster.sh control run starts the backup" \
    "$no_dump" recreate-kind-cluster.sh --yes --config "$WORK/kind-config.yaml"
STAGING="$RAM" expect "recreate-kind-cluster.sh refuses without a recipient before dumping" \
    'set BACKUP_AGE_RECIPIENT' "$no_dump" recreate-kind-cluster.sh --yes --config "$WORK/kind-config.yaml"
RECIPIENT="$good_recipient" STAGING="$WORK/disk-staging" expect "recreate-kind-cluster.sh refuses disk staging before dumping" \
    'must be tmpfs' "$no_dump" recreate-kind-cluster.sh --yes --config "$WORK/kind-config.yaml"

# --- migrate-to-host-storage.sh, forward and --rollback ---
# The migration stops the writers before its backup, so every refusal must
# come before `scale`.
export CLUSTER=eco718-refusals SCRATCH_STORAGE="$WORK/storage"
for mode in forward rollback; do
    args=(--yes)
    [ "$mode" = forward ] || args=(--rollback --ref HEAD --yes)
    RECIPIENT="$good_recipient" STAGING="$RAM" expect_reached "migrate $mode control run stops the writers" \
        ' scale ' migrate-to-host-storage.sh "${args[@]}"
    STAGING="$RAM" expect "migrate $mode refuses without a recipient before stopping writers" \
        'set BACKUP_AGE_RECIPIENT' ' scale |pg_dump' migrate-to-host-storage.sh "${args[@]}"
    RECIPIENT="$good_recipient" STAGING="$WORK/disk-staging" expect "migrate $mode refuses disk staging before stopping writers" \
        'must be tmpfs' ' scale |pg_dump' migrate-to-host-storage.sh "${args[@]}"
    RECIPIENT="$good_recipient" STAGING="$RAM" FAKE_DB_SIZE=$((1 << 60)) expect "migrate $mode refuses tmpfs too small for the databases before stopping writers" \
        'may need .* MiB of private tmpfs staging; nothing was stopped' ' scale |pg_dump' migrate-to-host-storage.sh "${args[@]}"
    RECIPIENT="$good_recipient" STAGING="$RAM" FAKE_DB_SIZE=unknown expect "migrate $mode refuses an unreadable database size before stopping writers" \
        "invalid size 'unknown'" ' scale |pg_dump' migrate-to-host-storage.sh "${args[@]}"
done
unset CLUSTER SCRATCH_STORAGE

if [ "$failed" -ne 0 ]; then
    echo "backup refusal checks failed"
    exit 1
fi
echo "backup refusal checks passed"
