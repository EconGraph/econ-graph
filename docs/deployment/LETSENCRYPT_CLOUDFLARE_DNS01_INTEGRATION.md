# Let's Encrypt with Cloudflare DNS-01 Challenge Integration

This guide provides comprehensive instructions for integrating Let's Encrypt with Cloudflare DNS using DNS-01 challenges for automatic wildcard SSL certificate generation in Kubernetes.

## Overview

The DNS-01 challenge integration enables:
- **Wildcard certificates** for `*.yourdomain.com`
- **Automatic certificate renewal** via cert-manager
- **No HTTP server requirements** for domain validation
- **Support for internal services** and subdomains
- **Production-ready SSL/TLS** with Let's Encrypt
- **Cloudflare optimization** with proper IP handling

## Prerequisites

### Cloudflare Requirements

- **Cloudflare Account**: Domain must be managed by Cloudflare
- **Cloudflare API Token**: With proper permissions for DNS management
- **Domain DNS**: Must be managed by Cloudflare (nameservers pointing to Cloudflare)

### System Requirements

- Kubernetes cluster (1.24+)
- kubectl configured and connected
- cert-manager already installed
- Domain DNS managed by Cloudflare
- Cloudflare API token with appropriate permissions

## Quick Start

### 1. Create Cloudflare API Token

