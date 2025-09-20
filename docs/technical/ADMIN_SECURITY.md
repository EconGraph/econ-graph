# EconGraph Administrative Interface Security

## Overview

The EconGraph administrative interface is a separate, secure web application designed for system administrators to monitor and manage the EconGraph platform. This interface is completely isolated from the public-facing application and includes multiple layers of security controls.

## Security Architecture

### Network Isolation

The admin interface is deployed with strict network isolation:

- **Separate Namespace**: Deployed in `econ-graph-admin` namespace
- **Network Policies**: Kubernetes network policies restrict traffic flow
- **Separate Ingress**: Uses dedicated ingress controller with strict access controls
- **Port Separation**: Runs on different ports from public interface

### Access Controls

#### IP Whitelisting
- Only specified IP ranges can access the admin interface
- Configurable via `allowed_admin_ips` in Terraform variables
- Enforced at ingress level with NGINX whitelist

#### Authentication & Authorization
- **Multi-Factor Authentication**: Required by default
- **Role-Based Access Control**: Three levels (read_only, admin, super_admin)
- **Session Management**: Short session timeouts (30 minutes default)
- **JWT Tokens**: Secure token-based authentication

#### Rate Limiting
- **Request Rate Limiting**: 10 requests per second per IP
- **Connection Limiting**: 5 concurrent connections per IP
- **Login Attempts**: Protection against brute force attacks

### Security Headers

The admin interface enforces strict security headers:

```
X-Frame-Options: DENY
X-Content-Type-Options: nosniff
X-XSS-Protection: 1; mode=block
Referrer-Policy: no-referrer
Content-Security-Policy: default-src 'self'
Strict-Transport-Security: max-age=31536000
```

### Monitoring & Alerting

#### Security Events
- All access attempts are logged
- Failed authentication attempts trigger alerts
- Unusual activity patterns are monitored
- Real-time security event tracking

#### Audit Logging
- Complete audit trail of all admin actions
- Immutable log storage
- Log retention for compliance requirements
- Integration with SIEM systems

## Deployment Configuration

### Terraform Variables

Required variables for admin interface deployment:

```hcl
# Admin domain (use internal domain)
admin_domain = "admin.internal.company.com"

# IP whitelist (customize for your environment)
allowed_admin_ips = [
  "10.0.0.0/8",      # Internal corporate network
  "172.16.0.0/12",   # VPN network
  "192.168.1.0/24",  # Admin workstation subnet
]

# Authentication secrets (generate strong random values)
admin_jwt_secret     = "256-bit-random-secret"
admin_session_key    = "256-bit-random-secret"
admin_encryption_key = "256-bit-random-secret"

# TLS certificates (use internal CA)
admin_tls_cert = "base64-encoded-certificate"
admin_tls_key  = "base64-encoded-private-key"
```

### Kubernetes Resources

The admin interface creates the following isolated resources:

- **Namespace**: `econ-graph-admin`
- **Deployment**: Admin frontend with security context
- **Service**: Internal ClusterIP service
- **NetworkPolicy**: Ingress/egress traffic restrictions
- **Ingress**: Secure external access with IP filtering
- **ConfigMaps**: Security configuration
- **Secrets**: Authentication credentials

### Network Policies

```yaml
# Example network policy (applied automatically)
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: admin-network-isolation
  namespace: econ-graph-admin
spec:
  podSelector: {}
  policyTypes: ["Ingress", "Egress"]
  ingress:
  - from:
    - ipBlock:
        cidr: 10.0.0.0/8  # Internal network only
    ports:
    - protocol: TCP
      port: 3000
  egress:
  - to:
    - namespaceSelector:
        matchLabels:
          name: econ-graph
    ports:
    - protocol: TCP
      port: 8081  # Backend admin API
```

## Security Best Practices

### Infrastructure Security

1. **VPN Access**: Deploy admin interface behind corporate VPN
2. **Bastion Host**: Use jump server for additional security layer
3. **Internal DNS**: Use internal domain names, not public DNS
4. **Certificate Management**: Use internal CA for TLS certificates
5. **Network Segmentation**: Isolate admin network from public traffic

### Operational Security

