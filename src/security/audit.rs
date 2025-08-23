/*
 * Bevy Debugger MCP Server - Security Audit Logging
 * Copyright (C) 2025 ladvien
 */

use crate::error::{Error, Result};
use crate::security::SecurityContext;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Configuration for audit logging
#[derive(Debug, Clone)]
pub struct AuditConfig {
    pub enable_file_logging: bool,
    pub log_file_path: String,
    pub max_memory_entries: usize,
    pub log_successful_operations: bool,
    pub log_failed_operations: bool,
    pub log_authentication_events: bool,
    pub log_authorization_events: bool,
    pub log_rate_limit_events: bool,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enable_file_logging: true,
            log_file_path: "bevy_debugger_audit.log".to_string(),
            max_memory_entries: 10000,
            log_successful_operations: true,
            log_failed_operations: true,
            log_authentication_events: true,
            log_authorization_events: true,
            log_rate_limit_events: true,
        }
    }
}

/// Types of audit events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    Authentication,
    Authorization,
    Operation,
    RateLimit,
    Security,
    System,
}

/// Audit event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub event_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub event_type: AuditEventType,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub client_ip: Option<std::net::IpAddr>,
    pub user_agent: Option<String>,
    pub operation: String,
    pub resource: Option<String>,
    pub success: bool,
    pub details: serde_json::Value,
    pub security_context: Option<SecurityContextSummary>,
}

/// Simplified security context for audit logs (no sensitive data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityContextSummary {
    pub user_id: String,
    pub role: String,
    pub session_id: String,
    pub authenticated_at: chrono::DateTime<chrono::Utc>,
}

impl From<&SecurityContext> for SecurityContextSummary {
    fn from(context: &SecurityContext) -> Self {
        Self {
            user_id: context.user_id.clone(),
            role: format!("{:?}", context.role),
            session_id: context.session_id.clone(),
            authenticated_at: context.authenticated_at,
        }
    }
}

/// Audit logger service
#[derive(Clone)]
pub struct AuditLogger {
    config: AuditConfig,
    memory_log: Arc<RwLock<VecDeque<AuditEvent>>>,
    failed_auth_count: Arc<RwLock<u64>>,
    authorization_denial_count: Arc<RwLock<u64>>,
}

impl AuditLogger {
    /// Create a new audit logger
    pub fn new(config: AuditConfig) -> Result<Self> {
        info!("Initializing audit logger with config: {:?}", config);
        
        Ok(Self {
            config,
            memory_log: Arc::new(RwLock::new(VecDeque::new())),
            failed_auth_count: Arc::new(RwLock::new(0)),
            authorization_denial_count: Arc::new(RwLock::new(0)),
        })
    }
    
    /// Log an authentication event
    pub async fn log_authentication(&self, context: &SecurityContext) -> Result<()> {
        if !self.config.log_authentication_events {
            return Ok(());
        }
        
        let event = AuditEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            event_type: AuditEventType::Authentication,
            user_id: Some(context.user_id.clone()),
            session_id: Some(context.session_id.clone()),
            client_ip: context.client_ip,
            user_agent: context.user_agent.clone(),
            operation: "authenticate".to_string(),
            resource: None,
            success: true,
            details: serde_json::json!({
                "role": format!("{:?}", context.role),
                "permissions_count": context.permissions.len()
            }),
            security_context: Some(context.into()),
        };
        
