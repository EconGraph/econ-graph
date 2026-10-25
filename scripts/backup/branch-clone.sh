#!/bin/bash
# Writable clones of a data-plane snapshot, for a branch's local data plane.
#
# Usage:
#   branch-clone.sh [--dry-run] create <branch> [SNAPSHOT]
#                                     clone SNAPSHOT (default: the newest
#                                     data-plane snapshot)
#   branch-clone.sh list
#   branch-clone.sh [--dry-run] [--yes] delete <branch>
#
# --dry-run (-n) runs every check, prints the plan and changes nothing.
# delete asks you to type the clone's name and refuses without a terminal;
# --yes (-y) skips that question (for scripted use only).
#
# A clone is `btrfs subvolume snapshot` of a read-only snapshot into a new
# writable subvolume under BRANCH_CLONE_DIR (default ~/econgraph-branches,
# which is on the same /home btrfs filesystem, as a snapshot requires). It
# takes no space until the branch writes to it, and production is never
# touched. Point the branch's Postgres (e.g. a docker run of PG_IMAGE with
# -v <clone>/pg:/var/lib/postgresql/data, PGDATA set to match) at <clone>/pg.
#
# create writes <clone>.source (the snapshot it came from) next to the clone.
# delete only touches a direct child of BRANCH_CLONE_DIR (not a symlink)
# that has that marker, is a writable btrfs subvolume, and whose name you
# typed back.
#
# No root needed: creating works because jmalicki owns the snapshot roots;
# deleting empties the clone from inside a container on the rootful daemon
# (the Postgres files belong to UID 999, which jmalicki cannot remove
# directly; root in that container is host root) and then removes the empty
# subvolume with rmdir, which the kernel allows for an empty subvolume's
# owner. lib.sh points docker at that daemon.
#
# Tests: scripts/backup/test-branch-clone.sh

set -euo pipefail

# shellcheck source=scripts/backup/lib.sh
source "$(dirname "$(readlink -f "$0")")/lib.sh"

BRANCH_CLONE_DIR="${BRANCH_CLONE_DIR:-${HOME}/econgraph-branches}"

usage() {
    die "usage: $(basename "$0") [--dry-run] create <branch> [SNAPSHOT] | list | [--dry-run] [--yes] delete <branch>"
}

slug() { printf '%s' "$1" | tr -cs 'A-Za-z0-9._-' '-' | sed 's/^-*//; s/-*$//'; }

