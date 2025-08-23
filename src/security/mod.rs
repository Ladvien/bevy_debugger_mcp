/*
 * Bevy Debugger MCP Server - Security Module
 * Copyright (C) 2025 ladvien
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

//! Comprehensive security module for production-grade authentication and authorization
//! 
//! This module provides:
//! - JWT-based authentication with secure token validation
//! - Role-based access control (RBAC) with viewer/developer/admin roles
//! - Configurable rate limiting to prevent abuse
//! - Comprehensive audit logging for security compliance
//! - Security middleware for MCP protocol integration

pub mod auth;
pub mod rbac;
pub mod rate_limit;
pub mod audit;
pub mod middleware;
pub mod config;

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Security context for a request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityContext {
    pub user_id: String,
    pub role: rbac::Role,
    pub session_id: String,
    pub authenticated_at: chrono::DateTime<chrono::Utc>,
    pub permissions: Vec<rbac::Permission>,
    pub client_ip: Option<std::net::IpAddr>,
    pub user_agent: Option<String>,
}

/// Security manager that coordinates all security components
#[derive(Clone)]
pub struct SecurityManager {
    jwt_service: auth::JwtService,
    rbac_service: rbac::RbacService,
    rate_limiter: rate_limit::RateLimiter,
    audit_logger: audit::AuditLogger,
    config: config::SecurityConfig,
}

impl SecurityManager {
    /// Create a new security manager with the given configuration
    pub fn new(config: config::SecurityConfig) -> Result<Self> {
        info!("Initializing SecurityManager with config: {:?}", config);
        
        let jwt_service = auth::JwtService::new(config.jwt.clone())?;
        let rbac_service = rbac::RbacService::new(config.rbac.clone());
        let rate_limiter = rate_limit::RateLimiter::new(config.rate_limit.clone());
        let audit_logger = audit::AuditLogger::new(config.audit.clone())?;

        Ok(Self {
            jwt_service,
            rbac_service,
            rate_limiter,
            audit_logger,
            config,
        })
    }

    /// Authenticate a request and return security context
    pub async fn authenticate(&self, token: &str, client_info: ClientInfo) -> Result<SecurityContext> {
        // Validate JWT token
        let claims = self.jwt_service.validate_token(token).await?;
        
        // Get user permissions
        let permissions = self.rbac_service.get_permissions(&claims.role).await?;
        
        // Create security context
        let context = SecurityContext {
            user_id: claims.sub,
            role: claims.role,
            session_id: claims.session_id,
            authenticated_at: chrono::DateTime::from_timestamp(claims.iat as i64, 0)
                .ok_or_else(|| Error::SecurityError("Invalid token timestamp".to_string()))?,
            permissions,
            client_ip: client_info.ip,
            user_agent: client_info.user_agent,
        };

        // Log authentication
        self.audit_logger.log_authentication(&context).await?;

        info!("Successfully authenticated user: {} with role: {:?}", 
              context.user_id, context.role);

        Ok(context)
    }

    /// Check if a request is authorized for the given operation
    pub async fn authorize(&self, context: &SecurityContext, operation: &str, resource: &str) -> Result<bool> {
        // Check rate limits first
        if !self.rate_limiter.check_limit(&context.user_id, operation).await? {
            warn!("Rate limit exceeded for user: {} operation: {}", context.user_id, operation);
            self.audit_logger.log_rate_limit_exceeded(context, operation).await?;
            return Ok(false);
        }

        // Check RBAC permissions
        let authorized = self.rbac_service.check_permission(context, operation, resource).await?;
        
        if !authorized {
            warn!("Authorization denied for user: {} operation: {} resource: {}", 
                  context.user_id, operation, resource);
            self.audit_logger.log_authorization_denied(context, operation, resource).await?;
        } else {
            self.audit_logger.log_operation(context, operation, resource).await?;
        }

        Ok(authorized)
    }

    /// Generate a new JWT token for a user
    pub async fn generate_token(&self, user_id: &str, role: rbac::Role) -> Result<String> {
        self.jwt_service.generate_token(user_id, role).await
    }

    /// Revoke a token (add to blacklist)
    pub async fn revoke_token(&self, token: &str) -> Result<()> {
        self.jwt_service.revoke_token(token).await
    }

    /// Get security metrics for monitoring
    pub async fn get_metrics(&self) -> SecurityMetrics {
        SecurityMetrics {
            active_sessions: self.jwt_service.get_active_session_count().await,
            rate_limit_violations: self.rate_limiter.get_violation_count().await,
            failed_authentications: self.audit_logger.get_failed_auth_count().await,
            authorization_denials: self.audit_logger.get_authorization_denial_count().await,
        }
    }
}

/// Client information for requests
#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub ip: Option<std::net::IpAddr>,
    pub user_agent: Option<String>,
}

/// Security metrics for monitoring
#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityMetrics {
    pub active_sessions: u64,
    pub rate_limit_violations: u64,
    pub failed_authentications: u64,
    pub authorization_denials: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[test]
    async fn test_security_manager_creation() {
        let config = config::SecurityConfig::default();
        let manager = SecurityManager::new(config);
        assert!(manager.is_ok());
    }

    #[test]
    async fn test_authentication_flow() {
        let config = config::SecurityConfig::default();
        let manager = SecurityManager::new(config).unwrap();
        
        // Generate a token
        let token = manager.generate_token("test_user", rbac::Role::Developer).await.unwrap();
        
        // Authenticate with the token
        let client_info = ClientInfo {
            ip: Some("127.0.0.1".parse().unwrap()),
            user_agent: Some("test-client".to_string()),
        };
        
        let context = manager.authenticate(&token, client_info).await.unwrap();
        assert_eq!(context.user_id, "test_user");
        assert_eq!(context.role, rbac::Role::Developer);
    }
}