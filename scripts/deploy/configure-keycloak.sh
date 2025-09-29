#!/bin/bash

# Configure Keycloak realm and clients
# This script sets up the EconGraph realm and initial clients

set -e

echo "🔧 Configuring Keycloak realm and clients..."
echo "============================================="

# Configuration
KEYCLOAK_URL="http://auth.econ-graph.local"
ADMIN_USER="admin"
ADMIN_PASSWORD="admin123"
REALM_NAME="econ-graph"

# Get access token
echo "🔑 Getting admin access token..."
ACCESS_TOKEN=$(curl -s -X POST \
  "${KEYCLOAK_URL}/realms/master/protocol/openid-connect/token" \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "username=${ADMIN_USER}" \
  -d "password=${ADMIN_PASSWORD}" \
  -d "grant_type=password" \
  -d "client_id=admin-cli" | jq -r '.access_token')

if [ "$ACCESS_TOKEN" = "null" ] || [ -z "$ACCESS_TOKEN" ]; then
    echo "❌ Failed to get access token. Make sure Keycloak is running and accessible."
    exit 1
fi

echo "✅ Access token obtained"

# Create realm
echo "🏛️  Creating realm: ${REALM_NAME}..."
curl -s -X POST \
  "${KEYCLOAK_URL}/admin/realms" \
  -H "Authorization: Bearer ${ACCESS_TOKEN}" \
  -H "Content-Type: application/json" \
  -d "{
    \"realm\": \"${REALM_NAME}\",
    \"enabled\": true,
    \"displayName\": \"EconGraph Platform\",
    \"sslRequired\": \"external\",
    \"accessTokenLifespan\": 3600,
    \"ssoSessionIdleTimeout\": 1800,
    \"ssoSessionMaxLifespan\": 36000,
    \"registrationAllowed\": true,
    \"registrationEmailAsUsername\": true,
    \"loginWithEmailAllowed\": true,
    \"duplicateEmailsAllowed\": false,
    \"resetPasswordAllowed\": true,
    \"editUsernameAllowed\": false,
    \"bruteForceProtected\": true
  }" > /dev/null

echo "✅ Realm created"

# Create frontend client (public)
echo "🌐 Creating frontend client..."
curl -s -X POST \
  "${KEYCLOAK_URL}/admin/realms/${REALM_NAME}/clients" \
  -H "Authorization: Bearer ${ACCESS_TOKEN}" \
  -H "Content-Type: application/json" \
  -d "{
    \"clientId\": \"econ-graph-frontend\",
    \"enabled\": true,
    \"publicClient\": true,
    \"standardFlowEnabled\": true,
    \"implicitFlowEnabled\": false,
    \"directAccessGrantsEnabled\": false,
    \"serviceAccountsEnabled\": false,
    \"redirectUris\": [
      \"http://admin.econ-graph.local/*\",
      \"http://localhost:3000/*\"
    ],
    \"webOrigins\": [
      \"http://admin.econ-graph.local\",
      \"http://localhost:3000\"
    ],
    \"protocol\": \"openid-connect\",
    \"attributes\": {
      \"pkce.code.challenge.method\": \"S256\"
    }
  }" > /dev/null

echo "✅ Frontend client created"

# Create backend client (confidential)
echo "🔧 Creating backend client..."
curl -s -X POST \
  "${KEYCLOAK_URL}/admin/realms/${REALM_NAME}/clients" \
  -H "Authorization: Bearer ${ACCESS_TOKEN}" \
  -H "Content-Type: application/json" \
  -d "{
    \"clientId\": \"econ-graph-backend\",
    \"enabled\": true,
    \"publicClient\": false,
    \"standardFlowEnabled\": false,
    \"implicitFlowEnabled\": false,
    \"directAccessGrantsEnabled\": false,
    \"serviceAccountsEnabled\": true,
    \"authorizationServicesEnabled\": true,
    \"clientAuthenticatorType\": \"client-secret\",
    \"secret\": \"backend-secret\",
    \"protocol\": \"openid-connect\",
    \"defaultClientScopes\": [
      \"profile\",
      \"email\",
      \"roles\"
    ],
    \"optionalClientScopes\": [
      \"address\",
      \"phone\",
      \"offline_access\",
      \"microprofile-jwt\"
    ]
  }" > /dev/null

echo "✅ Backend client created"

