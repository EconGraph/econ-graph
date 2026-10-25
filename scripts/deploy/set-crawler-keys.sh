#!/usr/bin/env bash
#
# Enter the crawler API keys (and, with --google, the Google sign-in client)
# into the kind cluster's Secrets without the values ever appearing on screen,
# in shell history, in process arguments, or in an AI agent's context.
#
# Usage (run it yourself, in your own terminal):
#   scripts/deploy/set-crawler-keys.sh [--google] [--no-start-crawler] [--dry-run]
#   scripts/deploy/set-crawler-keys.sh --tunnel [--dry-run]
#
# --tunnel asks only for the Cloudflare Tunnel token (Secret cloudflare-tunnel,
# key token; ECO-457), the one the dashboard shows for the tunnel's connector:
# paste the bare token, not the `cloudflared ... --token` command. Then
# cloudflared is started (or restarted) with it; deploy.sh does the same on
# its next run.
#
# Each key is read with hidden input. Press Enter to keep the value already in
# the cluster. The values go to scripts/deploy/create-secrets.sh through its
# environment, which keeps every other Secret as it is and never prints values.
# create-secrets.sh runs with an environment cut down to the basics plus the
# keys of the selected mode, so credentials exported in your shell for other
# keys (another mode's keys, database or Grafana passwords) are not written.
#
# Afterwards the crawler worker is scaled to one replica and restarted so it
# picks up the keys (skip with --no-start-crawler). With --google, Keycloak is
# restarted and the keycloak-realm-import Job is run again (as deploy.sh
# does), since that Job is what puts the Google client into the realm.
#
# --dry-run prints which keys are currently set (names only) and what would
# happen; it prompts for nothing and writes no key. It may create a temporary
# directory under $TMPDIR (a copy of the cluster's kubectl and kubeconfig),
# removed on exit; it changes nothing else.
#
# The script refuses to run without a terminal on stdin, so values cannot be
# piped in by another program.
set -euo pipefail
# Never trace: under bash -x the export of each entered key would print it.
set +x

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
NAMESPACE="${NAMESPACE:-econ-graph}"

DRY_RUN=false
GOOGLE=false
TUNNEL=false
START_CRAWLER=true
for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
        --google) GOOGLE=true ;;
        --tunnel) TUNNEL=true ;;
        --no-start-crawler) START_CRAWLER=false ;;
        -h | --help)
            sed -n '2,35p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "unknown argument: $arg (see --help)" >&2
            exit 2
            ;;
    esac
done

# shellcheck source=scripts/deploy/lib/kind-env.sh
source "$PROJECT_ROOT/scripts/deploy/lib/kind-env.sh"
check_docker_daemon
ensure_kind_kubectl

if [ "$TUNNEL" = true ] && [ "$GOOGLE" = true ]; then
    die "--tunnel and --google are separate runs; run the script once for each"
fi
if [ "$TUNNEL" = true ] &&
    ! grep -q 'CLOUDFLARE_TUNNEL_TOKEN' "$PROJECT_ROOT/scripts/deploy/create-secrets.sh"; then
    die "this checkout's create-secrets.sh does not know CLOUDFLARE_TUNNEL_TOKEN yet" \
        "(ECO-457); update the checkout first"
fi

CRAWLER_KEYS=(FRED_API_KEY BLS_API_KEY BEA_API_KEY CENSUS_API_KEY)
GOOGLE_KEYS=(KEYCLOAK_GOOGLE_CLIENT_ID KEYCLOAK_GOOGLE_CLIENT_SECRET)
TUNNEL_KEYS=(CLOUDFLARE_TUNNEL_TOKEN)

# Secret and key each variable lands in (see create-secrets.sh).
secret_of() {
    case "$1" in
        KEYCLOAK_*) echo econ-graph-keycloak ;;
        CLOUDFLARE_TUNNEL_TOKEN) echo cloudflare-tunnel ;;
        *) echo crawler-api-keys ;;
    esac
}
key_of() {
    case "$1" in
        FRED_API_KEY) echo fred-api-key ;;
        BLS_API_KEY) echo bls-api-key ;;
        BEA_API_KEY) echo bea-api-key ;;
        CENSUS_API_KEY) echo census-api-key ;;
        KEYCLOAK_GOOGLE_CLIENT_ID) echo google-idp-client-id ;;
        KEYCLOAK_GOOGLE_CLIENT_SECRET) echo google-idp-client-secret ;;
        CLOUDFLARE_TUNNEL_TOKEN) echo token ;;
    esac
}

# Prints "set" or "not set" for a key, never its value.
key_state() {
    local secret key
    secret="$(secret_of "$1")"
    key="$(key_of "$1")"
    if kubectl -n "$NAMESPACE" get secret "$secret" --ignore-not-found \
        -o "go-template={{if index .data \"$key\"}}set{{end}}" 2>/dev/null | grep -q set; then
        echo "set"
    else
        echo "not set"
    fi
}

if [ "$TUNNEL" = true ]; then
    # Only the token: the crawler is not touched.
    vars=("${TUNNEL_KEYS[@]}")
    START_CRAWLER=false
else
    vars=("${CRAWLER_KEYS[@]}")
fi
if [ "$GOOGLE" = true ]; then
    vars+=("${GOOGLE_KEYS[@]}")
