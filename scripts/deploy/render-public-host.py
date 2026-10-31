#!/usr/bin/env python3
"""Point the k8s manifests at a public host name, for scripts/deploy/lib/public-host.sh
and scripts/deploy/apply-gateway-routes.sh.

Usage: scripts/deploy/render-public-host.py HOST [MANIFEST ...]

Reads each manifest (or stdin when none or "-"), and prints the documents as YAML with
the public host https://HOST applied. The committed manifests stay localhost-only and
are printed unchanged apart from the env values below, so http://localhost keeps
working on the host:

  HTTPRoute NAME (host localhost,           plus a new HTTPRoute NAME-public for HOST on
  listener http)                            listener https (see below), labelled
                                            econgraph.io/public-host
  BackendTrafficPolicy                      also targets the -public copies of its routes,
                                            so the rate limits apply on HOST too
  (with any HTTPRoute)                      HTTPRoute econ-graph-public-redirect: plain
                                            http for HOST or www.HOST answers with a
                                            permanent redirect to https://HOST, same path;
                                            HTTPRoute econ-graph-www-redirect: https for
                                            www.HOST redirects to https://HOST
  Keycloak, realm import (env)              KC_HOSTNAME=https://HOST/idp
                                            KC_WEB_BASE_URL=https://HOST and
                                            KC_WEB_E2E_BASE_URL=https://HOST, so the
                                            econ-graph-web client's redirect URIs and web
                                            origins are the public host only (no
                                            http://localhost: a sign-in on the public
                                            host never returns to a local listener)
  Backend (env)                             OIDC_ISSUER=https://HOST/idp/realms/econ-graph
                                            (OIDC_JWKS_URL stays the in-cluster Service)
  ConfigMap econ-graph-config               https://HOST prepended to CORS_ORIGIN

The public routes are separate objects so that they can differ from the localhost
ones without touching them:
  - they attach to the gateway's https listener only, which the cloudflared tunnel
    connector reaches over https (k8s/manifests/cloudflared.yaml), so the scheme the
    gateway passes on (X-Forwarded-Proto) is the connection's real one, and a
    plain-http request for HOST, say from the LAN to the host's port 80, is
    redirected to https://HOST instead of reaching the app or Keycloak in clear
    text. No client header decides the scheme or host
    (k8s/manifests/gateway/client-traffic-policy.yaml).
  - the admin and debug paths (/admin, /api/admin, /playground) are left out; on
    http://localhost the routes do not serve them either.

www is redirected rather than served: sign-in goes through Keycloak on the app's own
origin, and Keycloak has one issuer that the backend checks exactly. Serving www too
would add a second origin to every redirect URI, web origin and CORS list and split
sessions between two hosts, for nothing.

Keycloak's issuer must equal the backend's OIDC_ISSUER, which is why both are set from
the one HOST here.

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
PUBLIC_LABEL = "econgraph.io/public-host"
# Paths served on localhost only, never on the public host (prefix match).
LOCAL_ONLY_PATHS = ("/admin", "/api/admin", "/playground")
REALM = "econ-graph"
GATEWAY = {"name": "econ-graph-gateway", "namespace": "econ-graph-gateway"}
REDIRECT_ROUTE = "econ-graph-public-redirect"
WWW_REDIRECT_ROUTE = "econ-graph-www-redirect"


def env_values(host):
    """Container env values to replace, by variable name."""
    base = f"https://{host}"
    return {
        "KC_HOSTNAME": f"{base}/idp",
        "KC_WEB_BASE_URL": base,
        # The realm's second origin; without a public host it stays http://localhost.
        "KC_WEB_E2E_BASE_URL": base,
        "OIDC_ISSUER": f"{base}/idp/realms/{REALM}",
    }


def set_env(doc, host):
    spec = (doc.get("spec") or {}).get("template", {}).get("spec") or {}
    values = env_values(host)
    for container in (spec.get("containers") or []) + (spec.get("initContainers") or []):
        for var in container.get("env") or []:
            if var.get("name") in values and "value" in var:
                var["value"] = values[var["name"]]


def local_only(match):
    p = (match.get("path") or {}).get("value", "/")
    return any(p == prefix or p.startswith(prefix + "/") for prefix in LOCAL_ONLY_PATHS)


def public_labels(meta):
    labels = dict(meta.get("labels") or {})
    labels[PUBLIC_LABEL] = "true"
    return labels


def public_route(doc, host):
    """The public copy of a localhost HTTPRoute, or None when it serves no localhost."""
    if "localhost" not in (doc.get("spec", {}).get("hostnames") or []):
        return None
    rules = []
    for rule in doc["spec"].get("rules") or []:
        rule = copy.deepcopy(rule)
        matches = rule.get("matches")
        if matches is not None:
            matches = [m for m in matches if not local_only(m)]
            if not matches:
                continue
            rule["matches"] = matches
        rules.append(rule)
    if not rules:
        return None
    meta = doc["metadata"]
    spec = copy.deepcopy(doc["spec"])
    spec["parentRefs"] = [dict(GATEWAY, sectionName="https")]
    spec["hostnames"] = [host]
    spec["rules"] = rules
    return {
        "apiVersion": doc["apiVersion"],
        "kind": "HTTPRoute",
        "metadata": {
            "name": f"{meta['name']}-public",
            "namespace": meta.get("namespace"),
            "labels": public_labels(meta),
        },
        "spec": spec,
    }


def redirect_routes(namespace, host):
    """Plain http for the public names, and https for www, redirect to https://HOST."""

    def route(name, section, hostnames):
        return {
            "apiVersion": "gateway.networking.k8s.io/v1",
            "kind": "HTTPRoute",
            "metadata": {"name": name, "namespace": namespace, "labels": {PUBLIC_LABEL: "true"}},
            "spec": {
                "parentRefs": [dict(GATEWAY, sectionName=section)],
                "hostnames": hostnames,
                "rules": [
                    {
                        "filters": [
                            {
                                "type": "RequestRedirect",
                                "requestRedirect": {
                                    "scheme": "https",
                                    "hostname": host,
                                    "statusCode": 301,
                                },
                            }
                        ]
                    }
                ],
            },
        }

    return [
        route(REDIRECT_ROUTE, "http", [host, f"www.{host}"]),
        route(WWW_REDIRECT_ROUTE, "https", [f"www.{host}"]),
    ]


