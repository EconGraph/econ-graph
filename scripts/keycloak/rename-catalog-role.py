#!/usr/bin/env python3
"""Rename one econ-graph-api catalog read role in a live Keycloak realm, keeping its grants.

Keycloak keeps role mappings, composites and scope mappings by role id, so renaming a role in
place moves every grant with it. Use it before redeploying a release from before a catalog role
rename (config/keycloak/README.md, "Renaming a catalog role"): that release's realm file and
preserve-catalog-grants.py know only the old name, so the role must carry the old name again
before its realm import runs.

Usage:
  KEYCLOAK_URL=http://localhost:18080/idp KEYCLOAK_USER=admin KEYCLOAK_PASSWORD=... \\
    scripts/keycloak/rename-catalog-role.py --from NEW_NAME --to OLD_NAME [--realm econ-graph] [--dry-run]

Idempotent: when --to already exists and --from does not, it reports that and changes nothing.
It refuses when both exist (two roles would need their grants merged), when neither exists,
and for any name that is not a catalog.*:read role.
"""
import argparse
import importlib.util
import os
import sys
import urllib.parse
from pathlib import Path

_spec = importlib.util.spec_from_file_location(
    "preserve", Path(__file__).resolve().parent / "preserve-catalog-grants.py")
preserve = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(preserve)


def rename_role(realm, old, new, get, send, dry_run=False):
    """Rename client role `old` of econ-graph-api to `new` in `realm`. Returns a one-line
    description of what was (or, with dry_run, would be) done."""
    for name in (old, new):
        if not preserve.catalog_read(name):
            raise RuntimeError("%s is not a catalog.*:read role" % name)
    if old == new:
        raise RuntimeError("--from and --to are the same role")
    prefix = "/admin/realms/" + urllib.parse.quote(realm, safe="")
    api = preserve.required(get(prefix + "/clients?clientId=" + preserve.API_CLIENT))
    if not api:
        raise RuntimeError("Realm %s has no %s client" % (realm, preserve.API_CLIENT))
    roles = prefix + "/clients/" + api[0]["id"] + "/roles/"
    current = get(roles + urllib.parse.quote(old, safe=""))
    target = get(roles + urllib.parse.quote(new, safe=""))
    if current is None and target is not None:
        return "%s already exists and %s does not; nothing to do" % (new, old)
    if current is None:
        raise RuntimeError("Neither %s nor %s exists in realm %s" % (old, new, realm))
    if target is not None:
        raise RuntimeError("Both %s and %s exist; move the grants of one to the other by hand "
                           "and delete it, then run this again" % (old, new))
    if dry_run:
        return "would rename %s to %s" % (old, new)
    body = dict(current, name=new)
    send("PUT", roles + urllib.parse.quote(old, safe=""), body)
    if get(roles + urllib.parse.quote(new, safe="")) is None:
        raise RuntimeError("Keycloak accepted the rename but %s is not there" % new)
    return "renamed %s to %s" % (old, new)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--from", dest="old", required=True, help="the role's current name")
    parser.add_argument("--to", dest="new", required=True, help="the name to give it")
    parser.add_argument("--realm", default="econ-graph")
    parser.add_argument("--dry-run", action="store_true", help="check and report; change nothing")
    args = parser.parse_args()
    client = preserve.Keycloak(os.environ["KEYCLOAK_URL"], os.environ["KEYCLOAK_USER"],
                               os.environ["KEYCLOAK_PASSWORD"], timeout=30)
    print(rename_role(args.realm, args.old, args.new, client.get, client.send, args.dry_run))


if __name__ == "__main__":
    try:
        main()
    except (KeyError, RuntimeError) as error:
        print("rename-catalog-role: " + (str(error) if isinstance(error, RuntimeError)
                                         else "set KEYCLOAK_URL, KEYCLOAK_USER and KEYCLOAK_PASSWORD"),
              file=sys.stderr)
        sys.exit(1)
