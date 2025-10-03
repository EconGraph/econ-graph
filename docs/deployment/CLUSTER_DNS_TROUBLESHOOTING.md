# Cluster DNS Troubleshooting Guide

## Overview

This document describes the DNS resolution issues encountered during Let's Encrypt Cloudflare DNS-01 integration and the solutions implemented.

## Problem Description

During Let's Encrypt certificate generation with Cloudflare DNS-01 challenges, cert-manager was unable to verify DNS records due to cluster DNS resolution issues.

### Symptoms Observed

```bash
# CoreDNS logs showed SERVFAIL errors
[INFO] 10.1.26.103:47908 - 44624 "SOA IN _acme-challenge.econgraph.com. udp 58 false 4096" SERVFAIL qr,rd,ra 47 0.017550642s

# cert-manager logs showed propagation check failures
E1003 07:07:01.123197       1 sync.go:208] "propagation check failed" err="Could not determine the zone for \"_acme-challenge.econgraph.com.\": When querying the SOA record for the domain '_acme-challenge.econgraph.com.' using nameservers [10.152.183.10:53], rcode was expected to be 'NOERROR' or 'NXDOMAIN', but got 'SERVFAIL'"
```

### Root Cause

The cluster's CoreDNS configuration was using unreliable upstream DNS servers that couldn't properly resolve Cloudflare DNS records, particularly for SOA (Start of Authority) queries required for DNS-01 challenge validation.

## Solution Implemented

### 1. Updated CoreDNS Configuration

**Before:**
```yaml
Corefile: |
  .:53 {
      errors
      health {
        lameduck 5s
      }
      ready
      log . {
        class error
      }
      kubernetes cluster.local in-addr.arpa ip6.arpa {
        pods insecure
        fallthrough in-addr.arpa ip6.arpa
      }
      prometheus :9153
      forward . /etc/resolv.conf
      cache 30
      loop
      reload
      loadbalance
  }
```

**After:**
```yaml
Corefile: |
  .:53 {
      errors
      health {
        lameduck 5s
      }
      ready
      log . {
        class error
      }
      kubernetes cluster.local in-addr.arpa ip6.arpa {
        pods insecure
        fallthrough in-addr.arpa ip6.arpa
      }
      prometheus :9153
      forward . 8.8.8.8 1.1.1.1 {
        except econgraph.com
      }
      forward econgraph.com 1.1.1.1 8.8.8.8
      cache 30
      loop
      reload
      loadbalance
  }
```

### 2. Key Changes

- **Explicit upstream DNS servers**: Using `8.8.8.8` (Google DNS) and `1.1.1.1` (Cloudflare DNS) instead of relying on `/etc/resolv.conf`
- **Domain-specific forwarding**: Special handling for `econgraph.com` to ensure proper Cloudflare DNS resolution
- **Exception handling**: Using `except econgraph.com` to prevent conflicts with domain-specific forwarding

### 3. Implementation Commands

```bash
# Update CoreDNS configuration
kubectl patch configmap/coredns -n kube-system --type merge -p='{"data":{"Corefile":".:53 {\n    errors\n    health {\n      lameduck 5s\n    }\n    ready\n    log . {\n      class error\n    }\n    kubernetes cluster.local in-addr.arpa ip6.arpa {\n      pods insecure\n      fallthrough in-addr.arpa ip6.arpa\n    }\n    prometheus :9153\n    forward . 8.8.8.8 1.1.1.1 {\n      except econgraph.com\n    }\n    forward econgraph.com 1.1.1.1 8.8.8.8\n    cache 30\n    loop\n    reload\n    loadbalance\n}"}}'

# Restart CoreDNS to apply changes
kubectl rollout restart deployment/coredns -n kube-system

# Wait for rollout to complete
kubectl rollout status deployment/coredns -n kube-system

# Restart cert-manager to clear cached DNS queries
kubectl rollout restart deployment/cert-manager -n cert-manager
```

## Verification

### 1. DNS Resolution Test

```bash
# Test external DNS resolution
dig NS econgraph.com
# Should show Cloudflare nameservers:
# econgraph.com.		86381	IN	NS	leia.ns.cloudflare.com.
# econgraph.com.		86381	IN	NS	seth.ns.cloudflare.com.

# Test ACME challenge records
dig TXT _acme-challenge.econgraph.com
# Should show Let's Encrypt challenge tokens
```

### 2. Certificate Generation Test

```bash
# Monitor certificate status
kubectl get certificate econgraph.com-wildcard-tls -n econ-graph -w

# Check challenge status
kubectl get challenges -A

# Verify domain validation
kubectl describe challenge <challenge-name> -n econ-graph
# Should show: Domain "econgraph.com" verified with "DNS-01" validation
```