def add_public_targets(doc):
    """A BackendTrafficPolicy also applies to the public copies of its routes."""
    refs = doc.setdefault("spec", {}).setdefault("targetRefs", [])
    names = {r.get("name") for r in refs}
    for ref in list(refs):
        if ref.get("kind") == "HTTPRoute" and not ref["name"].endswith("-public"):
            public = f"{ref['name']}-public"
            if public not in names:
                refs.append(dict(ref, name=public))
                names.add(public)


def add_cors_origin(doc, host):
    data = doc.setdefault("data", {})
    origin = f"https://{host}"
    current = [o.strip() for o in (data.get("CORS_ORIGIN") or "").split(",") if o.strip()]
    data["CORS_ORIGIN"] = ",".join([origin] + [o for o in current if o != origin])


def render(doc, host, present=()):
    """The documents to apply for DOC; PRESENT names the HTTPRoutes already in the input."""
    kind = doc.get("kind")
    meta = doc.get("metadata") or {}
    if kind == "HTTPRoute":
        if (meta.get("labels") or {}).get(PUBLIC_LABEL) == "true":
            return [doc]  # already rendered
        if f"{meta.get('name')}-public" in present:
            return [doc]
        public = public_route(doc, host)
        return [doc] + ([public] if public else [])
    if kind == "BackendTrafficPolicy":
        add_public_targets(doc)
    elif kind in ("Deployment", "StatefulSet", "Job"):
        set_env(doc, host)
    elif kind == "ConfigMap" and meta.get("name") == "econ-graph-config":
        add_cors_origin(doc, host)
    return [doc]


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
    routes = [d for d in docs if d.get("kind") == "HTTPRoute"]
    present = {(d.get("metadata") or {}).get("name") for d in routes}
    out = [r for d in docs for r in render(d, host, present)]
    if routes and REDIRECT_ROUTE not in present:
        out.extend(redirect_routes(routes[0]["metadata"].get("namespace"), host))
    yaml.safe_dump_all(out, sys.stdout, sort_keys=False, default_flow_style=False)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
