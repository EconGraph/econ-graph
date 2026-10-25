#!/bin/bash

# List the PersistentVolumes of the kind cluster that are Released (their PVC
# was deleted, e.g. with a namespace) and, on request, delete one of them.
#
# Usage:
#   scripts/deploy/released-pvs.sh [--dry-run]       report only (the default)
#   scripts/deploy/released-pvs.sh --delete PV_NAME  delete that one PV object
#
# The report shows each Released PV's reclaim policy, storage class, the PVC
# it was bound to, where its data is (hostPath or local path), and whether it
# is protected.
#
# --delete deletes only the PV object, after you type its name on a terminal
# (there is no --yes). It refuses a PV that:
#   - is not Released (Bound or Available PVs are in use or about to be);
#   - carries the label econgraph.io/protected=true (the host-storage PVs of
#     k8s/manifests/host-storage.yaml, which apply-host-storage.sh re-arms);
#   - has a reclaim policy other than Retain (deleting it could make the
#     provisioner delete the data).
# With Retain, deleting the PV object leaves its data where it is: this script
# never deletes a directory or a volume's contents, and nothing calls it
# automatically.
#
# A dry run may create a temporary directory under $TMPDIR (a copy of the
# cluster's kubectl and kubeconfig), removed on exit; it changes nothing else.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/kind-env.sh"

DELETE_PV=""
usage="usage: $0 [--dry-run] | --delete PV_NAME"
while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run|-n) DRY_RUN="true"; shift ;;
        --delete)
            [ $# -ge 2 ] || { echo "$usage" >&2; exit 2; }
            DELETE_PV="$2"
            shift 2
            ;;
        -h|--help) sed -n '3,26p' "$0"; exit 0 ;;
        *) echo "$usage" >&2; exit 2 ;;
    esac
done
if [ -n "${DELETE_PV}" ] && [ "${DRY_RUN}" != "true" ] && [ ! -t 0 ]; then
    die "--delete needs the PV name typed on a terminal; nothing was deleted"
fi

check_docker_daemon
ensure_kind_kubectl

# One line per PV: name phase policy class claim protected path. A go-template,
# so it works with whichever kubectl the cluster ships and needs no jq.
# shellcheck disable=SC2016  # $l is a template variable, not a shell one
PV_TEMPLATE='{{range .items}}{{.metadata.name}} {{.status.phase}} {{.spec.persistentVolumeReclaimPolicy}} {{or .spec.storageClassName "-"}} {{with .spec.claimRef}}{{.namespace}}/{{.name}}{{else}}-{{end}} {{with .metadata.labels}}{{or (index . "econgraph.io/protected") "-"}}{{else}}-{{end}} {{with .spec.hostPath}}{{.path}}{{else}}{{with .spec.local}}{{.path}}{{else}}-{{end}}{{end}}
{{end}}'

pvs="$(kubectl get pv -o go-template="${PV_TEMPLATE}")" || die "could not list PersistentVolumes"
released="$(awk '$2 == "Released"' <<<"${pvs}")"

log "Released PersistentVolumes in kind cluster '${KIND_CLUSTER}'"
if [ -z "${released}" ]; then
    echo "  none"
else
    {
        echo "NAME PHASE RECLAIM CLASS WAS-BOUND-TO PROTECTED DATA"
        echo "${released}"
    } | column -t | sed 's/^/  /'
fi

[ -n "${DELETE_PV}" ] || exit 0

read -r name phase policy _class claim protected path < <(awk -v pv="${DELETE_PV}" '$1 == pv' <<<"${pvs}") ||
    die "PV ${DELETE_PV} not found; nothing was deleted"
[ "${phase}" = "Released" ] || die "PV ${name} is ${phase}, not Released; nothing was deleted"
[ "${protected}" != "true" ] ||
    die "PV ${name} is labelled econgraph.io/protected=true; it is never deleted by a script. Nothing was deleted"
[ "${policy}" = "Retain" ] ||
    die "PV ${name} has reclaim policy ${policy}, not Retain; deleting it could delete its data. Nothing was deleted"

echo ""
echo "About to delete PV object ${name} (was bound to ${claim})."
echo "Its data stays where it is: ${path}"
if [ "${DRY_RUN}" = "true" ]; then
    echo "(dry run: would ask you to type '${name}' here)"
else
    read -r -p "Type the PV name (${name}) to delete it: " answer || die "aborted; nothing was deleted"
    [ "${answer}" = "${name}" ] || die "aborted; nothing was deleted"
    # Look again: the PV may have been re-armed, re-labelled or bound while
    # the prompt waited.
    now="$(kubectl get pv "${name}" -o go-template="{{.status.phase}} {{.spec.persistentVolumeReclaimPolicy}} {{with .metadata.labels}}{{or (index . \"econgraph.io/protected\") \"-\"}}{{else}}-{{end}}")" ||
        die "could not read PV ${name} again; nothing was deleted"
    [ "${now}" = "Released Retain ${protected}" ] ||
        die "PV ${name} changed while waiting (now: ${now}); nothing was deleted"
fi
run kubectl delete pv "${name}"
