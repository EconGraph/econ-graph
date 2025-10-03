#!/bin/bash

# Test Let's Encrypt with Cloudflare DNS-01 Challenge
# This script tests the certificate generation and validation process

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
NAMESPACE="econ-graph"
DOMAIN=""
USE_STAGING=true

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

Test Let's Encrypt with Cloudflare DNS-01 Challenge

OPTIONS:
    -d, --domain DOMAIN        Domain name to test (required)
    -n, --namespace NAMESPACE  Kubernetes namespace (default: econ-graph)
    --production               Use production Let's Encrypt server
    -h, --help                 Show this help message

EXAMPLES:
    # Test with staging environment
    $0 -d test.econgraph.com
    
    # Test with production environment
    $0 -d econgraph.com --production

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -d|--domain)
            DOMAIN="$2"
            shift 2
            ;;
        -n|--namespace)
            NAMESPACE="$2"
            shift 2
            ;;
        --production)
            USE_STAGING=false
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
if [[ -z "$DOMAIN" ]]; then
    print_error "Domain name is required"
    usage
    exit 1
fi

print_status "Testing Let's Encrypt with Cloudflare DNS-01 Challenge"
print_status "Domain: $DOMAIN"
print_status "Namespace: $NAMESPACE"
print_status "Environment: $([ "$USE_STAGING" = true ] && echo "staging" || echo "production")"

# Determine ClusterIssuer and ACME server
CLUSTER_ISSUER_NAME="$([ "$USE_STAGING" = true ] && echo "letsencrypt-staging-cloudflare" || echo "letsencrypt-prod-cloudflare")"
ACME_SERVER="$([ "$USE_STAGING" = true ] && echo "https://acme-staging-v02.api.letsencrypt.org/directory" || echo "https://acme-v02.api.letsencrypt.org/directory")"

print_status "Using ClusterIssuer: $CLUSTER_ISSUER_NAME"
print_status "Using ACME server: $ACME_SERVER"

# Check prerequisites
print_status "Checking prerequisites..."

# Check if kubectl is available
if ! command -v kubectl &> /dev/null; then
    print_error "kubectl is not installed or not in PATH"
    exit 1
fi

# Check if cluster is accessible
if ! kubectl cluster-info &> /dev/null; then
    print_error "Cannot connect to Kubernetes cluster"
    exit 1
fi

# Check if cert-manager is installed
if ! kubectl get namespace cert-manager &> /dev/null; then
    print_error "cert-manager namespace not found"
    exit 1
fi

# Check if Cloudflare API credentials exist
if ! kubectl get secret cloudflare-api-token-secret -n cert-manager &> /dev/null; then
    print_error "Cloudflare API credentials secret not found"
    exit 1
fi

# Check if ClusterIssuer exists
if ! kubectl get clusterissuer "$CLUSTER_ISSUER_NAME" &> /dev/null; then
    print_error "ClusterIssuer $CLUSTER_ISSUER_NAME not found"
    exit 1
fi

print_success "Prerequisites check passed"

# Create test certificate
CERT_NAME="${DOMAIN}-test-tls"
print_status "Creating test certificate: $CERT_NAME"

cat << EOF | kubectl apply -f -
apiVersion: cert-manager.io/v1
kind: Certificate
metadata:
  name: ${CERT_NAME}
  namespace: ${NAMESPACE}
spec:
  secretName: ${CERT_NAME}
  issuerRef:
    name: ${CLUSTER_ISSUER_NAME}
    kind: ClusterIssuer
  commonName: "*.${DOMAIN}"
  dnsNames:
  - "${DOMAIN}"
  - "*.${DOMAIN}"
  - "test.${DOMAIN}"
  duration: 2160h  # 90 days
  renewBefore: 720h  # Renew 30 days before expiration
EOF

print_success "Test certificate created"

# Wait for certificate to be issued
print_status "Waiting for certificate to be issued (this may take 2-5 minutes)..."

# Start monitoring in background
(
    while true; do
        if kubectl get certificate "$CERT_NAME" -n "$NAMESPACE" &> /dev/null; then
            STATUS=$(kubectl get certificate "$CERT_NAME" -n "$NAMESPACE" -o jsonpath='{.status.conditions[0].type}')
            if [[ "$STATUS" == "Ready" ]]; then
                break
            elif [[ "$STATUS" == "Failed" ]]; then
                break
            fi
        fi
        sleep 10
        echo -n "."
    done
) &
MONITOR_PID=$!

# Wait for certificate with timeout
TIMEOUT=600  # 10 minutes
ELAPSED=0
while [[ $ELAPSED -lt $TIMEOUT ]]; do
    if ! kill -0 $MONITOR_PID 2>/dev/null; then
        break
    fi
    sleep 5
    ELAPSED=$((ELAPSED + 5))
