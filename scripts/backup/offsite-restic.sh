#!/bin/bash
# Off-site copy of the logical dumps with restic.
#
# Usage: offsite-restic.sh backup|check|snapshots|init|restic <args...>
#
#   backup     restic backup of the verified dump directories. Nothing is
#              ever forgotten or pruned: the repository only grows.
#   check      restic check --read-data-subset=5% (spot-reads pack files)
#   snapshots  list restic snapshots
#   init       create the repository (once)
#   restic     run another restic command against the repository
#              (ls, stats, diff, restore, mount, ...). forget and prune
#              are refused: nothing here deletes backups.
#
# The repository location and password are NOT in the repo. They come from
# ~/.config/econgraph/restic.env (mode 600), which must set
#   RESTIC_REPOSITORY=...          e.g. sftp:user@host:/econgraph or s3:...
#   RESTIC_PASSWORD_FILE=...       a file holding only the password (600)
# plus whatever credentials the backend needs (AWS_ACCESS_KEY_ID, B2_*, ...).
# No off-site target has been chosen yet; until that file exists this script
# exits 2 and says so. See docs/deployment/BACKUPS.md, "Off-site".

set -euo pipefail

# shellcheck source=scripts/backup/lib.sh
source "$(dirname "$(readlink -f "$0")")/lib.sh"

# Anything restic writes here (cache, `restic restore` of a dump) is
# private: files 0600, directories 0700.
umask 077

RESTIC_ENV="${RESTIC_ENV:-${ECONGRAPH_CONFIG_DIR}/restic.env}"

if [[ ! -f "${RESTIC_ENV}" ]]; then
    cat >&2 <<EOF
offsite-restic: not configured (no ${RESTIC_ENV}).
No off-site target has been chosen yet. To configure one:
  mkdir -p ${ECONGRAPH_CONFIG_DIR} && chmod 700 ${ECONGRAPH_CONFIG_DIR}
  umask 077
  head -c 32 /dev/urandom | base64 > ${ECONGRAPH_CONFIG_DIR}/restic.password
  cat > ${RESTIC_ENV} <<'END'
RESTIC_REPOSITORY=<sftp:host:/path | s3:https://endpoint/bucket/econgraph | b2:bucket:econgraph | rest:https://...>
RESTIC_PASSWORD_FILE=${ECONGRAPH_CONFIG_DIR}/restic.password
# backend credentials, e.g. AWS_ACCESS_KEY_ID=... AWS_SECRET_ACCESS_KEY=...
END
  $(basename "$0") init
Keep a copy of the password somewhere that is not this machine: without it
the off-site copy is unreadable.
EOF
    exit 2
fi

perms="$(stat -c %a "${RESTIC_ENV}")"
[[ "${perms}" =~ ^[0-7]00$ ]] || die "${RESTIC_ENV} is mode ${perms}; chmod 600 it"

set -a
# shellcheck disable=SC1090
source "${RESTIC_ENV}"
set +a
[[ -n "${RESTIC_REPOSITORY:-}" ]] || die "RESTIC_REPOSITORY is not set in ${RESTIC_ENV}"
[[ -n "${RESTIC_PASSWORD_FILE:-}${RESTIC_PASSWORD:-}${RESTIC_PASSWORD_COMMAND:-}" ]] \
    || die "set RESTIC_PASSWORD_FILE (preferred) in ${RESTIC_ENV}"
command -v restic >/dev/null || die "restic is not installed"

cmd="${1:-}"
[[ -n "${cmd}" ]] || die "usage: $(basename "$0") backup|check|snapshots|init|restic <args>"
shift

case "${cmd}" in
    init)
        restic init
        ;;
    backup)
        mapfile -t dirs < <(find "${BACKUP_ROOT}" -mindepth 2 -maxdepth 2 -name VERIFIED -printf '%h\n' | sort)
        (( ${#dirs[@]} > 0 )) || die "no verified dump directories under ${BACKUP_ROOT}"
        log "restic backup of ${#dirs[@]} verified dump dir(s) to ${RESTIC_REPOSITORY}"
        restic backup --tag econgraph-dumps --host "$(hostname)" "${dirs[@]}"
        ;;
    check)
        restic check --read-data-subset="${RESTIC_CHECK_SUBSET:-5%}"
        ;;
    snapshots)
        restic snapshots --tag econgraph-dumps "$@"
        ;;
    restic)
        case "${1:-}" in
            forget|prune|key|unlock|migrate|repair|rewrite|tag)
                die "restic $1 is refused by this wrapper; run restic directly if Joe decides to"
                ;;
        esac
        restic "$@"
        ;;
    *)
        die "unknown command: ${cmd}"
        ;;
esac
