#!/usr/bin/env bash
#
# Mark the current kubectl context's cluster as production, the only cluster
# that may run the Cloudflare Tunnel connector (cloudflared) and hold its token.
#
# Usage:
#   scripts/deploy/mark-production-cluster.sh [--dry-run] [--unmark]
#
# Run it once after creating or recreating the production cluster, before
# deploying with PUBLIC_HOST or entering the tunnel token. It writes the
# ConfigMap kube-system/econgraph-cluster-role (role=production, plus the UID of
# the cluster's kube-system namespace), which scripts/deploy/lib/cluster-role.sh
# checks. A marker copied or restored into another cluster carries the other
# cluster's UID and does not count.
#
#   --dry-run  show what would change, change nothing (no confirmation needed)
#   --unmark   scale cloudflared to 0 replicas (also when there is no marker),
#              then remove the marker; it is kept if scaling fails
#
# Changes ask for the context name to be typed from a terminal. Idempotent: a
# cluster already marked (or, with --unmark, not marked and not running
# cloudflared) is left as it is. It deletes no data, volumes or clusters:
# --unmark removes only the marker.
#
# Other environment: KUBECTL (default kubectl), NAMESPACE (where cloudflared
# runs, default econ-graph).

set -euo pipefail

KUBECTL="${KUBECTL:-kubectl}"
NAMESPACE="${NAMESPACE:-econ-graph}"
DRY_RUN=false
UNMARK=false
for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
        --unmark) UNMARK=true ;;
        -h | --help)
            sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "mark-production-cluster: error: unknown argument: $arg" >&2
            exit 2
            ;;
    esac
done

die() {
    echo "mark-production-cluster: error: $*" >&2
    exit 1
}

log() {
    echo "mark-production-cluster: $*"
}

# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/cluster-role.sh"

command -v "$KUBECTL" >/dev/null 2>&1 || die "$KUBECTL not found"

context="$("$KUBECTL" config current-context)" || die "no current kubectl context"
[[ -n "$context" ]] || die "no current kubectl context"
uid="$(kube_system_uid)" || die "could not read the kube-system namespace UID of context $context"
marker="$(cluster_role_marker)" ||
    die "could not read ConfigMap $CLUSTER_ROLE_NAMESPACE/$CLUSTER_ROLE_CONFIGMAP of context $context"
log "context $context, kube-system UID $uid"

# Ask for the context name, typed at a terminal: a change that decides whether a
# cluster serves the public site is never made by a pipe or a script.
confirm() {
    local answer
    [[ -t 0 ]] || die "the confirmation is read from a terminal only; run it yourself"
    echo "$1"
    read -r -p "Type the context name ($context) to continue: " answer
    [[ "$answer" == "$context" ]] || die "confirmation did not match; nothing was changed"
}

if [[ "$UNMARK" == true ]]; then
    # cloudflared is stopped first, and even without a marker: once the marker is
    # gone a deploy with PUBLIC_HOST stops before apply_cloudflared, so a
    # connector still running then would never be scaled down by it.
    replicas="$("$KUBECTL" -n "$NAMESPACE" get deployment cloudflared --ignore-not-found \
        -o 'jsonpath={.spec.replicas}')" ||
        die "could not look up deployment/cloudflared in $NAMESPACE; nothing was changed"
    running=false
    [[ -z "$replicas" || "$replicas" == 0 ]] || running=true
    if [[ -z "$marker" && "$running" == false ]]; then
        log "not marked production and cloudflared is not running; nothing to do"
        exit 0
    fi
    if [[ "$DRY_RUN" == true ]]; then
        [[ "$running" == false ]] ||
            log "dry run: would scale deployment/cloudflared in $NAMESPACE from $replicas to 0 replicas"
        [[ -z "$marker" ]] ||
            log "dry run: would delete ConfigMap $CLUSTER_ROLE_NAMESPACE/$CLUSTER_ROLE_CONFIGMAP"
        exit 0
    fi
    confirm "Context $context will no longer be production: cloudflared stops, and the public site is no longer served from it."
    if [[ "$running" == true ]]; then
        "$KUBECTL" -n "$NAMESPACE" scale deployment/cloudflared --replicas=0 ||
            die "could not scale deployment/cloudflared in $NAMESPACE to 0; the marker was kept." \
                "Scale it to 0 yourself, then run --unmark again"
    fi
    if [[ -n "$marker" ]]; then
        "$KUBECTL" -n "$CLUSTER_ROLE_NAMESPACE" delete configmap "$CLUSTER_ROLE_CONFIGMAP" --ignore-not-found
    fi
    log "unmarked; deploy with PUBLIC_HOST= (empty) to remove the public routes too"
    exit 0
fi

if cluster_is_marked_production; then
    log "already marked production; nothing to do"
    exit 0
fi
log "$CLUSTER_ROLE_REASON"
if [[ "$DRY_RUN" == true ]]; then
    log "dry run: would write ConfigMap $CLUSTER_ROLE_NAMESPACE/$CLUSTER_ROLE_CONFIGMAP" \
        "(role=production, kube-system-uid=$uid)"
    exit 0
fi
confirm "Context $context will be production: it may then run cloudflared and hold the tunnel token, and serve the public site. Mark only the one cluster that serves it."
"$KUBECTL" -n "$CLUSTER_ROLE_NAMESPACE" create configmap "$CLUSTER_ROLE_CONFIGMAP" \
    --from-literal=role=production --from-literal="kube-system-uid=$uid" \
    --dry-run=client -o yaml |
    "$KUBECTL" apply -f -
cluster_is_marked_production || die "the marker was written but does not check out: $CLUSTER_ROLE_REASON"
log "marked production"