1. Log in to your Cloudflare account
2. Navigate to [API Tokens](https://dash.cloudflare.com/profile/api-tokens)
3. Click "Create Token"
4. Use "Custom token" template
5. Configure permissions:
   - **Zone:DNS:Edit** - To create/delete DNS records
   - **Zone:Zone:Read** - To read zone information
6. Set Zone Resources: **Include - All Zones**
7. Create token and copy it securely

### 2. Run the Setup Script

```bash
# Production setup
./scripts/deploy/setup-letsencrypt-cloudflare.sh \
  -d econgraph.com \
  -t "your-cloudflare-api-token"

# Staging setup (for testing)
./scripts/deploy/setup-letsencrypt-cloudflare.sh \
  -d test.econgraph.com \
  -t "your-cloudflare-api-token" \
  --staging
```

### 3. Verify Certificate Generation

```bash
# Check certificate status
kubectl get certificate -n econ-graph

# View certificate details
kubectl describe certificate econgraph.com-wildcard-tls -n econ-graph

# Check ClusterIssuer
kubectl get clusterissuer letsencrypt-prod-cloudflare
```

## Manual Setup

If you prefer manual setup or need to customize the configuration:

### 1. Create Cloudflare API Credentials Secret

```bash
kubectl create secret generic cloudflare-api-token-secret \
  --from-literal=api-token="YOUR_CLOUDFLARE_API_TOKEN" \
  --namespace=cert-manager
```

### 2. Create DNS-01 ClusterIssuer

```yaml
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: letsencrypt-prod-cloudflare
spec:
  acme:
    server: https://acme-v02.api.letsencrypt.org/directory
    email: admin@yourdomain.com
    privateKeySecretRef:
      name: letsencrypt-prod-cloudflare
    solvers:
    - dns01:
        cloudflare:
          apiTokenSecretRef:
            name: cloudflare-api-token-secret
            key: api-token
```

### 3. Create Wildcard Certificate

```yaml
apiVersion: cert-manager.io/v1
kind: Certificate
metadata:
  name: yourdomain.com-wildcard-tls
  namespace: econ-graph
spec:
  secretName: yourdomain.com-wildcard-tls
  issuerRef:
    name: letsencrypt-prod-cloudflare
    kind: ClusterIssuer
  commonName: "*.yourdomain.com"
  dnsNames:
  - "yourdomain.com"
  - "*.yourdomain.com"
  - "api.yourdomain.com"
  - "admin.yourdomain.com"
  - "grafana.yourdomain.com"
  duration: 2160h  # 90 days
  renewBefore: 720h  # Renew 30 days before expiration
```

### 4. Use the Certificate on the Gateway

The gateway's https listener gets its certificate from cert-manager, from the ClusterIssuer
named in `GATEWAY_TLS_ISSUER`; with a DNS-01 issuer no HTTP-01 challenge has to reach the
cluster:

```bash
PUBLIC_HOST=yourdomain.com GATEWAY_TLS_ISSUER=letsencrypt-prod-cloudflare \
  ./scripts/deploy/deploy.sh
```

The certificate (Secret `econ-graph-tls` in namespace `econ-graph-gateway`) covers the host
and its `www` name. Without `GATEWAY_TLS_ISSUER` it is self-signed, which is all the Cloudflare
Tunnel needs. See "Public site" in `k8s/README.md`.

## Configuration Options

### ClusterIssuer Settings

| Parameter | Description | Default | Options |
|-----------|-------------|---------|---------|
| `server` | Let's Encrypt ACME server | Production | Staging/Production |
| `email` | Contact email for Let's Encrypt | Required | Valid email address |
| `privateKeySecretRef` | Secret for ACME private key | Auto-generated | Kubernetes secret |

### Certificate Settings

| Parameter | Description | Default | Options |
|-----------|-------------|---------|---------|
| `duration` | Certificate validity period | `2160h` (90 days) | 1h-8760h |
| `renewBefore` | Renewal time before expiration | `720h` (30 days) | 1h-8760h |
| `dnsNames` | Subject Alternative Names | Required | Domain list |

## Troubleshooting

### Common Issues

#### 1. Cloudflare API Token Invalid

**Error**: `Access denied` or `Invalid API token`

**Solutions**:
- Verify API token is correct and not expired
- Check token has required permissions (Zone:DNS:Edit, Zone:Zone:Read)
- Ensure token has access to the specific zone
- Verify token is not using Global API Key (use API Token instead)

#### 2. Domain Not Managed by Cloudflare

**Error**: `Zone not found` or `Permission denied`

**Solutions**:
- Verify domain nameservers point to Cloudflare
- Check domain is added to Cloudflare account
- Ensure domain is active and not paused
- Verify API token has access to the zone

#### 3. Certificate Issuance Timeout

**Error**: `Certificate request timed out`

**Solutions**:
- Check DNS propagation (can take up to 24 hours)
- Verify Cloudflare proxy is disabled (gray cloud) during issuance
- Check for DNS record conflicts
- Monitor cert-manager logs for specific errors

#### 4. DNS-01 Challenge Failed

**Error**: `Failed to create DNS record` or `DNS record not found`

**Solutions**:
- Check Cloudflare API token permissions
- Verify domain DNS settings
- Disable Cloudflare proxy during certificate issuance
- Check for rate limiting on Cloudflare API

### Debug Commands

```bash
# Check certificate status
kubectl describe certificate yourdomain.com-wildcard-tls -n econ-graph

# View certificate events
kubectl get events -n econ-graph --field-selector involvedObject.name=yourdomain.com-wildcard-tls

# Check ClusterIssuer status
kubectl describe clusterissuer letsencrypt-prod-cloudflare

# Check Cloudflare API credentials
kubectl get secret cloudflare-api-token-secret -n cert-manager -o yaml

# Test DNS propagation
dig TXT _acme-challenge.yourdomain.com

# Check domain nameservers
dig NS yourdomain.com

# Monitor cert-manager logs
kubectl logs -f deployment/cert-manager -n cert-manager
```

### Log Analysis

#### Successful Certificate Issuance

```
Certificate status: Ready
Events:
  Normal  Requested   Certificate requested
  Normal  Validated   Domain validated successfully
  Normal  Issued      Certificate issued successfully
```

#### Failed Certificate Issuance

```
Certificate status: Failed
Events:
  Warning  FailedValidation  Domain validation failed
  Warning  FailedIssuance   Certificate issuance failed
```

## Security Considerations

### API Token Security

- Store Cloudflare API token as Kubernetes secret
- Use least-privilege API tokens with only required permissions
- Rotate API tokens regularly (quarterly recommended)
- Never commit API tokens to version control
- Monitor API token usage in Cloudflare dashboard

### Certificate Security

- Use production Let's Encrypt server for production environments
- Monitor certificate expiration and renewal status
- Implement proper TLS configuration with strong ciphers
- Security headers are set by the gateway's frontend route (`k8s/manifests/gateway/routes/httproutes.yaml`)

### Network Security

- Implement network policies to restrict access
- Monitor DNS record changes for unauthorized modifications

## Cloudflare-Specific Optimizations

### Client Addresses

The gateway's GraphQL rate limits count per client address as the Envoy proxy sees it (the TCP
peer). Behind the Cloudflare proxy (orange cloud) or a Cloudflare Tunnel, every request arrives
from Cloudflare or from the tunnel connector, so all visitors share those addresses' limits.

