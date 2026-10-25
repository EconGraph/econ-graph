#!/usr/bin/env python3
"""Point the k8s manifests at a public host name, for scripts/deploy/lib/public-host.sh.

Usage: scripts/deploy/render-public-host.py HOST [MANIFEST ...]

Reads each manifest (or stdin when none or "-"), and prints the documents as YAML with
the public host https://HOST applied. The committed manifests stay localhost-only and
are printed unchanged apart from the env values below, so http://localhost keeps
working on the host:

  Ingress NAME (rules for host localhost)   plus a new Ingress NAME-public for HOST (see
                                            below), labelled econgraph.io/public-host
  Keycloak, realm import (env)              KC_HOSTNAME=https://HOST/idp
                                            KC_WEB_BASE_URL=https://HOST (redirect URIs,
                                            web origins); KC_WEB_E2E_BASE_URL keeps
                                            http://localhost as the second allowed origin
  Backend (env)                             OIDC_ISSUER=https://HOST/idp/realms/econ-graph
                                            (OIDC_JWKS_URL stays the in-cluster Service)
  ConfigMap econ-graph-config               https://HOST prepended to CORS_ORIGIN

The public Ingresses are separate objects so that they can differ from the localhost
ones without touching them:
  - force-ssl-redirect: HOST is only served over TLS to ingress-nginx. cloudflared
    connects to ingress-nginx over https (k8s/manifests/cloudflared.yaml), so the scheme
    ingress-nginx passes on (X-Forwarded-Proto) is the connection's real one, and a
    plain-http request for HOST, say from the LAN to the host's port 80, is redirected
    to https://HOST instead of reaching the app or Keycloak in clear text. No client
    header decides the scheme or host: ingress-nginx runs without use-forwarded-headers.
  - the admin and debug routes (/admin, /api/admin, /playground) are left out; they stay
    reachable on http://localhost only.
  - econ-graph-ingress-public carries from-to-www-redirect: www.HOST answers with a
    permanent redirect to https://HOST, same path.

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
# The Ingress whose public copy carries the www redirect (ingress-nginx adds one redirect
# server per host, so one Ingress for the apex is enough).
WWW_REDIRECT_INGRESS = "econ-graph-ingress"
PUBLIC_LABEL = "econgraph.io/public-host"
# Paths served on localhost only, never on the public host (prefix match).
LOCAL_ONLY_PATHS = ("/admin", "/api/admin", "/playground")
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


def local_only(path):
    p = path.get("path", "/")
    return any(p == prefix or p.startswith(prefix + "/") for prefix in LOCAL_ONLY_PATHS)


def public_ingress(doc, host):
    """The public copy of a localhost Ingress, or None when it has no localhost rule."""
    rules = []
    for rule in doc.get("spec", {}).get("rules") or []:
        if rule.get("host") != "localhost":
            continue
        rule = copy.deepcopy(rule)
        rule["host"] = host
        paths = [p for p in rule.get("http", {}).get("paths", []) if not local_only(p)]
        if paths:
            rule["http"]["paths"] = paths
            rules.append(rule)
    if not rules:
        return None
    meta = doc["metadata"]
    name = meta["name"]
    annotations = dict(meta.get("annotations") or {})
    annotations["nginx.ingress.kubernetes.io/force-ssl-redirect"] = "true"
    if name == WWW_REDIRECT_INGRESS:
        annotations["nginx.ingress.kubernetes.io/from-to-www-redirect"] = "true"
    labels = dict(meta.get("labels") or {})
    labels[PUBLIC_LABEL] = "true"
    spec = copy.deepcopy(doc["spec"])
    spec["rules"] = rules
    spec.pop("tls", None)
    return {
        "apiVersion": doc["apiVersion"],
        "kind": "Ingress",
        "metadata": {
            "name": f"{name}-public",
            "namespace": meta.get("namespace"),
            "labels": labels,
            "annotations": annotations,
        },
        "spec": spec,
    }


def add_cors_origin(doc, host):
    data = doc.setdefault("data", {})
    origin = f"https://{host}"
    current = [o.strip() for o in (data.get("CORS_ORIGIN") or "").split(",") if o.strip()]
    data["CORS_ORIGIN"] = ",".join([origin] + [o for o in current if o != origin])


def render(doc, host, present=()):
    """The documents to apply for DOC; PRESENT names the Ingresses already in the input."""
    kind = doc.get("kind")
    meta = doc.get("metadata") or {}
    if kind == "Ingress":
        if (meta.get("labels") or {}).get(PUBLIC_LABEL) == "true":
            return [doc]  # already rendered
        if f"{meta.get('name')}-public" in present:
            return [doc]
        public = public_ingress(doc, host)
        return [doc] + ([public] if public else [])
    if kind in ("Deployment", "StatefulSet", "Job"):
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
    present = {(d.get("metadata") or {}).get("name") for d in docs if d.get("kind") == "Ingress"}
    out = [r for d in docs for r in render(d, host, present)]
    yaml.safe_dump_all(out, sys.stdout, sort_keys=False, default_flow_style=False)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
