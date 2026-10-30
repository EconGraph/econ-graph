#!/usr/bin/env bash
# The host's deploy config: a file of NAME=value lines, read and never sourced.
# release-deploy.sh takes PUBLIC_HOST from it. For a public host, release-deploy.sh,
# deploy.sh and restart-k8s-rollout.sh (lib/public-host.sh, before anything is
# applied) and apply-gateway-routes.sh take GATEWAY_TLS_ISSUER from it through
# tls_issuer_choose.
#
# The file is ECONGRAPH_DEPLOY_ENV, else deploy.env in ECONGRAPH_CONFIG_DIR, else
# in ${XDG_CONFIG_HOME:-~/.config}/econgraph. A missing file or line is no value; a
# file that exists but cannot be read is an error.

# Print the deploy config's path.
deploy_config_path() {
    printf '%s\n' "${ECONGRAPH_DEPLOY_ENV:-${ECONGRAPH_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/econgraph}/deploy.env}"
}

# deploy_config_read NAME [WHAT]: the one reader of the deploy config. Sets
# DEPLOY_CONFIG_VALUE to NAME's value from the last NAME= line, with surrounding
# blanks and one pair of matching quotes removed; nothing else is rewritten (empty
# without the file or the line). Returns 1, with the reason in DEPLOY_CONFIG_ERROR,
# on a file that exists but is not a readable regular file, an `export NAME=` line
# (refused rather than read differently from a plain one), a carriage return in the
# NAME line or blanks inside the value (WHAT names the value in that message: "write
# the WHAT alone"). Runs in the caller's shell, so the variables reach it.
deploy_config_read() {
    local name="$1" what="${2:-value}" file line value
    file="$(deploy_config_path)"
    DEPLOY_CONFIG_VALUE=""
    DEPLOY_CONFIG_ERROR=""
    [ -e "$file" ] || return 0
    if [ ! -f "$file" ] || [ ! -r "$file" ]; then
        DEPLOY_CONFIG_ERROR="${file} exists but is not a readable file"
        return 1
    fi
    if grep -qE "^[[:space:]]*export[[:space:]]+${name}=" "$file"; then
        DEPLOY_CONFIG_ERROR="${file} has an 'export ${name}=' line; it is read, not sourced: write ${name}=... without export"
        return 1
    fi
    # The last line wins, as it would if the file were sourced.
    line="$(grep -E "^[[:space:]]*${name}=" "$file" | tail -1)" || line=""
    [ -n "$line" ] || return 0
    value="${line#*=}"
    # A CRLF file would otherwise pass silently (the public-host renderer strips a
    # trailing CR): refuse it.
    if [[ "$value" == *$'\r'* ]]; then
        DEPLOY_CONFIG_ERROR="${file} has a carriage return in its ${name} line (CRLF line endings?); save it with LF endings"
        return 1
    fi
    value="${value#"${value%%[![:space:]]*}"}"
    value="${value%"${value##*[![:space:]]}"}"
    if [[ "$value" =~ ^\"(.*)\"$ || "$value" =~ ^\'(.*)\'$ ]]; then
        value="${BASH_REMATCH[1]}"
    fi
    # Blanks inside (or inside the quotes), such as a trailing comment: the renderer
    # would strip them, but the gateway step would not. Refuse, rather than guess.
    if [[ "$value" == *[[:space:]]* ]]; then
        DEPLOY_CONFIG_ERROR="${file} sets ${name}='${value}', with blanks in it; write the ${what} alone"
        return 1
    fi
    DEPLOY_CONFIG_VALUE="$value"
}

# deploy_config_value NAME [WHAT]: deploy_config_read, printing the value, or the
# reason on stderr.
deploy_config_value() {
    if ! deploy_config_read "$@"; then
        echo "error: ${DEPLOY_CONFIG_ERROR}" >&2
        return 1
    fi
    printf '%s\n' "$DEPLOY_CONFIG_VALUE"
}

# gateway_tls_current_issuer: print the ClusterIssuer of certificate
# econ-graph-gateway/econ-graph-tls, or nothing without the certificate or without
# cert-manager. Fails when the cluster cannot be asked. Uses the caller's kc.
gateway_tls_current_issuer() {
    local crd
    crd="$(kc get crd certificates.cert-manager.io --ignore-not-found -o name)" || return 1
    [ -n "$crd" ] || return 0
    kc -n econ-graph-gateway get certificate econ-graph-tls --ignore-not-found \
        -o 'jsonpath={.spec.issuerRef.name}'
}

# tls_issuer_choose CURRENT: choose the gateway certificate's ClusterIssuer, given
# the issuer of the certificate the cluster has now (empty without one), and set
# TLS_ISSUER and TLS_ISSUER_SOURCE. In order:
#   - GATEWAY_TLS_ISSUER in the environment: an explicit choice, which may change it;
#   - the deploy config's GATEWAY_TLS_ISSUER line, only when it matches CURRENT or
#     there is no certificate (a recreated cluster). One that differs is refused: a
#     deploy never changes the issuer implicitly, since a stale or mistyped line
#     would otherwise re-issue the certificate from an issuer the tunnel rejects;
#   - CURRENT;
#   - econ-graph-selfsigned.
# Fails, saying why on stderr, on that conflict, a bad config line, or a value that
# is not a resource name.
tls_issuer_choose() {
    local current="$1" configured
    if [ -n "${GATEWAY_TLS_ISSUER:-}" ]; then
        TLS_ISSUER="$GATEWAY_TLS_ISSUER"
        TLS_ISSUER_SOURCE="GATEWAY_TLS_ISSUER in the environment"
    else
        configured="$(deploy_config_value GATEWAY_TLS_ISSUER "issuer name")" || return 1
        if [ -n "$configured" ]; then
            if [ -n "$current" ] && [ "$current" != "$configured" ]; then
                echo "error: certificate econ-graph-tls is issued by ${current}, but $(deploy_config_path) says GATEWAY_TLS_ISSUER=${configured}; a deploy never changes the issuer implicitly. If ${configured} is right, set GATEWAY_TLS_ISSUER=${configured} in the environment (release-deploy.sh: --change-tls-issuer); otherwise correct the config" >&2
                return 1
            fi
            TLS_ISSUER="$configured"
            TLS_ISSUER_SOURCE="$(deploy_config_path)"
        elif [ -n "$current" ]; then
            TLS_ISSUER="$current"
            TLS_ISSUER_SOURCE="the certificate's current issuer"
        else
            TLS_ISSUER=econ-graph-selfsigned
            TLS_ISSUER_SOURCE="the default"
        fi
    fi
    if ! [[ "$TLS_ISSUER" =~ ^[a-z0-9]([a-z0-9.-]*[a-z0-9])?$ ]]; then
        echo "error: GATEWAY_TLS_ISSUER='${TLS_ISSUER}' (from ${TLS_ISSUER_SOURCE}) is not a resource name" >&2
        return 1
    fi
}
