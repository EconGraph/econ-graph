#!/bin/bash

# Configure Keycloak realm and clients for EconGraph
# This script sets up the econ-graph realm and necessary clients

set -e

echo "🔧 Configuring Keycloak for EconGraph..."

# Get the project root directory
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

# Configuration
KEYCLOAK_URL="http://auth.econ-graph.local"
ADMIN_USER="admin"
ADMIN_PASSWORD="admin123"
REALM_NAME="econ-graph"

# Check if Keycloak is accessible
echo "🔍 Checking Keycloak accessibility..."
if ! curl -s -o /dev/null -w "%{http_code}" "$KEYCLOAK_URL" | grep -q "200"; then
    echo "❌ Keycloak is not accessible at $KEYCLOAK_URL"
    echo "   Please ensure Keycloak is deployed and running."
    echo "   Run: kubectl get pods -l app=keycloak -n econ-graph"
    exit 1
fi

echo "✅ Keycloak is accessible"

# Wait for Keycloak to be fully ready (admin console available)
echo "⏳ Waiting for Keycloak admin console to be ready..."
for i in {1..30}; do
    if curl -s "$KEYCLOAK_URL/admin" | grep -q "Keycloak"; then
        echo "✅ Keycloak admin console is ready"
        break
    fi
    echo "   Waiting for admin console... (attempt $i/30)"
    sleep 10
done

# Get admin token
echo "🔑 Getting admin token..."
ADMIN_TOKEN=$(curl -s -X POST "$KEYCLOAK_URL/realms/master/protocol/openid-connect/token" \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "username=$ADMIN_USER" \
  -d "password=$ADMIN_PASSWORD" \
  -d "grant_type=password" \
  -d "client_id=admin-cli" | \
  jq -r '.access_token')

if [ "$ADMIN_TOKEN" = "null" ] || [ -z "$ADMIN_TOKEN" ]; then
    echo "❌ Failed to get admin token"
    echo "   Please check Keycloak admin credentials"
    exit 1
fi

echo "✅ Admin token obtained"

# Create realm
echo "🏗️  Creating econ-graph realm..."
REALM_EXISTS=$(curl -s -H "Authorization: Bearer $ADMIN_TOKEN" \
  "$KEYCLOAK_URL/admin/realms/$REALM_NAME" | \
  jq -r '.realm // empty')

if [ -z "$REALM_EXISTS" ]; then
    curl -s -X POST "$KEYCLOAK_URL/admin/realms" \
      -H "Authorization: Bearer $ADMIN_TOKEN" \
      -H "Content-Type: application/json" \
      -d '{
        "realm": "'$REALM_NAME'",
        "enabled": true,
        "displayName": "EconGraph",
        "displayNameHtml": "<div class=\"kc-logo-text\"><span>EconGraph</span></div>",
        "loginTheme": "keycloak",
        "accountTheme": "keycloak",
        "adminTheme": "keycloak",
        "emailTheme": "keycloak",
        "internationalizationEnabled": true,
        "supportedLocales": ["en", "es", "de", "fr"],
        "defaultLocale": "en"
      }'
    echo "✅ Realm '$REALM_NAME' created"
else
    echo "✅ Realm '$REALM_NAME' already exists"
fi

# Get realm admin token
echo "🔑 Getting realm admin token..."
REALM_ADMIN_TOKEN=$(curl -s -X POST "$KEYCLOAK_URL/realms/master/protocol/openid-connect/token" \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "username=$ADMIN_USER" \
  -d "password=$ADMIN_PASSWORD" \
  -d "grant_type=password" \
  -d "client_id=admin-cli" | \
  jq -r '.access_token')

# Create frontend client
echo "🖥️  Creating frontend client..."
FRONTEND_CLIENT_EXISTS=$(curl -s -H "Authorization: Bearer $REALM_ADMIN_TOKEN" \
  "$KEYCLOAK_URL/admin/realms/$REALM_NAME/clients?clientId=econ-graph-frontend" | \
  jq -r '.[0].id // empty')

if [ -z "$FRONTEND_CLIENT_EXISTS" ]; then
    FRONTEND_CLIENT_ID=$(curl -s -X POST "$KEYCLOAK_URL/admin/realms/$REALM_NAME/clients" \
      -H "Authorization: Bearer $REALM_ADMIN_TOKEN" \
      -H "Content-Type: application/json" \
      -d '{
        "clientId": "econ-graph-frontend",
        "name": "EconGraph Frontend",
        "description": "EconGraph React Frontend Application",
        "enabled": true,
        "clientAuthenticatorType": "client-secret",
        "secret": "frontend-client-secret",
        "redirectUris": [
          "http://localhost:3000/*",
          "http://admin.econ-graph.local/*",
          "https://www.econgraph.com/*",
          "https://econgraph.com/*"
        ],
        "webOrigins": [
          "http://localhost:3000",
          "http://admin.econ-graph.local",
          "https://www.econgraph.com",
          "https://econgraph.com"
        ],
        "protocol": "openid-connect",
        "publicClient": false,
        "standardFlowEnabled": true,
        "implicitFlowEnabled": false,
        "directAccessGrantsEnabled": true,
        "serviceAccountsEnabled": false,
        "attributes": {
          "access.token.lifespan": "3600",
          "client.secret.creation.time": "1640995200"
        }
      }' | jq -r '.id')
    echo "✅ Frontend client created with ID: $FRONTEND_CLIENT_ID"
