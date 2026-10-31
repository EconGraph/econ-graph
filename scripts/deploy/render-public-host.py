#!/usr/bin/env python3
"""Render public manifests while keeping PUBLIC_HOST as the base DNS name.

Usage: render-public-host.py HOST [MANIFEST ...]
MAIN_HOST defaults to www.HOST; set it to HOST to restore apex canonicalization.
Only the canonical HTTPS host serves public routes and Keycloak. HTTP on both
names and HTTPS on the alternate name redirect to that host, retaining path and
query. The redirects are permanent (301) to the apex, and to www.HOST only when
MAIN_HOST_PERMANENT names it (scripts/deploy/switch-main-host.sh --finalize);
otherwise they are temporary (302), so a rollback is not undone by redirects that
browsers and Cloudflare have cached. Localhost routes and the in-cluster JWKS URL remain unchanged.
Keycloak and backend share the canonical issuer; public CORS allows both names
and http://localhost. Tests: scripts/deploy/test-render-public-host.sh.
"""

import copy
import os
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
        "KC_REALM_FRONTEND_URL": f"{base}/idp",
        "KC_WEB_BASE_URL": base,
        # The realm's second origin; without a public host it stays http://localhost.
        "KC_WEB_E2E_BASE_URL": base,
        "OIDC_ISSUER": f"{base}/idp/realms/{REALM}",
    }


def set_env(doc, host):
    spec = (doc.get("spec") or {}).get("template", {}).get("spec") or {}
    values = env_values(host)
    for container in (spec.get("containers") or []) + (spec.get("initContainers") or []):
        if (
            doc.get("kind") == "Job"
            and doc.get("metadata", {}).get("name") == "keycloak-realm-import"
            and container.get("name") == "render-realm"
            and not any(e.get("name") == "KC_REALM_FRONTEND_URL" for e in container.get("env", []))
        ):
            container.setdefault("env", []).append(
                {"name": "KC_REALM_FRONTEND_URL", "value": values["KC_REALM_FRONTEND_URL"]}
            )
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


def redirect_status(host, main_host):
    """301 to the apex, or to a main host MAIN_HOST_PERMANENT names; else 302."""
    permanent = os.environ.get("MAIN_HOST_PERMANENT", "").strip().lower()
    return 301 if main_host == host or permanent == main_host else 302


def redirect_routes(namespace, host, main_host):
    """HTTP on both names and HTTPS on the alternate name go to MAIN_HOST."""
    status = redirect_status(host, main_host)

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
                                    "hostname": main_host,
                                    "statusCode": status,
                                },
                            }
                        ]
                    }
                ],
            },
        }

    return [
        route(REDIRECT_ROUTE, "http", [host, f"www.{host}"]),
        route(WWW_REDIRECT_ROUTE, "https", [host if main_host != host else f"www.{host}"]),
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


def set_cors_origin(doc, host):
    origins = [f"https://{host}", f"https://www.{host}", "http://localhost"]
    doc.setdefault("data", {})["CORS_ORIGIN"] = ",".join(origins)


def render(doc, host, main_host, present=()):
    """The documents to apply for DOC; PRESENT names the HTTPRoutes already in the input."""
    kind = doc.get("kind")
    meta = doc.get("metadata") or {}
    if kind == "HTTPRoute":
        if (meta.get("labels") or {}).get(PUBLIC_LABEL) == "true":
            # Rerendered manifests may come from the other canonical choice.
            # Keep their routes aligned with the issuer when switching either way.
            if meta.get("name") in (REDIRECT_ROUTE, WWW_REDIRECT_ROUTE):
                return [next(
                    route for route in redirect_routes(meta.get("namespace"), host, main_host)
                    if route["metadata"]["name"] == meta["name"]
                )]
            doc["spec"]["hostnames"] = [main_host]
            return [doc]
        if f"{meta.get('name')}-public" in present:
            return [doc]
        public = public_route(doc, main_host)
        return [doc] + ([public] if public else [])
    if kind == "BackendTrafficPolicy":
        add_public_targets(doc)
    elif kind in ("Deployment", "StatefulSet", "Job"):
        set_env(doc, main_host)
    elif kind == "ConfigMap" and meta.get("name") == "econ-graph-config":
        set_cors_origin(doc, host)
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
        print("render-public-host: give the base name, without the www prefix", file=sys.stderr)
        return 2
    main_host = os.environ.get("MAIN_HOST", f"www.{host}").strip().lower()
    if main_host not in (host, f"www.{host}"):
        print("render-public-host: MAIN_HOST must be HOST or www.HOST", file=sys.stderr)
        return 2
    docs = []
    for path in argv[2:] or ["-"]:
        stream = sys.stdin if path == "-" else open(path, encoding="utf-8")
        with stream:
            docs.extend(d for d in yaml.safe_load_all(stream) if d)
    routes = [d for d in docs if d.get("kind") == "HTTPRoute"]
    present = {(d.get("metadata") or {}).get("name") for d in routes}
    out = [r for d in docs for r in render(d, host, main_host, present)]
    if routes and REDIRECT_ROUTE not in present:
        out.extend(redirect_routes(routes[0]["metadata"].get("namespace"), host, main_host))
    yaml.safe_dump_all(out, sys.stdout, sort_keys=False, default_flow_style=False)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
