# shellcheck shell=bash
#
# Roll the app when its ConfigMap changes. Sourced by scripts/deploy/deploy.sh, which runs
# roll_app_on_config_change after it applies the Deployments.
#
# econ-graph-backend, crawler-worker and econ-graph-frontend read econ-graph-config only
# when a pod starts, so applying a changed ConfigMap leaves running pods on the old values.
# A new image tag rolls them anyway; a re-run of deploy.sh with the same tag (say, to pick
# up a new CORS_ORIGIN) does not. So each Deployment's pod template carries the annotation
# APP_CONFIG_ANNOTATION, a hash of the ConfigMap's data: patching in a different hash
# rolls the Deployment, the same hash changes nothing. The hash stays on the template
# (kubectl apply of the manifests, which don't name it, leaves it alone), so it always
# says which data the pods started with, even after a deploy that stopped between applying
# the ConfigMap and the Deployments.
#
# Uses: kubectl (deploy.sh's, on its pinned context), sha256sum, DRY_RUN (true: only say
# what would run).

APP_CONFIG_DEPLOYMENTS=(econ-graph-backend crawler-worker econ-graph-frontend)
APP_CONFIG_ANNOTATION="econgraph.com/app-config-hash"

# The hash of econ-graph-config's data (kubectl prints it as JSON with sorted keys), or
# nothing when the ConfigMap can't be read.
app_config_hash() {
    local data
    data="$(kubectl -n econ-graph get configmap econ-graph-config -o jsonpath='{.data}')" || return 0
    [ -n "$data" ] || return 0
    printf '%s' "$data" | sha256sum | cut -c1-16
}

roll_app_on_config_change() {
    if [ "${DRY_RUN:-false}" = true ]; then
        echo "[dry-run] set ${APP_CONFIG_ANNOTATION} on ${APP_CONFIG_DEPLOYMENTS[*]} (rolls them if econ-graph-config changed)"
        return 0
    fi
    local hash d current
    hash="$(app_config_hash)"
    if [ -z "$hash" ]; then
        echo "❌ could not read ConfigMap econ-graph-config to roll the app on a config change" >&2
        return 1
    fi
    for d in "${APP_CONFIG_DEPLOYMENTS[@]}"; do
        current="$(kubectl -n econ-graph get "deployment/$d" \
            -o jsonpath="{.spec.template.metadata.annotations.${APP_CONFIG_ANNOTATION//./\\.}}")" || return 1
        [ "$current" = "$hash" ] && continue
        echo "🔄 deployment/$d: econ-graph-config ${hash} (was ${current:-not recorded}); rolling it"
        kubectl -n econ-graph patch "deployment/$d" --type merge \
            -p "{\"spec\":{\"template\":{\"metadata\":{\"annotations\":{\"${APP_CONFIG_ANNOTATION}\":\"${hash}\"}}}}}" || return 1
    done
}
