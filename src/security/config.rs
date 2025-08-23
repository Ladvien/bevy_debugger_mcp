/*
 * Bevy Debugger MCP Server - Security Configuration
 * Copyright (C) 2025 ladvien
 */

use super::{audit::AuditConfig, rate_limit::RateLimitConfig, rbac::RbacConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub jwt: JwtConfig,
    pub rbac: RbacConfig,
    pub rate_limit: RateLimitConfig,
    pub audit: AuditConfig,
    pub middleware: MiddlewareConfig,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            jwt: JwtConfig::default(),
            rbac: RbacConfig::default(),
            rate_limit: RateLimitConfig::default(),
            audit: AuditConfig::default(),
            middleware: MiddlewareConfig::default(),
        }
    }
}

/// JWT configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    pub secret_key: String,
    pub algorithm: String,
    pub expiration_hours: u32,
    pub audience: String,
    pub issuer: String,
    pub enable_refresh_tokens: bool,
    pub refresh_token_expiration_days: u32,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret_key: String::new(), // Will be generated if empty
            algorithm: "HS256".to_string(),
            expiration_hours: 24,
            audience: "bevy-debugger-mcp".to_string(),
            issuer: "bevy-debugger-mcp".to_string(),
            enable_refresh_tokens: true,
            refresh_token_expiration_days: 30,
        }
    }
}

/// Middleware configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddlewareConfig {
    pub enable_cors: bool,
    pub allowed_origins: Vec<String>,
    pub enable_request_logging: bool,
    pub enable_response_headers: bool,
    pub security_headers: SecurityHeaders,
    pub enable_ip_whitelist: bool,
    pub ip_whitelist: Vec<String>,
}

impl Default for MiddlewareConfig {
    fn default() -> Self {
        Self {
            enable_cors: true,
            allowed_origins: vec!["http://localhost:3000".to_string()],
            enable_request_logging: true,
            enable_response_headers: true,
            security_headers: SecurityHeaders::default(),
            enable_ip_whitelist: false,
            ip_whitelist: vec!["127.0.0.1".to_string(), "::1".to_string()],
        }
    }
}

/// Security headers configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityHeaders {
    pub x_content_type_options: String,
    pub x_frame_options: String,
    pub x_xss_protection: String,
    pub strict_transport_security: String,
    pub content_security_policy: String,
    pub referrer_policy: String,
}

impl Default for SecurityHeaders {
    fn default() -> Self {
        Self {
            x_content_type_options: "nosniff".to_string(),
            x_frame_options: "DENY".to_string(),
            x_xss_protection: "1; mode=block".to_string(),
            strict_transport_security: "max-age=31536000; includeSubDomains".to_string(),
            content_security_policy: "default-src 'self'".to_string(),
            referrer_policy: "strict-origin-when-cross-origin".to_string(),
        }
    }
}

