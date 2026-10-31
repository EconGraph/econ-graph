#!/usr/bin/env bash
# Switch the canonical public host, roll it back, or make its redirects permanent,
# without changing release images. Run from the deployed checkout with an operator
# kubeconfig; see docs/deployment/MAIN_HOST.md.
#
# Usage: switch-main-host.sh --context NAME --host BASE [--main-host NAME]
#                            [--finalize] [--confirm-production UID-PREFIX] [--dry-run]
#
#   --main-host NAME   BASE or www.BASE (default www.BASE); --main-host BASE rolls back.
#   --finalize         After the switch is accepted: make the redirects to the main host
#                      permanent (301) by persisting MAIN_HOST_PERMANENT. Until then the
#                      redirects to www.BASE are temporary (302), so browsers and
#                      Cloudflare do not keep them after a rollback. Redirects to the
#                      apex are always permanent. Idempotent.
#   --confirm-production UID-PREFIX
#                      The first 8 characters of the production cluster's kube-system
#                      UID, shown in the cluster identity this script prints. Required
#                      for every run except --dry-run.
#   --dry-run          Contacts no cluster or network and reads or writes no deploy config.
#
# DNS and tunnel routes keep the existing two public names. A real run persists
# PUBLIC_HOST, MAIN_HOST and MAIN_HOST_PERMANENT in ECONGRAPH_DEPLOY_ENV (or the normal
# deploy.env) for later deploys. A switch to another main host drops a
# MAIN_HOST_PERMANENT that names a different host, so its redirects start temporary.
#
# The switch moves Keycloak's issuer first, the KC_HOSTNAME rollout and then the realm
# import that sets the realm's frontendUrl (which overrides KC_HOSTNAME), and only then
# the backend's OIDC_ISSUER. The backend then rejects tokens for the new issuer only
# from the end of the realm import until its own rollout completes (old pods), instead
# of until the import. A rollback runs the same steps in the same order.
set -euo pipefail
set +x
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DRY_RUN=false
FINALIZE=false
CONTEXT=""
HOST=""
MAIN=""
CONFIRM_PRODUCTION=""
usage() {
    echo "Usage: $0 --context NAME --host BASE [--main-host NAME] [--finalize] [--confirm-production UID-PREFIX] [--dry-run]"
}
die() { echo "error: $*" >&2; exit 1; }
while [ $# -gt 0 ]; do
    case "$1" in
        --context|--host|--main-host|--confirm-production)
            if [ $# -lt 2 ] || [ -z "$2" ]; then die "$1 needs a value"; fi
            case "$1" in
                --context) CONTEXT="$2" ;;
                --host) HOST="$2" ;;
                --main-host) MAIN="$2" ;;
                --confirm-production) CONFIRM_PRODUCTION="$2" ;;
            esac
            shift ;;
        --finalize) FINALIZE=true ;;
        --dry-run) DRY_RUN=true ;;
        -h|--help) usage; exit 0 ;;
        *) usage >&2; die "unknown argument $1" ;;
    esac
    shift
done
if [ -z "$CONTEXT" ] || [ -z "$HOST" ]; then usage >&2; exit 2; fi
# Host names are case-insensitive; every comparison and write uses the lowercase form.
HOST="${HOST,,}"
MAIN="${MAIN:-www.${HOST}}"
MAIN="${MAIN,,}"
[ "$MAIN" = "$HOST" ] || [ "$MAIN" = "www.${HOST}" ] || die "main host must be BASE or www.BASE"
if [ "$FINALIZE" = true ] && [ "$MAIN" = "$HOST" ]; then
    die "the redirects to the apex are always permanent; --finalize applies to www.${HOST} only"