else
    echo "✅ Frontend client already exists"
fi

# Create backend client
echo "⚙️  Creating backend client..."
BACKEND_CLIENT_EXISTS=$(curl -s -H "Authorization: Bearer $REALM_ADMIN_TOKEN" \
  "$KEYCLOAK_URL/admin/realms/$REALM_NAME/clients?clientId=econ-graph-backend" | \
  jq -r '.[0].id // empty')

if [ -z "$BACKEND_CLIENT_EXISTS" ]; then
    BACKEND_CLIENT_ID=$(curl -s -X POST "$KEYCLOAK_URL/admin/realms/$REALM_NAME/clients" \
      -H "Authorization: Bearer $REALM_ADMIN_TOKEN" \
      -H "Content-Type: application/json" \
      -d '{
        "clientId": "econ-graph-backend",
        "name": "EconGraph Backend",
        "description": "EconGraph Rust Backend API",
        "enabled": true,
        "clientAuthenticatorType": "client-secret",
        "secret": "backend-client-secret",
        "protocol": "openid-connect",
        "publicClient": false,
        "standardFlowEnabled": false,
        "implicitFlowEnabled": false,
        "directAccessGrantsEnabled": false,
        "serviceAccountsEnabled": true,
        "attributes": {
          "access.token.lifespan": "3600",
          "client.secret.creation.time": "1640995200"
        }
      }' | jq -r '.id')
    echo "✅ Backend client created with ID: $BACKEND_CLIENT_ID"
else
    echo "✅ Backend client already exists"
fi

# Create sample users and roles
echo "👥 Creating sample users and roles..."

# Create roles
echo "   Creating roles..."
curl -s -X POST "$KEYCLOAK_URL/admin/realms/$REALM_NAME/roles" \
  -H "Authorization: Bearer $REALM_ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "admin", "description": "Administrator role"}' >/dev/null || true

curl -s -X POST "$KEYCLOAK_URL/admin/realms/$REALM_NAME/roles" \
  -H "Authorization: Bearer $REALM_ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "analyst", "description": "Financial analyst role"}' >/dev/null || true

curl -s -X POST "$KEYCLOAK_URL/admin/realms/$REALM_NAME/roles" \
  -H "Authorization: Bearer $REALM_ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "viewer", "description": "View-only role"}' >/dev/null || true

echo "✅ Roles created"

# Create sample users
echo "   Creating sample users..."

# Admin user
curl -s -X POST "$KEYCLOAK_URL/admin/realms/$REALM_NAME/users" \
  -H "Authorization: Bearer $REALM_ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "admin",
    "email": "admin@econgraph.com",
    "firstName": "Admin",
    "lastName": "User",
    "enabled": true,
    "emailVerified": true,
    "credentials": [{
      "type": "password",
      "value": "admin123",
      "temporary": false
    }]
  }' >/dev/null || true

# Analyst user
curl -s -X POST "$KEYCLOAK_URL/admin/realms/$REALM_NAME/users" \
  -H "Authorization: Bearer $REALM_ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "analyst",
    "email": "analyst@econgraph.com",
    "firstName": "Financial",
    "lastName": "Analyst",
    "enabled": true,
    "emailVerified": true,
    "credentials": [{
      "type": "password",
      "value": "analyst123",
      "temporary": false
    }]
  }' >/dev/null || true

# Viewer user
curl -s -X POST "$KEYCLOAK_URL/admin/realms/$REALM_NAME/users" \
  -H "Authorization: Bearer $REALM_ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "viewer",
    "email": "viewer@econgraph.com",
    "firstName": "Data",
    "lastName": "Viewer",
    "enabled": true,
    "emailVerified": true,
    "credentials": [{
      "type": "password",
      "value": "viewer123",
      "temporary": false
    }]
  }' >/dev/null || true

echo "✅ Sample users created"

echo ""
echo "🎉 Keycloak configuration completed successfully!"
echo ""
echo "📋 Configuration Summary:"
echo "  Realm: $REALM_NAME"
echo "  Frontend Client: econ-graph-frontend"
echo "  Backend Client: econ-graph-backend"
echo ""
echo "👥 Sample Users:"
echo "  Admin: admin / admin123"
echo "  Analyst: analyst / analyst123"
echo "  Viewer: viewer / viewer123"
echo ""
echo "🌐 Access URLs:"
echo "  Admin Console: $KEYCLOAK_URL/admin"
echo "  Account Console: $KEYCLOAK_URL/realms/$REALM_NAME/account"
echo ""
echo "🔧 Next steps:"
echo "  1. Update backend environment variables to use Keycloak"
echo "  2. Configure frontend to use Keycloak authentication"
echo "  3. Test authentication flow"