impl SecurityConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();
        
        // JWT configuration from environment
        if let Ok(secret) = std::env::var("JWT_SECRET_KEY") {
            config.jwt.secret_key = secret;
        }
        
        if let Ok(expiration) = std::env::var("JWT_EXPIRATION_HOURS") {
            if let Ok(hours) = expiration.parse() {
                config.jwt.expiration_hours = hours;
            }
        }
        
        if let Ok(audience) = std::env::var("JWT_AUDIENCE") {
            config.jwt.audience = audience;
        }
        
        if let Ok(issuer) = std::env::var("JWT_ISSUER") {
            config.jwt.issuer = issuer;
        }
        
        // Rate limiting from environment
        if let Ok(rpm) = std::env::var("RATE_LIMIT_RPM") {
            if let Ok(requests) = rpm.parse() {
                config.rate_limit.requests_per_minute = requests;
            }
        }
        
        if let Ok(rph) = std::env::var("RATE_LIMIT_RPH") {
            if let Ok(requests) = rph.parse() {
                config.rate_limit.requests_per_hour = requests;
            }
        }
        
        // Audit configuration from environment
        if let Ok(enable_file) = std::env::var("AUDIT_ENABLE_FILE") {
            config.audit.enable_file_logging = enable_file.parse().unwrap_or(true);
        }
        
        if let Ok(log_path) = std::env::var("AUDIT_LOG_PATH") {
            config.audit.log_file_path = log_path;
        }
        
        // Middleware configuration from environment
        if let Ok(origins) = std::env::var("CORS_ALLOWED_ORIGINS") {
            config.middleware.allowed_origins = origins
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
        }
        
        if let Ok(whitelist) = std::env::var("IP_WHITELIST") {
            config.middleware.enable_ip_whitelist = true;
            config.middleware.ip_whitelist = whitelist
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
        }
        
        config
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate JWT configuration
        if self.jwt.expiration_hours == 0 {
            return Err("JWT expiration hours must be greater than 0".to_string());
        }
        
        if self.jwt.audience.is_empty() {
            return Err("JWT audience cannot be empty".to_string());
        }
        
        if self.jwt.issuer.is_empty() {
            return Err("JWT issuer cannot be empty".to_string());
        }
        
        // Validate rate limiting
        if self.rate_limit.requests_per_minute == 0 {
            return Err("Rate limit requests per minute must be greater than 0".to_string());
        }
        
        if self.rate_limit.requests_per_hour == 0 {
            return Err("Rate limit requests per hour must be greater than 0".to_string());
        }
        
        if self.rate_limit.requests_per_hour < self.rate_limit.requests_per_minute {
            return Err("Hourly rate limit must be greater than or equal to minute rate limit".to_string());
        }
        
        // Validate audit configuration
        if self.audit.max_memory_entries == 0 {
            return Err("Audit max memory entries must be greater than 0".to_string());
        }
        
        // Validate middleware configuration
        if self.middleware.enable_cors && self.middleware.allowed_origins.is_empty() {
            return Err("CORS allowed origins cannot be empty when CORS is enabled".to_string());
        }
        
        Ok(())
    }
    
    /// Get configuration summary for logging (without sensitive data)
    pub fn summary(&self) -> SecurityConfigSummary {
        SecurityConfigSummary {
            jwt_expiration_hours: self.jwt.expiration_hours,
            jwt_audience: self.jwt.audience.clone(),
            jwt_issuer: self.jwt.issuer.clone(),
            rate_limit_rpm: self.rate_limit.requests_per_minute,
            rate_limit_rph: self.rate_limit.requests_per_hour,
            rate_limit_burst: self.rate_limit.burst_size,
            audit_file_enabled: self.audit.enable_file_logging,
            audit_memory_entries: self.audit.max_memory_entries,
            cors_enabled: self.middleware.enable_cors,
            ip_whitelist_enabled: self.middleware.enable_ip_whitelist,
        }
    }
}

/// Non-sensitive configuration summary for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfigSummary {
    pub jwt_expiration_hours: u32,
    pub jwt_audience: String,
    pub jwt_issuer: String,
    pub rate_limit_rpm: u32,
    pub rate_limit_rph: u32,
    pub rate_limit_burst: u32,
    pub audit_file_enabled: bool,
    pub audit_memory_entries: usize,
    pub cors_enabled: bool,
    pub ip_whitelist_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SecurityConfig::default();
        assert!(config.validate().is_ok());
        
        assert_eq!(config.jwt.expiration_hours, 24);
        assert_eq!(config.jwt.audience, "bevy-debugger-mcp");
        assert_eq!(config.rate_limit.requests_per_minute, 60);
    }
    
    #[test]
    fn test_config_validation() {
        let mut config = SecurityConfig::default();
        
        // Valid config should pass
        assert!(config.validate().is_ok());
        
        // Invalid JWT expiration should fail
        config.jwt.expiration_hours = 0;
        assert!(config.validate().is_err());
        
        // Reset and test rate limiting
        config = SecurityConfig::default();
        config.rate_limit.requests_per_minute = 0;
        assert!(config.validate().is_err());
        
        // Test conflicting rate limits
        config = SecurityConfig::default();
        config.rate_limit.requests_per_hour = 30;
        config.rate_limit.requests_per_minute = 60;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_config_summary() {
        let config = SecurityConfig::default();
        let summary = config.summary();
        
        assert_eq!(summary.jwt_expiration_hours, 24);
        assert_eq!(summary.rate_limit_rpm, 60);
        assert_eq!(summary.cors_enabled, true);
    }
    
    #[test]
    fn test_from_env() {
        // Set some environment variables
        std::env::set_var("JWT_EXPIRATION_HOURS", "48");
        std::env::set_var("RATE_LIMIT_RPM", "120");
        std::env::set_var("AUDIT_ENABLE_FILE", "false");
        
        let config = SecurityConfig::from_env();
        
        assert_eq!(config.jwt.expiration_hours, 48);
        assert_eq!(config.rate_limit.requests_per_minute, 120);
        assert_eq!(config.audit.enable_file_logging, false);
        
        // Clean up
        std::env::remove_var("JWT_EXPIRATION_HOURS");
        std::env::remove_var("RATE_LIMIT_RPM");
        std::env::remove_var("AUDIT_ENABLE_FILE");
    }
}