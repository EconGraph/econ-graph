#!/usr/bin/env python3
"""Render an explicit independent-data realm overlay; never changes the base export."""
import argparse
import copy
import json
import sys


def render(realm):
    realm = copy.deepcopy(realm)
    clients = realm["clients"]
    if any(c["clientId"] in {"econ-graph-data", "econ-graph-dev-readonly"} for c in clients):
        raise ValueError("independent-data clients already exist; refusing duplicate overlay")
    web = next(c for c in clients if c["clientId"] == "econ-graph-web")
    audience = {
        "name": "econ-graph-data audience", "protocol": "openid-connect",
        "protocolMapper": "oidc-audience-mapper", "consentRequired": False,
        "config": {"included.client.audience": "econ-graph-data", "access.token.claim": "true",
                   "id.token.claim": "false", "introspection.token.claim": "true"},
    }
    web.setdefault("protocolMappers", []).append(audience)
    roles = copy.deepcopy(next(m for m in web["protocolMappers"]
                               if m["protocolMapper"] == "oidc-usermodel-client-role-mapper"))
    clients.append({
        "clientId": "econ-graph-data", "name": "Independent data service",
        "enabled": True, "bearerOnly": True, "publicClient": False,
        "protocol": "openid-connect", "standardFlowEnabled": False,
        "implicitFlowEnabled": False, "directAccessGrantsEnabled": False,
        "serviceAccountsEnabled": False,
    })
    clients.append({
        "clientId": "econ-graph-dev-readonly", "name": "Read-only local development",
        "enabled": True, "publicClient": True, "protocol": "openid-connect",
        "standardFlowEnabled": True, "implicitFlowEnabled": False,
        "directAccessGrantsEnabled": False, "serviceAccountsEnabled": False,
        "fullScopeAllowed": False,
        "redirectUris": ["http://127.0.0.1:8765/callback"], "webOrigins": [],
        "attributes": {"pkce.code.challenge.method": "S256"},
        "defaultClientScopes": ["basic", "profile", "email"], "optionalClientScopes": [],
        "protocolMappers": [copy.deepcopy(audience), roles],
    })
    catalog_roles = [r["name"] for r in realm["roles"]["client"]["econ-graph-api"]
                     if r["name"].startswith("catalog.") and r["name"].endswith(":read")]
    if not catalog_roles:
        raise ValueError("realm has no catalog read roles")
    realm.setdefault("clientScopeMappings", {}).setdefault("econ-graph-api", []).append({
        "client": "econ-graph-dev-readonly", "roles": catalog_roles,
    })
    return realm


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("realm")
    args = parser.parse_args()
    try:
        with open(args.realm, encoding="utf-8") as source:
            json.dump(render(json.load(source)), sys.stdout, indent=2)
        print()
    except (ValueError, KeyError, StopIteration) as error:
        parser.exit(2, f"invalid independent-data overlay: {error}\n")