done

# Kill monitoring process
kill $MONITOR_PID 2>/dev/null || true

echo ""

# Check certificate status
if kubectl get certificate "$CERT_NAME" -n "$NAMESPACE" &> /dev/null; then
    CERT_STATUS=$(kubectl get certificate "$CERT_NAME" -n "$NAMESPACE" -o jsonpath='{.status.conditions[0].type}')
    
    if [[ "$CERT_STATUS" == "Ready" ]]; then
        print_success "Certificate issued successfully!"
        
        # Display certificate details
        print_status "Certificate details:"
        kubectl describe certificate "$CERT_NAME" -n "$NAMESPACE" | grep -E "(Name:|Namespace:|Status:|Not Before:|Not After:|DNS Names:|Secret Name:)"
        
        # Check certificate content
        print_status "Certificate content:"
        kubectl get secret "$CERT_NAME" -n "$NAMESPACE" -o jsonpath='{.data.tls\.crt}' | base64 -d | openssl x509 -text -noout | grep -E "(Subject:|Issuer:|Not Before:|Not After:|DNS:|Serial Number:)"
        
        # Test TLS connectivity (if domain is accessible)
        print_status "Testing TLS connectivity..."
        if command -v openssl &> /dev/null; then
            if timeout 10 openssl s_client -connect "$DOMAIN:443" -servername "$DOMAIN" < /dev/null 2>/dev/null | grep -q "Verify return code: 0"; then
                print_success "TLS connectivity test passed"
            else
                print_warning "TLS connectivity test failed (domain may not be accessible or not using the certificate yet)"
            fi
        fi
        
        # Test DNS propagation
        print_status "Testing DNS propagation..."
        if command -v dig &> /dev/null; then
            print_status "Checking DNS records for $DOMAIN:"
            dig +short "$DOMAIN" A || print_warning "No A record found for $DOMAIN"
            dig +short "$DOMAIN" AAAA || print_warning "No AAAA record found for $DOMAIN"
        fi
        
    elif [[ "$CERT_STATUS" == "Failed" ]]; then
        print_error "Certificate issuance failed!"
        print_status "Certificate status:"
        kubectl describe certificate "$CERT_NAME" -n "$NAMESPACE"
        
        # Check for common issues
        print_status "Checking for common issues..."
        
        # Check ClusterIssuer status
        print_status "ClusterIssuer status:"
        kubectl describe clusterissuer "$CLUSTER_ISSUER_NAME"
        
        # Check certificate events
        print_status "Certificate events:"
        kubectl get events -n "$NAMESPACE" --field-selector involvedObject.name="$CERT_NAME" --sort-by='.metadata.creationTimestamp'
        
        # Check if domain is managed by Cloudflare
        print_status "Checking if domain is managed by Cloudflare..."
        if command -v dig &> /dev/null; then
            NS_RECORDS=$(dig +short "$DOMAIN" NS | tr '\n' ' ')
            print_status "Domain NS records: $NS_RECORDS"
            if echo "$NS_RECORDS" | grep -q "cloudflare"; then
                print_success "Domain appears to be managed by Cloudflare"
            else
                print_warning "Domain may not be managed by Cloudflare (NS records don't contain 'cloudflare')"
            fi
        fi
        
        exit 1
    else
        print_error "Certificate status unknown: $CERT_STATUS"
        kubectl describe certificate "$CERT_NAME" -n "$NAMESPACE"
        exit 1
    fi
else
    print_error "Certificate not found"
    exit 1
fi

# Clean up test certificate
print_status "Cleaning up test certificate..."
kubectl delete certificate "$CERT_NAME" -n "$NAMESPACE"
kubectl delete secret "$CERT_NAME" -n "$NAMESPACE" 2>/dev/null || true

print_success "Test certificate cleaned up"

echo ""
print_success "Cloudflare DNS-01 challenge test completed successfully!"
print_status "The Cloudflare DNS-01 challenge is working correctly."
print_status "You can now use the ClusterIssuer '$CLUSTER_ISSUER_NAME' for production certificates."

if [[ "$USE_STAGING" = true ]]; then
    print_warning "This test used the staging environment."
    print_status "For production certificates, use the production ClusterIssuer 'letsencrypt-prod-cloudflare'."
fi

print_status ""
print_status "NEXT STEPS:"
print_status "1. Verify your domain DNS is managed by Cloudflare"
print_status "2. Ensure Cloudflare API token has correct permissions"
print_status "3. Test with production environment if staging worked"
print_status "4. Set up monitoring for certificate renewal"
