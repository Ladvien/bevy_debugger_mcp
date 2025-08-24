/*
 * Bevy Debugger MCP Server - Production Security Configuration
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

use std::env;
use ring::rand::{SystemRandom, SecureRandom};
use base64::Engine as _;
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Production-ready security configuration with environment variable support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionSecurityConfig {
    /// JWT signing secret - MUST be set via environment variable in production
    pub jwt_secret: String,
    /// JWT token expiry time in hours
    pub jwt_expiry_hours: u64,
    /// Rate limiting: requests per minute per IP
    pub rate_limit_per_ip: u32,
    /// Rate limiting: requests per minute per user
    pub rate_limit_per_user: u32,
    /// Rate limiting: burst capacity
    pub rate_limit_burst: u32,
    /// Minimum password length
    pub password_min_length: usize,
    /// Require password complexity (uppercase, lowercase, numbers, symbols)
    pub password_require_complexity: bool,
    /// Password must not be in common password list
    pub password_blacklist_check: bool,
    /// Session timeout in hours
    pub session_timeout_hours: u64,
    /// Maximum failed login attempts before lockout
    pub max_failed_logins: u32,
    /// Account lockout duration in minutes
    pub lockout_duration_minutes: u64,
    /// Audit log retention in days
    pub audit_log_retention_days: u64,
    /// Enable persistent audit logging
    pub audit_log_persistence: bool,
    /// Force password change on first login
    pub force_initial_password_change: bool,
    /// Enable account lockout recovery
    pub enable_lockout_recovery: bool,
    /// Production mode (stricter security)
    pub production_mode: bool,
}

impl ProductionSecurityConfig {
    /// Create a production-ready security configuration
    pub fn new() -> Result<Self> {
        let production_mode = env::var("BEVY_MCP_ENV")
            .unwrap_or_else(|_| "development".to_string())
            .to_lowercase() == "production";

        let jwt_secret = if production_mode {
            // In production, JWT secret MUST be provided via environment variable
            env::var("BEVY_MCP_JWT_SECRET")
                .map_err(|_| Error::SecurityError(
                    "BEVY_MCP_JWT_SECRET environment variable is required in production mode".to_string()
                ))?
        } else {
            // In development, generate a secure random secret
            Self::generate_secure_jwt_secret()?
        };

        // Validate JWT secret strength
        if jwt_secret.len() < 32 {
            return Err(Error::SecurityError(
                "JWT secret must be at least 32 characters long".to_string()
            ));
        }

        let config = Self {
            jwt_secret,
            jwt_expiry_hours: env::var("BEVY_MCP_JWT_EXPIRY_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(if production_mode { 4 } else { 24 }),
            
            rate_limit_per_ip: env::var("BEVY_MCP_RATE_LIMIT_PER_IP")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            
            rate_limit_per_user: env::var("BEVY_MCP_RATE_LIMIT_PER_USER")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
            
            rate_limit_burst: env::var("BEVY_MCP_RATE_LIMIT_BURST")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            
            password_min_length: env::var("BEVY_MCP_PASSWORD_MIN_LENGTH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(if production_mode { 12 } else { 8 }),
            
            password_require_complexity: env::var("BEVY_MCP_PASSWORD_COMPLEXITY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(production_mode),
            
            password_blacklist_check: env::var("BEVY_MCP_PASSWORD_BLACKLIST")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(production_mode),
            
            session_timeout_hours: env::var("BEVY_MCP_SESSION_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(if production_mode { 4 } else { 8 }),
            
            max_failed_logins: env::var("BEVY_MCP_MAX_FAILED_LOGINS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            
            lockout_duration_minutes: env::var("BEVY_MCP_LOCKOUT_DURATION")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            
            audit_log_retention_days: env::var("BEVY_MCP_AUDIT_RETENTION")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(90),
            
            audit_log_persistence: env::var("BEVY_MCP_AUDIT_PERSISTENCE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(production_mode),
            
            force_initial_password_change: env::var("BEVY_MCP_FORCE_PASSWORD_CHANGE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(production_mode),
            
            enable_lockout_recovery: env::var("BEVY_MCP_LOCKOUT_RECOVERY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            
            production_mode,
        };

        // Log configuration warnings
        if production_mode {
            info!("Security configuration loaded in PRODUCTION mode");
            if config.jwt_expiry_hours > 8 {
                warn!("JWT expiry time is longer than recommended (>8 hours)");
            }
            if config.password_min_length < 12 {
                warn!("Password minimum length is less than recommended (<12 chars)");
            }
        } else {
            info!("Security configuration loaded in DEVELOPMENT mode");
            warn!("Development mode uses relaxed security settings - DO NOT use in production");
        }

        Ok(config)
    }

    /// Generate a cryptographically secure JWT secret
    fn generate_secure_jwt_secret() -> Result<String> {
        let rng = SystemRandom::new();
        let mut secret_bytes = vec![0u8; 64]; // 512-bit secret
        
        rng.fill(&mut secret_bytes)
            .map_err(|_| Error::SecurityError("Failed to generate secure JWT secret".to_string()))?;
        
        let secret = base64::engine::general_purpose::STANDARD_NO_PAD
            .encode(&secret_bytes);
        
        info!("Generated secure JWT secret for development mode");
        Ok(secret)
    }

    /// Validate password against security policy
    pub fn validate_password(&self, password: &str) -> Result<()> {
        if password.len() < self.password_min_length {
            return Err(Error::SecurityError(format!(
                "Password must be at least {} characters long",
                self.password_min_length
            )));
        }

        if self.password_require_complexity {
            let has_uppercase = password.chars().any(|c| c.is_uppercase());
            let has_lowercase = password.chars().any(|c| c.is_lowercase());
            let has_number = password.chars().any(|c| c.is_ascii_digit());
            let has_symbol = password.chars().any(|c| !c.is_alphanumeric());

            if !(has_uppercase && has_lowercase && has_number && has_symbol) {
                return Err(Error::SecurityError(
                    "Password must contain uppercase, lowercase, number, and symbol characters".to_string()
                ));
            }
        }

        if self.password_blacklist_check {
            if Self::is_common_password(password) {
                return Err(Error::SecurityError(
                    "Password is too common, please choose a more secure password".to_string()
                ));
            }
        }

        Ok(())
    }

    /// Check if password is in common password list
    fn is_common_password(password: &str) -> bool {
        // Simple check against most common passwords
        const COMMON_PASSWORDS: &[&str] = &[
            "password", "123456", "123456789", "12345678", "12345",
            "1234567890", "qwerty", "abc123", "password123", "admin",
            "admin123", "root", "user", "guest", "test", "demo",
            "welcome", "login", "passw0rd", "p@ssword", "p@ssw0rd"
        ];

        let password_lower = password.to_lowercase();
        COMMON_PASSWORDS.iter().any(|&common| password_lower == common)
    }

    /// Generate a secure random password for initial setup
    pub fn generate_initial_password() -> Result<String> {
        const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*";
        const PASSWORD_LENGTH: usize = 16;

        let rng = SystemRandom::new();
        let mut password = Vec::with_capacity(PASSWORD_LENGTH);

        for _ in 0..PASSWORD_LENGTH {
            let mut byte = [0u8; 1];
            rng.fill(&mut byte)
                .map_err(|_| Error::SecurityError("Failed to generate random password".to_string()))?;
            
            let idx = (byte[0] as usize) % CHARS.len();
            password.push(CHARS[idx]);
        }

        String::from_utf8(password)
            .map_err(|_| Error::SecurityError("Failed to create password string".to_string()))
    }

    /// Print security configuration summary (without secrets)
    pub fn print_security_summary(&self) {
        info!("=== Security Configuration Summary ===");
        info!("Production Mode: {}", self.production_mode);
        info!("JWT Expiry: {} hours", self.jwt_expiry_hours);
        info!("Rate Limit (IP): {} req/min", self.rate_limit_per_ip);
        info!("Rate Limit (User): {} req/min", self.rate_limit_per_user);
        info!("Password Min Length: {} chars", self.password_min_length);
        info!("Password Complexity: {}", self.password_require_complexity);
        info!("Session Timeout: {} hours", self.session_timeout_hours);
        info!("Max Failed Logins: {}", self.max_failed_logins);
        info!("Audit Persistence: {}", self.audit_log_persistence);
        info!("Force Password Change: {}", self.force_initial_password_change);
        info!("=====================================");
    }

    /// Validate environment variables for production deployment
    pub fn validate_production_environment() -> Result<()> {
        let required_env_vars = [
            "BEVY_MCP_JWT_SECRET",
        ];

        let mut missing_vars = Vec::new();

        for var in &required_env_vars {
            if env::var(var).is_err() {
                missing_vars.push(*var);
            }
        }

        if !missing_vars.is_empty() {
            return Err(Error::SecurityError(format!(
                "Missing required environment variables for production: {}",
                missing_vars.join(", ")
            )));
        }

        // Validate JWT secret strength
        let jwt_secret = env::var("BEVY_MCP_JWT_SECRET").unwrap();
        if jwt_secret.len() < 32 {
            return Err(Error::SecurityError(
                "BEVY_MCP_JWT_SECRET must be at least 32 characters long".to_string()
            ));
        }

        info!("Production environment validation passed");
        Ok(())
    }
}

impl Default for ProductionSecurityConfig {
    fn default() -> Self {
        Self::new().expect("Failed to create default security configuration")
    }
}

/// Environment variable documentation for deployment
pub const ENVIRONMENT_VARIABLES_HELP: &str = r#"
=== Bevy Debugger MCP Security Environment Variables ===

REQUIRED FOR PRODUCTION:
  BEVY_MCP_ENV=production              # Enable production security mode
  BEVY_MCP_JWT_SECRET=<secret>         # JWT signing secret (min 32 chars)

OPTIONAL CONFIGURATION:
  BEVY_MCP_JWT_EXPIRY_HOURS=4          # JWT token expiry (default: 4 in prod, 24 in dev)
  BEVY_MCP_RATE_LIMIT_PER_IP=60        # Rate limit per IP (default: 60 req/min)
  BEVY_MCP_RATE_LIMIT_PER_USER=100     # Rate limit per user (default: 100 req/min)
  BEVY_MCP_RATE_LIMIT_BURST=10         # Rate limit burst capacity (default: 10)
  BEVY_MCP_PASSWORD_MIN_LENGTH=12      # Minimum password length (default: 12 in prod, 8 in dev)
  BEVY_MCP_PASSWORD_COMPLEXITY=true    # Require password complexity (default: true in prod)
  BEVY_MCP_PASSWORD_BLACKLIST=true     # Check against common passwords (default: true in prod)
  BEVY_MCP_SESSION_TIMEOUT=4           # Session timeout in hours (default: 4 in prod, 8 in dev)
  BEVY_MCP_MAX_FAILED_LOGINS=5         # Max failed logins before lockout (default: 5)
  BEVY_MCP_LOCKOUT_DURATION=30         # Lockout duration in minutes (default: 30)
  BEVY_MCP_AUDIT_RETENTION=90          # Audit log retention in days (default: 90)
  BEVY_MCP_AUDIT_PERSISTENCE=true      # Enable persistent audit logging (default: true in prod)
  BEVY_MCP_FORCE_PASSWORD_CHANGE=true  # Force initial password change (default: true in prod)
  BEVY_MCP_LOCKOUT_RECOVERY=true       # Enable lockout recovery (default: true)

EXAMPLE PRODUCTION CONFIGURATION:
  export BEVY_MCP_ENV=production
  export BEVY_MCP_JWT_SECRET="$(openssl rand -base64 64)"
  export BEVY_MCP_JWT_EXPIRY_HOURS=4
  export BEVY_MCP_PASSWORD_MIN_LENGTH=12
  export BEVY_MCP_SESSION_TIMEOUT=4

SECURITY NOTES:
  - JWT_SECRET should be a cryptographically random string
  - Use different JWT secrets for different environments
  - Rotate JWT secrets periodically
  - Monitor audit logs for security events
"#;