        self.write_event(event).await
    }
    
    /// Log a failed authentication attempt
    pub async fn log_failed_authentication(&self, user_id: Option<String>, reason: &str, client_info: Option<(&std::net::IpAddr, &str)>) -> Result<()> {
        if !self.config.log_authentication_events {
            return Ok(());
        }
        
        {
            let mut count = self.failed_auth_count.write().await;
            *count += 1;
        }
        
        let (client_ip, user_agent) = if let Some((ip, ua)) = client_info {
            (Some(*ip), Some(ua.to_string()))
        } else {
            (None, None)
        };
        
        let event = AuditEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            event_type: AuditEventType::Authentication,
            user_id,
            session_id: None,
            client_ip,
            user_agent,
            operation: "authenticate".to_string(),
            resource: None,
            success: false,
            details: serde_json::json!({
                "failure_reason": reason
            }),
            security_context: None,
        };
        
        self.write_event(event).await
    }
    
    /// Log an authorization denial
    pub async fn log_authorization_denied(&self, context: &SecurityContext, operation: &str, resource: &str) -> Result<()> {
        if !self.config.log_authorization_events {
            return Ok(());
        }
        
        {
            let mut count = self.authorization_denial_count.write().await;
            *count += 1;
        }
        
        let event = AuditEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            event_type: AuditEventType::Authorization,
            user_id: Some(context.user_id.clone()),
            session_id: Some(context.session_id.clone()),
            client_ip: context.client_ip,
            user_agent: context.user_agent.clone(),
            operation: operation.to_string(),
            resource: Some(resource.to_string()),
            success: false,
            details: serde_json::json!({
                "role": format!("{:?}", context.role),
                "attempted_operation": operation,
                "attempted_resource": resource
            }),
            security_context: Some(context.into()),
        };
        
        self.write_event(event).await
    }
    
    /// Log a successful operation
    pub async fn log_operation(&self, context: &SecurityContext, operation: &str, resource: &str) -> Result<()> {
        if !self.config.log_successful_operations {
            return Ok(());
        }
        
        let event = AuditEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            event_type: AuditEventType::Operation,
            user_id: Some(context.user_id.clone()),
            session_id: Some(context.session_id.clone()),
            client_ip: context.client_ip,
            user_agent: context.user_agent.clone(),
            operation: operation.to_string(),
            resource: Some(resource.to_string()),
            success: true,
            details: serde_json::json!({
                "role": format!("{:?}", context.role)
            }),
            security_context: Some(context.into()),
        };
        
        self.write_event(event).await
    }
    
    /// Log a rate limit violation
    pub async fn log_rate_limit_exceeded(&self, context: &SecurityContext, operation: &str) -> Result<()> {
        if !self.config.log_rate_limit_events {
            return Ok(());
        }
        
        let event = AuditEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            event_type: AuditEventType::RateLimit,
            user_id: Some(context.user_id.clone()),
            session_id: Some(context.session_id.clone()),
            client_ip: context.client_ip,
            user_agent: context.user_agent.clone(),
            operation: operation.to_string(),
            resource: None,
            success: false,
            details: serde_json::json!({
                "violation_type": "rate_limit_exceeded"
            }),
            security_context: Some(context.into()),
        };
        
        self.write_event(event).await
    }
    
    /// Log a security event
    pub async fn log_security_event(&self, event_type: &str, details: serde_json::Value) -> Result<()> {
        let event = AuditEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            event_type: AuditEventType::Security,
            user_id: None,
            session_id: None,
            client_ip: None,
            user_agent: None,
            operation: event_type.to_string(),
            resource: None,
            success: true,
            details,
            security_context: None,
        };
        
        self.write_event(event).await
    }
    
    /// Get count of failed authentications
    pub async fn get_failed_auth_count(&self) -> u64 {
        *self.failed_auth_count.read().await
    }
    
    /// Get count of authorization denials
    pub async fn get_authorization_denial_count(&self) -> u64 {
        *self.authorization_denial_count.read().await
    }
    
    /// Get recent audit events (for admin access)
    pub async fn get_recent_events(&self, limit: usize) -> Vec<AuditEvent> {
        let memory_log = self.memory_log.read().await;
        memory_log
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }
    
    /// Search audit events by criteria
    pub async fn search_events(&self, criteria: AuditSearchCriteria) -> Vec<AuditEvent> {
        let memory_log = self.memory_log.read().await;
        
        memory_log
            .iter()
            .filter(|event| {
                // Filter by user
                if let Some(ref user_id) = criteria.user_id {
                    if event.user_id.as_ref() != Some(user_id) {
                        return false;
                    }
                }
                
                // Filter by event type
                if let Some(ref event_type) = criteria.event_type {
                    if std::mem::discriminant(&event.event_type) != std::mem::discriminant(event_type) {
                        return false;
                    }
                }
                
                // Filter by time range
                if let Some(start_time) = criteria.start_time {
                    if event.timestamp < start_time {
                        return false;
                    }
                }
                
                if let Some(end_time) = criteria.end_time {
                    if event.timestamp > end_time {
                        return false;
                    }
                }
                
                // Filter by success
                if let Some(success) = criteria.success {
                    if event.success != success {
                        return false;
                    }
                }
                
                true
            })
            .cloned()
            .collect()
    }
    
    /// Write an event to storage
    async fn write_event(&self, event: AuditEvent) -> Result<()> {
        // Write to memory log
        {
            let mut memory_log = self.memory_log.write().await;
            memory_log.push_back(event.clone());
            
            // Limit memory log size
            while memory_log.len() > self.config.max_memory_entries {
                memory_log.pop_front();
            }
        }
        
        // Write to file if enabled
        if self.config.enable_file_logging {
            self.write_to_file(&event).await?;
        }
        
        Ok(())
    }
    
    /// Write event to file
    async fn write_to_file(&self, event: &AuditEvent) -> Result<()> {
        use tokio::fs::OpenOptions;
        use tokio::io::AsyncWriteExt;
        
        let log_entry = serde_json::to_string(event)
            .map_err(|e| Error::SecurityError(format!("Failed to serialize audit event: {}", e)))?;
        
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.log_file_path)
            .await
            .map_err(|e| Error::SecurityError(format!("Failed to open audit log file: {}", e)))?;
        
        file.write_all(format!("{}\n", log_entry).as_bytes())
            .await
            .map_err(|e| Error::SecurityError(format!("Failed to write to audit log: {}", e)))?;
        
        file.flush()
            .await
            .map_err(|e| Error::SecurityError(format!("Failed to flush audit log: {}", e)))?;
        
        Ok(())
    }
}

