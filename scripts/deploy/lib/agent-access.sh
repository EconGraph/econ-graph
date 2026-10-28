# shellcheck shell=bash
# Restricted sessions are an operational default, not an OS security boundary.
# Never print config contents: they contain the short-lived bearer token.
econgraph_agent_session() {
    [ "${ECONGRAPH_AGENT_SESSION:-}" = 1 ] && return 0
    [ -n "${KUBECONFIG:-}" ] || return 1
    # Recognise both the generated JSON and a kubectl-converted YAML copy.
    # Inspect only locally, before a helper can replace it with admin credentials.
    # Conservative: an operator config containing an unused agent entry is refused
    # too. Select a separate operator-only config for credential handling.
    local config
    local -a configs
    IFS=: read -r -a configs <<<"$KUBECONFIG"
    for config in "${configs[@]}"; do
        if [ -f "$config" ] && grep -q 'econgraph-agent' "$config"; then
            return 0
        fi
    done
    return 1
}

require_operator_session() {
    if econgraph_agent_session; then
        echo "error: this step handles credentials and is disabled in an agent session." >&2
        echo "Run it yourself in an operator terminal with your operator kubeconfig." >&2
        return 1
    fi
}
