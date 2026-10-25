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
#   --unmark   scale cloudflared in econ-graph to 0 replicas (also when there
#              is no marker), then remove the marker (kept if scaling fails),
#              then offer to delete the tunnel token (Secret cloudflare-tunnel)
#
# Before a change it shows the cluster's identity (this host's name, the
# nodes, the API server and the kube-system UID) and asks, at a terminal, for
# the first 8 characters of that UID: context names such as kind-econ-graph are
# the same on every machine, the UID is not. Idempotent: a cluster already
# marked (or, with --unmark, not marked, not running cloudflared and without a
# token) is left as it is. It deletes no data, volumes or clusters: --unmark
# removes the marker and, only if you confirm, the token.
#
# Which cluster: without KUBECTL, the kind cluster econ-graph, through its own
# kubectl and kubeconfig (scripts/deploy/lib/kind-env.sh) on context
# kind-econ-graph; on this host plain kubectl is the microk8s snap wrapper,
# another cluster. With KUBECTL=/path/to/kubectl, that kubectl's current context.

set -euo pipefail

# Sourced before this script's own die/log/confirm, which replace kind-env.sh's.
USE_KIND=false
if [ -z "${KUBECTL:-}" ]; then
    USE_KIND=true
    # shellcheck source=scripts/deploy/lib/kind-env.sh
    source "$(dirname "${BASH_SOURCE[0]}")/lib/kind-env.sh"
fi
KUBECTL="${KUBECTL:-kubectl}"
# Where cloudflared and its token live; fixed, like in lib/public-host.sh.
TUNNEL_NAMESPACE=econ-graph
DRY_RUN=false
UNMARK=false
for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
        --unmark) UNMARK=true ;;
        -h | --help)
            sed -n '2,32p' "$0" | sed 's/^# \{0,1\}//'
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

# Every call below goes through kc, on this one context, never a
# CLUSTER_ROLE_CONTEXT inherited from the environment: kind-econ-graph with the
# kind kubectl, or the current context of an explicit KUBECTL.
unset CLUSTER_ROLE_CONTEXT
if [[ "$USE_KIND" == true ]]; then
    check_docker_daemon
    ensure_kind_kubectl
    KUBECTL="${ECONGRAPH_KUBE_DIR:-${ECONGRAPH_CACHE}}/bin/kubectl"
    cluster_role_use_context "kind-${KIND_CLUSTER}" || die "$KUBECTL has no context kind-${KIND_CLUSTER}"
else
    command -v "$KUBECTL" >/dev/null 2>&1 || die "$KUBECTL not found"
    cluster_role_pin_context || die "$KUBECTL has no current context"
fi
context="$CLUSTER_ROLE_CONTEXT"
uid="$(kube_system_uid)" || die "could not read the kube-system namespace UID of context $context"
marker="$(cluster_role_marker)" ||
    die "could not read ConfigMap $CLUSTER_ROLE_NAMESPACE/$CLUSTER_ROLE_CONFIGMAP of context $context"
code="${uid:0:8}"

# Show what this cluster is, then ask for the first 8 characters of its
# kube-system UID, typed at a terminal: a change that decides whether a cluster
# serves the public site is never made by a pipe or a script.
confirm() {
    local answer nodes server
    [[ -t 0 ]] || die "the confirmation is read from a terminal only; run it yourself"
    nodes="$(kc get nodes -o 'jsonpath={.items[*].metadata.name}' 2>/dev/null)" || nodes="(unknown)"
    server="$(kc config view --minify -o 'jsonpath={.clusters[0].cluster.server}' 2>/dev/null)" ||
        server="(unknown)"
    echo
    echo "  this host       $(hostname 2>/dev/null || echo '(unknown)')"
    echo "  context         $context ($KUBECTL)"
    echo "  API server      ${server:-(unknown)}"
    echo "  nodes           ${nodes:-(none)}"
    echo "  kube-system UID $uid"
    echo
    echo "$1"
    read -r -p "Type the first 8 characters of the kube-system UID above to continue: " answer
    [[ "$answer" == "$code" ]] || die "confirmation did not match; nothing was changed"
}

