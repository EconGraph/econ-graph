#!/bin/bash

# Setup Let's Encrypt with Cloudflare DNS-01 Challenge
# This script configures cert-manager to use Cloudflare DNS for DNS-01 challenges

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
NAMESPACE="econ-graph"
DOMAIN="econgraph.example.com"
CLOUDFLARE_API_TOKEN=""
USE_STAGING=false
CONTEXT="kind-econ-graph"
DRY_RUN=false

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to show usage
usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Setup Let's Encrypt with Cloudflare DNS-01 Challenge for EconGraph

OPTIONS:
    -d, --domain DOMAIN        Domain name (default: econgraph.example.com)
    -t, --token TOKEN          Cloudflare API token (required)
    -n, --namespace NAMESPACE  Kubernetes namespace (default: econ-graph)
    --staging                  Use Let's Encrypt staging server (for testing)
    --context CONTEXT          kubectl context (default: kind-econ-graph); the
                               current context is not changed
    --dry-run                  Show the cluster and what would be applied; change nothing
    -h, --help                 Show this help message

Every kubectl call goes through kc (scripts/deploy/lib/cluster-role.sh) on that
one context, with the kind cluster's own kubectl and kubeconfig from
~/.cache/econgraph when KUBECTL and KUBECONFIG are unset: plain kubectl may be
another cluster's. The cluster's identity is shown before anything is applied.

EXAMPLES:
    # Production setup
    $0 -d econgraph.com -t "your-cloudflare-api-token"
    
    # Staging setup for testing
    $0 -d test.econgraph.com -t "your-cloudflare-api-token" --staging

PREREQUISITES:
    1. Cloudflare account with domain DNS managed there
    2. Cloudflare API token with Zone:DNS:Edit and Zone:Zone:Read permissions
    3. kubectl configured and connected to your cluster
    4. cert-manager already installed
    5. Domain DNS managed by Cloudflare