fi
export PUBLIC_HOST="$HOST" MAIN_HOST="$MAIN"
# This validation is local, including for a dry run.
"$PROJECT_ROOT/scripts/deploy/render-public-host.py" "$PUBLIC_HOST" /dev/null >/dev/null
# shellcheck source=scripts/deploy/lib/deploy-config.sh
source "$PROJECT_ROOT/scripts/deploy/lib/deploy-config.sh"
CONFIG_FILE="$(deploy_config_path)"
echo "Main address: https://${MAIN_HOST}; context: ${CONTEXT}"
echo "DNS: keep the existing ${PUBLIC_HOST} and www.${PUBLIC_HOST} records; no change required."
echo "Tunnel: keep both existing host routes and their gateway origin; no change required."
if [ "$DRY_RUN" = true ]; then
    echo "[dry-run] verify marked-production context, --confirm-production UID prefix and matching realm ConfigMap"
    if [ "$FINALIZE" = true ]; then
        echo "[dry-run] verify the switch to ${MAIN_HOST} has completed: no pending sign-out marker, Keycloak hostname, realm frontendUrl, backend issuer and rollout, deploy config, served routes, and check-public-site.sh with the temporary redirects"
        echo "[dry-run] atomically retain other settings and write MAIN_HOST_PERMANENT=${MAIN_HOST} in ${CONFIG_FILE}"
        echo "[dry-run] apply gateway routes with permanent (301) redirects to ${MAIN_HOST}"
    else
        echo "[dry-run] atomically retain other settings and write PUBLIC_HOST/MAIN_HOST in ${CONFIG_FILE}; drop a MAIN_HOST_PERMANENT naming another host"
        echo "[dry-run] if issuer changes: mark deployment/keycloak so the sign-out below survives a rerun; any marker left by an earlier run also signs out"
        echo "[dry-run] patch Keycloak hostname/client origins and wait for its rollout; preserve release images"
        echo "[dry-run] rerun the catalog-preserving realm import, moving the realm frontendUrl to ${MAIN_HOST}"
        echo "[dry-run] if marked: sign out every realm session once, then clear the mark"
        echo "[dry-run] then patch the backend issuer; keep both public CORS origins plus http://localhost; roll the app on a config change (app-config hash) and wait"
        echo "[dry-run] apply canonical gateway routes (redirects $( [ "$MAIN_HOST" = "$PUBLIC_HOST" ] && echo 'permanent, 301' || echo 'temporary, 302, until --finalize'))"
    fi
    echo "[dry-run] check-public-site.sh --host ${PUBLIC_HOST} --main-host ${MAIN_HOST}"
    exit 0
fi
# Credential-handling and live writes are operator-only.
# shellcheck source=scripts/deploy/lib/agent-access.sh
source "$PROJECT_ROOT/scripts/deploy/lib/agent-access.sh"
require_operator_session
# kc and the production checks (lib/cluster-role.sh), public_main_host_in_cluster.
# shellcheck source=scripts/deploy/lib/public-host.sh
source "$PROJECT_ROOT/scripts/deploy/lib/public-host.sh"
# roll_app_on_config_change, through the context-pinned kubectl below.
# shellcheck source=scripts/deploy/lib/app-config.sh
source "$PROJECT_ROOT/scripts/deploy/lib/app-config.sh"
cluster_role_operator_context "$CONTEXT" || die "could not verify operator context"
cluster_is_marked_production || die "${CLUSTER_ROLE_REASON}"
# Every kubectl call, including lib/app-config.sh's, goes to the pinned context.
kubectl() { kc "$@"; }
# The production cluster, named by its kube-system UID as release-deploy.sh does:
# signing out every user and changing the issuer is never a context name alone.
cluster_uid="$(kube_system_uid)" || die "could not read the kube-system UID; nothing was changed"
[ -n "$CONFIRM_PRODUCTION" ] ||
    die "the production cluster needs --confirm-production with the first 8 characters of its kube-system UID (shown above); nothing was changed"
[ "$CONFIRM_PRODUCTION" = "${cluster_uid:0:8}" ] ||
    die "--confirm-production ${CONFIRM_PRODUCTION} does not match ${CONTEXT}'s kube-system UID; nothing was changed"
