#!/usr/bin/env python3
"""Set who reads each named catalog in a live econ-graph realm: admins only, nobody by
default, or everyone.

Run by scripts/keycloak/set-catalog-access.sh inside the cluster, with the same admin
credentials and URL as preserve-catalog-grants.py (KEYCLOAK_URL, KEYCLOAK_USER,
KEYCLOAK_PASSWORD). Usage:

  catalog-access.py [--dry-run] (--admin-only | --withdraw | --make-public) catalog.<source>:read ...

--withdraw removes the role from the catalog-public composite and from the anonymous
reader's scope mapping. --admin-only does the same and adds it to the admin composite
(super_admin includes admin). --make-public is the inverse of both: it adds the role to
catalog-public and the anonymous scope, and removes a direct grant on admin, which then
reads it through catalog-public. Each option takes one role and may repeat. Each step
runs only when needed, so a second run changes nothing. Grants are added before any is
removed, so a failure part way leaves admins their access; there is no rollback, and
running the same command again finishes the change. With --dry-run it reads Keycloak and prints what it would change, and changes
nothing. It refuses when the realm, the role or a composite is missing. It prints role and
composite names only, never a credential, token or response body.
"""
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request

REALM = "econ-graph"
API_CLIENT = "econ-graph-api"
ANONYMOUS_CLIENT = "econgraph-anonymous"
PUBLIC_ROLE = "catalog-public"
ADMIN_ROLE = "admin"
CATALOG_READ = re.compile(r"^catalog\.[a-z0-9_]+:read$")


class Refused(Exception):
    """A precondition failed; nothing was changed."""


class NoRedirect(urllib.request.HTTPRedirectHandler):
    """Keep administrator credentials on the configured Keycloak server."""
    def redirect_request(self, request, response, code, message, headers, new_url):
        return None


class Keycloak:
    """The Admin REST API of one realm. Errors name the request, never its body."""
    def __init__(self, base_url, user, password):
        self.base_url = base_url.rstrip("/")
        self.opener = urllib.request.build_opener(NoRedirect())
        data = urllib.parse.urlencode({"grant_type": "password", "client_id": "admin-cli",
                                       "username": user, "password": password}).encode()
        try:
            request = urllib.request.Request(self.base_url + "/realms/master/protocol/openid-connect/token", data=data)
            with self.opener.open(request, timeout=30) as response:
                self.token = json.load(response)["access_token"]
        except (urllib.error.URLError, TimeoutError, ValueError, KeyError):
            raise RuntimeError("Keycloak administrator authentication failed") from None
        self.prefix = "/admin/realms/" + urllib.parse.quote(REALM, safe="")

    def call(self, method, path, body=None, missing_ok=False):
        """JSON from METHOD on a realm path; None for 404 when missing_ok."""
        data = None if body is None else json.dumps(body).encode()
        request = urllib.request.Request(self.base_url + self.prefix + path, data=data, method=method,
                                         headers={"Authorization": "Bearer " + self.token,
                                                  "Content-Type": "application/json"})
        try:
            with self.opener.open(request, timeout=30) as response:
                raw = response.read()
                return json.loads(raw) if raw else None
        except urllib.error.HTTPError as error:
            if error.code == 404 and missing_ok:
                return None
            raise RuntimeError("%s %s answered HTTP %s" % (method, path.split("?")[0], error.code)) from None
        except (urllib.error.URLError, TimeoutError, ValueError):
            raise RuntimeError("%s %s failed or answered malformed JSON" % (method, path.split("?")[0])) from None


def q(name):
    return urllib.parse.quote(name, safe="")


def check_role(kc, role_name):
    """The API client's id and the role, refusing when the realm, client, role or a
    composite this script edits is missing."""
    if kc.call("GET", "", missing_ok=True) is None:
        raise Refused("realm %s does not exist" % REALM)
    api = kc.call("GET", "/clients?clientId=" + q(API_CLIENT))
    if not api:
        raise Refused("client %s does not exist" % API_CLIENT)
    api_id = api[0]["id"]
    role = kc.call("GET", "/clients/%s/roles/%s" % (api_id, q(role_name)), missing_ok=True)
    if role is None:
        raise Refused("%s has no role %s" % (API_CLIENT, role_name))
    for composite in (PUBLIC_ROLE, ADMIN_ROLE):
        if kc.call("GET", "/roles/" + q(composite), missing_ok=True) is None:
            raise Refused("realm role %s does not exist" % composite)
    return api_id, role


