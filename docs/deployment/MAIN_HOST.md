# Main public address

The base domain is `PUBLIC_HOST` (for production, `econgraph.com`). `MAIN_HOST`
selects the main public address and defaults to `www.${PUBLIC_HOST}`; an explicit
`MAIN_HOST=${PUBLIC_HOST}` selects the apex. Without a public host, the deployment
is localhost-only. Both settings are read from the environment or the operator's
deploy configuration (`deploy.env`, see `scripts/deploy/lib/deploy-config.sh`).

The gateway serves the application and Keycloak only on the main HTTPS host.
HTTP on both public names and HTTPS on the other name redirect to the main HTTPS
host, preserving the path and query. The TLS certificate covers both names.
Keycloak's hostname and realm `frontendUrl`, the web client's callback/logout URLs
and origins, and the backend's exact token issuer follow `MAIN_HOST`. The public
realm import sets `frontendUrl` as well as the client settings; an import without
a public host clears it. The frontend resolves its relative issuer
`/idp/realms/econ-graph` against the page origin, after canonicalization. Its
canonical link uses the page origin and path without the query or fragment.

Backend CORS permits `https://econgraph.com,https://www.econgraph.com,http://localhost`
whichever name is the main host. It does not permit `http://localhost:3000` or
arbitrary origins. CORS permission on both names does not permit a second token issuer.

## Redirect status

| Main host | Redirects to it |
| --- | --- |
| The apex | 301 (permanent) |
| `www.${PUBLIC_HOST}`, before `--finalize` | 302 (temporary) |
| `www.${PUBLIC_HOST}`, after `--finalize` | 301 (permanent) |

