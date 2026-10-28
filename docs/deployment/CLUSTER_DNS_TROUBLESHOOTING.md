# Cluster DNS Troubleshooting Guide

Use this runbook when a cert-manager DNS-01 challenge reports a DNS lookup or
propagation failure. First identify the resolver path and compare the failing
query through that path and its upstreams. Change forwarding only when those
results support the change.

The public Gateway uses a self-signed origin certificate by default; visitor TLS
terminates at Cloudflare. That self-signed issuer does not use DNS-01. Deployment
retains an existing Gateway Certificate's issuer unless `GATEWAY_TLS_ISSUER`
overrides it, so inspect the actual issuer before investigating ACME challenges.
For other public-site TLS or routing failures, use the
[Gateway troubleshooting guide](./GATEWAY_ROUTING_TROUBLESHOOTING.md) and
[public-site configuration](../../k8s/README.md#-public-site-cloudflare-tunnel).

## Recognize the failing lookup

These recorded failure excerpts show an SOA lookup failing through cluster DNS:

```text
[INFO] 10.1.26.103:47908 - 44624 "SOA IN _acme-challenge.econgraph.com. udp 58 false 4096" SERVFAIL qr,rd,ra 47 0.017550642s

E1003 07:07:01.123197       1 sync.go:208] "propagation check failed" err="Could not determine the zone for \"_acme-challenge.econgraph.com.\": When querying the SOA record for the domain '_acme-challenge.econgraph.com.' using nameservers [10.152.183.10:53], rcode was expected to be 'NOERROR' or 'NXDOMAIN', but got 'SERVFAIL'"
```

The address is an observation from that failure, not a resolver to copy into
another cluster. The excerpts establish failed zone discovery; they do not
identify the upstream that failed or establish a historical root cause.

By default, cert-manager uses recursive resolvers from its `/etc/resolv.conf` to
find authoritative nameservers, then queries the authoritative servers directly
for its DNS-01 self-check. SOA queries help find the zone; TXT queries check the
challenge record. The controller flags `--dns01-recursive-nameservers` and
`--dns01-recursive-nameservers-only` can change that path. Inspect them before
assuming that a cluster DNS change will affect the self-check. See
[cert-manager's nameserver settings](https://cert-manager.io/docs/configuration/acme/dns01/#setting-nameservers-for-dns01-self-check).

A recursive resolver resolves queries on behalf of its clients. An authoritative
server serves the zone's records. Cloudflare hosting the zone does not require
CoreDNS to forward to Cloudflare's public recursive resolver.

## Select the cluster and discover resources

Use the kubectl and kubeconfig intended for the target cluster; the repository's
[deployment entry point](../../k8s/README.md) describes kind. Run the following
inspection examples from an operator shell. Replace every `REPLACE_*` value from
the selected cluster before using later examples. Keep the explicit context in
all commands.

```bash
kubectl config get-contexts
DNS_CONTEXT='REPLACE_CONTEXT'
kubectl --context="$DNS_CONTEXT" get deployments,pods,services,configmaps -n kube-system
kubectl --context="$DNS_CONTEXT" get deployments,pods -A
kubectl --context="$DNS_CONTEXT" get gateways.gateway.networking.k8s.io,certificates,orders.acme.cert-manager.io,challenges.acme.cert-manager.io -A
```

Discover the DNS workload and its mounted ConfigMaps rather than assuming they
are called `coredns`. Follow the service endpoints to the DNS pods, and inspect
the workload's volume mounts, Corefile, imported files, and configuration owner
(for example, an add-on manager or Helm release). If NodeLocal DNSCache or another
resolver sits in front of CoreDNS, include it in the path.

```bash
DNS_NAMESPACE='REPLACE_DNS_NAMESPACE'
DNS_DEPLOYMENT='REPLACE_DNS_DEPLOYMENT'
DNS_CONFIGMAP='REPLACE_DNS_CONFIGMAP'
CM_NAMESPACE='REPLACE_CERT_MANAGER_NAMESPACE'
CM_DEPLOYMENT='REPLACE_CERT_MANAGER_CONTROLLER_DEPLOYMENT'
CM_POD='REPLACE_CERT_MANAGER_CONTROLLER_POD'
CM_CONTAINER='REPLACE_CERT_MANAGER_CONTROLLER_CONTAINER'

kubectl --context="$DNS_CONTEXT" -n "$DNS_NAMESPACE" get deployment "$DNS_DEPLOYMENT" -o yaml
kubectl --context="$DNS_CONTEXT" -n "$DNS_NAMESPACE" get configmap "$DNS_CONFIGMAP" -o yaml
kubectl --context="$DNS_CONTEXT" -n "$CM_NAMESPACE" get deployment "$CM_DEPLOYMENT" \
  -o jsonpath='{range .spec.template.spec.containers[*]}{.name}{"\n"}{.args}{"\n"}{end}'
kubectl --context="$DNS_CONTEXT" -n "$CM_NAMESPACE" get pod "$CM_POD" \
  -o jsonpath='{.spec.dnsPolicy}{"\n"}{.spec.dnsConfig}{"\n"}'
```

For a DNS workload that is not a Deployment, inspect its actual workload kind.
For the affected TLS connection, discover the Gateway and inspect its HTTPS
listener's `tls.certificateRefs`. The shipped public listener is `https` on
Gateway `econ-graph-gateway` in namespace `econ-graph-gateway`, referencing Secret
`econ-graph-tls`. The application HTTPRoutes can be in a different namespace;
their namespace does not determine where the certificate lives.

```bash
GATEWAY_NAMESPACE='REPLACE_GATEWAY_NAMESPACE'
GATEWAY='REPLACE_GATEWAY_NAME'
kubectl --context="$DNS_CONTEXT" -n "$GATEWAY_NAMESPACE" get gateway "$GATEWAY" \
  -o jsonpath='{range .spec.listeners[*]}{.name}{"\t"}{.protocol}{"\t"}{.tls.certificateRefs}{"\n"}{end}'

CERT_NAMESPACE='REPLACE_REFERENCED_SECRET_NAMESPACE'
kubectl --context="$DNS_CONTEXT" -n "$CERT_NAMESPACE" get certificates \
  -o custom-columns='NAME:.metadata.name,SECRET:.spec.secretName,DNS_NAMES:.spec.dnsNames,ISSUER:.spec.issuerRef.name'
```

For a Secret reference, use its explicit namespace, or the Gateway namespace if
omitted. In that namespace, select the Certificate whose `spec.secretName`
matches the reference and whose `spec.dnsNames` covers the affected domain. The
shipped Certificate is also named `econ-graph-tls`; discover the names in your
cluster rather than inferring them from a previous incident. No Secret contents
are needed for these checks.

```bash
CERTIFICATE='REPLACE_CERTIFICATE_NAME'
kubectl --context="$DNS_CONTEXT" -n "$CERT_NAMESPACE" describe certificate "$CERTIFICATE"
kubectl --context="$DNS_CONTEXT" -n "$CERT_NAMESPACE" get certificate "$CERTIFICATE" \
  -o jsonpath='{.spec.issuerRef}{"\n"}'
```

Read the referenced issuer's type and solver configuration. For a cert-manager
`Issuer` (the default when `issuerRef.kind` is omitted), use the Certificate's
namespace; for a `ClusterIssuer`, omit the namespace:

```bash
ISSUER='REPLACE_ISSUER_NAME'
# Run the command matching the selected Certificate's issuerRef.kind.
kubectl --context="$DNS_CONTEXT" -n "$CERT_NAMESPACE" get issuer "$ISSUER" \
  -o jsonpath='{.spec.selfSigned}{"\n"}{.spec.acme.server}{"\n"}{.spec.acme.solvers}{"\n"}'
kubectl --context="$DNS_CONTEXT" get clusterissuer "$ISSUER" \
  -o jsonpath='{.spec.selfSigned}{"\n"}{.spec.acme.server}{"\n"}{.spec.acme.solvers}{"\n"}'
```

Continue with this runbook when the selected Certificate uses an ACME DNS-01
solver and has an actual failing DNS-01 Challenge. An issuer can have multiple
solvers; check the solver selected for the affected name. A self-signed
Certificate has no ACME Order or Challenge to inspect.

```bash
CHALLENGE='REPLACE_ACTIVE_CHALLENGE_NAME'
kubectl --context="$DNS_CONTEXT" -n "$CERT_NAMESPACE" describe challenge "$CHALLENGE"
kubectl --context="$DNS_CONTEXT" -n "$CM_NAMESPACE" logs deployment/"$CM_DEPLOYMENT" --all-pods=true --since=15m
```

Use the Challenge's DNS name, reason, presentation state, and owner references to
follow the relevant Order and CertificateRequest. If presentation failed with a
provider API error, investigate the selected Issuer's Cloudflare zone access and
token permissions through the
[DNS-01 integration guide](./LETSENCRYPT_CLOUDFLARE_DNS01_INTEGRATION.md).
Changing CoreDNS will not repair an API authorization failure.

## Compare the same queries along the resolver path

Read the controller container's resolver configuration if its image supports
`cat`; otherwise use an approved diagnostic method that can inspect that
container's resolver file. A different pod's `/etc/resolv.conf` is not evidence
of the controller's settings.

```bash
kubectl --context="$DNS_CONTEXT" -n "$CM_NAMESPACE" exec "$CM_POD" -c "$CM_CONTAINER" -- cat /etc/resolv.conf
```

Run the queries below inside an existing approved diagnostic container with
`dig`, using network access comparable to the controller. Confirm its DNS policy
and resolver file. Different pod or node network policies can make results differ
from the controller's; direct upstream comparisons also need to account for the
DNS server's egress path. This runbook does not require creating a test workload.
An unqualified `dig` on a host tests that host's resolver, not cluster DNS.

Set the variables in that diagnostic shell. `SELF_CHECK_RECURSOR` is the actual
recursive resolver used by cert-manager, from its flags or resolver file.
`CURRENT_UPSTREAM` comes from the active forwarding configuration and, when it
uses `/etc/resolv.conf`, the DNS server container's resolver file.
`CANDIDATE_UPSTREAM` is a resolver permitted by the cluster's network and DNS
policy. Use fully qualified names with a trailing dot. For a wildcard certificate,
the challenge name normally omits `*.`; account for any configured challenge
zone delegation or CNAME following.

```bash
CHALLENGE_FQDN='REPLACE_CHALLENGE_FQDN_WITH_TRAILING_DOT'
ZONE_FQDN='REPLACE_ZONE_FQDN_WITH_TRAILING_DOT'
SELF_CHECK_RECURSOR='REPLACE_RESOLVER_IP'
CURRENT_UPSTREAM='REPLACE_CURRENT_UPSTREAM_IP'
CANDIDATE_UPSTREAM='REPLACE_APPROVED_CANDIDATE_IP'

for resolver in "$SELF_CHECK_RECURSOR" "$CURRENT_UPSTREAM" "$CANDIDATE_UPSTREAM"; do
  dig @"$resolver" "$CHALLENGE_FQDN" SOA +time=3 +tries=1
  dig @"$resolver" "$ZONE_FQDN" SOA +time=3 +tries=1
  dig @"$resolver" "$CHALLENGE_FQDN" TXT +time=3 +tries=1
  dig @"$resolver" "$ZONE_FQDN" NS +time=3 +tries=1
done
```

Repeat for every configured upstream, and use `+tcp` to compare TCP when UDP
fails or times out. Retain the response status, answer, authority section, and
server address; `+short` hides useful failure evidence. If necessary, repeat the
comparisons from the DNS server's network path before attributing a difference
to forwarding.

Use the discovered NS records to identify the authoritative servers, resolve
their addresses, then query each server directly from the diagnostic container:

```bash
AUTHORITATIVE_IP='REPLACE_DISCOVERED_AUTHORITATIVE_SERVER_IP'
dig @"$AUTHORITATIVE_IP" "$ZONE_FQDN" SOA +norecurse +time=3 +tries=1
dig @"$AUTHORITATIVE_IP" "$CHALLENGE_FQDN" TXT +norecurse +time=3 +tries=1
```

Repeat authoritative and public recursive checks from an external network when
comparing internal and public visibility. Label those results with their origin;
they establish external visibility, not the controller's connectivity. See
[cert-manager's DNS-01 troubleshooting](https://cert-manager.io/docs/troubleshooting/acme/#dns01-troubleshooting).

| Observation | Interpretation and next check |
| --- | --- |
| SOA lookup returns `NOERROR` with no answer, or `NXDOMAIN`, at the challenge name | These can be valid results while discovering the enclosing zone. Check the authority section and parent-zone SOA; neither response establishes TXT propagation. |
| SOA lookup returns `SERVFAIL`, `REFUSED`, or times out | Zone discovery cannot proceed through that path. Compare the same name and record type through each upstream before selecting a repair. |
| Cluster path fails while current upstreams answer from a comparable network path | Inspect DNS service endpoints, local caches, CoreDNS forwarding rules, and network policy. This alone does not justify replacing the upstreams. |
| Current upstream fails but an approved candidate answers the same SOA queries from the DNS server's path | An upstream change may help; confirm other required zones and network policy before changing forwarding. |
| Authoritative servers lack the expected TXT during an active, presented challenge | Investigate presentation, zone selection, delegation, or authoritative propagation. TXT absence outside an active challenge is expected after cleanup. |
| Authoritative servers have the expected TXT but recursive resolvers do not | Compare cache TTLs and split DNS views. Allow for propagation and negative caching; changing resolvers does not establish public visibility. |
| SOA and active-challenge TXT checks pass but issuance remains blocked | Follow Challenge, Order, CertificateRequest, and Certificate conditions and events. DNS success alone does not establish certificate issuance. |

## Repair forwarding only when the comparison supports it

Before changing DNS, preserve the complete current ConfigMap, any imported
configuration, the workload definition, and the configuration owner's source.
For example, the following saves the selected ConfigMap locally:

```bash
kubectl --context="$DNS_CONTEXT" -n "$DNS_NAMESPACE" get configmap "$DNS_CONFIGMAP" -o yaml > coredns-before.yaml
```

Prepare the change through that owner's supported configuration mechanism so an
add-on reconciliation does not overwrite it. Limit the edit to the failing
forwarding path. Preserve cluster service resolution, reverse zones, private
zones, imports, plugin settings, and unrelated upstream options. Record the exact
previous forwarding settings so the same mechanism can restore them.

For example, if `forward . /etc/resolv.conf` sends external queries to an upstream
that fails the comparisons, replacing that directive's upstream list with tested,
approved recursive resolver addresses is a possible repair. Test private zones
before changing a catch-all rule: public resolvers cannot resolve private DNS.
This is a conditional configuration change, not a replacement Corefile.

A suffix-specific rule is appropriate only when that suffix needs a distinct
resolution path, such as a private zone. In CoreDNS, `forward` matches a domain
suffix and `except` excludes matching names from that rule. Reversing the order
of the same two recursive upstreams in another rule does not create a route to
the zone's authoritative provider: the default selection policy is random.
See the [CoreDNS forward reference](https://coredns.io/plugins/forward/).

Apply an authorized change through the configuration owner, then verify that all
DNS replicas accepted it. If the mounted Corefile uses `reload`, allow for volume
propagation and the reload interval, and inspect each replica's logs for the new
configuration and any reload errors. A running pod may still serve the previous
configuration after a rejected reload. Use the deployment's supported rollout
procedure if reload is unavailable; do not restart cert-manager merely to test
DNS. The [CoreDNS reload reference](https://coredns.io/plugins/reload/) explains
reload behavior and failure evidence.

If the change is rejected or breaks required lookups, restore the preserved
settings through the same configuration owner. For an unmanaged ConfigMap,
restore its saved data while retaining current Kubernetes metadata; do not
blindly apply an old whole-object export over concurrent changes. Confirm that
every replica accepted the restored configuration and repeat the baseline queries.

## Verify recovery at each layer

Inspect each DNS replica's startup or reload output and readiness, then rerun the
same SOA and active-challenge TXT queries through the controller's actual resolver
path. Also test a known cluster Service and any private zones affected by the
edit. Viewing a ConfigMap only shows stored text; it does not parse the Corefile
or prove that the running DNS process accepted it.

```bash
kubectl --context="$DNS_CONTEXT" -n "$DNS_NAMESPACE" get pods
kubectl --context="$DNS_CONTEXT" -n "$DNS_NAMESPACE" logs deployment/"$DNS_DEPLOYMENT" --all-pods=true --since=15m
kubectl --context="$DNS_CONTEXT" -n "$CERT_NAMESPACE" get certificate "$CERTIFICATE" -w
```

If the Corefile contains `log . { class error }`, successful queries are excluded
from query logs. `SERVFAIL` query entries can have an `[INFO]` prefix, so filtering
only for the word `error` can hide them. Use explicit DNS responses as evidence of
successful resolution; use logs to diagnose failures and configuration acceptance.
See the [CoreDNS log reference](https://coredns.io/plugins/log/).

Finally, confirm that the relevant Challenge and Order advance and the Certificate
reports `Ready=True`, with the expected DNS names, issuer, and validity period.
Challenges can disappear after completion, so use their owning resources and
events to follow issuance. Continue monitoring Certificate readiness and renewal
conditions; a successful lookup alone does not demonstrate renewal health.

## Related documentation

- [Let's Encrypt Cloudflare DNS-01 Integration](./LETSENCRYPT_CLOUDFLARE_DNS01_INTEGRATION.md)
- [Gateway Routing Troubleshooting](./GATEWAY_ROUTING_TROUBLESHOOTING.md)
- [Kubernetes Deployment Guide](./KUBERNETES_DEPLOYMENT.md)