# Create chart API service client
echo "📊 Creating chart API service client..."
curl -s -X POST \
  "${KEYCLOAK_URL}/admin/realms/${REALM_NAME}/clients" \
  -H "Authorization: Bearer ${ACCESS_TOKEN}" \
  -H "Content-Type: application/json" \
  -d "{
    \"clientId\": \"econ-graph-chart-api\",
    \"enabled\": true,
    \"publicClient\": false,
    \"standardFlowEnabled\": false,
    \"implicitFlowEnabled\": false,
    \"directAccessGrantsEnabled\": false,
    \"serviceAccountsEnabled\": true,
    \"clientAuthenticatorType\": \"client-secret\",
    \"secret\": \"chart-api-secret\",
    \"protocol\": \"openid-connect\",
    \"defaultClientScopes\": [
      \"profile\",
      \"email\",
      \"roles\"
    ]
  }" > /dev/null

echo "✅ Chart API service client created"

# Create MCP server client
echo "🔌 Creating MCP server client..."
curl -s -X POST \
  "${KEYCLOAK_URL}/admin/realms/${REALM_NAME}/clients" \
  -H "Authorization: Bearer ${ACCESS_TOKEN}" \
  -H "Content-Type: application/json" \
  -d "{
    \"clientId\": \"econ-graph-mcp-server\",
    \"enabled\": true,
    \"publicClient\": false,
    \"standardFlowEnabled\": false,
    \"implicitFlowEnabled\": false,
    \"directAccessGrantsEnabled\": false,
    \"serviceAccountsEnabled\": true,
    \"clientAuthenticatorType\": \"client-secret\",
    \"secret\": \"mcp-server-secret\",
    \"protocol\": \"openid-connect\",
    \"defaultClientScopes\": [
      \"profile\",
      \"email\",
      \"roles\"
    ]
  }" > /dev/null

echo "✅ MCP server client created"

# Create realm roles
echo "👥 Creating realm roles..."
curl -s -X POST \
  "${KEYCLOAK_URL}/admin/realms/${REALM_NAME}/roles" \
  -H "Authorization: Bearer ${ACCESS_TOKEN}" \
  -H "Content-Type: application/json" \
  -d "{
    \"name\": \"admin\",
    \"description\": \"Full platform access\"
  }" > /dev/null

curl -s -X POST \
  "${KEYCLOAK_URL}/admin/realms/${REALM_NAME}/roles" \
  -H "Authorization: Bearer ${ACCESS_TOKEN}" \
  -H "Content-Type: application/json" \
  -d "{
    \"name\": \"analyst\",
    \"description\": \"Data analysis and visualization\"
  }" > /dev/null

curl -s -X POST \
  "${KEYCLOAK_URL}/admin/realms/${REALM_NAME}/roles" \
  -H "Authorization: Bearer ${ACCESS_TOKEN}" \
  -H "Content-Type: application/json" \
  -d "{
    \"name\": \"viewer\",
    \"description\": \"Read-only access\"
  }" > /dev/null

echo "✅ Realm roles created"

echo ""
echo "🎉 Keycloak configuration completed successfully!"
echo "============================================="
echo ""
echo "🔐 Keycloak Configuration Summary:"
echo "  Realm: ${REALM_NAME}"
echo "  Frontend Client: econ-graph-frontend (public)"
echo "  Backend Client: econ-graph-backend (confidential)"
echo "  Chart API Client: econ-graph-chart-api (service account)"
echo "  MCP Server Client: econ-graph-mcp-server (service account)"
echo ""
echo "👥 Default Roles:"
echo "  - admin: Full platform access"
echo "  - analyst: Data analysis and visualization"
echo "  - viewer: Read-only access"
echo ""
echo "🌐 Access URLs:"
echo "  Admin Console: ${KEYCLOAK_URL}"
echo "  Realm URL: ${KEYCLOAK_URL}/realms/${REALM_NAME}"
echo "  OIDC Discovery: ${KEYCLOAK_URL}/realms/${REALM_NAME}/.well-known/openid_configuration"
echo ""
echo "🔧 Next Steps:"
echo "  1. Create test users in the Keycloak admin console"
echo "  2. Configure OAuth providers (Google, Facebook) if needed"
echo "  3. Set up client roles and permissions"
echo "  4. Test authentication flows"