The status covers every redirect the gateway makes to the main host: https on the
other name and plain http on both names, including the main host's own http-to-https
redirect, which is therefore also a 302 until `--finalize`. Through Cloudflare, plain
http never reaches the gateway (the tunnel's origin is its https listener):
Cloudflare's Always Use HTTPS answers it with its own 301 to https on the same name,
and `check-public-site.sh --via-cloudflare` checks that hop and then the https one.

A permanent redirect is cached by browsers with no expiry, and by Cloudflare. A
temporary one is not kept, so a rollback from www to the apex works for every
visitor until the switch is finalized. `switch-main-host.sh --finalize` records
`MAIN_HOST_PERMANENT=www.${PUBLIC_HOST}` in the deploy configuration; later deploys
render permanent redirects from it. A switch to a different main host drops a
`MAIN_HOST_PERMANENT` that names another host, so the new direction starts temporary.

## Operator procedure

Run the production steps from a reviewed checkout that contains the script, with the
kind cluster's own kubectl. Tests and dry runs use stubbed `kubectl` and `curl`; they
do not touch a live cluster, production data, the operator's deploy configuration,
or host ports 80 and 443.

Normal deploys and release deploys refuse to change a live `MAIN_HOST`: a change
of main host runs through `scripts/deploy/switch-main-host.sh`, which invalidates
existing realm sessions with the issuer change. The live `keycloak-realm` ConfigMap
must match the checkout, including the realm file, renderer, and catalog-grant
preservation helper; the switch refuses a mismatch, so deploy the release that
contains the checkout first, with `MAIN_HOST` set to the main host it serves.
`cutover-to-gateway.sh` resolves `MAIN_HOST` the same way and stops when Keycloak's
live `KC_HOSTNAME` is on the other name. `render-public-host.py` has no default main
host: a render by hand (such as a realm import rerun) names the served one in
`MAIN_HOST`.

Every run except `--dry-run` needs `--confirm-production` with the first 8
characters of the production cluster's kube-system UID, as `release-deploy.sh`
does. The script prints the cluster's identity, UID included, before it checks it.

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

DNS and tunnel configuration need no change: both names reach the same Cloudflare
Tunnel, and both tunnel hostname entries forward the visitor's Host header to the
gateway over HTTPS. The script reports these as no-op steps; keep both entries,
their DNS records, the tunnel credentials, and the existing TLS trust settings. A
different base domain needs both names provisioned before using this procedure.

Apply the coordinated gateway and sign-in change:

```bash
scripts/deploy/switch-main-host.sh --context kind-econ-graph --host econgraph.com --main-host www.econgraph.com --confirm-production UID-PREFIX
```

The script updates the existing deployments without selecting new application
images. In order, it:

1. records the main host in the operator deploy configuration, so later deploys
   use it;
2. when the issuer changes, marks `deployment/keycloak` with the annotation
   `econgraph.io/issuer-change-logout-pending`;
3. sets Keycloak's hostname and web client origins and waits for its rollout;
4. reruns the realm import, which preserves catalog grants and moves the realm
   `frontendUrl` (it overrides the hostname) to the main host, so Keycloak now
   issues tokens for the new issuer;
5. when marked, signs out every realm session, then removes the mark; sessions begun
   during steps 3 and 4 are signed out too, and a rerun after an interruption still
   signs out;
6. sets the backend's expected issuer and the CORS origins, rolls the app on the
   configuration change (the app-config hash `deploy.sh` uses) and waits;
7. applies the gateway routes and runs `check-public-site.sh`.

Keycloak's issuer moves before the backend's, so old backend pods reject tokens for
the new issuer (including the anonymous client-credentials renewal) only from the end
of the realm import until the backend's rollout completes. Until the gateway routes
change in step 7, sign-in started on the old main address fails, since the web
client accepts only the new one. Every step is idempotent: on a partial
failure, inspect the reported failed step and rerun the same command after fixing its
cause.

After the switch, purge the Cloudflare cache (below), then check through Cloudflare
as a visitor:

```bash
scripts/deploy/check-public-site.sh --host econgraph.com --main-host www.econgraph.com --via-cloudflare
```

Also complete the [browser and token checks](../release/qa-scenarios.md#main-address-and-sign-in-checks).
A successful command-line check does not prove a browser has no cached permanent
redirect. Browsers can retain an earlier www-to-apex 301; combined with the
apex-to-www redirect it produces a client-side loop. Test a fresh browser profile
and a previously used profile. Affected users must clear cached site redirects/data
and reopen the main address.

Browser tabs left open on the old main address need a reload: their `/graphql`
POSTs are redirected and arrive as GETs.

### Finalize

Once the switch is accepted, make the redirects to www permanent:

```bash
scripts/deploy/switch-main-host.sh --context kind-econ-graph --host econgraph.com --finalize --confirm-production UID-PREFIX
```

It refuses, changing nothing, unless the switch to www completed: Keycloak's
hostname, the realm `frontendUrl`, the backend's issuer and finished rollout, the
deploy configuration and the served routes are all on www, and `check-public-site.sh`
passes with the temporary redirects. It then records `MAIN_HOST_PERMANENT`, reapplies
the gateway routes and checks the public site again. It changes neither Keycloak nor the backend, and rerunning it changes
nothing. Purge the Cloudflare cache afterwards. After finalizing, a rollback leaves
browsers that cached the 301 looping until they clear it.

## Cloudflare cache purge

Envoy's redirects carry no `Cache-Control`, and Cloudflare caches 301s for cacheable
paths (such as `/robots.txt`) for about two hours by default. After each switch,
finalize and rollback, purge the cache for both names in the Cloudflare dashboard
(Caching, Configuration, Purge Cache: purge everything, or custom purge by hostname
for `econgraph.com` and `www.econgraph.com`). The scripts do not call Cloudflare.

## Rollback

Select the apex explicitly with the same script. Preview it first:

```bash
scripts/deploy/switch-main-host.sh --context kind-econ-graph --host econgraph.com --main-host econgraph.com --dry-run
```

Then apply the rollback and purge the Cloudflare cache:

```bash
scripts/deploy/switch-main-host.sh --context kind-econ-graph --host econgraph.com --main-host econgraph.com --confirm-production UID-PREFIX
```

The rollback runs the same steps in the same order. It restores the apex routes,
issuer, realm `frontendUrl`, web client settings and canonical page origin together,
records `MAIN_HOST=econgraph.com` and drops `MAIN_HOST_PERMANENT` for later deploys.
Both public CORS origins, DNS/tunnel entries, and TLS certificate names stay in
place. It is safe to rerun after a partial rollback. Switching the issuer back forces
another sign-in; do not accept tokens from both issuers to hide the interruption.
Redirects to the apex are permanent. Before finalize, the apex-to-www redirect was
temporary, so browsers have not kept it; after finalize, browsers that cached the
301 must clear it.
