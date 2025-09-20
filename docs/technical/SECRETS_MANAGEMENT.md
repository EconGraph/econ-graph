# EconGraph Secrets Management

> **Security**: This document describes the secure secrets management approach used in the EconGraph platform.

## Overview

EconGraph uses a **Sealed Secrets + Git Submodule** approach for managing sensitive configuration data. This provides secure, version-controlled secret management suitable for both development and production environments.

## Architecture

```
econ-graph5/                    # Main codebase
├── k8s/
│   └── secrets/               # Git submodule (private repo)
│       ├── sealed-secrets/    # Encrypted secrets
│       ├── keys/             # Public encryption keys
│       └── templates/        # Secret templates
└── scripts/deploy/           # Deployment scripts

econ-graph-secrets/           # Private secrets repository
├── sealed-secrets/           # Encrypted Sealed Secrets YAML
├── keys/                     # Public keys for encryption
└── templates/                # Template files
```

## Components

### 1. Sealed Secrets Controller
- **Purpose**: Encrypts secrets so they can be stored safely in Git
- **Location**: Kubernetes cluster (kube-system namespace)
- **Encryption**: Uses cluster-specific public/private key pairs
- **Decryption**: Only the target cluster can decrypt secrets

### 2. Private Secrets Repository
- **Repository**: `https://github.com/jmalicki/econ-graph-secrets`
- **Access**: Private repository with team-based permissions
- **Content**: Encrypted secrets, public keys, templates
- **Integration**: Git submodule in main repository

### 3. Git Submodule
- **Location**: `k8s/secrets/` in main repository
- **Purpose**: Separates secrets from main codebase
- **Access Control**: Different teams can have different access levels
- **Version Control**: Tracks secret changes independently

## Secret Types

### Application Secrets (`econ-graph-secrets`)
- JWT signing secrets
- OAuth client secrets (Google, Facebook)
- API keys (FRED, BLS)
- Session encryption keys

### Database Secrets (`postgres-secrets`)
- Database username and password
- Database name
- Connection strings

### OAuth Secrets (`oauth-secrets`)
- Google OAuth client ID and secret
- Facebook OAuth app ID and secret

## Workflow

### For Developers
```bash
# Clone with submodules
git clone --recursive https://github.com/jmalicki/econ-graph5.git
cd econ-graph5

# Update secrets
git submodule update --remote k8s/secrets

# Deploy secrets
./scripts/deploy/deploy-secrets.sh
```

### For DevOps/Security Team
```bash
# Update secrets in private repo
cd k8s/secrets
# ... make changes ...
git add .
git commit -m "Update JWT secret"
git push

# Update main repository
cd ../..
git add k8s/secrets
git commit -m "Update secrets submodule"
git push
```

## Creating New Secrets

### 1. Create Template
```yaml
# k8s/secrets/templates/new-secret.yaml
apiVersion: v1
kind: Secret
metadata:
  name: new-secret
  namespace: default
type: Opaque
data:
  secret-key: <base64-encoded-value>
```

### 2. Encrypt with Sealed Secrets
```bash
# Get public key
kubeseal --fetch-cert > k8s/secrets/keys/public.pem

# Encrypt secret
kubeseal --format=yaml --cert=k8s/secrets/keys/public.pem \
  < k8s/secrets/templates/new-secret.yaml \
  > k8s/secrets/sealed-secrets/new-secret.yaml
```

### 3. Commit and Deploy
```bash
# Commit encrypted secret
cd k8s/secrets
git add sealed-secrets/new-secret.yaml
git commit -m "Add new secret"
git push

# Update main repository
cd ../..
git add k8s/secrets
git commit -m "Update secrets submodule"
git push

# Deploy
./scripts/deploy/deploy-secrets.sh
```

## Security Features

### Encryption
- **Method**: Bitnami Sealed Secrets
- **Algorithm**: RSA + AES encryption
- **Key Management**: Cluster-specific keys
- **Rotation**: Manual key rotation process

### Access Control
- **Repository**: Private with team-based access
- **Branch Protection**: Required pull request reviews
- **Audit Trail**: Git-based change tracking
- **Pre-commit Hooks**: Prevent accidental secret commits

