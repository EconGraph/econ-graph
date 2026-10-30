#!/usr/bin/env bash
# The host's deploy config: a file of NAME=value lines, read and never sourced.
# release-deploy.sh takes PUBLIC_HOST from it, and apply-gateway-routes.sh (so
# deploy.sh and restart-k8s-rollout.sh too) takes GATEWAY_TLS_ISSUER from it when
# the environment does not set one.
#
# The file is ECONGRAPH_DEPLOY_ENV, else deploy.env in ECONGRAPH_CONFIG_DIR, else
# in ${XDG_CONFIG_HOME:-~/.config}/econgraph. A missing file or line is no value.

# Print the deploy config's path.
deploy_config_path() {
    printf '%s\n' "${ECONGRAPH_DEPLOY_ENV:-${ECONGRAPH_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/econgraph}/deploy.env}"
}

# deploy_config_value NAME [WHAT]: print NAME's value from the last NAME= line, with
# surrounding blanks and one pair of matching quotes removed; nothing else is
# rewritten. Fails, saying why on stderr, on a carriage return in that line or blanks
# inside the value (WHAT names the value in that message: "write the WHAT alone").
deploy_config_value() {
    local name="$1" what="${2:-value}" file line value
    file="$(deploy_config_path)"
    [ -f "$file" ] || return 0
    # The last line wins, as it would if the file were sourced.
    line="$(grep -E "^[[:space:]]*${name}=" "$file" | tail -1)" || line=""
    [ -n "$line" ] || return 0
    value="${line#*=}"
    # A CRLF file would otherwise pass silently (the public-host renderer strips a
    # trailing CR): refuse it.
    if [[ "$value" == *$'\r'* ]]; then
        echo "error: ${file} has a carriage return in its ${name} line (CRLF line endings?); save it with LF endings" >&2
        return 1
    fi
    value="${value#"${value%%[![:space:]]*}"}"
    value="${value%"${value##*[![:space:]]}"}"
    if [[ "$value" =~ ^\"(.*)\"$ || "$value" =~ ^\'(.*)\'$ ]]; then
        value="${BASH_REMATCH[1]}"
    fi
    # Blanks inside (or inside the quotes), such as a trailing comment: refuse,
    # rather than guess.
    if [[ "$value" == *[[:space:]]* ]]; then
        echo "error: ${file} sets ${name}='${value}', with blanks in it; write the ${what} alone" >&2
        return 1
    fi
    printf '%s\n' "$value"
}
