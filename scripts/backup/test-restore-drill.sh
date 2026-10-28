#!/usr/bin/env bash
# Check restore-drill.sh without a Docker daemon, cluster or real backup:
#   - with no DUMP_FILE, it refuses the newest VERIFIED bundle when that bundle
#     is encrypted, names the open-backup.sh command, and starts no container;
#   - the throwaway Postgres it starts keeps its data directory on tmpfs and
#     mounts the dump directory read-only, with no other volume.
# A docker stub first on PATH records every call; `docker run` fails after it
# is recorded, so nothing is restored.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT
mkdir -p "$work/bin" "$work/home" "$work/backups/20260101T000000Z" "$work/plain"
cat >"$work/bin/docker" <<EOF
#!/bin/bash
printf '%s\n' "\$*" >>"$work/docker.log"
case "\$1" in
    info) echo "28.0.0 /var/lib/docker [name=seccomp]" ;;
    run) exit 1 ;;
esac
exit 0
EOF
chmod +x "$work/bin/docker"
failed=0
fail() { echo "FAIL $*"; failed=1; }
drill() {
    env -i HOME="$work/home" PATH="$work/bin:/usr/bin:/bin" BACKUP_ROOT="$work/backups" \
        ECONGRAPH_CONFIG_DIR="$work/no-config" ECONGRAPH_STATE_DIR="$work/state" \
        "$root/scripts/backup/restore-drill.sh" "$@" 2>&1
}

# The newest verified bundle is encrypted: refuse, and say how to open it.
bundle="$work/backups/20260101T000000Z"
printf 'ciphertext' >"$bundle/backup.tar.age"
touch "$bundle/VERIFIED"
: >"$work/docker.log"
if out="$(drill)"; then
    fail "the drill ran unattended on an encrypted bundle"
fi
grep -q "encrypted backup: run .*open-backup.sh .*$bundle" <<<"$out" ||
    fail "the drill did not name the open-backup.sh command: $out"
grep -q 'no unattended restore was performed' <<<"$out" || fail "the drill did not say nothing was restored: $out"
grep -q '^run' "$work/docker.log" && fail "the drill started a container for an encrypted bundle"

# A plaintext dump (as open-backup.sh hands it over): PGDATA on tmpfs, the
# dump directory read-only, no other volume.
printf 'PGDMP' >"$work/plain/econ_graph.dump"
: >"$work/docker.log"
drill "$work/plain/econ_graph.dump" >"$work/plain.log" || true
run_args="$(grep '^run ' "$work/docker.log" || true)"
if [[ -z "$run_args" ]]; then
    fail "the drill started no container: $(cat "$work/plain.log")"
else
    grep -q -- '--tmpfs /var/lib/postgresql\( \|$\)' <<<"$run_args" ||
        fail "the drill's Postgres data directory is not on tmpfs: $run_args"
    [[ "$(grep -o -- ' -v [^ ]*' <<<"$run_args")" == " -v $work/plain:/dumps:ro" ]] ||
        fail "the drill mounts something other than the dump directory read-only: $run_args"
    grep -q -- '--mount' <<<"$run_args" && fail "the drill adds a mount: $run_args"
fi

if [[ "$failed" -ne 0 ]]; then
    echo "restore drill checks failed"
    exit 1
fi
echo "restore drill checks passed"
