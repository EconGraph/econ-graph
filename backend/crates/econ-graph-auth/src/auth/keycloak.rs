/**
 * REQUIREMENT: Keycloak authentication integration for parallel authentication system
 * PURPOSE: Provide Keycloak token validation alongside existing JWT authentication
 * This enables testing Keycloak integration without disrupting existing auth flows
 */
use crate::auth::models::*;
use crate::auth::permissions::EconGraphPermission;
use chrono::Utc;
use econ_graph_core::error::{AppError, AppResult};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use uuid::Uuid;

/// Keycloak token claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct KeycloakClaims {
    pub sub: String, // Subject (user ID)
    pub email: String,
    pub name: String,
    pub preferred_username: Option<String>,
    pub realm_access: RealmAccess,
    pub resource_access: ResourceAccess,
    pub exp: usize,  // Expiration time
    pub iat: usize,  // Issued at
    pub iss: String, // Issuer
    pub aud: String, // Audience
    // EconGraph-specific claims
    #[serde(rename = "econ-graph")]
    pub econ_graph: Option<EconGraphClaims>,
    #[serde(flatten)]
    pub additional_claims: HashMap<String, serde_json::Value>,
}

/// EconGraph-specific claims in JWT token
#[derive(Debug, Serialize, Deserialize)]
pub struct EconGraphClaims {
    pub permissions: Vec<String>,
    pub subscription_tier: Option<String>,
    pub organization: Option<String>,
    pub usage_limits: Option<UsageLimits>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UsageLimits {
    pub graphs_per_month: Option<u32>,
    pub exports_per_month: Option<u32>,
    pub api_requests_per_hour: Option<u32>,
    pub storage_gb: Option<u32>,
    pub team_members: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RealmAccess {
    pub roles: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceAccess {
    #[serde(rename = "econ-graph-backend")]
    pub backend: Option<ClientAccess>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientAccess {
    pub roles: Vec<String>,
}

/// Keycloak public key response
#[derive(Debug, Deserialize)]
struct KeycloakPublicKeys {
    keys: Vec<KeycloakPublicKey>,
}

#[derive(Debug, Serialize, Deserialize)]
struct KeycloakPublicKey {
    kid: String,
    kty: String,
    alg: String,
    use_: String,
    n: String,
    e: String,
    #[serde(rename = "use")]
    _use: Option<String>,
}

/// Keycloak authentication service
#[derive(Clone)]
pub struct KeycloakAuthService {
    http_client: Client,
    keycloak_url: String,
    realm: String,
    client_id: String,
    client_secret: String,
    cached_public_keys: Option<HashMap<String, String>>,
}

impl KeycloakAuthService {
    /// Create new Keycloak authentication service
    pub fn new() -> AppResult<Self> {
        let keycloak_url =
            env::var("KEYCLOAK_URL").unwrap_or_else(|_| "http://auth.econ-graph.local".to_string());
        let realm = env::var("KEYCLOAK_REALM").unwrap_or_else(|_| "econ-graph".to_string());
        let client_id =
            env::var("KEYCLOAK_CLIENT_ID").unwrap_or_else(|_| "econ-graph-backend".to_string());
        let client_secret =
            env::var("KEYCLOAK_CLIENT_SECRET").unwrap_or_else(|_| "backend-secret".to_string());

        Ok(KeycloakAuthService {
            http_client: Client::new(),
            keycloak_url,
            realm,
            client_id,
            client_secret,
            cached_public_keys: None,
        })
    }

    /// Fetch Keycloak public keys for token validation
    async fn fetch_public_keys(&mut self) -> AppResult<HashMap<String, String>> {
        let url = format!(
            "{}/realms/{}/protocol/openid-connect/certs",
            self.keycloak_url, self.realm
        );

        let response = self.http_client.get(&url).send().await.map_err(|e| {
            AppError::AuthenticationError(format!("Failed to fetch Keycloak keys: {}", e))
        })?;

        if !response.status().is_success() {
            return Err(AppError::AuthenticationError(format!(
                "Keycloak keys endpoint returned status: {}",
                response.status()
            )));
        }

        let keys_response: KeycloakPublicKeys = response.json().await.map_err(|e| {
            AppError::AuthenticationError(format!("Failed to parse Keycloak keys: {}", e))
        })?;

        let mut public_keys = HashMap::new();
        for key in keys_response.keys {
            if key.use_ == "sig" && key.alg == "RS256" {
                // Convert JWK to PEM format (simplified - in production use a proper JWK library)
                let pem_key = self.jwk_to_pem(&key)?;
                public_keys.insert(key.kid, pem_key);
            }
        }

        Ok(public_keys)
    }

    /// Convert JWK to PEM format (simplified implementation)
    fn jwk_to_pem(&self, jwk: &KeycloakPublicKey) -> AppResult<String> {
        // This is a simplified implementation
        // In production, you should use a proper JWK library like `jsonwebtoken` with JWK support
        // or a dedicated JWK library

        // For now, we'll use the raw JWK data
        // This is not secure for production - use proper JWK to PEM conversion
        let jwk_json = serde_json::to_string(jwk).map_err(|e| {
            AppError::AuthenticationError(format!("Failed to serialize JWK: {}", e))
        })?;

        // In a real implementation, you would:
        // 1. Decode the base64url encoded n and e values
        // 2. Construct the RSA public key
        // 3. Convert to PEM format
        // For now, we'll return the JWK as a placeholder
        Ok(format!(
            "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----",
            jwk_json
        ))
    }

    /// Validate Keycloak JWT token
    pub async fn validate_token(&mut self, token: &str) -> AppResult<KeycloakClaims> {
        // Get token header to find the key ID
        let header = jsonwebtoken::decode_header(token)
            .map_err(|e| AppError::AuthenticationError(format!("Invalid token header: {}", e)))?;

        let kid = header
            .kid
            .ok_or_else(|| AppError::AuthenticationError("Token missing key ID".to_string()))?;

        // Fetch public keys if not cached
        if self.cached_public_keys.is_none() {
            self.cached_public_keys = Some(self.fetch_public_keys().await?);
        }

        let public_keys = self.cached_public_keys.as_ref().unwrap();
        let public_key = public_keys
            .get(&kid)
            .ok_or_else(|| AppError::AuthenticationError(format!("Unknown key ID: {}", kid)))?;

        // Validate token
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[format!("{}/realms/{}", self.keycloak_url, self.realm)]);
        validation.set_audience(&[&self.client_id]);

        let token_data = decode::<KeycloakClaims>(
            token,
            &DecodingKey::from_rsa_pem(public_key.as_bytes())?,
            &validation,
        )
        .map_err(|e| AppError::AuthenticationError(format!("Token validation failed: {}", e)))?;

        Ok(token_data.claims)
    }

    /// Check if user has specific role
    pub fn has_realm_role(&self, claims: &KeycloakClaims, role: &str) -> bool {
        claims.realm_access.roles.contains(&role.to_string())
    }

    /// Check if user has specific client role
    pub fn has_client_role(&self, claims: &KeycloakClaims, role: &str) -> bool {
        if let Some(backend_access) = &claims.resource_access.backend {
            backend_access.roles.contains(&role.to_string())
        } else {
            false
        }
    }

    /// Convert Keycloak claims to our internal User model
    pub fn claims_to_user(&self, claims: &KeycloakClaims) -> AppResult<User> {
        // Extract user ID from subject
        let user_id = Uuid::parse_str(&claims.sub).map_err(|e| {
            AppError::AuthenticationError(format!("Invalid user ID in token: {}", e))
        })?;

        // Determine user role from Keycloak roles
        let role = if self.has_realm_role(claims, "admin") {
            UserRole::Admin
        } else if self.has_realm_role(claims, "analyst") {
            UserRole::Analyst
        } else {
            UserRole::Viewer
        };

        // Extract organization from custom claims if present
        let organization = claims
            .additional_claims
            .get("organization")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(User {
            id: user_id,
            email: claims.email.clone(),
            name: claims.name.clone(),
            avatar: None,
            provider: AuthProvider::Email, // Keycloak users are treated as email-based
            provider_id: claims.sub.clone(),
            role,
            organization,
            preferences: UserPreferences::default(),
            created_at: Utc::now(),
            last_login_at: Utc::now(),
            is_active: true,
        })
    }

    /// Get service account token for internal service communication
    pub async fn get_service_token(&self) -> AppResult<String> {
        let token_url = format!(
            "{}/realms/{}/protocol/openid-connect/token",
            self.keycloak_url, self.realm
        );

        let params = [
            ("grant_type", "client_credentials"),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
        ];

        let response = self
            .http_client
            .post(&token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| {
                AppError::AuthenticationError(format!("Failed to get service token: {}", e))
            })?;

        if !response.status().is_success() {
            return Err(AppError::AuthenticationError(format!(
                "Service token request failed with status: {}",
                response.status()
            )));
        }

        let token_response: serde_json::Value = response.json().await.map_err(|e| {
            AppError::AuthenticationError(format!("Failed to parse token response: {}", e))
        })?;

        token_response
            .get("access_token")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::AuthenticationError("No access token in response".to_string()))
    }

    /// Validate service token (for service-to-service authentication)
    pub async fn validate_service_token(&mut self, token: &str) -> AppResult<String> {
        let claims = self.validate_token(token).await?;

        // Check if this is a service account token
        if claims.aud == self.client_id {
            Ok(claims.sub)
        } else {
            Err(AppError::AuthenticationError(
                "Invalid service token".to_string(),
            ))
        }
    }

    /// Extract EconGraph permissions from JWT token
    pub async fn extract_permissions(
        &mut self,
        token: &str,
    ) -> AppResult<Vec<EconGraphPermission>> {
        let claims = self.validate_token(token).await?;

        let mut permissions = Vec::new();

        // Extract permissions from EconGraph claims
        if let Some(econ_graph) = &claims.econ_graph {
            for permission_str in &econ_graph.permissions {
                if let Some(permission) = EconGraphPermission::from_string(permission_str) {
                    permissions.push(permission);
                }
            }
        }

        // Also extract permissions from realm roles (fallback)
        for role in &claims.realm_access.roles {
            match role.as_str() {
                "admin" => {
                    permissions.extend(vec![
                        EconGraphPermission::AdminUsers,
                        EconGraphPermission::AdminSystem,
                        EconGraphPermission::AdminAudit,
                        EconGraphPermission::AdminSecurity,
                        EconGraphPermission::AdminBilling,
                    ]);
                }
                "analyst" => {
                    permissions.extend(vec![
                        EconGraphPermission::DataRead,
                        EconGraphPermission::GraphCreate,
                        EconGraphPermission::GraphSave,
                        EconGraphPermission::ExportImage,
                        EconGraphPermission::NotesAdd,
                        EconGraphPermission::AnalyticsBasic,
                    ]);
                }
                "viewer" => {
                    permissions.extend(vec![EconGraphPermission::DataRead]);
                }
                _ => {}
            }
        }

        Ok(permissions)
    }

    /// Extract subscription tier from JWT token
    pub async fn extract_subscription_tier(&mut self, token: &str) -> AppResult<Option<String>> {
        let claims = self.validate_token(token).await?;
        Ok(claims.econ_graph.and_then(|eg| eg.subscription_tier))
    }

    /// Extract usage limits from JWT token
    pub async fn extract_usage_limits(&mut self, token: &str) -> AppResult<Option<UsageLimits>> {
        let claims = self.validate_token(token).await?;
        Ok(claims.econ_graph.and_then(|eg| eg.usage_limits))
    }

    /// Extract organization from JWT token
    pub async fn extract_organization(&mut self, token: &str) -> AppResult<Option<String>> {
        let claims = self.validate_token(token).await?;
        Ok(claims.econ_graph.and_then(|eg| eg.organization))
    }

    /// Check if user has specific permission
    pub async fn has_permission(
        &mut self,
        token: &str,
        permission: &EconGraphPermission,
    ) -> AppResult<bool> {
        let permissions = self.extract_permissions(token).await?;
        Ok(permissions.contains(permission))
    }

    /// Check if export requires watermark
    pub async fn export_requires_watermark(&mut self, token: &str) -> AppResult<bool> {
        let has_watermark_free = self
            .has_permission(token, &EconGraphPermission::ExportWatermarkFree)
            .await?;
        Ok(!has_watermark_free) // Requires watermark if user doesn't have watermark-free permission
    }
}