### Monitoring
- **Secret Access**: Kubernetes audit logs
- **Change Tracking**: Git commit history
- **Deployment Status**: Kubernetes secret status
- **Alerting**: Failed secret deployments

## Deployment Integration

### Deployment Scripts
```bash
# scripts/deploy/deploy-secrets.sh
#!/bin/bash
set -e

echo "Updating secrets submodule..."
git submodule update --remote k8s/secrets

echo "Applying sealed secrets..."
kubectl apply -f k8s/secrets/sealed-secrets/

echo "Verifying secrets..."
kubectl get secrets

echo "Secrets deployed successfully!"
```

### CI/CD Integration
```yaml
# .github/workflows/deploy-secrets.yml
name: Deploy Secrets
on:
  push:
    paths:
      - 'k8s/secrets/**'
jobs:
  deploy-secrets:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v3
      with:
        submodules: recursive
    - name: Deploy Sealed Secrets
      run: |
        kubectl apply -f k8s/secrets/sealed-secrets/
```

## Best Practices

### Do's
- ✅ Always use Sealed Secrets for encryption
- ✅ Store encrypted secrets in private repository
- ✅ Use Git submodules for separation
- ✅ Implement pre-commit hooks
- ✅ Regular secret rotation
- ✅ Monitor secret access
- ✅ Use descriptive commit messages

### Don'ts
- ❌ Never commit unencrypted secrets
- ❌ Don't store secrets in main codebase
- ❌ Avoid hardcoded secrets in code
- ❌ Don't share encryption keys
- ❌ Never use public repositories for secrets
- ❌ Don't skip access controls

## Troubleshooting

### Common Issues

#### Secret Not Found
```bash
# Check if secret exists
kubectl get secrets

# Check sealed secret status
kubectl get sealedsecrets
```

#### Decryption Failed
```bash
# Verify public key
kubectl get secret -n kube-system sealed-secrets-key -o yaml

# Check sealed secret controller logs
kubectl logs -n kube-system -l name=sealed-secrets-controller
```

#### Submodule Issues
```bash
# Update submodule
git submodule update --init --recursive

# Reset submodule
git submodule deinit k8s/secrets
git submodule update --init k8s/secrets
```

## Migration from Hardcoded Secrets

### 1. Identify Hardcoded Secrets
```bash
# Search for hardcoded secrets
grep -r "your-.*-secret" backend/src/
grep -r "password.*=" k8s/manifests/
```

### 2. Create Sealed Secrets
```bash
# Create secret templates
# Encrypt with Sealed Secrets
# Store in private repository
```

### 3. Update Code
```rust
// Remove hardcoded defaults
pub fn get_jwt_secret() -> Result<String, AuthError> {
    std::env::var("JWT_SECRET")
        .map_err(|_| AuthError::ConfigurationError("JWT_SECRET not set".to_string()))
}
```

### 4. Update Kubernetes Manifests
```yaml
# Use secretKeyRef instead of hardcoded values
env:
  - name: JWT_SECRET
    valueFrom:
      secretKeyRef:
        name: econ-graph-secrets
        key: jwt-secret
```

## Compliance

### Security Standards
- **OWASP**: Secure secret management practices
- **NIST**: Cybersecurity framework compliance
- **SOC 2**: Access control and monitoring
- **GDPR**: Data protection requirements

### Audit Requirements
- **Access Logging**: Track who accesses secrets
- **Change Tracking**: Monitor secret modifications
- **Encryption Verification**: Ensure proper encryption
- **Key Rotation**: Regular key rotation schedule

## Related Documentation

- [Security Implementation Plan](../projects/SECURITY_IMPLEMENTATION_PLAN.md)
- [Security Findings Report](../projects/SECURITY_FINDINGS_REPORT.md)
- [Admin Security](ADMIN_SECURITY.md)
- [Deployment Guide](../deployment/)

---

*This secrets management approach provides secure, scalable, and maintainable secret management for the EconGraph platform while maintaining development velocity and operational simplicity.*