/// Criteria for searching audit events
#[derive(Debug, Clone)]
pub struct AuditSearchCriteria {
    pub user_id: Option<String>,
    pub event_type: Option<AuditEventType>,
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub success: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::{SecurityContext, rbac::Role};
    
    fn create_test_context() -> SecurityContext {
        SecurityContext {
            user_id: "test_user".to_string(),
            role: Role::Developer,
            session_id: "test_session".to_string(),
            authenticated_at: chrono::Utc::now(),
            permissions: vec![],
            client_ip: Some("127.0.0.1".parse().unwrap()),
            user_agent: Some("test-agent".to_string()),
        }
    }

    #[tokio::test]
    async fn test_audit_logging() {
        let config = AuditConfig {
            enable_file_logging: false, // Disable file logging for tests
            ..Default::default()
        };
        
        let logger = AuditLogger::new(config).unwrap();
        let context = create_test_context();
        
        // Test authentication logging
        logger.log_authentication(&context).await.unwrap();
        
        // Test operation logging
        logger.log_operation(&context, "observe", "entities").await.unwrap();
        
        // Test authorization denial logging
        logger.log_authorization_denied(&context, "admin_operation", "system").await.unwrap();
        
        // Check that events were logged
        let events = logger.get_recent_events(10).await;
        assert_eq!(events.len(), 3);
        
        // Check counters
        assert_eq!(logger.get_authorization_denial_count().await, 1);
    }
    
    #[tokio::test]
    async fn test_event_search() {
        let config = AuditConfig {
            enable_file_logging: false,
            ..Default::default()
        };
        
        let logger = AuditLogger::new(config).unwrap();
        let context = create_test_context();
        
        // Log some events
        logger.log_authentication(&context).await.unwrap();
        logger.log_operation(&context, "observe", "entities").await.unwrap();
        
        // Search for authentication events
        let criteria = AuditSearchCriteria {
            user_id: Some("test_user".to_string()),
            event_type: Some(AuditEventType::Authentication),
            start_time: None,
            end_time: None,
            success: Some(true),
        };
        
        let results = logger.search_events(criteria).await;
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].event_type, AuditEventType::Authentication));
    }
}