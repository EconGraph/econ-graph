#!/bin/bash
# Writable clones of a data-plane snapshot, for a branch's local data plane.
#
# Usage:
#   branch-clone.sh create <branch> [SNAPSHOT]   clone SNAPSHOT (default: the
#                                                newest data-plane snapshot)
#   branch-clone.sh list
#   branch-clone.sh delete <branch>
#
# A clone is `btrfs subvolume snapshot` of a read-only snapshot into a new
# writable subvolume under BRANCH_CLONE_DIR (default ~/econgraph-branches,
# which is on the same /home btrfs filesystem, as a snapshot requires). It
# takes no space until the branch writes to it, and production is never
# touched. Point the branch's Postgres (e.g. a docker run of PG_IMAGE with
# -v <clone>/pg:/var/lib/postgresql/data, PGDATA set to match) at <clone>/pg.
#
# No root needed: creating works because jmalicki owns the snapshot roots;
# deleting empties the clone from inside a container on the rootful daemon
# (the Postgres files belong to UID 999, which jmalicki cannot remove
# directly; root in that container is host root) and then removes the empty
# subvolume with rmdir, which the kernel allows for an empty subvolume's
# owner. lib.sh points docker at that daemon.

set -euo pipefail

# shellcheck source=scripts/backup/lib.sh
source "$(dirname "$(readlink -f "$0")")/lib.sh"

BRANCH_CLONE_DIR="${BRANCH_CLONE_DIR:-${HOME}/econgraph-branches}"

slug() { printf '%s' "$1" | tr -cs 'A-Za-z0-9._-' '-' | sed 's/^-*//; s/-*$//'; }

newest_data_plane_snapshot() {
    # Names sort chronologically within each naming scheme; compare the
    # embedded timestamps across both (hourly and snapshot-now).
    find "${SNAPSHOT_DIR}" -mindepth 1 -maxdepth 1 -name 'data-plane.*' -printf '%f\n' 2>/dev/null \
        | sed -E 's/^data-plane\.(pre-)?([0-9]{8}T[0-9]{4,6}).*/\2 &/' | sort | tail -n 1 | cut -d' ' -f2
}

cmd="${1:-}"
case "${cmd}" in
    create)
        [[ $# -ge 2 ]] || die "usage: $(basename "$0") create <branch> [SNAPSHOT]"
        name="$(slug "$2")"
        [[ -n "${name}" ]] || die "bad branch name: $2"
        snap="${3:-}"
        if [[ -z "${snap}" ]]; then
            snap="$(newest_data_plane_snapshot)"
            [[ -n "${snap}" ]] || die "no data-plane snapshots in ${SNAPSHOT_DIR}"
        fi
        [[ "${snap}" == /* ]] || snap="${SNAPSHOT_DIR}/${snap}"
        [[ -d "${snap}" && "$(stat -c %i "${snap}")" == 256 ]] || die "${snap} is not a snapshot"
        mkdir -p "${BRANCH_CLONE_DIR}"
        [[ "$(findmnt -n -o UUID -T "${snap}")" == "$(findmnt -n -o UUID -T "${BRANCH_CLONE_DIR}")" ]] \
            || die "${BRANCH_CLONE_DIR} is not on the same btrfs filesystem as ${snap}"
        dest="${BRANCH_CLONE_DIR}/${name}"
        [[ ! -e "${dest}" ]] || die "${dest} already exists"
        btrfs subvolume snapshot "${snap}" "${dest}" >/dev/null
        printf '%s\n' "${snap}" >"${dest}.source"
        echo "clone: ${dest} (from ${snap})"
        echo "Postgres data directory: ${dest}/pg"
        ;;
    list)
        shopt -s nullglob
        for src in "${BRANCH_CLONE_DIR}"/*.source; do
            echo "${src%.source}  <- $(cat "${src}")"
        done
        ;;
    delete)
        [[ $# -eq 2 ]] || die "usage: $(basename "$0") delete <branch>"
        dest="${BRANCH_CLONE_DIR}/$(slug "$2")"
        [[ -d "${dest}" && "$(stat -c %i "${dest}")" == 256 ]] || die "${dest} is not a clone subvolume"
        [[ "$(btrfs property get "${dest}" ro)" == "ro=false" ]] || die "${dest} is read-only; refusing"
        docker run --rm --network none -v "${dest}:/clone" "${PG_IMAGE}" \
            find /clone -mindepth 1 -delete
        rmdir "${dest}"
        rm -f "${dest}.source"
        echo "deleted ${dest}"
        ;;
    *)
        die "usage: $(basename "$0") create <branch> [SNAPSHOT] | list | delete <branch>"
        ;;
esac
