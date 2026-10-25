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
export STUB_LOG="${tmp}/restic.log"
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

if [[ "${failures}" -ne 0 ]]; then
    echo "${failures} offsite-restic test(s) failed"
    exit 1
fi
echo "all offsite-restic tests passed"
