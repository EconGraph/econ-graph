#!/usr/bin/env bash
# Tests that offsite-restic.sh's `restic` passthrough refuses forget, prune
# and the other refused subcommands wherever they appear in the arguments,
# and passes everything else through. Uses a stub restic that only records
# its arguments: no repository, network or real restic needed.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
script="${repo_root}/scripts/backup/offsite-restic.sh"
failures=0

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

mkdir -p "${tmp}/bin" "${tmp}/config"
cat >"${tmp}/bin/restic" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"${STUB_LOG}"
EOF
chmod +x "${tmp}/bin/restic"
(umask 077 && echo dummy >"${tmp}/config/restic.password")
(umask 077 && printf 'RESTIC_REPOSITORY=%s\nRESTIC_PASSWORD_FILE=%s\n' \
    "${tmp}/repo" "${tmp}/config/restic.password" >"${tmp}/config/restic.env")

export ECONGRAPH_CONFIG_DIR="${tmp}/config"
export BACKUP_ROOT="${tmp}/dumps"
export KEYCLOAK_BACKUP_ROOT="${tmp}/upgrade-dumps"
export STUB_LOG="${tmp}/restic.log"
# Where offsite-restic.sh backup records a success (never the operator's own state).
export ECONGRAPH_STATE_DIR="${tmp}/state"
export PATH="${tmp}/bin:${PATH}"

# expect <exit code> <text the output must contain> <args...>
expect() {
    local want="$1" text="$2" output status=0
    shift 2
    : >"${STUB_LOG}"
    output="$("${script}" "$@" </dev/null 2>&1)" || status=$?
    if [[ "${status}" -ne "${want}" ]] || ! grep -qF -- "${text}" <<<"${output}"; then
        echo "FAIL offsite-restic.sh $*: want exit ${want} and output containing '${text}', got exit ${status}:"
        echo "${output}"
        failures=$((failures + 1))
    elif [[ "${want}" -ne 0 && -s "${STUB_LOG}" ]]; then
        echo "FAIL offsite-restic.sh $*: refused, but restic still ran: $(cat "${STUB_LOG}")"
        failures=$((failures + 1))
    else
        echo "ok   offsite-restic.sh $*"
    fi
}

for sub in forget prune key unlock migrate repair rewrite tag; do
    expect 1 "restic ${sub} is refused" restic "${sub}"
done
expect 1 "restic forget is refused" restic -r "${tmp}/repo" forget --keep-last 1
expect 1 "restic prune is refused" restic --verbose prune
expect 1 "restic forget is refused" restic --no-lock forget --prune
expect 1 "restic prune is refused" restic -o s3.region=x prune
expect 1 "restic tag is refused" restic snapshots --tag tag

expect 0 "" restic stats
if [[ "$(cat "${STUB_LOG}")" != "stats" ]]; then
    echo "FAIL offsite-restic.sh restic stats: restic got '$(cat "${STUB_LOG}")'"
    failures=$((failures + 1))
fi
expect 0 "" restic --verbose ls latest
if [[ "$(cat "${STUB_LOG}")" != "--verbose ls latest" ]]; then
    echo "FAIL offsite-restic.sh restic --verbose ls latest: restic got '$(cat "${STUB_LOG}")'"
    failures=$((failures + 1))
fi

# Both encrypted host and Keycloak-upgrade bundles are selected, including
# when their roots differ. Incomplete upgrade payloads are left out.
mkdir -p "$BACKUP_ROOT/nightly" "$KEYCLOAK_BACKUP_ROOT/upgrade" "$KEYCLOAK_BACKUP_ROOT/incomplete"
mkdir -p "$KEYCLOAK_BACKUP_ROOT/unpublished.partial"
# The sealer writes the marker before its final rename; this is still unpublished.
touch "$KEYCLOAK_BACKUP_ROOT/unpublished.partial/VERIFIED"
touch "$BACKUP_ROOT/nightly/VERIFIED" "$KEYCLOAK_BACKUP_ROOT/upgrade/VERIFIED"
expect 0 "" backup
for expected in "$BACKUP_ROOT/nightly" "$KEYCLOAK_BACKUP_ROOT/upgrade"; do
    if ! grep -qF "$expected" "$STUB_LOG"; then
        echo "FAIL offsite backup omitted $expected"
        failures=$((failures + 1))
    fi
