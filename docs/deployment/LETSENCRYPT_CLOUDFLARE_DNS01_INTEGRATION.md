# Let's Encrypt with Cloudflare DNS-01

`scripts/deploy/setup-letsencrypt-cloudflare.sh` configures a cert-manager
ClusterIssuer and a wildcard certificate using Cloudflare DNS-01. It does not
change ingress or gateway routes. The public site's Cloudflare Tunnel token is a
separate credential; this script needs a DNS API token.

## Create a token for one zone

In [Cloudflare API Tokens](https://dash.cloudflare.com/profile/api-tokens),
create a custom token with **Zone:DNS:Edit** and **Zone:Zone:Read**. Set Zone
Resources to **Include → Specific zone → econgraph.com** (or the zone you are
issuing for). Grant no account permissions or access to other zones. Do not use
the Global API Key.

DNS edit is needed to create and remove challenge TXT records. Zone read is
also required by [cert-manager's Cloudflare solver](https://cert-manager.io/docs/configuration/acme/dns01/cloudflare/)
to discover the zone. An existing token with broader scope must be reissued by
the maintainer in Cloudflare; changing a local script does not narrow it.

## Configure the certificate

Install cert-manager and select the intended Kubernetes context first. By
default the script uses the cached kind kubectl and kubeconfig when available,
matching the other deployment scripts. Explicit `KUBECTL` and `KUBECONFIG` select
another cluster. The context is pinned for the credential and certificate
writes.

Run the script yourself and paste the token at the hidden prompt:

```bash
scripts/deploy/setup-letsencrypt-cloudflare.sh -d econgraph.com --staging
# After testing, use the production ACME issuer:
scripts/deploy/setup-letsencrypt-cloudflare.sh -d econgraph.com
```

A secure environment source may supply `CLOUDFLARE_API_TOKEN` instead. Never type
the token into a command, shell history, repository file or chat. `-t` and
`--token` are refused. Avoid shell tracing in the caller as well; the scripts
disable their own tracing before handling credentials.

The script passes the token through the environment to
`create-secrets.sh --cloudflare-dns-only`, which writes only
`cert-manager/cloudflare-api-token-secret` (key `api-token`). That mode uses
private temporary files, removes them on exit, and applies the Secret
server-side, without putting its value in kubectl arguments or the
last-applied-configuration annotation. Other application Secrets are untouched.
For a credential-only update, use that mode directly after supplying the token
through a secure environment source; `--dry-run` validates access and inputs
without writing.

The ClusterIssuer is `letsencrypt-staging-cloudflare` with `--staging`, otherwise
`letsencrypt-prod-cloudflare`. `ACME_EMAIL` overrides the default contact
`support@econgraph.com`. `--namespace` selects the Certificate namespace (default
`econ-graph`); the credential remains in cert-manager's default ClusterIssuer
resource namespace, `cert-manager`.

The generated Certificate and TLS Secret are named `<domain>-wildcard-tls`.
Configure the intended gateway or ingress listener to reference that Secret in
the same namespace. Issuing a certificate alone does not switch live routing.

## Verify and troubleshoot

```bash
kubectl describe certificate econgraph.com-wildcard-tls -n econ-graph
kubectl get clusterissuer letsencrypt-prod-cloudflare
kubectl get challenges -A
kubectl logs deployment/cert-manager -n cert-manager
kubectl get secret cloudflare-api-token-secret -n cert-manager -o name
```

Check the token's permissions and selected zone if zone discovery or DNS writes
fail. Inspect challenge events and authoritative DNS propagation if issuance
waits. DNS-01 uses TXT records; changing the proxy setting on A/AAAA records is
not required. Do not print Secret YAML or decode the token when troubleshooting.
After replacing a token, verify certificate renewal before revoking the old one.