fi

echo "==> Keys in namespace ${NAMESPACE}"
for v in "${vars[@]}"; do
    printf '  %-30s %s/%s: %s\n' "$v" "$(secret_of "$v")" "$(key_of "$v")" "$(key_state "$v")"
done

if [ "$DRY_RUN" = true ]; then
    echo "==> Dry run: would prompt (hidden) for each key above, then run create-secrets.sh"
    [ "$START_CRAWLER" = true ] && echo "  then: kubectl -n ${NAMESPACE} scale deployment/crawler-worker --replicas=1 and restart it"
    [ "$GOOGLE" = true ] && echo "  then: kubectl -n ${NAMESPACE} rollout restart deployment/keycloak," \
        "then delete and re-apply job/keycloak-realm-import and wait for it"
    [ "$TUNNEL" = true ] && echo "  then: kubectl -n ${NAMESPACE} scale deployment/cloudflared --replicas=1 and restart it"
    exit 0
fi

if [ ! -t 0 ]; then
    die "stdin is not a terminal; run this yourself so the keys are typed, not piped"
fi

echo "==> Enter each key (input is hidden; Enter keeps the current value)"
entered=0
for v in "${vars[@]}"; do
    value=""
    read -rs -p "  ${v}: " value
    echo
    if [ -n "$value" ]; then
        export "${v}=${value}"
        entered=$((entered + 1))
    else
        unset "$v"
    fi
done
unset value

if [ "$entered" -eq 0 ]; then
    echo "==> No keys entered; nothing changed"
else
    echo "==> Writing ${entered} key(s) through create-secrets.sh"
    # In a subshell: drop every exported variable except the basics and the
    # selected keys, so create-secrets.sh cannot pick up other credentials
    # from the caller's environment. The values stay in the environment (not
    # on any command line). KUBECTL is dropped too, so create-secrets.sh uses
    # the same kind kubectl (on PATH) as this script.
    (
        keep=" PATH HOME USER LOGNAME LANG LC_ALL TERM TMPDIR KUBECONFIG ${vars[*]} "
        while IFS= read -r name; do
            case "$keep" in
                *" $name "*) ;;
                *) [[ "$name" == LC_* ]] || unset "$name" 2>/dev/null || true ;;
            esac
        done < <(compgen -e)
        export NAMESPACE
        exec "$PROJECT_ROOT/scripts/deploy/create-secrets.sh"
    )
fi
for v in "${vars[@]}"; do
    unset "$v"
done

if [ "$START_CRAWLER" = true ] &&
    kubectl -n "$NAMESPACE" get deployment crawler-worker >/dev/null 2>&1; then
    echo "==> Starting the crawler worker with the new keys"
    kubectl -n "$NAMESPACE" scale deployment/crawler-worker --replicas=1
    kubectl -n "$NAMESPACE" rollout restart deployment/crawler-worker
fi
if [ "$GOOGLE" = true ] && [ "$entered" -gt 0 ] &&
    kubectl -n "$NAMESPACE" get deployment keycloak >/dev/null 2>&1; then
    echo "==> Restarting Keycloak for the new Google client"
    kubectl -n "$NAMESPACE" rollout restart deployment/keycloak
    kubectl -n "$NAMESPACE" rollout status deployment/keycloak --timeout=600s
    # The realm-import Job renders the Google client from the Secret into
    # the realm; a restart alone does not re-run it. A finished Job cannot be
    # re-run in place, so the old one goes first (as in deploy.sh).
    echo "==> Re-running the Keycloak realm import for the new Google client"
    kubectl -n "$NAMESPACE" delete job keycloak-realm-import --ignore-not-found
    kubectl -n "$NAMESPACE" apply -f "$PROJECT_ROOT/k8s/manifests/keycloak/realm-import-job.yaml"
    realm_import_status=""
    for _ in $(seq 1 120); do
        realm_import_status="$(kubectl -n "$NAMESPACE" get job keycloak-realm-import \
            -o jsonpath='{.status.conditions[?(@.status=="True")].type}' 2>/dev/null || true)"
        case "$realm_import_status" in
            *Complete* | *Failed*) break ;;
        esac
        sleep 5
    done
    case "$realm_import_status" in
        *Complete*) echo "==> Keycloak realm applied with the new Google client" ;;
        *)
            kubectl -n "$NAMESPACE" logs job/keycloak-realm-import --all-containers || true
            die "the Keycloak realm import did not complete (status: ${realm_import_status:-none})"
            ;;
    esac
fi
if [ "$TUNNEL" = true ] && [ "$entered" -gt 0 ]; then
    if kubectl -n "$NAMESPACE" get deployment cloudflared >/dev/null 2>&1; then
        # deploy.sh leaves it at 0 replicas while the token is missing.
        echo "==> Starting cloudflared with the new token"
        kubectl -n "$NAMESPACE" scale deployment/cloudflared --replicas=1
        kubectl -n "$NAMESPACE" rollout restart deployment/cloudflared
    else
        echo "==> cloudflared is not deployed yet; scripts/deploy/deploy.sh starts it"
    fi
fi

echo "==> Keys now"
for v in "${vars[@]}"; do
    printf '  %-30s %s\n' "$v" "$(key_state "$v")"
done