# Refuse another public domain, or a checkout that would apply a different realm.
configured="$(deploy_config_value PUBLIC_HOST 'public host')" || exit 1
[ -z "$configured" ] || [ "${configured,,}" = "$PUBLIC_HOST" ] || die "deploy config PUBLIC_HOST=${configured} differs from --host"
configured_main="$(deploy_config_value MAIN_HOST 'host name')" || exit 1
configured_permanent="$(deploy_config_value MAIN_HOST_PERMANENT 'host name')" || exit 1
configured_main="${configured_main,,}"
configured_permanent="${configured_permanent,,}"
for entry in \
    'econ-graph-realm.json=config/keycloak/econ-graph-realm.json' \
    'render-realm.sh=scripts/keycloak/render-realm.sh' \
    'preserve-catalog-grants.py=scripts/keycloak/preserve-catalog-grants.py'; do
    key="${entry%%=*}"
    file="${entry#*=}"
    live="$(kc -n econ-graph get configmap keycloak-realm -o "go-template={{index .data \"${key}\"}}")" || die "could not read keycloak-realm"
    [ "$live" = "$(cat "$PROJECT_ROOT/$file")" ] || die "keycloak-realm ${key} differs from this checkout; use the deployed checkout or deploy the matching release first"
done
current_hostname="$(kc -n econ-graph get deployment keycloak -o 'jsonpath={.spec.template.spec.containers[?(@.name=="keycloak")].env[?(@.name=="KC_HOSTNAME")].value}')"
[ -n "$current_hostname" ] || die "Keycloak's current hostname is missing"

# write_config NAME=VALUE... NAME...: rewrite the deploy config atomically, preserving
# every other line and the file mode: each NAME=VALUE replaces that name's lines, and a
# bare NAME removes them. A persisted target makes a partial run recoverable by rerunning.
write_config() {
    python3 - "$CONFIG_FILE" "$@" <<'PY'
import os
import pathlib
import re
import stat
import sys
import tempfile
path = pathlib.Path(sys.argv[1])
updates = sys.argv[2:]
names = [u.split("=", 1)[0] for u in updates]
if path.is_symlink():
    raise SystemExit("deploy config must not be a symlink")
text = path.read_bytes().decode("utf-8") if path.exists() else ""
pattern = re.compile(r"^\s*(?:export\s+)?(?:%s)=" % "|".join(map(re.escape, names)))
text = "".join(line for line in text.splitlines(keepends=True) if not pattern.match(line))
if text and not text.endswith("\n"):
    text += "\n"
text += "".join(u + "\n" for u in updates if "=" in u)
path.parent.mkdir(parents=True, exist_ok=True)
mode = stat.S_IMODE(path.stat().st_mode) if path.exists() else 0o600
fd, temporary = tempfile.mkstemp(prefix=".deploy.env.", dir=path.parent)
try:
    with os.fdopen(fd, "w") as output:
        output.write(text)
        output.flush()
        os.fsync(output.fileno())
    os.chmod(temporary, mode)
    os.replace(temporary, path)
finally:
    if os.path.exists(temporary):
        os.unlink(temporary)
PY
}

# Authenticate inside the pod; no password or access token reaches this host.
# Its temporary CLI config is removed even if an admin command fails.
keycloak_admin() {
    # shellcheck disable=SC2016 # expanded in the Keycloak pod, never on the host
    kc -n econ-graph exec deployment/keycloak -- /bin/bash -ceu '
        config=$(mktemp)
        trap '\''rm -f "$config"'\'' EXIT
        cli=/opt/keycloak/bin/kcadm.sh
        "$cli" config credentials --server http://localhost:8080/idp --realm master \
            --user "$KC_BOOTSTRAP_ADMIN_USERNAME" --password "$KC_BOOTSTRAP_ADMIN_PASSWORD" \
            --config "$config" >/dev/null
        "$cli" "$@" --config "$config"
    ' -- "$@"
}
# realm_frontend_url: the realm's frontendUrl attribute, without a trailing slash, or
# nothing. kcadm's `--fields attributes` answers an empty object on Keycloak 26; only
# naming the attribute returns it (scripts/keycloak/test-realm-import.sh runs this query
# against a real Keycloak). Reads only that attribute, never credentials or tokens.
realm_frontend_url() {
    local answer
    answer="$(keycloak_admin get realms/econ-graph --fields 'attributes(frontendUrl)')" || return 1
    python3 -c 'import json, sys; print((json.loads(sys.argv[1]).get("attributes") or {}).get("frontendUrl", "").rstrip("/"))' "$answer"
}
# A legacy frontendUrl overrides KC_HOSTNAME, so it participates in the logout decision too.
current_frontend_url="$(realm_frontend_url)" || die "could not read the realm frontendUrl; nothing was changed"
LOGOUT_ANNOTATION="econgraph.io/issuer-change-logout-pending"
logout_pending="$(kc -n econ-graph get deployment keycloak -o "jsonpath={.metadata.annotations.${LOGOUT_ANNOTATION//./\\.}}")" ||
    die "could not read deployment/keycloak; nothing was changed"
kc -n econ-graph get deployment econ-graph-backend >/dev/null
if [ "$FINALIZE" = true ]; then
    # Only a completed switch is made permanent: Keycloak's hostname, the realm's
    # frontendUrl, the backend's issuer and rollout, the deploy config and the served
    # routes are all on MAIN_HOST, and the public site passes with its temporary
    # redirects, before anything is written.
    finalize_refuse() { die "$1: run the switch to ${MAIN_HOST} first (or rerun it); nothing was changed"; }
    # A sign-out marker means a switch stopped before its sign-out; the rerun finishes it.
    [ -z "$logout_pending" ] ||
        finalize_refuse "a sign-out for ${logout_pending} is still pending (${LOGOUT_ANNOTATION} on deployment/keycloak)"
    [ "$current_hostname" = "https://${MAIN_HOST}/idp" ] ||
        finalize_refuse "Keycloak's hostname is ${current_hostname}, not https://${MAIN_HOST}/idp"
    [ "$current_frontend_url" = "https://${MAIN_HOST}/idp" ] ||
        finalize_refuse "the realm frontendUrl is '${current_frontend_url}', not https://${MAIN_HOST}/idp"
    backend_issuer="$(kc -n econ-graph get deployment econ-graph-backend -o 'jsonpath={.spec.template.spec.containers[*].env[?(@.name=="OIDC_ISSUER")].value}')" ||
        die "could not read the backend's issuer; nothing was changed"
    [ "$backend_issuer" = "https://${MAIN_HOST}/idp/realms/econ-graph" ] ||
        finalize_refuse "the backend expects the issuer '${backend_issuer}'"
    [ "$configured_main" = "$MAIN_HOST" ] ||
        finalize_refuse "deploy config MAIN_HOST is '${configured_main}', not ${MAIN_HOST}"
    served_main="$(public_main_host_in_cluster "$PUBLIC_HOST")" ||
        die "could not read the served main host; nothing was changed"
    [ "$served_main" = "$MAIN_HOST" ] ||
        finalize_refuse "the gateway serves ${served_main}, not ${MAIN_HOST}"
    kc -n econ-graph rollout status deployment/econ-graph-backend --timeout=60s ||
        finalize_refuse "the backend's rollout has not completed"
    MAIN_HOST_PERMANENT="$configured_permanent" "$PROJECT_ROOT/scripts/deploy/check-public-site.sh" \
        --host "$PUBLIC_HOST" --main-host "$MAIN_HOST" ||
        finalize_refuse "the public site does not pass check-public-site.sh as it is"
    write_config "PUBLIC_HOST=${PUBLIC_HOST}" "MAIN_HOST=${MAIN_HOST}" "MAIN_HOST_PERMANENT=${MAIN_HOST}"
    export MAIN_HOST_PERMANENT="$MAIN_HOST"
    "$PROJECT_ROOT/scripts/deploy/apply-gateway-routes.sh" --context "$CONTEXT"
    "$PROJECT_ROOT/scripts/deploy/check-public-site.sh" --host "$PUBLIC_HOST" --main-host "$MAIN_HOST"
    echo "Redirects to https://${MAIN_HOST} are permanent (301). Purge the Cloudflare cache for ${PUBLIC_HOST} and www.${PUBLIC_HOST} (docs/deployment/MAIN_HOST.md)."
    exit 0
fi
current_cors="$(kc -n econ-graph get configmap econ-graph-config -o 'jsonpath={.data.CORS_ORIGIN}')"
# Render locally before the first mutation; the normal import preserves catalog grants.
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
"$PROJECT_ROOT/scripts/deploy/render-public-host.py" "$PUBLIC_HOST" \
    "$PROJECT_ROOT/k8s/manifests/keycloak/realm-import-job.yaml" > "$WORK/realm-import.yaml"
# Redirects stay permanent only for the host the operator finalized; moving to another
# main host drops it, so the new direction starts temporary.
if [ "$configured_permanent" = "$MAIN_HOST" ]; then
    write_config "PUBLIC_HOST=${PUBLIC_HOST}" "MAIN_HOST=${MAIN_HOST}" "MAIN_HOST_PERMANENT=${MAIN_HOST}"
    export MAIN_HOST_PERMANENT="$MAIN_HOST"
else
    write_config "PUBLIC_HOST=${PUBLIC_HOST}" "MAIN_HOST=${MAIN_HOST}" MAIN_HOST_PERMANENT
    export MAIN_HOST_PERMANENT=""
fi
# The issuer changes when either KC_HOSTNAME or the realm frontendUrl is elsewhere. Every
# realm session is then signed out once the import has moved the issuer, so sessions
# begun during the Keycloak rollout or the import go too. A marker on the Keycloak
# Deployment, set before the first change and cleared after the sign-out, makes a rerun
# after an interruption still sign out. Any marker signs out on the next run, even when
# the issuer is not changing now: Keycloak's current hostname and frontendUrl can't show
# whether the marked issuer served sessions before the run stopped.
if [ "$current_hostname" != "https://${MAIN_HOST}/idp" ] ||
    { [ -n "$current_frontend_url" ] && [ "$current_frontend_url" != "https://${MAIN_HOST}/idp" ]; }; then
    echo "Issuer is changing: realm sessions will be signed out after the realm import; users must sign in again."
    kc -n econ-graph annotate deployment/keycloak --overwrite "${LOGOUT_ANNOTATION}=https://${MAIN_HOST}/idp" >/dev/null
    logout_pending="https://${MAIN_HOST}/idp"
elif [ -n "$logout_pending" ]; then
    echo "A sign-out is pending from an earlier run (${logout_pending}): realm sessions will be signed out after the realm import."
fi
# 1. Keycloak's issuer. set env patches existing Deployments: applying repo manifests
# would reset release images. The realm's frontendUrl overrides KC_HOSTNAME, so
# Keycloak keeps issuing the old issuer until the realm import below moves it too.
kc -n econ-graph set env deployment/keycloak \
    "KC_HOSTNAME=https://${MAIN_HOST}/idp" "KC_WEB_BASE_URL=https://${MAIN_HOST}" "KC_WEB_E2E_BASE_URL=https://${MAIN_HOST}"
kc -n econ-graph rollout status deployment/keycloak --timeout=900s
kc -n econ-graph delete job keycloak-realm-import --ignore-not-found
kc apply -f "$WORK/realm-import.yaml"
kc -n econ-graph wait --for=condition=complete job/keycloak-realm-import --timeout=900s
if [ -n "$logout_pending" ]; then
    echo "Signing out realm sessions: the issuer is now https://${MAIN_HOST}/idp."
    # Logout-all prevents old refresh sessions from silently minting new tokens.
    keycloak_admin create logout-all -r econ-graph >/dev/null
    kc -n econ-graph annotate deployment/keycloak "${LOGOUT_ANNOTATION}-" >/dev/null
fi
# 2. Only now the backend's expected issuer: old backend pods reject the new issuer's
# tokens (including the anonymous client-credentials renewal) from the end of the
# import until this rollout completes.
cors="https://${PUBLIC_HOST},https://www.${PUBLIC_HOST},http://localhost"
if [ "$current_cors" != "$cors" ]; then
    kc -n econ-graph patch configmap econ-graph-config --type=merge -p "{\"data\":{\"CORS_ORIGIN\":\"${cors}\"}}"
fi
kc -n econ-graph set env deployment/econ-graph-backend "OIDC_ISSUER=https://${MAIN_HOST}/idp/realms/econ-graph"
# The app reads econ-graph-config only at start: the same app-config hash deploy.sh
# sets. It also converges pods an interrupted earlier run left on the old ConfigMap.
roll_app_on_config_change || die "could not roll the app onto econ-graph-config"
for deployment in "${APP_CONFIG_DEPLOYMENTS[@]}"; do
    kc -n econ-graph rollout status "deployment/${deployment}" --timeout=900s
done
"$PROJECT_ROOT/scripts/deploy/apply-gateway-routes.sh" --context "$CONTEXT"
"$PROJECT_ROOT/scripts/deploy/check-public-site.sh" --host "$PUBLIC_HOST" --main-host "$MAIN_HOST"
echo "Main address verified: https://${MAIN_HOST}. Rerun this command after a partial failure."
echo "Purge the Cloudflare cache for ${PUBLIC_HOST} and www.${PUBLIC_HOST} now (docs/deployment/MAIN_HOST.md)."
if [ "$MAIN_HOST" != "$PUBLIC_HOST" ] && [ "${MAIN_HOST_PERMANENT}" != "$MAIN_HOST" ]; then
    echo "Redirects to https://${MAIN_HOST} are temporary (302). Once the switch is accepted:"
    echo "  $0 --context ${CONTEXT} --host ${PUBLIC_HOST} --finalize --confirm-production ${CONFIRM_PRODUCTION}"
fi
echo "Rollback: $0 --context ${CONTEXT} --host ${PUBLIC_HOST} --main-host ${PUBLIC_HOST} --confirm-production ${CONFIRM_PRODUCTION}"
