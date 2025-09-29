#!/bin/bash

# Setup mTLS service-to-service authentication with internal CA
# This script deploys Vault as internal CA and configures cert-manager

set -e

echo "🔐 Setting up mTLS service-to-service authentication..."
echo "====================================================="

# Get the project root directory
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

# Check if kind cluster exists
if ! kind get clusters | grep -q "econ-graph"; then
    echo "❌ Kind cluster 'econ-graph' not found. Please run terraform first."
    echo "   cd terraform/k8s && terraform init && terraform apply"
    exit 1
fi

# Set kubectl context
kubectl config use-context kind-econ-graph

# Step 1: Deploy Vault as internal CA
echo "📋 Step 1: Deploying Vault as internal Certificate Authority..."
kubectl apply -f k8s/manifests/vault-config.yaml
kubectl apply -f k8s/manifests/vault-secrets.yaml
kubectl apply -f k8s/manifests/vault-service.yaml
kubectl apply -f k8s/manifests/vault-deployment.yaml

# Wait for Vault to be ready
echo "⏳ Waiting for Vault to be ready..."
kubectl wait --for=condition=ready pod -l app=vault -n econ-graph --timeout=300s

# Step 2: Initialize Vault
echo "📋 Step 2: Initializing Vault and setting up PKI..."
kubectl exec -n econ-graph deployment/vault -- vault operator init -key-shares=1 -key-threshold=1 -format=json > vault-init.json

# Extract unseal key and root token
UNSEAL_KEY=$(jq -r '.unseal_keys_b64[0]' vault-init.json)
ROOT_TOKEN=$(jq -r '.root_token' vault-init.json)

echo "🔑 Vault initialized successfully"
echo "   Unseal Key: ${UNSEAL_KEY:0:10}..."
echo "   Root Token: ${ROOT_TOKEN:0:10}..."

# Step 3: Unseal Vault
echo "📋 Step 3: Unsealing Vault..."
kubectl exec -n econ-graph deployment/vault -- vault operator unseal "$UNSEAL_KEY"

# Step 4: Configure PKI secrets engine
echo "📋 Step 4: Configuring PKI secrets engine..."
kubectl exec -n econ-graph deployment/vault -- sh -c "
export VAULT_TOKEN=$ROOT_TOKEN
vault secrets enable pki
vault secrets tune -max-lease-ttl=87600h pki
vault write -field=certificate pki/root/generate/internal common_name='EconGraph Root CA' ttl=87600h > /tmp/root_ca.crt
vault write pki/config/urls issuing_certificates='http://vault:8200/v1/pki/ca' crl_distribution_points='http://vault:8200/v1/pki/crl'
"

# Step 5: Create PKI role for service certificates
echo "📋 Step 5: Creating PKI role for service certificates..."
kubectl exec -n econ-graph deployment/vault -- sh -c "
export VAULT_TOKEN=$ROOT_TOKEN
vault write pki/roles/econ-graph-services \
    allowed_domains='econ-graph.svc.cluster.local,svc.cluster.local,cluster.local' \
    allow_subdomains=true \
    max_ttl=720h \
    ttl=720h
"

# Step 6: Deploy cert-manager
echo "📋 Step 6: Deploying cert-manager..."
kubectl apply -f k8s/manifests/cert-manager-deployment.yaml

# Wait for cert-manager to be ready
echo "⏳ Waiting for cert-manager to be ready..."
kubectl wait --for=condition=ready pod -l app=cert-manager -n cert-manager --timeout=300s

# Step 7: Configure Vault issuer for cert-manager
echo "📋 Step 7: Configuring Vault issuer for cert-manager..."
cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Secret
metadata:
  name: vault-issuer-secret
  namespace: econ-graph
type: Opaque
data:
  token: $(echo -n "$ROOT_TOKEN" | base64)
---
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: vault-issuer
spec:
  vault:
    server: http://vault:8200
    path: pki/sign/econ-graph-services
    auth:
      tokenSecretRef:
        name: vault-issuer-secret
        key: token
EOF

echo "✅ mTLS infrastructure setup completed successfully!"
echo ""
echo "🔐 Vault Configuration:"
echo "  Vault URL: http://vault.econ-graph.svc.cluster.local:8200"
echo "  Root Token: ${ROOT_TOKEN:0:10}... (stored in vault-init.json)"
echo "  Unseal Key: ${UNSEAL_KEY:0:10}... (stored in vault-init.json)"
echo ""
echo "📋 Next Steps:"
echo "  1. Configure Istio service mesh for automatic mTLS"
echo "  2. Create service certificates using cert-manager"
echo "  3. Update service configurations to use mTLS"
echo "  4. Test service-to-service communication"
echo ""
echo "🔧 Useful Commands:"
echo "  kubectl port-forward service/vault 8200:8200 -n econ-graph"
echo "  kubectl exec -n econ-graph deployment/vault -- vault status"
echo "  kubectl get certificates -n econ-graph"
echo ""
echo "⚠️  Security Notes:"
echo "  - Vault is running in dev mode for simplicity"
echo "  - Root token and unseal key are stored in vault-init.json"
echo "  - In production, use proper Vault HA setup with Consul/etcd"
echo "  - Enable TLS for Vault API communication"
