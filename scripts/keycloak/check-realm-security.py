#!/usr/bin/env python3
"""Check the econ-graph realm's account-containment settings (ECO-889).

The backend verifies access tokens offline and has no revocation, so a token for a disabled,
de-roled or deleted account works until it expires. The API relies on these realm settings:

  - Access tokens live at most MAX_ACCESS_TOKEN_LIFESPAN seconds (5 minutes), realm-wide and
    on every client. Disabling a user in Keycloak therefore stops their API access within
    5 minutes, which is the containment path for a compromised account.
  - Staff must pass an OTP. The realm's browser flow, and the post-login flow of every
    identity provider, run an OTP form for anyone holding STAFF_ROLE (support; admin and
    super_admin include it), and every realm role that grants an admin.* capability includes
    STAFF_ROLE.
  - No client offers the password grant, which skips the browser flow and so the OTP. Only
    local docker-compose turns it on (KC_WEB_DIRECT_GRANTS=true).

It reads a realm JSON file: by default config/keycloak/econ-graph-realm.json, whose
${VAR:default} placeholders it reads at their defaults. To check a live realm, pass an
export of it with --realm. In the admin console, open the econ-graph realm, Realm settings,
Action, Partial export, with "Include groups and roles" and "Include clients" on. This script
never contacts Keycloak.

Usage:
  scripts/keycloak/check-realm-security.py [--realm PATH]

Exits 0 when every check passes, 1 when one fails, 2 on a usage or input error.
"""

import argparse
import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
DEFAULT_REALM = REPO_ROOT / "config" / "keycloak" / "econ-graph-realm.json"
API_CLIENT = "econ-graph-api"
# The longest access-token lifetime the API relies on, in seconds. Keep the realm file's
# accessTokenLifespan, docs/security/API_REFERENCE.md and config/keycloak/README.md in step.
MAX_ACCESS_TOKEN_LIFESPAN = 300
# The realm role every staff composite includes; the OTP condition tests for it.
STAFF_ROLE = "support"
# Clients Keycloak creates in every realm. A live export lists them; admin-cli allows the
# password grant by default, but its tokens are not for econ-graph-api, so the API refuses them.
KEYCLOAK_CLIENTS = {
    "account",
    "account-console",
    "admin-cli",
    "broker",
    "realm-management",
    "security-admin-console",
}
PLACEHOLDER = re.compile(r"^\$\{[A-Za-z_][A-Za-z0-9_]*(?::(.*))?\}$")


def value(raw):
    """A realm value, with a ${VAR:default} placeholder read at its default."""
    if isinstance(raw, str):
        match = PLACEHOLDER.match(raw)
        if match:
            return match.group(1)
    return raw


def as_int(raw):
    raw = value(raw)
    try:
        return int(raw)
    except (TypeError, ValueError):
        return None


def truthy(raw):
    raw = value(raw)
    if isinstance(raw, str):
        return raw.strip().lower() == "true"
    return bool(raw)


def check_token_lifespan(realm):
    problems = []
    lifespan = as_int(realm.get("accessTokenLifespan"))
    if lifespan is None:
        problems.append("accessTokenLifespan is not set; Keycloak's default would apply")
    elif not 0 < lifespan <= MAX_ACCESS_TOKEN_LIFESPAN:
        problems.append(
            f"accessTokenLifespan is {lifespan}s; the API relies on at most {MAX_ACCESS_TOKEN_LIFESPAN}s"
        )
    for client in realm.get("clients") or []:
        override = (client.get("attributes") or {}).get("access.token.lifespan")
        if override in (None, ""):
            continue
        seconds = as_int(override)
        if seconds is None or not 0 < seconds <= MAX_ACCESS_TOKEN_LIFESPAN:
            problems.append(
                f"client {client.get('clientId')!r} sets access.token.lifespan to {override!r}; "
                f"the API relies on at most {MAX_ACCESS_TOKEN_LIFESPAN}s"
            )
    return problems


def check_no_password_grant(realm):
    problems = []
    for client in realm.get("clients") or []:
        if client.get("clientId") in KEYCLOAK_CLIENTS:
            continue
        if truthy(client.get("directAccessGrantsEnabled")):
            problems.append(
                f"client {client.get('clientId')!r} allows the password grant, which skips the staff OTP"
            )
    return problems


def flows_by_alias(realm):
    return {flow.get("alias"): flow for flow in realm.get("authenticationFlows") or []}


def configs_by_alias(realm):
    return {config.get("alias"): config.get("config") or {} for config in realm.get("authenticatorConfig") or []}


