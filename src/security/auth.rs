/*
 * Bevy Debugger MCP Server - JWT Authentication Service
 * Copyright (C) 2025 ladvien
 */

use crate::error::{Error, Result};
use crate::security::rbac::Role;
use crate::security::config::JwtConfig;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

/// JWT claims structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,           // Subject (user ID)
    pub role: Role,            // User role
    pub session_id: String,    // Session ID for revocation
    pub iat: usize,           // Issued at
    pub exp: usize,           // Expiration
    pub aud: String,          // Audience (bevy-debugger-mcp)
    pub iss: String,          // Issuer (bevy-debugger-mcp)
}

/// JWT service for token management
#[derive(Clone)]
pub struct JwtService {
    config: JwtConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    validation: Validation,
    revoked_tokens: Arc<RwLock<HashSet<String>>>,
    active_sessions: Arc<RwLock<HashSet<String>>>,
}

impl JwtService {
    /// Create a new JWT service
    pub fn new(config: JwtConfig) -> Result<Self> {
        // Generate or load the secret key
        let secret = if config.secret_key.is_empty() {
            // Generate a secure random key if none provided
            use ring::rand::SystemRandom;
            use ring::rand::SecureRandom;
            
            let rng = SystemRandom::new();
            let mut key_bytes = vec![0u8; 64]; // 512-bit key
            rng.fill(&mut key_bytes)
                .map_err(|e| Error::SecurityError(format!("Failed to generate JWT secret: {:?}", e)))?;
            
            base64::encode(&key_bytes)
        } else {
            config.secret_key.clone()
        };

        let encoding_key = EncodingKey::from_secret(secret.as_ref());
        let decoding_key = DecodingKey::from_secret(secret.as_ref());

        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&[&config.audience]);
        validation.set_issuer(&[&config.issuer]);

        info!("JWT service initialized with audience: {} issuer: {}", 
              config.audience, config.issuer);

        Ok(Self {
            config,
            encoding_key,
            decoding_key,
            validation,
            revoked_tokens: Arc::new(RwLock::new(HashSet::new())),
            active_sessions: Arc::new(RwLock::new(HashSet::new())),
        })
    }

    /// Generate a new JWT token
    pub async fn generate_token(&self, user_id: &str, role: Role) -> Result<String> {
        let now = chrono::Utc::now().timestamp() as usize;
        let exp = now + (self.config.expiration_hours * 3600) as usize;
        let session_id = Uuid::new_v4().to_string();

        let claims = Claims {
            sub: user_id.to_string(),
            role,
            session_id: session_id.clone(),
            iat: now,
            exp,
            aud: self.config.audience.clone(),
            iss: self.config.issuer.clone(),
        };

        let token = encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| Error::SecurityError(format!("Failed to encode JWT: {}", e)))?;

        // Track active session
        {
            let mut sessions = self.active_sessions.write().await;
            sessions.insert(session_id);
        }

        info!("Generated JWT token for user: {} role: {:?}", user_id, role);
        Ok(token)
    }

    /// Validate a JWT token
    pub async fn validate_token(&self, token: &str) -> Result<Claims> {
        // Check if token is revoked
        {
            let revoked = self.revoked_tokens.read().await;
            if revoked.contains(token) {
                warn!("Attempted to use revoked token");
                return Err(Error::SecurityError("Token has been revoked".to_string()));
            }
        }

        // Decode and validate token
        let token_data = decode::<Claims>(token, &self.decoding_key, &self.validation)
            .map_err(|e| {
                error!("JWT validation failed: {}", e);
                Error::SecurityError(format!("Invalid token: {}", e))
            })?;

        // Check if session is still active
        {
            let sessions = self.active_sessions.read().await;
            if !sessions.contains(&token_data.claims.session_id) {
                warn!("Token for inactive session: {}", token_data.claims.session_id);
                return Err(Error::SecurityError("Session not active".to_string()));
            }
        }

        Ok(token_data.claims)
    }

    /// Revoke a token (add to blacklist)
    pub async fn revoke_token(&self, token: &str) -> Result<()> {
        // First validate the token to get session info
        let claims = self.validate_token(token).await?;
        
        // Add to revoked list
        {
            let mut revoked = self.revoked_tokens.write().await;
            revoked.insert(token.to_string());
        }

        // Remove from active sessions
        {
            let mut sessions = self.active_sessions.write().await;
            sessions.remove(&claims.session_id);
        }

        info!("Revoked token for user: {} session: {}", claims.sub, claims.session_id);
        Ok(())
    }

    /// Get count of active sessions
    pub async fn get_active_session_count(&self) -> u64 {
        let sessions = self.active_sessions.read().await;
        sessions.len() as u64
    }

    /// Clean up expired tokens (should be called periodically)
    pub async fn cleanup_expired_tokens(&self) -> Result<usize> {
        let now = chrono::Utc::now().timestamp() as usize;
        let mut removed = 0;

        // This is a simplified cleanup - in production you'd want to store token metadata
        // to avoid having to decode every token
        let mut revoked = self.revoked_tokens.write().await;
        let mut to_remove = Vec::new();

        for token in revoked.iter() {
            // Try to decode token to check expiration
            if let Ok(token_data) = decode::<Claims>(token, &self.decoding_key, &self.validation) {
                if token_data.claims.exp <= now {
                    to_remove.push(token.clone());
                }
            } else {
                // If we can't decode it, it's expired or invalid, so remove it
                to_remove.push(token.clone());
            }
        }

        for token in to_remove {
            revoked.remove(&token);
            removed += 1;
        }

        if removed > 0 {
            info!("Cleaned up {} expired tokens from revocation list", removed);
        }

        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::config::JwtConfig;

    #[tokio::test]
    async fn test_jwt_token_generation_and_validation() {
        let config = JwtConfig::default();
        let service = JwtService::new(config).unwrap();

        let token = service.generate_token("test_user", Role::Developer).await.unwrap();
        let claims = service.validate_token(&token).await.unwrap();

        assert_eq!(claims.sub, "test_user");
        assert_eq!(claims.role, Role::Developer);
    }

    #[tokio::test]
    async fn test_token_revocation() {
        let config = JwtConfig::default();
        let service = JwtService::new(config).unwrap();

        let token = service.generate_token("test_user", Role::Developer).await.unwrap();
        
        // Token should be valid initially
        assert!(service.validate_token(&token).await.is_ok());
        
        // Revoke the token
        service.revoke_token(&token).await.unwrap();
        
        // Token should be invalid after revocation
        assert!(service.validate_token(&token).await.is_err());
    }

    #[tokio::test]
    async fn test_session_tracking() {
        let config = JwtConfig::default();
        let service = JwtService::new(config).unwrap();

        assert_eq!(service.get_active_session_count().await, 0);

        let _token = service.generate_token("test_user", Role::Developer).await.unwrap();
        assert_eq!(service.get_active_session_count().await, 1);
    }
}