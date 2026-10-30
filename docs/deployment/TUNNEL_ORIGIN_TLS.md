# Verify the tunnel's connection to the gateway

The remotely managed tunnel initially accepts the gateway's self-signed certificate
without verification. A private cert-manager CA can authenticate that in-cluster hop.
The connector mounts only the public CA certificate; the signing key stays in the
cert-manager namespace. These steps require operator access to the selected cluster and
Cloudflare dashboard. They do not change visitor TLS at Cloudflare's edge.

## Stage trust before changing verification

Use the gateway deployment and choose its kubeconfig context explicitly. The script
assumes cert-manager's default cluster-resource namespace, `cert-manager`.

```sh
scripts/deploy/stage-tunnel-origin-ca.sh --context kind-econ-graph --dry-run
scripts/deploy/stage-tunnel-origin-ca.sh --context kind-econ-graph --apply
```

Staging creates the private CA and the `econ-graph-origin-ca` ClusterIssuer, waits for
readiness, copies only the root's public `tls.crt` into ConfigMap
`econ-graph/cloudflared-origin-ca`, and patches the connector's read-only mount. It leaves
the connector's replicas and token intact, then restarts it and waits for the rollout so
existing Pods receive the CA file before staging completes. When the ConfigMap already
holds that root and the mount is in place, it changes nothing and does not restart the
connector. Without a cloudflared Deployment it publishes only the CA and ConfigMap. It
does not select the gateway certificate issuer or change the remote tunnel configuration.
The normal connector manifest has the same optional mount, so a later deployment
preserves it.

The public certificate is at `/etc/cloudflared/origin-ca/ca.crt` in each connector. Confirm
the connector rollout is healthy before proceeding. Staging refuses to overwrite a
ConfigMap containing a different root, other than a renewal of the same root; use the
rotation procedure below instead.

## Select and verify the gateway certificate

Explicitly select the CA issuer for the public listener, keeping the existing public host.
Use the kind cluster's own kubectl, as the deploy scripts do (`KUBECTL`, else
`~/.cache/econgraph/bin/kubectl` with its kubeconfig); a plain `kubectl` on the host may
reach another cluster:

```sh
PUBLIC_HOST=econgraph.com GATEWAY_TLS_ISSUER=econ-graph-origin-ca \
  scripts/deploy/apply-gateway-routes.sh --context kind-econ-graph --dry-run
PUBLIC_HOST=econgraph.com GATEWAY_TLS_ISSUER=econ-graph-origin-ca \
  scripts/deploy/apply-gateway-routes.sh --context kind-econ-graph
KUBECONFIG=~/.cache/econgraph/kubeconfig-econ-graph ~/.cache/econgraph/bin/kubectl \
  --context kind-econ-graph -n econ-graph-gateway wait \
  --for=condition=Ready certificate/econ-graph-tls --timeout=120s
```

With issuer `econ-graph-origin-ca`, `apply-gateway-routes.sh` (and so `deploy.sh` and
`restart-k8s-rollout.sh`, which run it) first runs
`stage-tunnel-origin-ca.sh --context <context> --apply`, which is idempotent; if staging
fails, it stops without changing the certificate. Its `--dry-run` prints that command
without running it. On later deploys the certificate's existing issuer is retained unless
`GATEWAY_TLS_ISSUER` overrides it, and the retained `econ-graph-origin-ca` is staged again
the same way.

Then record the selection in the host's deploy config, `~/.config/econgraph/deploy.env`
(the file that holds `PUBLIC_HOST`; `ECONGRAPH_CONFIG_DIR` or `ECONGRAPH_DEPLOY_ENV` name
another), by adding this line:

```sh
GATEWAY_TLS_ISSUER=econ-graph-origin-ca
```

The issuer is chosen in this order: `GATEWAY_TLS_ISSUER` in the environment, then that
line, then the certificate's existing issuer, then `econ-graph-selfsigned`.
`apply-gateway-routes.sh`, `deploy.sh` and `release-deploy.sh` all read the line through
`scripts/deploy/lib/deploy-config.sh`, and `release-deploy.sh` shows the issuer and its
source in its plan.

The leaf covers the public host and `www` name. In the dashboard, use the public host as
**Origin Server Name** even though the Service URL uses the internal proxy Service DNS.
Do not override the HTTP Host header: public HTTPRoutes match the visitor's host.

From a disposable diagnostic pod permitted to reach the gateway, use the public root to
verify the leaf's chain, hostname and validity. For example, with the root mounted at
`/etc/cloudflared/origin-ca/ca.crt`, use an image with OpenSSL:

```sh
openssl s_client \
  -connect econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local:443 \
  -servername econgraph.com -verify_hostname econgraph.com \
  -verify_return_error -CAfile /etc/cloudflared/origin-ca/ca.crt </dev/null
```

