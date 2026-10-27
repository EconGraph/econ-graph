# Let's Encrypt with Cloudflare DNS-01

Use this procedure to request an optional Let's Encrypt certificate
stored in a Kubernetes TLS Secret. cert-manager proves control of Cloudflare-hosted
DNS, requests the certificate and renews it. DNS-01 supports wildcard names and
services without an inbound HTTP validation endpoint.

The [public-site route](../../k8s/README.md#-public-site-cloudflare-tunnel) is a
separate configuration: visitor TLS terminates at Cloudflare, and cloudflared
connects to the Envoy Gateway HTTPS listener with **No TLS Verify**. Issuing a
Kubernetes certificate neither changes the visitor certificate nor configures the
Tunnel, application DNS records or public routes. Follow that
public-site guide for routing and trusted client-IP handling.

## Prerequisites and credentials

Choose the intended kubectl context for the standalone commands below. The target
cluster needs cert-manager and its `cert-manager.io/v1` CRDs, plus namespace
`econ-graph` for the standalone wildcard requests. Installing cert-manager is
outside this procedure; use its [installation guidance](https://cert-manager.io/docs/installation/)
for a version compatible with the cluster.

The example uses `yourdomain.com`, namespace `econ-graph`, and a Cloudflare API-token
Secret named `cloudflare-api-token-secret` with key `api-token`. Replace the domain
with a zone you control and the ACME email with an operator contact. Keep resource
names consistent through the issuer and Certificate steps. The wildcard exercise
creates its own TLS Secret; the managed public Gateway requests a separate
certificate, as described below.

A human operator must provision that credential Secret through a trusted mechanism
that follows the [secrets policy](../technical/SECRETS_MANAGEMENT.md). Credential
entry is operator-only: agents must never read or enter the token, and values must
not appear in command arguments, logs, example files or public git. This procedure
starts with an existing Secret; it does not provide a credential-entry wrapper.
The token needs `Zone:DNS:Edit` and `Zone:Zone:Read`, scoped to the zone being
validated. See the [cert-manager Cloudflare configuration](https://cert-manager.io/docs/configuration/acme/dns01/cloudflare/)
for the solver's credential fields and permission diagnostics.

For a ClusterIssuer, referenced Secrets live in cert-manager's *cluster resource
namespace*, normally `cert-manager`. If that setting differs, use the configured
namespace in the checks below. See [issuer configuration](https://cert-manager.io/docs/configuration/).
The ACME account-key Secret is distinct from the Cloudflare token Secret;
cert-manager creates it at the `privateKeySecretRef.name` configured below.

Check only the credential Secret's name and required key presence:

```bash
kubectl config current-context
kubectl get secret cloudflare-api-token-secret -n cert-manager \
  -o custom-columns=NAME:.metadata.name,TYPE:.type
kubectl get secret cloudflare-api-token-secret -n cert-manager \
  -o go-template='{{if index .data "api-token"}}api-token key present{{else}}api-token key missing or empty{{end}}{{"\n"}}'
```

These checks do not establish that Cloudflare accepts the token or grants access
to the zone. Issuance conditions and events provide that diagnostic evidence.
Do not dump the Secret as YAML or JSON; base64 encoding does not protect a value.

## Request a staging certificate

Prepare the following non-secret manifests locally and run the commands from their
directory. Use staging first to check DNS permissions and issuance without using
production issuance limits. [Staging certificates](https://letsencrypt.org/docs/staging-environment/)
are not publicly trusted; keep their Secret separate from production consumers.

Save this as `issuer-staging.yaml`:

```yaml
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: letsencrypt-staging-cloudflare
spec:
  acme:
    server: https://acme-staging-v02.api.letsencrypt.org/directory
    email: admin@yourdomain.com
    privateKeySecretRef:
      name: letsencrypt-staging-cloudflare
    solvers:
    - dns01:
        cloudflare:
          apiTokenSecretRef:
            name: cloudflare-api-token-secret
            key: api-token
```

Save this as `certificate-staging.yaml`:

```yaml
apiVersion: cert-manager.io/v1
kind: Certificate
metadata:
  name: yourdomain.com-wildcard-staging
  namespace: econ-graph
spec:
  secretName: yourdomain.com-wildcard-staging
  issuerRef:
    name: letsencrypt-staging-cloudflare
    kind: ClusterIssuer
  dnsNames:
  - yourdomain.com
  - "*.yourdomain.com"
```

The apex is listed separately because `*.yourdomain.com` covers a single subdomain
label, such as `api.yourdomain.com`, but not `yourdomain.com` or
`nested.api.yourdomain.com`. Add any deeper names explicitly if needed.

```bash
kubectl apply -f issuer-staging.yaml
kubectl wait --for=condition=Ready clusterissuer/letsencrypt-staging-cloudflare --timeout=120s
kubectl apply -f certificate-staging.yaml
kubectl wait --for=condition=Ready certificate/yourdomain.com-wildcard-staging -n econ-graph --timeout=600s
kubectl describe certificate yourdomain.com-wildcard-staging -n econ-graph
```

A wait timeout is a reason to inspect issuance, not proof that it has permanently
failed. During [DNS-01 validation](https://letsencrypt.org/docs/challenge-types/#dns-01-challenge),
cert-manager presents a TXT record at `_acme-challenge.yourdomain.com` through the
Cloudflare API and removes its challenge record afterwards. Both names can require
TXT records at that location. Application A, AAAA or CNAME proxy status does not
need to change for this validation.

## Request a production wildcard certificate

After staging succeeds, copy `issuer-staging.yaml` to `issuer-production.yaml` and
`certificate-staging.yaml` to `certificate-production.yaml`. Change these fields:

| Resource | Field | Production value |
| --- | --- | --- |
| ClusterIssuer | `metadata.name` | `letsencrypt-prod-cloudflare` |
| ClusterIssuer | `spec.acme.server` | `https://acme-v02.api.letsencrypt.org/directory` |
| ClusterIssuer | `spec.acme.privateKeySecretRef.name` | `letsencrypt-prod-cloudflare` |
| Certificate | `metadata.name` | `yourdomain.com-wildcard-tls` |
| Certificate | `spec.secretName` | `yourdomain.com-wildcard-tls` |
| Certificate | `spec.issuerRef.name` | `letsencrypt-prod-cloudflare` |

Keep the contact email, DNS names, namespace and token reference appropriate for
the target installation. Separate account-key and TLS Secret names keep staging
and production resources independent.

```bash
kubectl apply -f issuer-production.yaml
kubectl wait --for=condition=Ready clusterissuer/letsencrypt-prod-cloudflare --timeout=120s
kubectl apply -f certificate-production.yaml
kubectl wait --for=condition=Ready certificate/yourdomain.com-wildcard-tls -n econ-graph --timeout=600s
kubectl get secret yourdomain.com-wildcard-tls -n econ-graph \
  -o custom-columns=NAME:.metadata.name,TYPE:.type
```

cert-manager writes the certificate and private key into the Certificate's
`spec.secretName` in the Certificate's namespace. This standalone wildcard Secret
is not attached to the repository's public Gateway. Adding a wildcard to a
certificate does not create subdomain routes; each intended hostname still needs
routing and application configuration.

## Select the managed Gateway's issuer

The [public-site guide](../../k8s/README.md#-public-site-cloudflare-tunnel) describes
the supported Gateway and Tunnel deployment, including its
[production-cluster prerequisite](../../k8s/README.md#production-marker).
After the production ClusterIssuer is ready, select it for the Gateway through
the normal deployment from the repository root:

```bash
PUBLIC_HOST=yourdomain.com GATEWAY_TLS_ISSUER=letsencrypt-prod-cloudflare \
  ./scripts/deploy/deploy.sh
```

This deployment configures the public host as well as its certificate.
`PUBLIC_HOST` retains the base domain, while `MAIN_HOST` selects the served
address and defaults to `www.${PUBLIC_HOST}`; see [Main public address](MAIN_HOST.md). It uses the
explicit Certificate `econ-graph-tls` in namespace `econ-graph-gateway`, defined in
[the HTTPS manifest](../../k8s/manifests/gateway/gateway-https.yaml). That Certificate
requests `yourdomain.com` and `www.yourdomain.com` from the selected ClusterIssuer
and writes Secret `econ-graph-tls` in the same namespace. The Gateway's `https`
listener references that Secret. It does not consume the wildcard Secret in
`econ-graph`, and selecting the issuer does not configure the Cloudflare Tunnel or
application DNS records. Keep the managed manifest as the owner; do not create a
second Gateway or Certificate for its serving Secret.

When `GATEWAY_TLS_ISSUER` is unset, the
[route application script](../../scripts/deploy/apply-gateway-routes.sh) first uses
a saved issuer in the operator deploy configuration, otherwise retains the existing
Certificate's issuer. If no override, saved issuer or existing issuer is available,
it uses `econ-graph-selfsigned`, which the Tunnel connector accepts
without verification. Choosing a Let's Encrypt issuer does not change the visitor
certificate served by Cloudflare or enable origin verification in the Tunnel.

Check the Certificate that the Gateway actually uses, keeping Secret checks to
metadata:

```bash
kubectl get certificate econ-graph-tls -n econ-graph-gateway \
  -o custom-columns=NAME:.metadata.name,ISSUER:.spec.issuerRef.name,SECRET:.spec.secretName,DNS_NAMES:.spec.dnsNames
kubectl wait --for=condition=Ready certificate/econ-graph-tls -n econ-graph-gateway --timeout=600s
kubectl describe certificate econ-graph-tls -n econ-graph-gateway
kubectl get secret econ-graph-tls -n econ-graph-gateway \
  -o custom-columns=NAME:.metadata.name,TYPE:.type
kubectl get gateway econ-graph-gateway -n econ-graph-gateway \
  -o jsonpath='{range .spec.listeners[?(@.name=="https")].tls.certificateRefs[*]}{.name}{"\n"}{end}'
```

Certificate readiness does not establish that the public routes or Tunnel work;
use the public-site guide's checks for those. That guide also explains visitor
address handling: the Gateway trusts forwarding information from pod addresses,
so Tunnel visitors retain individual addresses, while in-cluster callers can
claim an address. This trust scope is separate from certificate issuance.

## Existing setup script

The [setup script](../../scripts/deploy/setup-letsencrypt-cloudflare.sh) creates a
Cloudflare credential Secret, selects a staging or production ClusterIssuer,
requests `${DOMAIN}-wildcard-tls` in the selected namespace, and waits for it.
Its ACME email comes from `ACME_EMAIL`, defaulting to `support@econgraph.com`.
It does not attach that wildcard Secret to the Gateway.

The script and `test-letsencrypt-cloudflare.sh` select one context,
`kind-econ-graph` unless `--context` names another, without changing the current
context. When `KUBECTL` and `KUBECONFIG` are unset, they use the kind cluster's own
kubectl and kubeconfig in `~/.cache/econgraph`; plain kubectl on the production
host selects MicroK8s. All credential and certificate writes use that one context.
The script prints the cluster identity first; `--dry-run` stops before writes.

A human operator can run the script and paste the token at its hidden prompt:

```bash
scripts/deploy/setup-letsencrypt-cloudflare.sh -d yourdomain.com --staging
# After testing, use the production ACME issuer:
scripts/deploy/setup-letsencrypt-cloudflare.sh -d yourdomain.com
```

A secure environment source may supply `CLOUDFLARE_API_TOKEN` instead. `-t` and
`--token` are refused. Never type a token into arguments, shell history,
repository files or chat; disable caller tracing as well as the script's tracing.
The script passes the credential through the environment to
`create-secrets.sh --cloudflare-dns-only --context=<context>`. That mode writes
only `cert-manager/cloudflare-api-token-secret`, key `api-token`, through private
temporary files that are removed on exit. It applies server-side without putting
the value in kubectl arguments or a last-applied-configuration annotation.

`--namespace` chooses the Certificate namespace (default `econ-graph`); the
credential stays in cert-manager's default cluster resource namespace,
`cert-manager`. The setup script uses the same `${DOMAIN}-wildcard-tls` name for
staging and production, unlike the separate names in the manual procedure above.
Keep staging requests isolated from production consumers. Credential-only updates
can use `create-secrets.sh --cloudflare-dns-only` with a secure environment source;
its `--dry-run` validates access and inputs without writing.

## Renewal and monitoring

cert-manager renews the Certificate and updates its TLS Secret. The CA determines
the issued lifetime; `spec.duration` is a request, not a guarantee. The examples
leave renewal settings unset so cert-manager schedules renewal at two-thirds of
the actual issued lifetime by default. Inspect `status.renewalTime` and
`status.notAfter` on the Certificate.

If choosing explicit settings, `duration: 2160h` requests 90 days and
`renewBefore: 720h` asks to renew 30 days before expiry. These are example choices,
not an allowed range or a CA promise. `duration` must exceed `renewBefore`; an
absolute renewal interval too close to the actual issued lifetime can cause a
renewal loop. See the [Certificate renewal guidance](https://cert-manager.io/docs/usage/certificate/#reissuance-triggered-by-expiry-renewal)
for defaults, constraints and percentage-based renewal supported by the installed
version.

Monitor Certificate readiness as well as expiry. If Prometheus scrapes cert-manager
and Prometheus Operator selects rules from the chosen namespace and labels, adapt
this `PrometheusRule` to the installation. The [metrics setup](https://cert-manager.io/docs/devops-tips/prometheus-metrics/)
is separate from creating this rule. The seven-day threshold allows intervention
near expiry; a rule does not detect missing scrape targets or prove renewal works.

```yaml
apiVersion: monitoring.coreos.com/v1
kind: PrometheusRule
metadata:
  name: certificate-expiry
  namespace: econ-graph
spec:
  groups:
  - name: certificates
    rules:
    - alert: CertificateExpiringSoon
      expr: certmanager_certificate_expiration_timestamp_seconds - time() < 86400 * 7
      for: 5m
      labels:
        severity: warning
      annotations:
        summary: "Certificate expiring soon"
        description: "Certificate {{ $labels.name }} expires in {{ $value | humanizeDuration }}"
```

Keep the token restricted to the necessary zone. When rotating it, the human
operator updates the existing credential Secret through the trusted provisioning
mechanism, then checks subsequent issuance or renewal conditions without printing
the value. Keep the previous issuer available during a certificate migration.
For separate Secret consumers, retain the old serving Secret until the replacement
is issued and its consumers have been checked. The managed Gateway updates the
same serving Secret, so changing its issuer back can require another issuance.

## Diagnose issuance

Inspect the Certificate first, then its CertificateRequest, Order and Challenge
resources. Use the actual object names returned by these commands for a detailed
`kubectl describe` of a failed request or Challenge:

```bash
kubectl describe certificate yourdomain.com-wildcard-tls -n econ-graph
kubectl describe clusterissuer letsencrypt-prod-cloudflare
kubectl get certificaterequests,orders,challenges -n econ-graph
kubectl get events -n econ-graph --field-selector involvedObject.name=yourdomain.com-wildcard-tls
kubectl logs deployment/cert-manager -n cert-manager --tail=100
```

Use `yourdomain.com-wildcard-staging` and `letsencrypt-staging-cloudflare` for the
staging resources. To diagnose the managed Gateway certificate, use
`econ-graph-tls` and namespace `econ-graph-gateway` instead of the wildcard
Certificate and `econ-graph` in these commands. `Ready=True` on a Certificate
indicates issuance readiness; issuer readiness alone does not prove that its
Cloudflare solver can update DNS.
Challenge listings show current state, not a historical success rate. Review logs
locally and remove any sensitive content before sharing diagnostics.

For authentication or permission errors, have the operator check token expiry,
zone scope and both required permissions. A zone-discovery error can also mean
cert-manager inferred the wrong zone from DNS; the Cloudflare solver reference
above explains that distinction. A missing key requires repairing provisioning,
not printing or decoding the Secret.

For a propagation or validation error, inspect the Challenge's reason and expected
DNS name. Query that name while the challenge is active, and compare recursive
answers with the zone's authoritative nameservers:

```bash
dig NS yourdomain.com
dig TXT _acme-challenge.yourdomain.com
# Replace ns.example.net with an authoritative nameserver returned above.
dig @ns.example.net TXT _acme-challenge.yourdomain.com
```

Investigate delegation, conflicting records, DNS reachability, API rate limits or
ACME errors indicated by the resource conditions. Propagation time depends on DNS
caching and delegation; changing the application's Cloudflare proxy status does
not fix TXT validation. TXT cleanup after successful validation is expected.

## Migrate from HTTP-01

Create the DNS-01 issuer alongside the existing HTTP-01 issuer and test the
separate staging Certificate first. Prepare the production DNS-01 issuer and
verify it with the standalone production request before selecting it for the
managed Gateway. Record the current `econ-graph-tls` issuer, host and serving
configuration for rollback, then deploy with `GATEWAY_TLS_ISSUER` set to the
production DNS-01 issuer as shown above. The deployment updates the existing
explicit Certificate; do not introduce another Certificate for `econ-graph-tls`.

Check that the Gateway Certificate becomes ready, verify the serving configuration
with the public-site checks and monitor renewal. Keep the previous issuer
available during the transition. If rollback is needed, redeploy the same public
host with `GATEWAY_TLS_ISSUER` explicitly set to the previous issuer; omitting the
variable retains the newly selected issuer. Recheck issuance and serving behavior
after rollback. Remove the HTTP-01 issuer only when no remaining Certificates
depend on it. Public Tunnel routing remains a separate operator concern.
