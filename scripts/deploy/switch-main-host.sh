#!/usr/bin/env bash
# Switch the canonical public host, or roll it back, without changing release images.
# Joe runs this from the deployed checkout with an operator kubeconfig.
# Usage: switch-main-host.sh --context NAME --host BASE [--main-host NAME] [--dry-run]
# MAIN defaults to www.BASE; --main-host BASE rolls back. --dry-run contacts no
# cluster or network and reads/writes no deploy config. DNS and tunnel routes keep
# the existing two public names. The real run persists PUBLIC_HOST/MAIN_HOST in
# ECONGRAPH_DEPLOY_ENV (or the normal deploy.env) for subsequent deployments.
set -euo pipefail
set +x
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DRY_RUN=false
CONTEXT=""
HOST=""
MAIN=""
usage() { echo "Usage: $0 --context NAME --host BASE [--main-host NAME] [--dry-run]"; }
die() { echo "error: $*" >&2; exit 1; }
while [ $# -gt 0 ]; do
    case "$1" in
        --context|--host|--main-host)
            [ $# -ge 2 ] && [ -n "$2" ] || die "$1 needs a value"
            case "$1" in --context) CONTEXT="$2" ;; --host) HOST="$2" ;; --main-host) MAIN="$2" ;; esac
            shift ;;
        --dry-run) DRY_RUN=true ;;
        -h|--help) usage; exit 0 ;;
        *) usage >&2; die "unknown argument $1" ;;
    esac
    shift
done
[ -n "$CONTEXT" ] && [ -n "$HOST" ] || { usage >&2; exit 2; }
MAIN="${MAIN:-www.${HOST}}"
[ "$MAIN" = "$HOST" ] || [ "$MAIN" = "www.${HOST}" ] || die "main host must be BASE or www.BASE"
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
    echo "[dry-run] verify marked-production context and matching realm ConfigMap"
    echo "[dry-run] atomically retain other settings and write PUBLIC_HOST/MAIN_HOST in ${CONFIG_FILE}"
    echo "[dry-run] if issuer changes: sign out existing realm sessions once"
    echo "[dry-run] patch Keycloak hostname/client origins and backend issuer; preserve release images"
    echo "[dry-run] keep both public CORS origins plus http://localhost; reconcile backend CORS checksum (restart only when it changes)"
    echo "[dry-run] wait for rollouts; rerun the catalog-preserving realm admin API import"
    echo "[dry-run] apply canonical gateway routes, then check-public-site.sh --host ${PUBLIC_HOST} --main-host ${MAIN_HOST}"
    exit 0
fi
# Credential-handling and live writes are operator-only.
# shellcheck source=scripts/deploy/lib/agent-access.sh
source "$PROJECT_ROOT/scripts/deploy/lib/agent-access.sh"
require_operator_session
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$PROJECT_ROOT/scripts/deploy/lib/cluster-role.sh"
cluster_role_operator_context "$CONTEXT" || die "could not verify operator context"
cluster_is_marked_production || die "${CLUSTER_ROLE_REASON}"
# Refuse another public domain, or a checkout that would apply a different realm.
configured="$(deploy_config_value PUBLIC_HOST 'public host')" || exit 1
[ -z "$configured" ] || [ "$configured" = "$PUBLIC_HOST" ] || die "deploy config PUBLIC_HOST=${configured} differs from --host"
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
# A legacy frontendUrl overrides KC_HOSTNAME, so it participates in the logout
# decision too. Capture only realm attributes, never credentials or tokens.
realm_attributes="$(keycloak_admin get realms/econ-graph --fields attributes)"
current_frontend_url="$(python3 -c 'import json, sys; print((json.loads(sys.argv[1]).get("attributes") or {}).get("frontendUrl", "").rstrip("/"))' "$realm_attributes")"
kc -n econ-graph get deployment econ-graph-backend >/dev/null
current_cors="$(kc -n econ-graph get configmap econ-graph-config -o 'jsonpath={.data.CORS_ORIGIN}')"
# Render locally before the first mutation; the normal import preserves catalog grants.
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
"$PROJECT_ROOT/scripts/deploy/render-public-host.py" "$PUBLIC_HOST" \
    "$PROJECT_ROOT/k8s/manifests/keycloak/realm-import-job.yaml" > "$WORK/realm-import.yaml"