# A clone name must be one ordinary path component: not empty, no '/', and
# not starting with '.' (which rules out '.' and '..').
check_clone_name() {
    local n="$1"
    [[ -n "${n}" ]] || die "empty clone name"
    [[ "${n}" != */* ]] || die "bad clone name '${n}': contains '/'"
    [[ "${n}" != .* ]] || die "bad clone name '${n}': starts with '.'"
}

newest_data_plane_snapshot() {
    # Names sort chronologically within each naming scheme; compare the
    # embedded timestamps across both (hourly and snapshot-now). Only
    # directories named data-plane.[pre-]YYYYMMDDTHHMM[SS]... are candidates;
    # anything else under SNAPSHOT_DIR is ignored.
    find "${SNAPSHOT_DIR}" -mindepth 1 -maxdepth 1 -type d -name 'data-plane.*' -printf '%f\n' 2>/dev/null \
        | sed -nE 's/^data-plane\.(pre-)?([0-9]{8}T[0-9]{4,6})([^0-9].*)?$/\2 &/p' \
        | sort | tail -n 1 | cut -d' ' -f2
}

DRY_RUN="false"
YES="false"
args=()
for a in "$@"; do
    case "$a" in
        --dry-run|-n) DRY_RUN="true" ;;
        --yes|-y) YES="true" ;;
        -h|--help) sed -n '2,34p' "$0"; exit 0 ;;
        -*) usage ;;
        *) args+=("$a") ;;
    esac
done
set -- "${args[@]+"${args[@]}"}"

cmd="${1:-}"
case "${cmd}" in
    create)
        [[ $# -ge 2 && $# -le 3 ]] || usage
        name="$(slug "$2")"
        check_clone_name "${name}"
        dest="${BRANCH_CLONE_DIR}/${name}"
        # -L as well as -e: a dangling symlink fails -e, and writing the
        # marker through one would create a file outside BRANCH_CLONE_DIR.
        [[ ! -e "${dest}" && ! -L "${dest}" && ! -e "${dest}.source" && ! -L "${dest}.source" ]] \
            || die "${dest} (or ${dest}.source) already exists, possibly as a symlink"
        snap="${3:-}"
        if [[ -z "${snap}" ]]; then
            snap="$(newest_data_plane_snapshot)"
            [[ -n "${snap}" ]] || die "no data-plane snapshots in ${SNAPSHOT_DIR}"
        fi
        [[ "${snap}" == /* ]] || snap="${SNAPSHOT_DIR}/${snap}"
        [[ -d "${snap}" && "$(stat -c %i "${snap}")" == 256 ]] || die "${snap} is not a snapshot"
        # findmnt needs a path that exists; before the first clone that is
        # BRANCH_CLONE_DIR's parent.
        fs_probe="${BRANCH_CLONE_DIR}"
        [[ -d "${fs_probe}" ]] || fs_probe="$(dirname "${BRANCH_CLONE_DIR}")"
        [[ "$(findmnt -n -o UUID -T "${snap}")" == "$(findmnt -n -o UUID -T "${fs_probe}")" ]] \
            || die "${BRANCH_CLONE_DIR} is not on the same btrfs filesystem as ${snap}"
        if [[ "${DRY_RUN}" == "true" ]]; then
            [[ -d "${BRANCH_CLONE_DIR}" ]] || echo "+ mkdir -p ${BRANCH_CLONE_DIR}"
            echo "+ btrfs subvolume snapshot ${snap} ${dest}"
            echo "+ write ${dest}.source"
            echo "(dry run: nothing created)"
            exit 0
        fi
        mkdir -p "${BRANCH_CLONE_DIR}"
        btrfs subvolume snapshot "${snap}" "${dest}" >/dev/null
        # noclobber: refuse to follow or overwrite anything that appeared
        # at the marker path since the check above.
        (set -o noclobber; printf '%s\n' "${snap}" >"${dest}.source")
        echo "clone: ${dest} (from ${snap})"
        echo "Postgres data directory: ${dest}/pg"
        ;;
    list)
        [[ $# -eq 1 ]] || usage
        shopt -s nullglob
        for src in "${BRANCH_CLONE_DIR}"/*.source; do
            echo "${src%.source}  <- $(cat "${src}")"
        done
        ;;
    delete)
        [[ $# -eq 2 ]] || usage
        # Used exactly as given, not slugged: what is deleted must be what
        # was typed.
        name="$2"
        check_clone_name "${name}"
        [[ -d "${BRANCH_CLONE_DIR}" ]] || die "${BRANCH_CLONE_DIR} does not exist"
        base="$(realpath -e "${BRANCH_CLONE_DIR}")"
        [[ ! -L "${base}/${name}" ]] || die "${base}/${name} is a symlink; refusing"
        [[ -e "${base}/${name}" ]] || die "no clone named ${name} in ${base}"
        dest="$(realpath -e "${base}/${name}")"
        # Strict child of base, after resolving every symlink.
        [[ "${dest}" != "${base}" && "$(dirname "${dest}")" == "${base}" ]] \
            || die "${base}/${name} resolves to ${dest}, not a direct child of ${base}; refusing"
        [[ -f "${dest}.source" && ! -L "${dest}.source" && -s "${dest}.source" ]] \
            || die "${dest}.source is missing, so branch-clone.sh create did not make ${dest}; refusing"
        [[ -d "${dest}" && "$(stat -c %i "${dest}")" == 256 ]] \
            || die "${dest} is not a btrfs subvolume; refusing"
        [[ "$(btrfs property get "${dest}" ro 2>/dev/null)" == "ro=false" ]] \
            || die "${dest} is read-only (or not btrfs); refusing"

        echo "Will PERMANENTLY delete the branch clone ${dest}"
        echo "  (cloned from $(head -n 1 "${dest}.source")) and ${dest}.source:"
        echo "+ docker run --rm --network none -v ${dest}:/clone ${PG_IMAGE} find /clone -xdev -mindepth 1 -delete"
        echo "+ rmdir ${dest}"
        echo "+ rm ${dest}.source"
        if [[ "${DRY_RUN}" == "true" ]]; then
            echo "(dry run: nothing deleted)"
            exit 0
        fi
        if [[ "${YES}" != "true" ]]; then
            [[ -t 0 ]] || die "no terminal to confirm on; refusing (pass --yes only if you mean it)"
            read -r -p "Type the clone name (${name}) to delete it: " answer
            [[ "${answer}" == "${name}" ]] || die "aborted; nothing deleted"
        fi
        docker run --rm --network none -v "${dest}:/clone" "${PG_IMAGE}" \
            find /clone -xdev -mindepth 1 -delete
        rmdir "${dest}"
        rm -f "${dest}.source"
        echo "deleted ${dest}"
        ;;
    *)
        usage
        ;;
esac