1. **Least Privilege**: Grant minimum required permissions
2. **Regular Audits**: Review access logs and user permissions
3. **Key Rotation**: Regularly rotate authentication secrets
4. **Session Management**: Implement automatic session timeout
5. **Incident Response**: Have procedures for security incidents

### User Management

1. **Strong Authentication**: Enforce MFA for all admin users
2. **Account Lifecycle**: Promptly disable unused accounts
3. **Password Policy**: Enforce strong password requirements
4. **Access Reviews**: Regular review of user access rights
5. **Training**: Security awareness training for admin users

## Monitoring & Alerting

### Security Metrics

The admin interface provides metrics for:

- Authentication attempts (successful/failed)
- Session duration and timeout events
- Permission denied events
- Unusual access patterns
- System configuration changes

### Alert Conditions

Automatic alerts are triggered for:

- Multiple failed login attempts
- Access from unauthorized IP addresses
- Privilege escalation attempts
- Suspicious activity patterns
- System configuration changes
- Service availability issues

### Log Analysis

Key log sources for security analysis:

- **Authentication Logs**: Login attempts and session events
- **Access Logs**: All HTTP requests with detailed metadata
- **Audit Logs**: Administrative actions and configuration changes
- **Security Events**: Custom security events from the application
- **Infrastructure Logs**: Kubernetes and ingress controller logs

## Incident Response

### Security Incident Procedures

1. **Detection**: Automated alerts and monitoring
2. **Assessment**: Determine severity and scope
3. **Containment**: Isolate affected systems
4. **Investigation**: Analyze logs and gather evidence
5. **Recovery**: Restore normal operations
6. **Post-Incident**: Document lessons learned

### Emergency Procedures

#### Suspected Compromise
1. Immediately disable admin interface access
2. Revoke all active sessions
3. Change all authentication secrets
4. Review all recent admin actions
5. Analyze logs for indicators of compromise

#### System Lockout
1. Use emergency access procedures
2. Check network policies and IP whitelists
3. Verify certificate validity
4. Review authentication service status
5. Use kubectl for direct cluster access if needed

## Compliance & Governance

### Security Standards

The admin interface is designed to meet:

- **SOC 2 Type II**: Security controls and monitoring
- **ISO 27001**: Information security management
- **NIST Cybersecurity Framework**: Risk management
- **CIS Controls**: Security best practices

### Documentation Requirements

Maintain documentation for:

- Security policies and procedures
- User access rights and reviews
- Incident response procedures
- Change management processes
- Risk assessments and mitigation

### Regular Security Activities

- **Quarterly**: Access reviews and permission audits
- **Monthly**: Security patch updates and vulnerability scans
- **Weekly**: Log analysis and security metric reviews
- **Daily**: Monitoring alert review and incident response

## Troubleshooting

### Common Issues

#### Cannot Access Admin Interface
1. Verify IP address is in whitelist
2. Check DNS resolution for admin domain
3. Verify TLS certificate validity
4. Check ingress controller status
5. Review network policies

#### Authentication Failures
1. Verify user credentials and MFA setup
2. Check JWT secret configuration
3. Review session timeout settings
4. Analyze authentication service logs
5. Verify user role assignments

#### Performance Issues
1. Check resource limits and usage
2. Review rate limiting configuration
3. Analyze database connection status
4. Monitor backend API performance
5. Check network latency and connectivity

### Emergency Access

If normal admin access is unavailable:

1. **kubectl Access**: Direct cluster administration
   ```bash
   kubectl get pods -n econ-graph-admin
   kubectl logs -n econ-graph-admin -l app=admin-frontend
   ```

2. **Port Forwarding**: Bypass ingress for troubleshooting
   ```bash
   kubectl port-forward -n econ-graph-admin service/admin-frontend-service 8080:3000
   ```

3. **Emergency User**: Create temporary admin user via kubectl
   ```bash
   kubectl exec -n econ-graph deployment/backend -- /app/create-emergency-user
   ```

## Contact Information

For security incidents or questions:

- **Security Team**: security@company.com
- **On-Call Engineer**: +1-555-ONCALL
- **Emergency Escalation**: Follow incident response procedures

---

**⚠️ SECURITY WARNING**: This administrative interface provides full control over the EconGraph platform. Unauthorized access could result in data loss, system compromise, or service disruption. Always follow security procedures and report any suspicious activity immediately.
