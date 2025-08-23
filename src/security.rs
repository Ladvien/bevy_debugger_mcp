/*
 * Bevy Debugger MCP Server - Security & Authentication Module
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

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{rand_core::OsRng, SaltString};
use governor::{Quota, RateLimiter, state::{direct::NotKeyed, InMemoryState}, clock::DefaultClock, middleware::NoOpMiddleware};
use chrono::{DateTime, Utc};
use dashmap::DashMap;

use crate::error::{Error, Result};

/// User roles with hierarchical permissions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    /// Can only observe and query (read-only access)
    Viewer,
    /// Can observe, query, and modify state (full debugging)
    Developer,
    /// Can do everything including user management and configuration
    Admin,
}

impl Role {
    /// Check if this role has permission for an operation
    pub fn has_permission(&self, required_role: &Role) -> bool {
        match (self, required_role) {
            (Role::Admin, _) => true,
            (Role::Developer, Role::Viewer | Role::Developer) => true,
            (Role::Viewer, Role::Viewer) => true,
            _ => false,
        }
    }

    /// Get the minimum role level as a number for comparisons
    pub fn level(&self) -> u8 {
        match self {
            Role::Viewer => 1,
            Role::Developer => 2,
            Role::Admin => 3,
        }
    }
}

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,        // Subject (user ID)
    pub role: Role,         // User role
    pub exp: u64,          // Expiration time
    pub iat: u64,          // Issued at
    pub jti: String,       // JWT ID for revocation
    pub session_id: String, // Session tracking
}

/// User information for authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub active: bool,
}

/// Audit log entry for security tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub action: String,
    pub resource: Option<String>,
    pub success: bool,
    pub error_message: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub session_id: Option<String>,
}

/// Security configuration
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub jwt_secret: String,
    pub jwt_expiry_hours: u64,
    pub rate_limit_requests_per_minute: u32,
    pub rate_limit_burst: u32,
    pub password_min_length: usize,
    pub session_timeout_hours: u64,
    pub max_failed_logins: u32,
    pub lockout_duration_minutes: u64,
    pub audit_log_retention_days: u64,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "bevy_debugger_mcp_secret_change_in_production".to_string(),
            jwt_expiry_hours: 24,
            rate_limit_requests_per_minute: 60,
            rate_limit_burst: 10,
            password_min_length: 8,
            session_timeout_hours: 8,
            max_failed_logins: 5,
            lockout_duration_minutes: 30,
            audit_log_retention_days: 90,
        }
    }
}

/// Active session tracking
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Failed login attempt tracking
#[derive(Debug, Clone)]
pub struct FailedLogin {
    pub count: u32,
    pub first_attempt: DateTime<Utc>,
    pub last_attempt: DateTime<Utc>,
    pub locked_until: Option<DateTime<Utc>>,
}

/// Main security manager
pub struct SecurityManager {
    config: SecurityConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    users: Arc<RwLock<HashMap<String, User>>>,
    revoked_tokens: Arc<DashMap<String, DateTime<Utc>>>,
    active_sessions: Arc<DashMap<String, Session>>,
    failed_logins: Arc<DashMap<String, FailedLogin>>,
    audit_log: Arc<RwLock<Vec<AuditEntry>>>,
    rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>>,
}

impl SecurityManager {
    /// Create a new security manager with configuration
    pub fn new(config: SecurityConfig) -> Result<Self> {
        let encoding_key = EncodingKey::from_secret(config.jwt_secret.as_ref());
        let decoding_key = DecodingKey::from_secret(config.jwt_secret.as_ref());

        // Setup rate limiter
        let quota = Quota::per_minute(config.rate_limit_requests_per_minute)
            .allow_burst(config.rate_limit_burst.try_into().unwrap_or(10));
        let rate_limiter = Arc::new(RateLimiter::direct(quota));

        let manager = Self {
            config,
            encoding_key,
            decoding_key,
            users: Arc::new(RwLock::new(HashMap::new())),
            revoked_tokens: Arc::new(DashMap::new()),
            active_sessions: Arc::new(DashMap::new()),
            failed_logins: Arc::new(DashMap::new()),
            audit_log: Arc::new(RwLock::new(Vec::new())),
            rate_limiter,
        };

        // Create default admin user if none exists
        tokio::spawn({
            let manager = manager.clone();
            async move {
                if let Err(e) = manager.initialize_default_users().await {
                    error!("Failed to initialize default users: {}", e);
                }
            }
        });

        Ok(manager)
    }

    /// Initialize default users for first-time setup
    async fn initialize_default_users(&self) -> Result<()> {
        let mut users = self.users.write().await;
        
        if users.is_empty() {
            info!("No users found, creating default admin user");
            
            let password = "admin123"; // Should be changed immediately
            let password_hash = self.hash_password(password)?;
            
            let admin_user = User {
                id: "admin".to_string(),
                username: "admin".to_string(),
                password_hash,
                role: Role::Admin,
                created_at: Utc::now(),
                last_login: None,
                active: true,
            };
            
            users.insert("admin".to_string(), admin_user);
            warn!("Default admin user created with password 'admin123' - CHANGE IMMEDIATELY!");
            
            // Create a developer user for testing
            let dev_password_hash = self.hash_password("dev123")?;
            let dev_user = User {
                id: "developer".to_string(),
                username: "developer".to_string(),
                password_hash: dev_password_hash,
                role: Role::Developer,
                created_at: Utc::now(),
                last_login: None,
                active: true,
            };
            
            users.insert("developer".to_string(), dev_user);
            
            // Create a viewer user for testing
            let viewer_password_hash = self.hash_password("viewer123")?;
            let viewer_user = User {
                id: "viewer".to_string(),
                username: "viewer".to_string(),
                password_hash: viewer_password_hash,
                role: Role::Viewer,
                created_at: Utc::now(),
                last_login: None,
                active: true,
            };
            
            users.insert("viewer".to_string(), viewer_user);
            info!("Default users created: admin, developer, viewer");
        }
        
        Ok(())
    }

    /// Hash a password using Argon2
    pub fn hash_password(&self, password: &str) -> Result<String> {
        if password.len() < self.config.password_min_length {
            return Err(Error::SecurityError(format!(
                "Password must be at least {} characters long",
                self.config.password_min_length
            )));
        }

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| Error::SecurityError(format!("Password hashing failed: {}", e)))?;
            
        Ok(password_hash.to_string())
    }

    /// Verify a password against its hash
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| Error::SecurityError(format!("Invalid password hash: {}", e)))?;
            
        let argon2 = Argon2::default();
        Ok(argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok())
    }

    /// Authenticate user and return JWT token
    pub async fn authenticate(&self, username: &str, password: &str, ip_address: Option<String>, user_agent: Option<String>) -> Result<String> {
        // Check rate limiting first
        if self.rate_limiter.check().is_err() {
            self.log_audit("authentication", username, None, false, Some("Rate limit exceeded"), ip_address.as_deref(), user_agent.as_deref(), None).await;
            return Err(Error::SecurityError("Rate limit exceeded".to_string()));
        }

        // Check for account lockout
        if let Some(failed) = self.failed_logins.get(username) {
            if let Some(locked_until) = failed.locked_until {
                if Utc::now() < locked_until {
                    self.log_audit("authentication", username, None, false, Some("Account locked"), ip_address.as_deref(), user_agent.as_deref(), None).await;
                    return Err(Error::SecurityError("Account is temporarily locked".to_string()));
                }
            }
        }

        let users = self.users.read().await;
        let user = users.get(username).ok_or_else(|| {
            tokio::spawn({
                let security = self.clone();
                let username = username.to_string();
                let ip = ip_address.clone();
                let ua = user_agent.clone();
                async move {
                    security.record_failed_login(&username).await;
                    security.log_audit("authentication", &username, None, false, Some("User not found"), ip.as_deref(), ua.as_deref(), None).await;
                }
            });
            Error::SecurityError("Invalid credentials".to_string())
        })?;

        if !user.active {
            self.log_audit("authentication", username, None, false, Some("User account disabled"), ip_address.as_deref(), user_agent.as_deref(), None).await;
            return Err(Error::SecurityError("Account is disabled".to_string()));
        }

        // Verify password
        if !self.verify_password(password, &user.password_hash)? {
            tokio::spawn({
                let security = self.clone();
                let username = username.to_string();
                let ip = ip_address.clone();
                let ua = user_agent.clone();
                async move {
                    security.record_failed_login(&username).await;
                    security.log_audit("authentication", &username, None, false, Some("Invalid password"), ip.as_deref(), ua.as_deref(), None).await;
                }
            });
            return Err(Error::SecurityError("Invalid credentials".to_string()));
        }

        // Clear failed login attempts on successful login
        self.failed_logins.remove(username);

        // Create session
        let session_id = Uuid::new_v4().to_string();
        let session = Session {
            id: session_id.clone(),
            user_id: user.id.clone(),
            created_at: Utc::now(),
            last_activity: Utc::now(),
            ip_address: ip_address.clone(),
            user_agent: user_agent.clone(),
        };
        self.active_sessions.insert(session_id.clone(), session);

        // Generate JWT token
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let exp = now + (self.config.jwt_expiry_hours * 3600);
        
        let claims = Claims {
            sub: user.id.clone(),
            role: user.role.clone(),
            exp,
            iat: now,
            jti: Uuid::new_v4().to_string(),
            session_id: session_id.clone(),
        };

        let token = encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| Error::SecurityError(format!("Token generation failed: {}", e)))?;

        // Update user's last login
        drop(users);
        let mut users = self.users.write().await;
        if let Some(user) = users.get_mut(username) {
            user.last_login = Some(Utc::now());
        }

        self.log_audit("authentication", username, None, true, None, ip_address.as_deref(), user_agent.as_deref(), Some(&session_id)).await;
        info!("User {} authenticated successfully", username);

        Ok(token)
    }

    /// Validate JWT token and return claims
    pub async fn validate_token(&self, token: &str) -> Result<Claims> {
        // Check if token is revoked
        let mut validation = Validation::new(Algorithm::HS256);
        validation.leeway = 30; // Allow 30 seconds leeway for clock skew
        
        let token_data = decode::<Claims>(token, &self.decoding_key, &validation)
            .map_err(|e| Error::SecurityError(format!("Invalid token: {}", e)))?;

        let claims = token_data.claims;

        // Check if token is revoked
        if self.revoked_tokens.contains_key(&claims.jti) {
            return Err(Error::SecurityError("Token has been revoked".to_string()));
        }

        // Check if session is still active
        if let Some(session) = self.active_sessions.get(&claims.session_id) {
            let mut session = session.clone();
            session.last_activity = Utc::now();
            self.active_sessions.insert(claims.session_id.clone(), session);
        } else {
            return Err(Error::SecurityError("Session not found or expired".to_string()));
        }

        // Verify user still exists and is active
        let users = self.users.read().await;
        let user = users.get(&claims.sub).ok_or_else(|| 
            Error::SecurityError("User no longer exists".to_string())
        )?;

        if !user.active {
            return Err(Error::SecurityError("User account is disabled".to_string()));
        }

        Ok(claims)
    }

    /// Check if user has permission for a specific operation
    pub async fn check_permission(&self, token: &str, required_role: &Role, operation: &str) -> Result<Claims> {
        let claims = self.validate_token(token).await?;
        
        if !claims.role.has_permission(required_role) {
            self.log_audit("authorization", &claims.sub, Some(operation), false, Some("Insufficient permissions"), None, None, Some(&claims.session_id)).await;
            return Err(Error::SecurityError(format!(
                "Insufficient permissions: {} role required, user has {} role",
                serde_json::to_string(required_role).unwrap_or_default(),
                serde_json::to_string(&claims.role).unwrap_or_default()
            )));
        }

        self.log_audit("authorization", &claims.sub, Some(operation), true, None, None, None, Some(&claims.session_id)).await;
        Ok(claims)
    }

    /// Revoke a JWT token
    pub async fn revoke_token(&self, token: &str) -> Result<()> {
        let claims = self.validate_token(token).await?;
        
        // Add to revoked tokens
        self.revoked_tokens.insert(claims.jti.clone(), Utc::now());
        
        // Remove active session
        self.active_sessions.remove(&claims.session_id);
        
        self.log_audit("token_revocation", &claims.sub, None, true, None, None, None, Some(&claims.session_id)).await;
        info!("Token revoked for user {}", claims.sub);
        
        Ok(())
    }

    /// Record a failed login attempt
    async fn record_failed_login(&self, username: &str) {
        let now = Utc::now();
        
        match self.failed_logins.get_mut(username) {
            Some(mut entry) => {
                entry.count += 1;
                entry.last_attempt = now;
                
                if entry.count >= self.config.max_failed_logins {
                    entry.locked_until = Some(now + chrono::Duration::minutes(self.config.lockout_duration_minutes as i64));
                    warn!("Account {} locked due to {} failed login attempts", username, entry.count);
                }
            }
            None => {
                let failed = FailedLogin {
                    count: 1,
                    first_attempt: now,
                    last_attempt: now,
                    locked_until: None,
                };
                self.failed_logins.insert(username.to_string(), failed);
            }
        }
    }

    /// Log an audit entry
    async fn log_audit(&self, action: &str, user_id: &str, resource: Option<&str>, success: bool, error_message: Option<&str>, ip_address: Option<&str>, user_agent: Option<&str>, session_id: Option<&str>) {
        let entry = AuditEntry {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            username: user_id.to_string(), // For simplicity, using user_id as username
            action: action.to_string(),
            resource: resource.map(|s| s.to_string()),
            success,
            error_message: error_message.map(|s| s.to_string()),
            timestamp: Utc::now(),
            ip_address: ip_address.map(|s| s.to_string()),
            user_agent: user_agent.map(|s| s.to_string()),
            session_id: session_id.map(|s| s.to_string()),
        };

        let mut audit_log = self.audit_log.write().await;
        audit_log.push(entry);

        // Cleanup old entries
        let retention_cutoff = Utc::now() - chrono::Duration::days(self.config.audit_log_retention_days as i64);
        audit_log.retain(|entry| entry.timestamp > retention_cutoff);
    }

    /// Get audit log entries (admin only)
    pub async fn get_audit_log(&self, token: &str, limit: Option<usize>, offset: Option<usize>) -> Result<Vec<AuditEntry>> {
        self.check_permission(token, &Role::Admin, "audit_log_access").await?;
        
        let audit_log = self.audit_log.read().await;
        let start = offset.unwrap_or(0);
        let end = if let Some(limit) = limit {
            std::cmp::min(start + limit, audit_log.len())
        } else {
            audit_log.len()
        };
        
        Ok(audit_log[start..end].to_vec())
    }

    /// Create a new user (admin only)
    pub async fn create_user(&self, token: &str, username: &str, password: &str, role: Role) -> Result<()> {
        self.check_permission(token, &Role::Admin, "user_management").await?;
        
        let password_hash = self.hash_password(password)?;
        let user = User {
            id: username.to_string(),
            username: username.to_string(),
            password_hash,
            role,
            created_at: Utc::now(),
            last_login: None,
            active: true,
        };

        let mut users = self.users.write().await;
        if users.contains_key(username) {
            return Err(Error::SecurityError("User already exists".to_string()));
        }
        
        users.insert(username.to_string(), user);
        info!("User {} created", username);
        
        Ok(())
    }

    /// Delete a user (admin only)
    pub async fn delete_user(&self, token: &str, username: &str) -> Result<()> {
        let claims = self.check_permission(token, &Role::Admin, "user_management").await?;
        
        // Prevent self-deletion
        if claims.sub == username {
            return Err(Error::SecurityError("Cannot delete your own account".to_string()));
        }
        
        let mut users = self.users.write().await;
        if users.remove(username).is_none() {
            return Err(Error::SecurityError("User not found".to_string()));
        }
        
        // Revoke all sessions for this user
        let user_sessions: Vec<_> = self.active_sessions
            .iter()
            .filter(|entry| entry.user_id == username)
            .map(|entry| entry.key().clone())
            .collect();
            
        for session_id in user_sessions {
            self.active_sessions.remove(&session_id);
        }
        
        info!("User {} deleted", username);
        Ok(())
    }

    /// List all users (admin only)
    pub async fn list_users(&self, token: &str) -> Result<Vec<User>> {
        self.check_permission(token, &Role::Admin, "user_management").await?;
        
        let users = self.users.read().await;
        let mut user_list: Vec<User> = users.values().cloned().collect();
        user_list.sort_by(|a, b| a.username.cmp(&b.username));
        
        Ok(user_list)
    }

    /// Get active sessions (admin only)
    pub async fn get_active_sessions(&self, token: &str) -> Result<Vec<Session>> {
        self.check_permission(token, &Role::Admin, "session_management").await?;
        
        let sessions: Vec<Session> = self.active_sessions
            .iter()
            .map(|entry| entry.value().clone())
            .collect();
            
        Ok(sessions)
    }

    /// Cleanup expired sessions and revoked tokens
    pub async fn cleanup(&self) {
        let now = Utc::now();
        let session_timeout = chrono::Duration::hours(self.config.session_timeout_hours as i64);
        
        // Remove expired sessions
        let expired_sessions: Vec<_> = self.active_sessions
            .iter()
            .filter(|entry| now.signed_duration_since(entry.last_activity) > session_timeout)
            .map(|entry| entry.key().clone())
            .collect();
            
        for session_id in expired_sessions {
            self.active_sessions.remove(&session_id);
        }
        
        // Remove old revoked tokens (keep for JWT expiry time)
        let token_retention = chrono::Duration::hours(self.config.jwt_expiry_hours as i64 * 2);
        let revoked_cutoff = now - token_retention;
        
        self.revoked_tokens.retain(|_, &mut revoked_at| revoked_at > revoked_cutoff);
        
        debug!("Security cleanup completed");
    }
}

impl Clone for SecurityManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            encoding_key: self.encoding_key.clone(),
            decoding_key: self.decoding_key.clone(),
            users: self.users.clone(),
            revoked_tokens: self.revoked_tokens.clone(),
            active_sessions: self.active_sessions.clone(),
            failed_logins: self.failed_logins.clone(),
            audit_log: self.audit_log.clone(),
            rate_limiter: self.rate_limiter.clone(),
        }
    }
}

/// Security middleware for tool access control
pub struct SecurityMiddleware {
    security_manager: Arc<SecurityManager>,
}

impl SecurityMiddleware {
    pub fn new(security_manager: Arc<SecurityManager>) -> Self {
        Self { security_manager }
    }

    /// Check if a tool operation is allowed for the given role
    pub fn check_tool_permission(operation: &str, role: &Role) -> bool {
        match operation {
            // Viewer permissions (read-only operations)
            "observe" | "hypothesis" | "detect_anomaly" => role.level() >= 1,
            
            // Developer permissions (can modify state)
            "experiment" | "stress_test" | "time_travel_replay" => role.level() >= 2,
            
            // Admin permissions (system management)
            "user_management" | "audit_log_access" | "session_management" => role.level() >= 3,
            
            // Default to requiring developer role
            _ => role.level() >= 2,
        }
    }

    /// Validate token and check permissions for a tool operation
    pub async fn authorize_tool_call(&self, token: Option<&str>, operation: &str) -> Result<Claims> {
        let token = token.ok_or_else(|| 
            Error::SecurityError("Authentication token required".to_string())
        )?;

        // Validate token
        let claims = self.security_manager.validate_token(token).await?;

        // Check tool-specific permissions
        if !Self::check_tool_permission(operation, &claims.role) {
            return Err(Error::SecurityError(format!(
                "Insufficient permissions for operation: {}",
                operation
            )));
        }

        Ok(claims)
    }
}

/// Security audit utilities
pub struct SecurityAudit {
    security_manager: Arc<SecurityManager>,
}

impl SecurityAudit {
    pub fn new(security_manager: Arc<SecurityManager>) -> Self {
        Self { security_manager }
    }

    /// Run security vulnerability scan
    pub async fn run_security_scan(&self, token: &str) -> Result<SecurityScanReport> {
        self.security_manager.check_permission(token, &Role::Admin, "security_scan").await?;
        
        let mut report = SecurityScanReport {
            scan_time: Utc::now(),
            vulnerabilities: Vec::new(),
            recommendations: Vec::new(),
        };

        // Check for default passwords
        let users = self.security_manager.users.read().await;
        for user in users.values() {
            if user.username == "admin" && self.security_manager.verify_password("admin123", &user.password_hash).unwrap_or(false) {
                report.vulnerabilities.push("Default admin password detected".to_string());
                report.recommendations.push("Change the default admin password immediately".to_string());
            }
        }

        // Check for weak JWT secret
        if self.security_manager.config.jwt_secret.contains("change_in_production") {
            report.vulnerabilities.push("Default JWT secret detected".to_string());
            report.recommendations.push("Configure a strong, random JWT secret".to_string());
        }

        // Check password policy
        if self.security_manager.config.password_min_length < 12 {
            report.vulnerabilities.push("Weak password policy".to_string());
            report.recommendations.push("Increase minimum password length to 12+ characters".to_string());
        }

        info!("Security scan completed, found {} vulnerabilities", report.vulnerabilities.len());
        Ok(report)
    }
}

/// Security scan report
#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityScanReport {
    pub scan_time: DateTime<Utc>,
    pub vulnerabilities: Vec<String>,
    pub recommendations: Vec<String>,
}