def plan_and_apply(kc, role_name, mode, dry_run, out):
    """Print each change for one role and, unless dry_run, make it. mode is "admin-only",
    "withdraw" or "public". Returns the number of changes."""
    api_id, role = check_role(kc, role_name)
    anonymous = kc.call("GET", "/clients?clientId=" + q(ANONYMOUS_CLIENT))

    def holds(path):
        return any(r["name"] == role_name for r in kc.call("GET", path))

    edge = [{"id": role["id"], "name": role["name"], "clientRole": True, "containerId": api_id}]
    public_path = "/roles/%s/composites" % q(PUBLIC_ROLE)
    admin_path = "/roles/%s/composites" % q(ADMIN_ROLE)
    public_read = "/roles/%s/composites/clients/%s" % (q(PUBLIC_ROLE), api_id)
    admin_read = "/roles/%s/composites/clients/%s" % (q(ADMIN_ROLE), api_id)
    scope_path = ("/clients/%s/scope-mappings/clients/%s" % (anonymous[0]["id"], api_id)
                  if anonymous else None)
    if not anonymous:
        out("note: there is no %s client, so anonymous visitors read no catalog" % ANONYMOUS_CLIENT)

    # Where each grant must end up: None leaves it as it is (--withdraw leaves admin alone).
    want = {
        "admin-only": {"public": False, "admin": True, "scope": False},
        "withdraw": {"public": False, "admin": None, "scope": False},
        "public": {"public": True, "admin": False, "scope": True},
    }[mode]
    grants = {"public": (PUBLIC_ROLE, public_read, public_path),
              "admin": (ADMIN_ROLE, admin_read, admin_path)}
    if scope_path:
        grants["scope"] = ("the %s scope" % ANONYMOUS_CLIENT, scope_path, scope_path)
    # The order keeps every partial failure safe: grants are added before any is removed, so
    # admins never lose a catalog midway, and catalog-public changes before the anonymous scope.
    # There is no rollback; the steps are idempotent, so running the command again finishes it.
    order = ["admin", "public", "scope"] if mode != "public" else ["public", "scope", "admin"]
    steps = []  # (description, method, path)
    for key in order:
        if key not in grants or want[key] is None:
            continue
        name, read_path, write_path = grants[key]
        if holds(read_path) != want[key]:
            if want[key]:
                steps.append(("add %s to %s" % (role_name, name), "POST", write_path))
            elif key == "admin":
                # admin reaches catalog-public through support and user, so the direct grant
                # would only be left over.
                steps.append(("remove %s from %s (it reads it through %s)" % (role_name, name, PUBLIC_ROLE),
                              "DELETE", write_path))
            else:
                steps.append(("remove %s from %s" % (role_name, name), "DELETE", write_path))

    target = {"admin-only": "admin-only", "withdraw": "not public", "public": "public"}[mode]
    if not steps:
        out("%s is already %s; nothing to change" % (role_name, target))
        return 0
    for description, method, path in steps:
        if dry_run:
            out("would " + description)
        else:
            kc.call(method, path, edge)
            out(description)
    if dry_run:
        return len(steps)
    # Read back every grant the change set.
    wrong = []
    for key in order:
        if key in grants and want[key] is not None:
            name, read_path, _ = grants[key]
            if holds(read_path) != want[key]:
                wrong.append("%s %s it" % (name, "lacks" if want[key] else "still holds"))
    if wrong:
        raise RuntimeError("after the change %s is not %s: %s; run the command again"
                           % (role_name, target, "; ".join(wrong)))
    if anonymous and mode != "public":
        account = kc.call("GET", "/clients/%s/service-account-user" % anonymous[0]["id"], missing_ok=True)
        if account and holds("/users/%s/role-mappings/clients/%s/composite" % (account["id"], api_id)):
            out("note: the %s service account still holds %s through another mapping; its scope no longer "
                "admits it, so anonymous tokens leave it out" % (ANONYMOUS_CLIENT, role_name))
    out("%s is now %s" % (role_name, target))
    return len(steps)


MODES = {"--admin-only": "admin-only", "--withdraw": "withdraw", "--make-public": "public"}


def parse(args):
    """[(mode, role)] from the arguments, or None when they don't parse."""
    pairs = []
    while args:
        if args[0] not in MODES or len(args) < 2:
            return None
        pairs.append((MODES[args[0]], args[1]))
        args = args[2:]
    roles = [role for _, role in pairs]
    return pairs if pairs and len(set(roles)) == len(roles) else None


def main(argv):
    args = argv[1:]
    dry_run = "--dry-run" in args
    pairs = parse([a for a in args if a != "--dry-run"])
    if pairs is None:
        print(__doc__.split("\n\n")[1], file=sys.stderr)
        print("(each role at most once)", file=sys.stderr)
        return 2
    for _, role_name in pairs:
        if not CATALOG_READ.match(role_name):
            print("refused: %r is not a catalog read role (catalog.<source>:read)" % role_name, file=sys.stderr)
            return 2
    try:
        kc = Keycloak(os.environ["KEYCLOAK_URL"], os.environ["KEYCLOAK_USER"], os.environ["KEYCLOAK_PASSWORD"])
        # Check every role before changing any, so a typo in the second changes nothing.
        for _, role_name in pairs:
            check_role(kc, role_name)
        for mode, role_name in pairs:
            plan_and_apply(kc, role_name, mode, dry_run, lambda line: print(line, flush=True))
        if dry_run:
            print("dry run: nothing changed", flush=True)
    except Refused as error:
        print("refused: %s; nothing changed" % error, file=sys.stderr)
        return 3
    except KeyError as error:
        print("error: %s is not set" % error, file=sys.stderr)
        return 1
    except RuntimeError as error:
        # Steps printed above were made; nothing after the failure was. There is no rollback:
        # every step is idempotent, so running the same command again completes the change.
        print("error: %s. Only the changes printed above were made; run the same command again "
              "to finish" % error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