done
if grep -qF -e "$KEYCLOAK_BACKUP_ROOT/incomplete" -e "$KEYCLOAK_BACKUP_ROOT/unpublished.partial" "$STUB_LOG"; then
    echo "FAIL offsite backup included an incomplete upgrade payload"
    failures=$((failures + 1))
fi
# A successful backup is recorded for check-freshness.sh; a failed one is not.
recorded="$(sed -n 's/^passed_epoch=//p' "$ECONGRAPH_STATE_DIR/offsite.last" 2>/dev/null || true)"
if [[ "$recorded" =~ ^[0-9]+$ ]] && (( $(date +%s) - recorded < 60 )); then
    echo "ok   offsite-restic.sh backup records its success in offsite.last"
else
    echo "FAIL offsite-restic.sh backup did not record its success: '$recorded'"
    failures=$((failures + 1))
fi
echo "passed_epoch=1" >"$ECONGRAPH_STATE_DIR/offsite.last"
# shellcheck disable=SC2016 # the stub's $ expand when it runs, not here
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$*" >>"${STUB_LOG}"\nexit 1\n' >"${tmp}/bin/restic"
if "${script}" backup </dev/null >/dev/null 2>&1; then
    echo "FAIL offsite-restic.sh backup succeeded although restic failed"
    failures=$((failures + 1))
fi
if [[ "$(cat "$ECONGRAPH_STATE_DIR/offsite.last")" == "passed_epoch=1" ]]; then
    echo "ok   a failed restic backup records nothing"
else
    echo "FAIL a failed restic backup changed offsite.last: $(cat "$ECONGRAPH_STATE_DIR/offsite.last")"
    failures=$((failures + 1))
fi
# shellcheck disable=SC2016
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$*" >>"${STUB_LOG}"\n' >"${tmp}/bin/restic"

# An explicitly configured missing root must not silently omit upgrade backups.
KEYCLOAK_BACKUP_ROOT="$tmp/missing-upgrade-root" expect 1 "configured Keycloak backup root is missing" backup
# The absent default root remains optional on a host without upgrade backups.
env -u KEYCLOAK_BACKUP_ROOT ECONGRAPH_BACKUP_ROOT="$tmp/unused-default" "$script" backup >/dev/null

# A root supplied through persistent backup.env must match the deploy writer,
# including quoted paths, and take precedence over an inherited environment.
configured_root="$tmp/configured root's literal \$value"
mkdir -p "$configured_root/upgrade"
touch "$configured_root/upgrade/VERIFIED"
printf 'KEYCLOAK_BACKUP_ROOT=%q\nECONGRAPH_DOCKER_HOST=%q\nKIND_CLUSTER=must-not-leak\nCLUSTER_ROLE_CONTEXT=must-not-leak\n' \
    "$configured_root" unix:///must-not-replace-writer.sock >"$ECONGRAPH_CONFIG_DIR/backup.env"
writer_config="$(ECONGRAPH_DOCKER_HOST=unix:///writer.sock CLUSTER_ROLE_CONTEXT=writer-context bash -c '
    source "$1/scripts/deploy/lib/keycloak-db.sh"
    printf "%s\n%s\n%s\n%s" "$KDB_BACKUP_ROOT" "$DOCKER_HOST" "$KIND_CLUSTER" "$CLUSTER_ROLE_CONTEXT"