def staff_otp_subflows(realm, alias, seen=None):
    """Aliases of the CONDITIONAL subflows reachable from flow `alias` that run a required OTP
    form for STAFF_ROLE, and nothing else as a condition."""
    flows = flows_by_alias(realm)
    configs = configs_by_alias(realm)
    seen = set() if seen is None else seen
    found = []
    flow = flows.get(alias)
    if flow is None or alias in seen:
        return found
    seen.add(alias)
    for item in flow.get("authenticationExecutions") or []:
        if not item.get("authenticatorFlow"):
            continue
        child = item.get("flowAlias")
        if item.get("requirement") == "CONDITIONAL" and is_staff_otp(flows.get(child), configs):
            found.append(child)
        elif item.get("requirement") in ("REQUIRED", "ALTERNATIVE", "CONDITIONAL"):
            found.extend(staff_otp_subflows(realm, child, seen))
    return found


def is_staff_otp(flow, configs):
    if flow is None:
        return False
    conditions, otp = [], False
    for item in flow.get("authenticationExecutions") or []:
        if item.get("requirement") == "DISABLED":
            continue
        authenticator = item.get("authenticator") or ""
        if authenticator.startswith("conditional-"):
            conditions.append(item)
        elif authenticator == "auth-otp-form" and item.get("requirement") == "REQUIRED":
            otp = True
    if not otp or len(conditions) != 1 or conditions[0].get("authenticator") != "conditional-user-role":
        return False
    config = configs.get(conditions[0].get("authenticatorConfig")) or {}
    return config.get("condUserRole") == STAFF_ROLE and str(config.get("negate", "false")).lower() != "true"


def check_staff_otp(realm):
    problems = []
    flows = flows_by_alias(realm)
    browser = realm.get("browserFlow")
    if not browser:
        problems.append("browserFlow is not set, so Keycloak's default browser flow (no staff OTP) runs")
    elif browser not in flows:
        problems.append(f"browserFlow {browser!r} is not in authenticationFlows")
    elif not staff_otp_subflows(realm, browser):
        problems.append(
            f"browser flow {browser!r} has no conditional subflow requiring an OTP for role {STAFF_ROLE!r}"
        )
    for provider in realm.get("identityProviders") or []:
        alias = provider.get("alias")
        post = provider.get("postBrokerLoginFlowAlias")
        if not post:
            problems.append(f"identity provider {alias!r} has no post-login flow, so staff signing in there skip the OTP")
        elif post not in flows:
            problems.append(f"identity provider {alias!r} names post-login flow {post!r}, which is not in authenticationFlows")
        elif not staff_otp_subflows(realm, post):
            problems.append(
                f"identity provider {alias!r}'s post-login flow {post!r} does not require an OTP for role {STAFF_ROLE!r}"
            )

    realm_roles = {role.get("name"): role for role in (realm.get("roles") or {}).get("realm") or []}
    if STAFF_ROLE not in realm_roles:
        problems.append(f"realm role {STAFF_ROLE!r}, which the staff OTP tests for, is not defined")

    def includes(name, seen):
        """Realm roles and econ-graph-api roles that realm role `name` grants, itself included."""
        if name in seen:
            return set(), set()
        seen.add(name)
        composites = (realm_roles.get(name) or {}).get("composites") or {}
        realm_names, api_names = {name}, set(((composites.get("client") or {}).get(API_CLIENT)) or [])
        for child in composites.get("realm") or []:
            child_realm, child_api = includes(child, seen)
            realm_names |= child_realm
            api_names |= child_api
        return realm_names, api_names

    for name in sorted(realm_roles):
        realm_names, api_names = includes(name, set())
        staff = sorted(role for role in api_names if role.startswith("admin."))
        if staff and STAFF_ROLE not in realm_names:
            problems.append(
                f"realm role {name!r} grants {', '.join(staff)} without including {STAFF_ROLE!r}, "
                "so its holders are not asked for an OTP"
            )
    return problems


def check(realm):
    """Every problem with `realm`; empty when it passes."""
    return check_token_lifespan(realm) + check_no_password_grant(realm) + check_staff_otp(realm)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--realm", type=Path, default=DEFAULT_REALM, help="realm JSON file (default: the repo's)")
    args = parser.parse_args(argv)
    try:
        realm = json.loads(args.realm.read_text())
    except (OSError, ValueError) as error:
        print(f"error: cannot read {args.realm}: {error}", file=sys.stderr)
        return 2
    if not isinstance(realm, dict):
        print(f"error: {args.realm} is not a realm export", file=sys.stderr)
        return 2
    problems = check(realm)
    for problem in problems:
        print(f"FAIL: {problem}", file=sys.stderr)
    if problems:
        return 1
    print(
        f"{args.realm}: access tokens last at most {MAX_ACCESS_TOKEN_LIFESPAN}s, staff need an OTP, "
        "no client offers the password grant"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