CLOUDFLARE API TOKEN SETUP:
    1. Go to https://dash.cloudflare.com/profile/api-tokens
    2. Click "Create Token"
    3. Use "Custom token" template
    4. Set permissions:
       - Zone:DNS:Edit
       - Zone:Zone:Read
    5. Set Zone Resources: Include - All Zones
    6. Create token and copy it

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -d|--domain)
            DOMAIN="$2"
            shift 2
            ;;
        -t|--token)
            CLOUDFLARE_API_TOKEN="$2"
            shift 2
            ;;
        -n|--namespace)
            NAMESPACE="$2"
            shift 2
            ;;
        --staging)
            USE_STAGING=true
            shift
            ;;
        --context)
            [ $# -ge 2 ] && [ -n "$2" ] || { print_error "--context needs a non-empty value"; exit 2; }
            CONTEXT="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

# Validate required parameters
if [[ -z "$CLOUDFLARE_API_TOKEN" ]]; then
    print_error "Cloudflare API token is required"
    usage
    exit 1
fi

print_status "Setting up Let's Encrypt with Cloudflare DNS-01 Challenge"
print_status "Domain: $DOMAIN"
print_status "Namespace: $NAMESPACE"
print_status "Environment: $([ "$USE_STAGING" = true ] && echo "staging" || echo "production")"

# Check prerequisites
print_status "Checking prerequisites..."

# One kubectl and one context for every call below (kc, lib/cluster-role.sh),
# never plain kubectl.
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "${PROJECT_ROOT}/scripts/deploy/lib/cluster-role.sh"
if ! cluster_role_operator_context "$CONTEXT"; then
    print_error "Nothing was changed."
    exit 1
fi
kubectl() { kc "$@"; }

if [[ "$DRY_RUN" = true ]]; then
    print_status "Dry run: on context $CONTEXT, would create Secret cert-manager/cloudflare-api-token-secret,"
    print_status "  apply the ClusterIssuer and a wildcard Certificate for $DOMAIN in $NAMESPACE, wait for it,"
    print_status "  and apply k8s/manifests/ingress-cloudflare-dns01.yaml. Nothing was changed."
    exit 0
fi

# Check if cluster is accessible
if ! kubectl cluster-info &> /dev/null; then
    print_error "Cannot connect to Kubernetes cluster"
    exit 1
fi

# Check if cert-manager is installed
if ! kubectl get namespace cert-manager &> /dev/null; then
    print_error "cert-manager namespace not found. Please install cert-manager first"
    exit 1
fi

print_success "Prerequisites check passed"

# Create Cloudflare API credentials secret
print_status "Creating Cloudflare API credentials secret..."
kubectl create secret generic cloudflare-api-token-secret \
    --from-literal=api-token="$CLOUDFLARE_API_TOKEN" \
    --namespace=cert-manager \
    --dry-run=client -o yaml | kubectl apply -f -

print_success "Cloudflare API credentials secret created"

# Create ClusterIssuer with DNS-01 challenge
print_status "Creating ClusterIssuer with Cloudflare DNS-01 challenge..."

# Determine which ClusterIssuer to use
CLUSTER_ISSUER_NAME="$([ "$USE_STAGING" = true ] && echo "letsencrypt-staging-cloudflare" || echo "letsencrypt-prod-cloudflare")"
ACME_SERVER="$([ "$USE_STAGING" = true ] && echo "https://acme-staging-v02.api.letsencrypt.org/directory" || echo "https://acme-v02.api.letsencrypt.org/directory")"

cat << EOF | kubectl apply -f -
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: ${CLUSTER_ISSUER_NAME}
spec:
  acme:
    server: ${ACME_SERVER}
    email: ${ACME_EMAIL:-support@econgraph.com}
    privateKeySecretRef:
      name: ${CLUSTER_ISSUER_NAME}
    solvers:
    - dns01:
        cloudflare:
          apiTokenSecretRef:
            name: cloudflare-api-token-secret
            key: api-token
EOF

print_success "ClusterIssuer created: $CLUSTER_ISSUER_NAME"

# Create wildcard certificate
print_status "Creating wildcard certificate for $DOMAIN..."
cat << EOF | kubectl apply -f -
apiVersion: cert-manager.io/v1
kind: Certificate
metadata:
  name: ${DOMAIN}-wildcard-tls
  namespace: ${NAMESPACE}
spec:
  secretName: ${DOMAIN}-wildcard-tls
  issuerRef:
    name: ${CLUSTER_ISSUER_NAME}
    kind: ClusterIssuer
  commonName: "*.${DOMAIN}"
  dnsNames:
  - "${DOMAIN}"
  - "*.${DOMAIN}"
  - "api.${DOMAIN}"
  - "admin.${DOMAIN}"
  - "grafana.${DOMAIN}"
  duration: 2160h  # 90 days
  renewBefore: 720h  # Renew 30 days before expiration
EOF

print_success "Wildcard certificate created"

# Wait for certificate to be issued
print_status "Waiting for certificate to be issued (this may take 2-5 minutes)..."
kubectl wait --for=condition=Ready --timeout=600s "certificate/${DOMAIN}-wildcard-tls" -n "${NAMESPACE}"

# Check certificate status
CERT_STATUS=$(kubectl get certificate "${DOMAIN}-wildcard-tls" -n "${NAMESPACE}" -o jsonpath='{.status.conditions[0].type}')
if [[ "$CERT_STATUS" == "Ready" ]]; then
    print_success "Certificate issued successfully!"
    
    # Display certificate details
    print_status "Certificate details:"
    kubectl describe certificate "${DOMAIN}-wildcard-tls" -n "${NAMESPACE}"
else
    print_error "Certificate issuance failed or timed out"
    print_status "Certificate status:"
    kubectl describe certificate "${DOMAIN}-wildcard-tls" -n "${NAMESPACE}"
    exit 1
fi

# Display final status
print_status "Setup completed successfully!"
echo ""
print_status "Certificate information:"
kubectl get certificate "${DOMAIN}-wildcard-tls" -n "${NAMESPACE}"
echo ""
print_status "ClusterIssuer information:"
kubectl get clusterissuer "${CLUSTER_ISSUER_NAME}"
echo ""
print_status "Secret information:"
kubectl get secret cloudflare-api-token-secret -n cert-manager

echo ""
print_success "Let's Encrypt with Cloudflare DNS-01 challenge setup complete!"
print_status "Your wildcard certificate for *.${DOMAIN} is now ready to use."
print_status "Certificate will be automatically renewed before expiration."

if [[ "$USE_STAGING" = true ]]; then
    print_warning "You used the staging environment. For production, run without --staging flag."
fi

print_status ""
print_status "IMPORTANT NOTES:"
print_status "1. Ensure your domain DNS is managed by Cloudflare"
print_status "2. If using Cloudflare proxy (orange cloud), consider disabling it during certificate issuance"
print_status "3. Monitor certificate renewal and DNS propagation"
print_status "4. Keep your Cloudflare API token secure and rotate it regularly"