' bash "$repo_root")"
if [[ "$writer_config" != "$configured_root"$'\n'unix:///writer.sock$'\n'econ-graph$'\n'writer-context ]]; then
    echo "FAIL writer root disagrees with persistent backup config or changed its Docker/context settings"
    failures=$((failures + 1))
fi
expect 0 "" backup
grep -qF -- "$configured_root/upgrade" "$STUB_LOG" || {
    echo "FAIL offsite backup did not use the writer's persistent root"
    failures=$((failures + 1))
}
printf 'KEYCLOAK_BACKUP_ROOT=%q\n' "$tmp/missing-persistent-root" >"$ECONGRAPH_CONFIG_DIR/backup.env"
expect 1 "configured Keycloak backup root is missing" backup
# Clearing an inherited explicit root in the file restores optional semantics;
# ECONGRAPH_BACKUP_ROOT still controls the writer's base, never nightly BACKUP_ROOT.
printf 'KEYCLOAK_BACKUP_ROOT=\nECONGRAPH_BACKUP_ROOT=%q\n' "$tmp/optional-base" >"$ECONGRAPH_CONFIG_DIR/backup.env"
writer_root="$(bash -c 'source "$1/scripts/deploy/lib/keycloak-db.sh"; printf "%s" "$KDB_BACKUP_ROOT"' bash "$repo_root")"
[[ "$writer_root" == "$tmp/optional-base/keycloak" ]] || {
    echo "FAIL writer ignored the persistent base or used the nightly root"
    failures=$((failures + 1))
}
expect 0 "" backup
mkdir -p "$tmp/optional-base/keycloak/upgrade"
touch "$tmp/optional-base/keycloak/upgrade/VERIFIED"
expect 0 "" backup
grep -qF -- "$tmp/optional-base/keycloak/upgrade" "$STUB_LOG" || {
    echo "FAIL offsite backup ignored the persistent writer base"
    failures=$((failures + 1))
}
printf 'return 23\n' >"$ECONGRAPH_CONFIG_DIR/backup.env"
expect 1 "could not read Keycloak backup root configuration" backup
rm "$ECONGRAPH_CONFIG_DIR/backup.env"

# Follow command-line root symlinks, but still exclude unpublished bundles.
ln -s "$KEYCLOAK_BACKUP_ROOT" "$tmp/linked-upgrade-root"
KEYCLOAK_BACKUP_ROOT="$tmp/linked-upgrade-root" expect 0 "" backup
grep -qF -- "$tmp/linked-upgrade-root/upgrade" "$STUB_LOG" || {
    echo "FAIL offsite backup silently omitted a symlink root"
    failures=$((failures + 1))
}
if grep -qF -- "$tmp/linked-upgrade-root/unpublished.partial" "$STUB_LOG"; then
    echo "FAIL symlink-root traversal included an unpublished bundle"
    failures=$((failures + 1))
fi

# An absent optional root is allowed; inability to inspect it is not absence.
mkdir -p "$tmp/inaccessible"
chmod 000 "$tmp/inaccessible"
if [[ "$(id -u)" != 0 ]]; then
    : >"$STUB_LOG"
    env -u KEYCLOAK_BACKUP_ROOT ECONGRAPH_BACKUP_ROOT="$tmp/inaccessible/base" \
        "$script" backup >"$tmp/inaccessible.log" 2>&1 && {
        echo "FAIL inaccessible optional root was treated as absent"
        failures=$((failures + 1))
    }
    grep -q 'could not inspect Keycloak backup root' "$tmp/inaccessible.log" || {
        echo "FAIL inaccessible root had no inspection error"
        failures=$((failures + 1))
    }
    [[ ! -s "$STUB_LOG" ]] || {
        echo "FAIL restic ran despite an inaccessible optional root"
        failures=$((failures + 1))
    }
fi
chmod 700 "$tmp/inaccessible"
# A non-directory ancestor is an inspection failure too, even for a default root.
touch "$tmp/not-a-directory"
: >"$STUB_LOG"
if env -u KEYCLOAK_BACKUP_ROOT ECONGRAPH_BACKUP_ROOT="$tmp/not-a-directory/base" \
    "$script" backup >"$tmp/not-directory.log" 2>&1; then
    echo "FAIL non-directory optional-root ancestor was treated as absent"
    failures=$((failures + 1))
fi
grep -q 'could not inspect Keycloak backup root' "$tmp/not-directory.log" || {
    echo "FAIL non-directory ancestor had no inspection error"
    failures=$((failures + 1))
}
[[ ! -s "$STUB_LOG" ]] || {
    echo "FAIL restic ran despite a non-directory ancestor"
    failures=$((failures + 1))
}
# Failed enumeration of the nightly root must not produce a partial success.
mv "$BACKUP_ROOT" "$BACKUP_ROOT.saved"
expect 1 "could not enumerate all configured backup roots" backup
mv "$BACKUP_ROOT.saved" "$BACKUP_ROOT"

# A failure in the second root must not upload only the successful first root.
real_find="$(command -v find)"
cat >"$tmp/bin/find" <<EOF
#!/usr/bin/env bash
if [[ "\$1" == -H ]]; then shift; fi
if [[ "\$1" == "\$KEYCLOAK_BACKUP_ROOT" ]]; then exit 1; fi
exec "$real_find" "\$@"
EOF
chmod +x "$tmp/bin/find"
expect 1 "could not enumerate all configured backup roots" backup

if [[ "${failures}" -ne 0 ]]; then
    echo "${failures} offsite-restic test(s) failed"
    exit 1
fi
echo "all offsite-restic tests passed"