Require verification success. Repeat with `-verify_hostname wrong.invalid` and require
failure; repeat using an unrelated test CA and require failure. The connector image has
no shell/OpenSSL, so use a separate diagnostic pod rather than weakening that image.
Respect the gateway's NetworkPolicies when selecting the diagnostic pod's namespace.

## Switch the remote tunnel

For both public hostname routes in the Cloudflare dashboard, set:

| Setting | Value |
| --- | --- |
| Service URL | `https://econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local:443` |
| Origin Server Name (`originServerName`) | `econgraph.com` |
| CA Pool (`caPool`) | `/etc/cloudflared/origin-ca/ca.crt` |
| No TLS Verify (`noTLSVerify`) | **off / false** |
| Match SNI to host (`matchSNItoHost`) | false |
| HTTP Host header override | unset |

These settings are remotely managed; changing the Kubernetes Deployment alone does not
turn on verification. CA Pool is a path inside each connector container, not a file
uploaded to Cloudflare. Cloudflare documents that remote route settings accept this
connector-local `.crt`/PEM bundle. Keep Match SNI to host false so it does not override
Origin Server Name. See Cloudflare's [origin parameters](https://developers.cloudflare.com/tunnel/reference/origin-parameters/)
and [HTTPS-origin troubleshooting](https://developers.cloudflare.com/tunnel/troubleshooting/https-origins/).

Before the production change, prove cloudflared itself enforces these settings using an
operator-approved, access-restricted temporary test hostname/tunnel. Send its authenticated
requests through a test connector with the same public CA mount to this HTTPS origin;
only that test route may override HTTP Host to `econgraph.com` for routing. Require a
successful origin response with the correct name and trust bundle. Then set only the
test route's Origin Server Name to `wrong.invalid`, keep Match SNI to host false, and
require an origin TLS failure plus a connector x509 hostname error. Restore the correct
name, point only the test route at an unrelated public test-CA bundle mounted in the test
connector, and require an unknown-authority failure. Restore the valid settings and
require success again. Do not put intentionally invalid trust settings on production
routes. Record route configuration, responses and connector errors; OpenSSL tests alone
do not prove the remote connector is enforcing verification.

After that rehearsal, switch the production routes. Check both public hostnames,
Keycloak sign-in, `/graphql`, `/data/graphql` and the negative
admin-path checks with `scripts/deploy/check-public-site.sh`. Confirm the connector logs
show no x509/unknown-authority/name errors and the dashboard still shows healthy
connectors. Record the successful checks and the negative certificate tests before
considering the transition complete. A healthy tunnel connection to Cloudflare alone
does not prove its origin TLS is verified.

## After a cluster recreate

A recreated cluster has none of the CA, its ClusterIssuer, the trust ConfigMap or the
gateway certificate. With `GATEWAY_TLS_ISSUER=econ-graph-origin-ca` in the deploy config,
its first normal deploy stages a new CA and trust bundle and then issues the certificate
from it. CA Pool names the connector-local file, so the new root needs no dashboard
change. Wait for the certificate as above and repeat the positive origin check.

Without that line, the first deploy has no existing issuer to retain and issues the
certificate from `econ-graph-selfsigned`, which connectors verifying the origin reject.
Add the line and rerun `apply-gateway-routes.sh` as in the previous section, or turn No
TLS Verify back on for the public routes until the certificate is issued from the CA.

## Rollback and CA rotation

If public requests fail during cutover, first restore the dashboard's prior No TLS Verify
setting. This restores the earlier behavior while investigating. Retain the CA, trust
bundle and signing Secret; no database, backup or token changes are involved. If needed,
select the prior gateway issuer explicitly with `GATEWAY_TLS_ISSUER`, and remove or change
the deploy config's `GATEWAY_TLS_ISSUER` line so later deploys keep it; changing the issuer
can renew the leaf, so wait for readiness and retest.

cert-manager renews leaf certificates using the CA. The root has a ten-year lifetime and
reuses its key on renewal (`rotationPolicy: Never`); operators must monitor both root and
leaf expiry and rerun staging after root renewal before the old trust certificate expires.
Staging replaces the published root only with a renewal of it: a single certificate with
the same subject and public key, which still verifies leaves the old one signed. It
refuses any other change, including a different key or a two-root bundle. For a key rotation, issue a new CA under
a distinct Secret/issuer name, publish both roots to every connector, restart connectors
and verify that all replicas trust the new root, then switch the gateway leaf to the new
issuer. Repeat the positive and negative origin checks before retiring the old root from
the public bundle. Never mount a signing key into a connector or delete a root Secret as
part of staging.
