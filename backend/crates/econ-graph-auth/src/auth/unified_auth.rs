/**
 * REQUIREMENT: Unified authentication service supporting both legacy and Keycloak authentication
 * PURPOSE: Provide seamless authentication that can handle both existing JWT tokens and Keycloak tokens
 * This enables parallel testing of Keycloak integration without disrupting existing auth flows
 */

use crate::auth::models::*;
use crate::auth::services::AuthService;
use crate::auth::keycloak::KeycloakAuthService;
use econ_graph_core::error::{AppError, AppResult};
use std::env;

/// Authentication method detected from token
#[derive(Debug, Clone, PartialEq)]
pub enum AuthMethod {
    Legacy,     // Existing JWT tokens
    Keycloak,   // Keycloak OIDC tokens
}

/// Unified authentication service that supports both legacy and Keycloak authentication
#[derive(Clone)]
pub struct UnifiedAuthService {
    legacy_auth: AuthService,
    keycloak_auth: Option<KeycloakAuthService>,
}

impl UnifiedAuthService {
    /// Create new unified authentication service
    pub fn new(db_pool: econ_graph_core::database::DatabasePool) -> Self {
        let legacy_auth = AuthService::new(db_pool);
        
        // Initialize Keycloak auth if configured
        let keycloak_auth = match Self::is_keycloak_enabled() {
            true => {
                match KeycloakAuthService::new() {
                    Ok(keycloak) => {
                        tracing::info!("Keycloak authentication enabled");
                        Some(keycloak)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to initialize Keycloak auth: {}", e);
                        None
                    }
                }
            }
            false => {
                tracing::info!("Keycloak authentication disabled");
                None
            }
        };

        UnifiedAuthService {
            legacy_auth,
            keycloak_auth,
        }
    }

    /// Check if Keycloak authentication is enabled
    fn is_keycloak_enabled() -> bool {
        env::var("KEYCLOAK_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false)
    }

    /// Detect authentication method from token
    fn detect_auth_method(&self, token: &str) -> AppResult<AuthMethod> {
        // Try to decode token header to check issuer
        match jsonwebtoken::decode_header(token) {
            Ok(header) => {
                // Check if token has Keycloak-style claims
                if let Ok(claims) = jsonwebtoken::decode::<serde_json::Value>(
                    token,
                    &jsonwebtoken::DecodingKey::from_secret("dummy".as_ref()),
                    &jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256)
                ) {
                    // Check issuer field to determine auth method
                    if let Some(iss) = claims.claims.get("iss") {
                        if let Some(iss_str) = iss.as_str() {
                            if iss_str.contains("keycloak") || iss_str.contains("auth.econ-graph.local") {
                                return Ok(AuthMethod::Keycloak);
                            }
                        }
                    }
                }
                Ok(AuthMethod::Legacy)
            }
            Err(_) => {
                // If we can't decode the header, assume legacy
                Ok(AuthMethod::Legacy)
            }
        }
    }

    /// Verify token using appropriate authentication method
    pub async fn verify_token(&mut self, token: &str) -> AppResult<(User, AuthMethod)> {
        let auth_method = self.detect_auth_method(token)?;

        match auth_method {
            AuthMethod::Legacy => {
                // Use existing legacy authentication
                let claims = self.legacy_auth.verify_token(token)?;
                let user = self.legacy_auth.get_user_by_id(&claims.sub).await?;
                Ok((user, AuthMethod::Legacy))
            }
            AuthMethod::Keycloak => {
                // Use Keycloak authentication
                if let Some(ref mut keycloak_auth) = self.keycloak_auth {
                    let claims = keycloak_auth.validate_token(token).await?;
                    let user = keycloak_auth.claims_to_user(&claims)?;
                    Ok((user, AuthMethod::Keycloak))
                } else {
                    Err(AppError::AuthenticationError("Keycloak authentication not available".to_string()))
                }
            }
        }
    }

    /// Verify token and return only the user (backward compatibility)
    pub async fn verify_token_user(&mut self, token: &str) -> AppResult<User> {
        let (user, _) = self.verify_token(token).await?;
        Ok(user)
    }

    /// Check if user has specific role using appropriate authentication method
    pub async fn has_role(&mut self, token: &str, role: &str) -> AppResult<bool> {
        let (user, auth_method) = self.verify_token(token).await?;

        match auth_method {
            AuthMethod::Legacy => {
                // Legacy role checking
                Ok(user.role.to_string().to_lowercase() == role.to_lowercase())
            }
            AuthMethod::Keycloak => {
                // Keycloak role checking
                if let Some(ref mut keycloak_auth) = self.keycloak_auth {
                    // Re-verify token to get claims for role checking
                    let claims = keycloak_auth.validate_token(token).await?;
                    Ok(keycloak_auth.has_realm_role(&claims, role) || 
                       keycloak_auth.has_client_role(&claims, role))
                } else {
                    // Fallback to user role
                    Ok(user.role.to_string().to_lowercase() == role.to_lowercase())
                }
            }
        }
    }

    /// Get service token for internal service communication
    pub async fn get_service_token(&mut self) -> AppResult<String> {
        if let Some(ref mut keycloak_auth) = self.keycloak_auth {
            keycloak_auth.get_service_token().await
        } else {
            Err(AppError::AuthenticationError("Service tokens only available with Keycloak".to_string()))
        }
    }

    /// Validate service token
    pub async fn validate_service_token(&mut self, token: &str) -> AppResult<String> {
        if let Some(ref mut keycloak_auth) = self.keycloak_auth {
            keycloak_auth.validate_service_token(token).await
        } else {
            Err(AppError::AuthenticationError("Service tokens only available with Keycloak".to_string()))
        }
    }

    /// Check if Keycloak authentication is available
    pub fn is_keycloak_available(&self) -> bool {
        self.keycloak_auth.is_some()
    }

    /// Get authentication method for a token without full verification
    pub fn get_auth_method(&self, token: &str) -> AppResult<AuthMethod> {
        self.detect_auth_method(token)
    }

    /// Get legacy auth service for backward compatibility
    pub fn get_legacy_auth(&self) -> &AuthService {
        &self.legacy_auth
    }

    /// Get Keycloak auth service if available
    pub fn get_keycloak_auth(&self) -> Option<&KeycloakAuthService> {
        self.keycloak_auth.as_ref()
    }
}

/// Middleware function that works with unified authentication
pub async fn unified_auth_middleware(
    auth_service: &mut UnifiedAuthService,
    token: &str,
) -> AppResult<User> {
    auth_service.verify_token_user(token).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use econ_graph_core::database::DatabasePool;

    #[tokio::test]
    async fn test_auth_method_detection() {
        // This would require setting up test infrastructure
        // For now, just test the detection logic
        let legacy_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJlY29uLWdyYXBoIiwic3ViIjoiMTIzIn0.test";
        let keycloak_token = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJodHRwOi8vYXV0aC5lY29uLWdyYXBoLmxvY2FsL3JlYWxtcy9lY29uLWdyYXBoIn0.test";
        
        // Mock database pool for testing
        let db_pool = DatabasePool::new("postgresql://test").unwrap();
        let unified_auth = UnifiedAuthService::new(db_pool);
        
        // Test legacy detection
        let method = unified_auth.get_auth_method(legacy_token).unwrap();
        assert_eq!(method, AuthMethod::Legacy);
        
        // Test Keycloak detection
        let method = unified_auth.get_auth_method(keycloak_token).unwrap();
        assert_eq!(method, AuthMethod::Keycloak);
    }
}