# Write atomically, preserving all unrelated bytes and the existing file mode.
# A persisted target makes a partial run recoverable by rerunning this command.
python3 - "$CONFIG_FILE" "$PUBLIC_HOST" "$MAIN_HOST" <<'PY'
import os
import pathlib
import re
import stat
import sys
import tempfile
path = pathlib.Path(sys.argv[1])
if path.is_symlink():
    raise SystemExit("deploy config must not be a symlink")
text = path.read_bytes().decode("utf-8") if path.exists() else ""
text = "".join(line for line in text.splitlines(keepends=True)
               if not re.match(r"^\s*(?:export\s+)?(?:PUBLIC_HOST|MAIN_HOST)=", line))
if text and not text.endswith("\n"):
    text += "\n"
text += f"PUBLIC_HOST={sys.argv[2]}\nMAIN_HOST={sys.argv[3]}\n"
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
if [ "$current_hostname" != "https://${MAIN_HOST}/idp" ] ||
    { [ -n "$current_frontend_url" ] && [ "$current_frontend_url" != "https://${MAIN_HOST}/idp" ]; }; then
    echo "Issuer is changing: signing out realm sessions; users must sign in again."
    # Logout-all prevents old refresh sessions from silently minting new tokens.
    keycloak_admin create logout-all -r econ-graph >/dev/null
fi
# set env patches existing Deployments: applying repo manifests would reset release images.
kc -n econ-graph set env deployment/keycloak \
    "KC_HOSTNAME=https://${MAIN_HOST}/idp" "KC_WEB_BASE_URL=https://${MAIN_HOST}" "KC_WEB_E2E_BASE_URL=https://${MAIN_HOST}"
kc -n econ-graph set env deployment/econ-graph-backend "OIDC_ISSUER=https://${MAIN_HOST}/idp/realms/econ-graph"
cors="https://${PUBLIC_HOST},https://www.${PUBLIC_HOST},http://localhost"
if [ "$current_cors" != "$cors" ]; then
    kc -n econ-graph patch configmap econ-graph-config --type=merge -p "{\"data\":{\"CORS_ORIGIN\":\"${cors}\"}}"
fi
# Always reconcile the template checksum, including when an interrupted previous
# run wrote the ConfigMap but never replaced pods. Reapplying the same annotation
# is a no-op; running backend pods converge on the desired CORS configuration.
cors_checksum="$(python3 -c 'import hashlib, sys; print(hashlib.sha256(sys.argv[1].encode()).hexdigest())' "$cors")"
kc -n econ-graph patch deployment econ-graph-backend --type=merge \
    -p "{\"spec\":{\"template\":{\"metadata\":{\"annotations\":{\"econgraph.io/cors-origins-sha256\":\"${cors_checksum}\"}}}}}"
kc -n econ-graph rollout status deployment/keycloak --timeout=900s
kc -n econ-graph rollout status deployment/econ-graph-backend --timeout=900s
kc -n econ-graph delete job keycloak-realm-import --ignore-not-found
kc apply -f "$WORK/realm-import.yaml"
kc -n econ-graph wait --for=condition=complete job/keycloak-realm-import --timeout=900s
"$PROJECT_ROOT/scripts/deploy/apply-gateway-routes.sh" --context "$CONTEXT"
"$PROJECT_ROOT/scripts/deploy/check-public-site.sh" --host "$PUBLIC_HOST" --main-host "$MAIN_HOST"
echo "Main address verified: https://${MAIN_HOST}. Rerun this command after a partial failure."
echo "Rollback: $0 --context ${CONTEXT} --host ${PUBLIC_HOST} --main-host ${PUBLIC_HOST}"