### 3. CoreDNS Logs Verification

```bash
# Check CoreDNS logs for successful resolution
kubectl logs -n kube-system deployment/coredns --tail=20
# Should show successful DNS queries without SERVFAIL errors
```

## Results

### Before Fix
- ❌ DNS-01 challenges failing with `SERVFAIL` errors
- ❌ Certificate generation stuck in "Issuing" state
- ❌ CoreDNS unable to resolve Cloudflare DNS records

### After Fix
- ✅ DNS-01 challenges succeeding with domain validation
- ✅ Certificate generation progressing normally
- ✅ CoreDNS resolving Cloudflare DNS records correctly
- ✅ Let's Encrypt able to verify domain ownership

## Monitoring and Maintenance

### 1. Regular DNS Health Checks

```bash
# Check CoreDNS pod health
kubectl get pods -n kube-system | grep coredns

# Monitor CoreDNS logs for errors
kubectl logs -n kube-system deployment/coredns --tail=50 | grep -i error

# Test DNS resolution from within cluster
kubectl run dns-test --image=busybox --rm -it --restart=Never -- nslookup econgraph.com
```

### 2. Certificate Monitoring

```bash
# Monitor certificate status
kubectl get certificates -A

# Check certificate expiration
kubectl get certificate econgraph.com-wildcard-tls -n econ-graph -o yaml | grep -A 5 -B 5 "notAfter"

# Monitor cert-manager logs
kubectl logs -n cert-manager deployment/cert-manager --tail=20
```

### 3. DNS Configuration Backup

```bash
# Backup current CoreDNS configuration
kubectl get configmap coredns -n kube-system -o yaml > coredns-backup.yaml

# Restore configuration if needed
kubectl apply -f coredns-backup.yaml
kubectl rollout restart deployment/coredns -n kube-system
```

## Troubleshooting Common Issues

### 1. CoreDNS Pod Restart Issues

```bash
# Check CoreDNS pod status
kubectl describe pod -n kube-system -l k8s-app=kube-dns

# Check for configuration syntax errors
kubectl logs -n kube-system deployment/coredns | grep -i error

# Validate Corefile syntax
kubectl get configmap coredns -n kube-system -o yaml | grep -A 20 "Corefile:"
```

### 2. DNS Resolution Still Failing

```bash
# Test upstream DNS servers directly
nslookup econgraph.com 8.8.8.8
nslookup econgraph.com 1.1.1.1

# Check if domain is properly managed by Cloudflare
dig NS econgraph.com

# Verify Cloudflare API token permissions
# Check Cloudflare dashboard for API token permissions
```

### 3. Certificate Generation Issues

```bash
# Check challenge status
kubectl get challenges -A -o wide

# Describe challenge for detailed status
kubectl describe challenge <challenge-name> -n econ-graph

# Check cert-manager logs for specific errors
kubectl logs -n cert-manager deployment/cert-manager | grep -i "error\|failed"
```

## Best Practices

### 1. DNS Configuration

- **Use reliable upstream DNS servers**: Google DNS (8.8.8.8) and Cloudflare DNS (1.1.1.1)
- **Implement domain-specific forwarding**: For domains managed by specific DNS providers
- **Monitor DNS resolution**: Regular health checks and log monitoring
- **Backup configurations**: Keep backups of working DNS configurations

### 2. Certificate Management

- **Use staging environment**: Test certificate generation with Let's Encrypt staging server first
- **Monitor certificate lifecycle**: Track expiration and renewal status
- **Implement proper error handling**: Handle DNS resolution failures gracefully
- **Document troubleshooting steps**: Keep detailed records of issues and solutions

### 3. Cluster DNS Maintenance

- **Regular updates**: Keep CoreDNS and cert-manager updated
- **Health monitoring**: Implement monitoring for DNS resolution failures
- **Automated recovery**: Consider automated DNS health checks and recovery
- **Documentation**: Maintain detailed documentation of DNS configuration changes

## Related Documentation

- [Let's Encrypt Cloudflare DNS-01 Integration](./LETSENCRYPT_CLOUDFLARE_DNS01_INTEGRATION.md)
- [Kubernetes Deployment Guide](./KUBERNETES_DEPLOYMENT.md)
- [cert-manager Configuration](./CERT_MANAGER_CONFIGURATION.md)

## Conclusion

The DNS resolution issues were successfully resolved by updating the CoreDNS configuration to use reliable upstream DNS servers and implementing domain-specific forwarding for Cloudflare-managed domains. This fix enabled successful Let's Encrypt certificate generation with Cloudflare DNS-01 challenges.

The solution provides a robust foundation for automated SSL certificate management in the Kubernetes cluster while maintaining proper DNS resolution for all cluster services.