if [[ "$UNMARK" == true ]]; then
    # cloudflared is stopped first, and even without a marker: once the marker is
    # gone a deploy with PUBLIC_HOST stops before apply_cloudflared, so a
    # connector still running then would never be scaled down by it.
    replicas="$(kc -n "$TUNNEL_NAMESPACE" get deployment cloudflared --ignore-not-found \
        -o 'jsonpath={.spec.replicas}')" ||
        die "could not look up deployment/cloudflared in $TUNNEL_NAMESPACE; nothing was changed"
    token="$(kc -n "$TUNNEL_NAMESPACE" get secret cloudflare-tunnel --ignore-not-found -o name)" ||
        die "could not look up Secret cloudflare-tunnel in $TUNNEL_NAMESPACE; nothing was changed"
    running=false
    [[ -z "$replicas" || "$replicas" == 0 ]] || running=true
    if [[ -z "$marker" && "$running" == false && -z "$token" ]]; then
        log "context $context: not marked production, cloudflared not running, no tunnel token; nothing to do"
        exit 0
    fi
    if cluster_is_marked_production; then
        echo
        echo "!!! WARNING: context $context is the cluster marked production, the one serving"
        echo "!!! econgraph.com now. Unmarking it stops cloudflared and takes the public site down."
        echo "!!! Go on only if another cluster is taking over, or the site is meant to go down."
    fi
    if [[ "$DRY_RUN" == true ]]; then
        [[ "$running" == false ]] ||
            log "dry run: would scale deployment/cloudflared in $TUNNEL_NAMESPACE from $replicas to 0 replicas"
        [[ -z "$marker" ]] ||
            log "dry run: would delete ConfigMap $CLUSTER_ROLE_NAMESPACE/$CLUSTER_ROLE_CONFIGMAP"
        [[ -z "$token" ]] ||
            log "dry run: would offer to delete Secret $TUNNEL_NAMESPACE/cloudflare-tunnel (the tunnel token)"
        exit 0
    fi
    confirm "This cluster will not be production: cloudflared stops, and the public site is no longer served from it."
    if [[ "$running" == true ]]; then
        kc -n "$TUNNEL_NAMESPACE" scale deployment/cloudflared --replicas=0 ||
            die "could not scale deployment/cloudflared in $TUNNEL_NAMESPACE to 0; the marker was kept." \
                "Scale it to 0 yourself, then run --unmark again"
    fi
    if [[ -n "$marker" ]]; then
        kc -n "$CLUSTER_ROLE_NAMESPACE" delete configmap "$CLUSTER_ROLE_CONFIGMAP" --ignore-not-found
    fi
    log "unmarked"
    # The token is a credential, not data: delete it only when asked to, typed.
    if [[ -n "$token" ]]; then
        echo "Secret $TUNNEL_NAMESPACE/cloudflare-tunnel holds the tunnel token. Only the production"
        echo "cluster may hold it: anything that starts cloudflared with it serves part of the site."
        # End of input means keep it.
        read -r -p "Type delete to delete it now, or press Enter to keep it: " answer || answer=""
        if [[ "$answer" == delete ]]; then
            kc -n "$TUNNEL_NAMESPACE" delete secret cloudflare-tunnel --ignore-not-found
            log "deleted Secret $TUNNEL_NAMESPACE/cloudflare-tunnel"
        else
            log "kept Secret $TUNNEL_NAMESPACE/cloudflare-tunnel"
        fi
    fi
    log "deploy with PUBLIC_HOST= (empty) to remove the public routes too"
    exit 0
fi

if cluster_is_marked_production; then
    log "context $context (kube-system UID $uid) is already marked production; nothing to do"
    exit 0
fi
log "$CLUSTER_ROLE_REASON"
if [[ "$DRY_RUN" == true ]]; then
    log "dry run: would write ConfigMap $CLUSTER_ROLE_NAMESPACE/$CLUSTER_ROLE_CONFIGMAP" \
        "(role=production, kube-system-uid=$uid)"
    exit 0
fi
confirm "This cluster will be production: it may then run cloudflared, hold the tunnel token and serve econgraph.com. Mark only the one cluster that serves it."
kc -n "$CLUSTER_ROLE_NAMESPACE" create configmap "$CLUSTER_ROLE_CONFIGMAP" \
    --from-literal=role=production --from-literal="kube-system-uid=$uid" \
    --dry-run=client -o yaml |
    kc apply -f -
cluster_is_marked_production || die "the marker was written but does not check out: $CLUSTER_ROLE_REASON"
log "marked production"