### Proxy Settings

For DNS-01 challenges:
- **Disable Cloudflare proxy** (gray cloud) during certificate issuance
- **Enable proxy** (orange cloud) after certificate is issued
- This prevents Cloudflare from interfering with DNS validation

### DNS Propagation

Cloudflare typically has fast DNS propagation, but:
- Allow 1-2 minutes for DNS changes to propagate
- Use Cloudflare's DNS API for faster propagation
- Monitor DNS propagation with external tools

## Monitoring and Alerting

### Certificate Monitoring

```yaml
apiVersion: monitoring.coreos.com/v1
kind: PrometheusRule
metadata:
  name: certificate-expiry
spec:
  groups:
  - name: certificates
    rules:
    - alert: CertificateExpiringSoon
      expr: cert_manager_certificate_expiration_timestamp_seconds - time() < 86400 * 30
      for: 5m
      labels:
        severity: warning
      annotations:
        summary: "Certificate expiring soon"
        description: "Certificate {{ $labels.name }} expires in {{ $value | humanizeDuration }}"
```

### DNS Challenge Monitoring

Monitor DNS-01 challenge success rates:

```bash
# Check challenge success rate
kubectl get challenges -A -o wide

# Monitor certificate renewal attempts
kubectl logs -f deployment/cert-manager -n cert-manager | grep -i "dns01"
```

## Best Practices

### 1. Use Staging Environment for Testing

Always test DNS-01 configuration with Let's Encrypt staging server first:

```yaml
spec:
  acme:
    server: https://acme-staging-v02.api.letsencrypt.org/directory
```

### 2. Cloudflare Proxy Management

- Disable proxy during certificate issuance
- Enable proxy after certificate is issued
- Use automation to manage proxy settings

### 3. API Token Management

- Create dedicated API tokens for cert-manager
- Use least-privilege permissions
- Rotate tokens regularly
- Monitor token usage

### 4. DNS Record Management

- Monitor DNS record creation/deletion
- Set up alerts for unauthorized DNS changes
- Keep DNS records clean and organized

### 5. Certificate Lifecycle

- Monitor certificate expiration
- Test renewal process regularly
- Keep certificate policies up to date

## Migration from HTTP-01

To migrate from HTTP-01 to DNS-01 challenges:

1. **Create DNS-01 ClusterIssuer** alongside existing HTTP-01 issuer
2. **Test with staging environment** using a subdomain
3. **Gradually migrate services** to use DNS-01 certificates
4. **Point the gateway at the new issuer** (`GATEWAY_TLS_ISSUER`)
5. **Monitor certificate renewal** for both challenge types
6. **Remove HTTP-01 ClusterIssuers** once migration is complete

## Support and Resources

- [cert-manager Documentation](https://cert-manager.io/docs/)
- [Cloudflare API Documentation](https://developers.cloudflare.com/api/)
- [Let's Encrypt Documentation](https://letsencrypt.org/docs/)
- [DNS-01 Challenge Specification](https://tools.ietf.org/html/rfc8555#section-8.4)
- [Cloudflare DNS Management](https://developers.cloudflare.com/dns/)

## Conclusion

The Cloudflare DNS-01 challenge integration provides a robust, automated solution for wildcard SSL certificate management with Cloudflare DNS. By following this guide and implementing proper monitoring and security practices, you can achieve production-ready SSL/TLS for your Kubernetes deployments with automatic certificate renewal and minimal maintenance overhead.

The integration takes advantage of Cloudflare's fast DNS propagation and reliable API, making it an excellent choice for automated certificate management in production environments.
