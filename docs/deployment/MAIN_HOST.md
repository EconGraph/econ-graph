# Main public address

The public address is `https://www.econgraph.com`. The base domain remains
`PUBLIC_HOST=econgraph.com`; `MAIN_HOST` selects the main host and defaults to
`www.${PUBLIC_HOST}`. An explicit `MAIN_HOST=${PUBLIC_HOST}` selects the apex.
Without a public host, the deployment remains localhost-only.

The gateway serves the application and Keycloak only on the main HTTPS host.
HTTP on both public names and HTTPS on the other name return a 301 to the main
HTTPS host, preserving the path and query. The TLS certificate covers both names.
Keycloak's hostname and realm `frontendUrl`, the web client's callback/logout URLs
and origins, and the backend's exact token issuer follow `MAIN_HOST`. The public
realm import explicitly updates `frontendUrl` as well as the client settings.
The frontend resolves its relative issuer `/idp/realms/econ-graph` against the page origin, after canonicalization. Its
canonical link uses the page origin and path without the query or fragment. Release
images must still pass the check that their issuer is nonempty.

Backend CORS permits `https://econgraph.com,https://www.econgraph.com,http://localhost`
through the switch. It does not permit `http://localhost:3000` or arbitrary origins.
CORS permission on both names does not permit a second token issuer.

## Operator procedure

Joe runs the production steps. Agents validate this procedure only with stubbed
`kubectl` and `curl`; they do not access the live clusters `econ-graph` or
`econgraph-train1-qa`, production databases, `/srv/econgraph`,
`~/econgraph-backups`, `~/.config/econgraph`, or host ports 80 and 443.

Use a reviewed checkout containing the script and the kind cluster's own kubectl.
Normal deploys and release deploys refuse to change a live `MAIN_HOST`: the switch
must run through the operator script so existing realm sessions are invalidated.
For a production site still using the apex, first retain `MAIN_HOST=econgraph.com`
in its deploy configuration and deploy the reviewed release with that setting.
This installs the new realm helper without changing the live issuer. The live
`keycloak-realm` ConfigMap must match this checkout, including the realm file,
renderer, and catalog-grant preservation helper; the switch refuses a mismatch.
Then run the switch below to select www.

Before changing the main host, confirm that the Google OAuth client authorizes
`https://www.econgraph.com/idp/realms/econ-graph/broker/google/endpoint`. Keep the apex
broker redirect authorized while rollback remains possible. Take the normal verified
Keycloak backup and schedule a brief sign-in interruption. A failed intermediate
step can leave sign-in unavailable until the same command or the rollback completes.

Preview the complete plan without contacting a cluster or reading the operator's
configuration:

```bash
scripts/deploy/switch-main-host.sh --context kind-econ-graph --host econgraph.com --main-host www.econgraph.com --dry-run
```

DNS and tunnel configuration need no change: both names already reach the same
Cloudflare Tunnel, and both tunnel hostname entries already forward the visitor's
Host header to the gateway over HTTPS. The script reports these as no-op steps;
keep both entries, their DNS records, the tunnel credentials, and the existing TLS
trust settings. A different base domain needs both names provisioned before using
this procedure.

Apply the coordinated gateway and sign-in change:

```bash
scripts/deploy/switch-main-host.sh --context kind-econ-graph --host econgraph.com --main-host www.econgraph.com
```

The script updates the existing deployments without selecting new application
images, applies the realm through its admin API while preserving catalog grants,
updates the backend's issuer and CORS, and changes the gateway routes. It persists
the main-host choice in the operator deploy configuration so subsequent deploys use
it. It waits for readiness and checks the resulting public behavior. The steps are
idempotent: on a partial failure, inspect the reported failed step and rerun the same
command after fixing its cause.

Check through Cloudflare as a visitor:

```bash
scripts/deploy/check-public-site.sh --host econgraph.com --main-host www.econgraph.com --via-cloudflare
```

Also complete the [browser and token checks](../release/qa-scenarios.md#main-address-and-sign-in-checks).
A successful command-line check does not prove a browser has no cached permanent
redirect. Browsers can retain the previous www-to-apex 301; combining it with the
new apex-to-www 301 can produce a client-side loop. Test a fresh browser profile
and a previously used profile. Affected users must clear cached site redirects/data
and reopen the main address. Check any configured edge redirect cache as well.

## Rollback

Select the apex explicitly with the same script. Preview it first:

```bash
scripts/deploy/switch-main-host.sh --context kind-econ-graph --host econgraph.com --main-host econgraph.com --dry-run
```

Then apply the rollback:

```bash
scripts/deploy/switch-main-host.sh --context kind-econ-graph --host econgraph.com --main-host econgraph.com
```

This restores the apex routes, issuer, web client settings and canonical page
origin together, and records `MAIN_HOST=econgraph.com` for later deploys. Both public
CORS origins, DNS/tunnel entries, and TLS certificate names stay in place. It is
safe to rerun after a partial rollback. Switching the issuer back forces another
sign-in; do not accept tokens from both issuers to hide the interruption. Recheck
cached redirects in existing browsers: an apex-to-www 301 cached during the switch
can conflict with the restored www-to-apex redirect. Clear those redirects before
judging rollback behavior.

## Release notice

The main public address is now **https://www.econgraph.com**. Existing links to
https://econgraph.com redirect to it. The sign-in issuer changes with the address,
so everyone is signed out once and must sign in again; existing sessions and tokens
are no longer usable. If an existing browser reports a redirect loop, clear its
cached site redirects/data and reopen https://www.econgraph.com.
