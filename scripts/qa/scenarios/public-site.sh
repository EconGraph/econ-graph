#!/usr/bin/env bash
# qa-scenario: public-site
# qa-mutates: no
# qa-profiles: lab, ci, production
# qa-timeout: 300
# qa-description: scripts/deploy/check-public-site.sh: routing, issuer, sign-in page, CORS, header spoofing, admin and metrics not served.
#
# Runs scripts/deploy/check-public-site.sh against the target's host and records each of its
# ok/FAIL lines as a check.
#
# Inputs: QA_TARGET, an https origin on the default port (skipped otherwise: the script checks
# a public HTTPS host with its alternate-host redirect and Keycloak under /idp). QA_PUBLIC_SITE_ROUTE is
# `dns` (real DNS, the way a visitor arrives; --via-cloudflare) or `gateway` (curl
# --connect-to the gateway on this host, with LOCAL_INGRESS_IP, LOCAL_HTTPS_PORT and
# LOCAL_HTTP_PORT passed through), for a runner on the host whose gateway serves the target. It
# defaults to `dns`. QA_CHECK_PUBLIC_SITE overrides the script's path. MAIN_HOST_PERMANENT
# reaches the script as it is: set it to the target's host when the redirects to it are
# permanent (301; scripts/deploy/switch-main-host.sh --finalize), else they must be 302s.
set -euo pipefail

repo="${QA_REPO_ROOT:?run through scripts/qa/run-scenarios.sh}"
script="${QA_CHECK_PUBLIC_SITE:-$repo/scripts/deploy/check-public-site.sh}"
evidence="${QA_EVIDENCE_FILE:-/dev/null}"

skip() {
    python3 -c 'import json, sys; print(json.dumps({"skip": sys.argv[1]}))' "$1" >>"$evidence"
    echo "skip: $1"
    exit 77
}

read -r scheme host port path < <(python3 -c '
import sys, urllib.parse
u = urllib.parse.urlsplit(sys.argv[1])
print(u.scheme, u.hostname or "-", u.port or "-", u.path.strip("/") or "-")
' "$QA_TARGET")
[ "$scheme" = https ] || skip "check-public-site.sh checks an https public host; the target is ${scheme}"
[ "$port" = - ] || skip "check-public-site.sh checks the default https port; the target uses ${port}"
[ "$path" = - ] || skip "check-public-site.sh checks a host's root; the target has the path /${path}"

route="${QA_PUBLIC_SITE_ROUTE:-dns}"
# The target is the canonical origin; --host is independently the base domain.
# An apex target explicitly checks an apex rollback instead of guessing www.www.
args=(--host "${host#www.}" --main-host "$host")
case "$route" in
    dns) args+=(--via-cloudflare) ;;
    gateway) ;;
    *)
        echo "QA_PUBLIC_SITE_ROUTE must be dns or gateway, not ${route}" >&2
        exit 2
        ;;
esac

set +e
output="$("$script" "${args[@]}" 2>&1)"
status=$?
set -e
printf '%s\n' "$output"

# Each "  ok    ..." or "  FAIL  ..." line is one check.
printf '%s\n' "$output" | python3 -c '
import json, re, sys
route = sys.argv[1]
for line in sys.stdin:
    m = re.match(r"^\s+(ok|FAIL)\s+(.*\S)\s*$", line)
    if m:
        print(json.dumps({"check": m.group(2), "ok": m.group(1) == "ok", "detail": "via " + route}))
' "$route" >>"$evidence"
exit "$status"
