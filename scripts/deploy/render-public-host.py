#!/usr/bin/env python3
"""Point the k8s manifests at a public host name, for scripts/deploy/deploy.sh.

Usage: scripts/deploy/render-public-host.py HOST [MANIFEST ...]

Reads each manifest (or stdin when none or "-"), and prints the documents as YAML with
the public host https://HOST applied. The committed manifests stay localhost-only, and
everything below is added to them, so http://localhost keeps working on the host:

  Ingress (every rule for host localhost)   the same rule again for HOST
  Ingress econ-graph-ingress                from-to-www-redirect: www.HOST answers with a
                                            permanent redirect to HOST, same path
  Keycloak, realm import (env)              KC_HOSTNAME=https://HOST/idp
                                            KC_WEB_BASE_URL=https://HOST (redirect URIs,
                                            web origins); KC_WEB_E2E_BASE_URL keeps
                                            http://localhost as the second allowed origin
  Backend (env)                             OIDC_ISSUER=https://HOST/idp/realms/econ-graph
                                            (OIDC_JWKS_URL stays the in-cluster Service)
  ConfigMap econ-graph-config               https://HOST prepended to CORS_ORIGIN

The scheme is always https: the public site is served by Cloudflare, through the
cloudflared tunnel (k8s/manifests/cloudflared.yaml). Keycloak's issuer must equal the
backend's OIDC_ISSUER, which is why both are set from the one HOST here.

www is redirected rather than served: sign-in goes through Keycloak on the app's own
origin, and Keycloak has one issuer that the backend checks exactly. Serving www too
would add a second origin to every redirect URI, web origin and CORS list and split
sessions between two hosts, for nothing. ingress-nginx builds the redirect from
X-Forwarded-Proto (use-forwarded-headers, set by deploy.sh), so it stays on https.

Needs python3 with PyYAML (also needed by k8s/monitoring/check-prometheus-rules.sh).
Tests: scripts/deploy/test-render-public-host.sh.
"""

import copy
import re
import sys

import yaml

# One DNS name: labels of letters, digits and inner hyphens, at least one dot.
HOST_RE = re.compile(
    r"^(?=.{1,253}$)([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]([a-z0-9-]{0,61}[a-z0-9])?$"
)
# The Ingress that carries the www redirect (ingress-nginx adds one redirect server per
# host, so one Ingress for the apex is enough).
WWW_REDIRECT_INGRESS = "econ-graph-ingress"
REALM = "econ-graph"


def env_values(host):
    """Container env values to replace, by variable name."""
    base = f"https://{host}"
    return {
        "KC_HOSTNAME": f"{base}/idp",
        "KC_WEB_BASE_URL": base,
        "OIDC_ISSUER": f"{base}/idp/realms/{REALM}",
    }


def set_env(doc, host):
    spec = (doc.get("spec") or {}).get("template", {}).get("spec") or {}
    values = env_values(host)
    for container in (spec.get("containers") or []) + (spec.get("initContainers") or []):
        for var in container.get("env") or []:
            if var.get("name") in values and "value" in var:
                var["value"] = values[var["name"]]


def add_ingress_host(doc, host):
    rules = doc.get("spec", {}).get("rules") or []
    if not any(rule.get("host") == host for rule in rules):
        copies = [
            dict(copy.deepcopy(rule), host=host) for rule in rules if rule.get("host") == "localhost"
        ]
        doc["spec"]["rules"] = rules + copies
    if doc["metadata"].get("name") == WWW_REDIRECT_INGRESS:
        doc["metadata"].setdefault("annotations", {})[
            "nginx.ingress.kubernetes.io/from-to-www-redirect"
        ] = "true"


def add_cors_origin(doc, host):
    data = doc.setdefault("data", {})
    origin = f"https://{host}"
    current = [o.strip() for o in (data.get("CORS_ORIGIN") or "").split(",") if o.strip()]
    data["CORS_ORIGIN"] = ",".join([origin] + [o for o in current if o != origin])


def render(doc, host):
    kind = doc.get("kind")
    name = (doc.get("metadata") or {}).get("name")
    if kind == "Ingress":
        add_ingress_host(doc, host)
    elif kind in ("Deployment", "StatefulSet", "Job"):
        set_env(doc, host)
    elif kind == "ConfigMap" and name == "econ-graph-config":
        add_cors_origin(doc, host)
    return doc


def main(argv):
    if len(argv) < 2:
        print("usage: render-public-host.py HOST [MANIFEST ...]", file=sys.stderr)
        return 2
    host = argv[1].strip().lower()
    if host == "localhost" or not HOST_RE.match(host):
        print(f"render-public-host: {argv[1]!r} is not a public DNS name", file=sys.stderr)
        return 2
    if host.startswith("www."):
        print("render-public-host: give the apex name; www.<host> redirects to it", file=sys.stderr)
        return 2
    docs = []
    for path in argv[2:] or ["-"]:
        stream = sys.stdin if path == "-" else open(path, encoding="utf-8")
        with stream:
            docs.extend(d for d in yaml.safe_load_all(stream) if d)
    yaml.safe_dump_all(
        [render(d, host) for d in docs], sys.stdout, sort_keys=False, default_flow_style=False
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
