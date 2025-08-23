/*
 * Bevy Debugger MCP Server - BRP Command Validation
 * Copyright (C) 2025 ladvien
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

use crate::brp_messages::{BrpRequest, BrpErrorCode, ComponentTypeId, EntityId};
use crate::error::{Error, ErrorContext, ErrorSeverity, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Maximum request size in bytes (1MB default)
pub const DEFAULT_MAX_REQUEST_SIZE: usize = 1024 * 1024;

/// Maximum entities per query (1000 default)
pub const DEFAULT_MAX_ENTITIES_PER_QUERY: usize = 1000;

/// Default rate limit (operations per second)
pub const DEFAULT_RATE_LIMIT: u32 = 100;

/// Rate limiting window duration
pub const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(1);

/// BRP command validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    /// Maximum request size in bytes
    pub max_request_size: usize,
    
    /// Maximum entities per query
    pub max_entities_per_query: usize,
    
    /// Rate limit (operations per second)
    pub rate_limit: u32,
    
    /// Whether to enforce entity existence checks
    pub enforce_entity_existence: bool,
    
    /// Whether to enforce component type registry checks
    pub enforce_component_registry: bool,
    
    /// Whether to enforce permission checks
    pub enforce_permissions: bool,
    
    /// Additional size limits
    pub limits: ValidationLimits,
}

/// Additional validation limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationLimits {
    /// Maximum component name length
    pub max_component_name_length: usize,
    
    /// Maximum component value size in bytes
    pub max_component_value_size: usize,
    
    /// Maximum query filter complexity
    pub max_filter_complexity: usize,
    
    /// Maximum batch operation size
    pub max_batch_size: usize,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            max_request_size: DEFAULT_MAX_REQUEST_SIZE,
            max_entities_per_query: DEFAULT_MAX_ENTITIES_PER_QUERY,
            rate_limit: DEFAULT_RATE_LIMIT,
            enforce_entity_existence: true,
            enforce_component_registry: true,
            enforce_permissions: true,
            limits: ValidationLimits::default(),
        }
    }
}

impl Default for ValidationLimits {
    fn default() -> Self {
        Self {
            max_component_name_length: 128,
            max_component_value_size: 65536, // 64KB
            max_filter_complexity: 50,
            max_batch_size: 100,
        }
    }
}

/// Permission levels for BRP operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PermissionLevel {
    /// Read-only access (Query, Get, ListEntities, ListComponents)
    Read = 1,
    
    /// Write access (Set, Spawn, Destroy)
    Write = 2,
    
    /// Administrative access (Debug commands, system operations)
    Admin = 3,
}

/// User session for permission tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    /// Unique session ID
    pub session_id: String,
    
    /// User permission level
    pub permission_level: PermissionLevel,
    
    /// Operations performed counter
    pub operations_count: u32,
    
    /// Last operation timestamp
    pub last_operation: Instant,
    
    /// Rate limiting state
    pub rate_state: RateLimitState,
}

/// Rate limiting state tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitState {
    /// Operations in current window
    pub operations_in_window: u32,
    
    /// Window start time
    #[serde(skip)]
    pub window_start: Instant,
}

impl Default for RateLimitState {
    fn default() -> Self {
        Self {
            operations_in_window: 0,
            window_start: Instant::now(),
        }
    }
}

/// Component type registry for validation
#[derive(Debug, Clone)]
pub struct ComponentRegistry {
    /// Registered component types
    registered_types: HashSet<ComponentTypeId>,
    
    /// Type metadata (size, schema, etc.)
    type_metadata: HashMap<ComponentTypeId, ComponentTypeMetadata>,
}

/// Metadata for component types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentTypeMetadata {
    /// Component size in bytes
    pub size_bytes: usize,
    
    /// Whether component is required for certain operations
    pub is_required: bool,
    
    /// JSON schema for validation
    pub schema: Option<serde_json::Value>,
    
    /// Whether component can be modified
    pub is_mutable: bool,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            registered_types: HashSet::new(),
            type_metadata: HashMap::new(),
        };
        
        // Register common Bevy component types
        registry.register_common_types();
        registry
    }
    
    /// Register common Bevy component types
    fn register_common_types(&mut self) {
        let common_types = vec![
            ("Transform", ComponentTypeMetadata {
                size_bytes: 48, // 3x Vec3 + Quat
                is_required: false,
                schema: None,
                is_mutable: true,
            }),
            ("GlobalTransform", ComponentTypeMetadata {
                size_bytes: 48,
                is_required: false,
                schema: None,
                is_mutable: false, // Usually computed
            }),
            ("Visibility", ComponentTypeMetadata {
                size_bytes: 1,
                is_required: false,
                schema: None,
                is_mutable: true,
            }),
            ("Name", ComponentTypeMetadata {
                size_bytes: 24, // String with capacity
                is_required: false,
                schema: None,
                is_mutable: true,
            }),
        ];
        
        for (type_name, metadata) in common_types {
            self.registered_types.insert(type_name.to_string());
            self.type_metadata.insert(type_name.to_string(), metadata);
        }
    }
    
    /// Check if component type is registered
    pub fn is_registered(&self, type_id: &ComponentTypeId) -> bool {
        self.registered_types.contains(type_id)
    }
    
    /// Register a new component type
    pub fn register_type(&mut self, type_id: ComponentTypeId, metadata: ComponentTypeMetadata) {
        self.registered_types.insert(type_id.clone());
        self.type_metadata.insert(type_id, metadata);
    }
    
    /// Get component metadata
    pub fn get_metadata(&self, type_id: &ComponentTypeId) -> Option<&ComponentTypeMetadata> {
        self.type_metadata.get(type_id)
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Entity existence tracker for validation
#[derive(Debug, Clone)]
pub struct EntityTracker {
    /// Set of known existing entities
    existing_entities: HashSet<EntityId>,
    
    /// Last update timestamp
    last_update: Instant,
    
    /// Cache validity duration
    cache_ttl: Duration,
}

impl EntityTracker {
    pub fn new() -> Self {
        Self {
            existing_entities: HashSet::new(),
            last_update: Instant::now(),
            cache_ttl: Duration::from_secs(30), // 30 second cache
        }
    }
    
    /// Check if entity exists
    pub fn entity_exists(&self, entity_id: EntityId) -> bool {
        self.existing_entities.contains(&entity_id)
    }
    
    /// Update entity existence (would be called from BRP client)
    pub fn update_entities(&mut self, entities: Vec<EntityId>) {
        self.existing_entities.clear();
        self.existing_entities.extend(entities);
        self.last_update = Instant::now();
    }
    
    /// Check if cache is stale
    pub fn is_cache_stale(&self) -> bool {
        self.last_update.elapsed() > self.cache_ttl
    }
}

impl Default for EntityTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Comprehensive BRP command validator
#[derive(Debug)]
pub struct BrpValidator {
    /// Validation configuration
    config: ValidationConfig,
    
    /// Component type registry
    component_registry: Arc<RwLock<ComponentRegistry>>,
    
    /// Entity existence tracker
    entity_tracker: Arc<RwLock<EntityTracker>>,
    
    /// User sessions for permission and rate limiting
    user_sessions: Arc<RwLock<HashMap<String, UserSession>>>,
}

impl BrpValidator {
    /// Create a new BRP validator with default configuration
    pub fn new() -> Self {
        Self::with_config(ValidationConfig::default())
    }
    
    /// Create a new BRP validator with custom configuration
    pub fn with_config(config: ValidationConfig) -> Self {
        Self {
            config,
            component_registry: Arc::new(RwLock::new(ComponentRegistry::new())),
            entity_tracker: Arc::new(RwLock::new(EntityTracker::new())),
            user_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Validate a BRP request comprehensively
    pub async fn validate_request(
        &self,
        request: &BrpRequest,
        session_id: &str,
        request_size_bytes: usize,
    ) -> Result<()> {
        let context = ErrorContext::new("validate_request", "BrpValidator");
        
        // 1. Basic request size validation
        self.validate_request_size(request_size_bytes, &context).await?;
        
        // 2. Rate limiting validation
        self.validate_rate_limit(session_id, &context).await?;
        
        // 3. Permission validation
        self.validate_permissions(request, session_id, &context).await?;
        
        // 4. Request-specific validation
        self.validate_request_specifics(request, &context).await?;
        
        // 5. Entity existence validation (if enabled)
        if self.config.enforce_entity_existence {
            self.validate_entity_existence(request, &context).await?;
        }
        
        // 6. Component registry validation (if enabled)
        if self.config.enforce_component_registry {
            self.validate_component_types(request, &context).await?;
        }
        
        Ok(())
    }
    
    /// Validate request size limits
    async fn validate_request_size(
        &self,
        request_size: usize,
        context: &ErrorContext,
    ) -> Result<()> {
        if request_size > self.config.max_request_size {
            return Err(Error::Validation(format!(
                "Request size {} exceeds maximum allowed size {}. Consider splitting large requests into smaller batches.",
                request_size, self.config.max_request_size
            )));
        }
        Ok(())
    }
    
    /// Validate rate limiting
    async fn validate_rate_limit(&self, session_id: &str, context: &ErrorContext) -> Result<()> {
        let mut sessions = self.user_sessions.write().await;
        let session = sessions.entry(session_id.to_string()).or_insert_with(|| {
            UserSession {
                session_id: session_id.to_string(),
                permission_level: PermissionLevel::Read, // Default to read-only
                operations_count: 0,
                last_operation: Instant::now(),
                rate_state: RateLimitState::default(),
            }
        });
        
        let now = Instant::now();
        
        // Reset window if needed
        if now.duration_since(session.rate_state.window_start) >= RATE_LIMIT_WINDOW {
            session.rate_state.operations_in_window = 0;
            session.rate_state.window_start = now;
        }
        
        // Check rate limit
        if session.rate_state.operations_in_window >= self.config.rate_limit {
            let reset_in = RATE_LIMIT_WINDOW
                .checked_sub(now.duration_since(session.rate_state.window_start))
                .unwrap_or(Duration::ZERO);
            
            return Err(Error::Validation(format!(
                "Rate limit exceeded: {} operations per second. Try again in {:?}. Consider reducing request frequency or using batch operations.",
                self.config.rate_limit, reset_in
            )));
        }
        
        // Update counters
        session.rate_state.operations_in_window += 1;
        session.operations_count += 1;
        session.last_operation = now;
        
        Ok(())
    }
    
    /// Validate permissions for the request
    async fn validate_permissions(
        &self,
        request: &BrpRequest,
        session_id: &str,
        context: &ErrorContext,
    ) -> Result<()> {
        if !self.config.enforce_permissions {
            return Ok(());
        }
        
        let required_permission = self.get_required_permission(request);
        
        let sessions = self.user_sessions.read().await;
        let session = sessions.get(session_id).ok_or_else(|| {
            Error::Validation("Session not found. Please authenticate first.".to_string())
        })?;
        
        if session.permission_level < required_permission {
            return Err(Error::Validation(format!(
                "Insufficient permissions: operation requires {:?} level, but session has {:?} level. Contact administrator for access upgrade.",
                required_permission, session.permission_level
            )));
        }
        
        Ok(())
    }
    
    /// Get required permission level for a request
    fn get_required_permission(&self, request: &BrpRequest) -> PermissionLevel {
        match request {
            BrpRequest::Query { .. }
            | BrpRequest::Get { .. }
            | BrpRequest::ListEntities { .. }
            | BrpRequest::ListComponents => PermissionLevel::Read,
            
            BrpRequest::Set { .. }
            | BrpRequest::Spawn { .. }
            | BrpRequest::Destroy { .. }
            | BrpRequest::SpawnEntity { .. }
            | BrpRequest::ModifyEntity { .. }
            | BrpRequest::DeleteEntity { .. }
            | BrpRequest::QueryEntity { .. }
            | BrpRequest::Insert { .. }
            | BrpRequest::Remove { .. }
            | BrpRequest::Reparent { .. } => PermissionLevel::Write,
            
            BrpRequest::Screenshot { .. }
            | BrpRequest::Debug { .. } => PermissionLevel::Admin,
        }
    }
    
    /// Validate request-specific constraints
    async fn validate_request_specifics(
        &self,
        request: &BrpRequest,
        context: &ErrorContext,
    ) -> Result<()> {
        match request {
            BrpRequest::Query { limit, .. } => {
                if let Some(limit) = limit {
                    if *limit > self.config.max_entities_per_query {
                        return Err(Error::Validation(format!(
                            "Query limit {} exceeds maximum {}. Use pagination with smaller limits.",
                            limit, self.config.max_entities_per_query
                        )));
                    }
                }
            },
            
            BrpRequest::Set { components, .. } => {
                // Validate component values size
                for (type_id, value) in components {
                    let value_size = serde_json::to_vec(value)
                        .map_err(|e| Error::Validation(format!("Invalid component value: {}", e)))?
                        .len();
                    
                    if value_size > self.config.limits.max_component_value_size {
                        return Err(Error::Validation(format!(
                            "Component '{}' value size {} exceeds maximum {}",
                            type_id, value_size, self.config.limits.max_component_value_size
                        )));
                    }
                }
            },
            
            BrpRequest::Spawn { components } => {
                // Similar validation for spawn
                for (type_id, value) in components {
                    let value_size = serde_json::to_vec(value)
                        .map_err(|e| Error::Validation(format!("Invalid component value: {}", e)))?
                        .len();
                    
                    if value_size > self.config.limits.max_component_value_size {
                        return Err(Error::Validation(format!(
                            "Component '{}' value size {} exceeds maximum {}",
                            type_id, value_size, self.config.limits.max_component_value_size
                        )));
                    }
                }
            },
            
            _ => {}, // Other requests don't need specific validation
        }
        
        Ok(())
    }
    
    /// Validate entity existence
    async fn validate_entity_existence(
        &self,
        request: &BrpRequest,
        context: &ErrorContext,
    ) -> Result<()> {
        let entity_tracker = self.entity_tracker.read().await;
        
        let entities_to_check: Vec<EntityId> = match request {
            BrpRequest::Get { entity, .. }
            | BrpRequest::Set { entity, .. }
            | BrpRequest::Destroy { entity } => vec![*entity],
            
            BrpRequest::ModifyEntity { entity_id, .. }
            | BrpRequest::DeleteEntity { entity_id }
            | BrpRequest::QueryEntity { entity_id } => vec![*entity_id],
            
            BrpRequest::Insert { entity, .. }
            | BrpRequest::Remove { entity, .. }
            | BrpRequest::Reparent { entity, .. } => vec![*entity],
            
            _ => Vec::new(), // No specific entities to validate
        };
        
        for entity_id in entities_to_check {
            if !entity_tracker.entity_exists(entity_id) {
                return Err(Error::Validation(format!(
                    "Entity {} does not exist or has been despawned. Refresh entity list or check entity ID.",
                    entity_id
                )));
            }
        }
        
        Ok(())
    }
    
    /// Validate component types against registry
    async fn validate_component_types(
        &self,
        request: &BrpRequest,
        context: &ErrorContext,
    ) -> Result<()> {
        let component_registry = self.component_registry.read().await;
        
        let component_types: Vec<&ComponentTypeId> = match request {
            BrpRequest::Get { components: Some(components), .. } => {
                components.iter().collect()
            },
            
            BrpRequest::Set { components, .. } => {
                components.keys().collect()
            },
            
            BrpRequest::Spawn { components } => {
                components.keys().collect()
            },
            
            BrpRequest::SpawnEntity { components } => {
                components.iter().map(|(type_id, _)| type_id).collect()
            },
            
            BrpRequest::ModifyEntity { components, .. } => {
                components.iter().map(|(type_id, _)| type_id).collect()
            },
            
            BrpRequest::Insert { components, .. } => {
                components.keys().collect()
            },
            
            BrpRequest::Remove { components, .. } => {
                components.iter().collect()
            },
            
            _ => Vec::new(), // No component types to validate
        };
        
        for type_id in component_types {
            if !component_registry.is_registered(type_id) {
                return Err(Error::Validation(format!(
                    "Component type '{}' is not registered. Available types can be retrieved using ListComponents.",
                    type_id
                )));
            }
            
            // Check component name length
            if type_id.len() > self.config.limits.max_component_name_length {
                return Err(Error::Validation(format!(
                    "Component type name '{}' exceeds maximum length {}",
                    type_id, self.config.limits.max_component_name_length
                )));
            }
        }
        
        Ok(())
    }
    
    /// Update user session permissions
    pub async fn update_session_permissions(
        &self,
        session_id: &str,
        permission_level: PermissionLevel,
    ) -> Result<()> {
        let mut sessions = self.user_sessions.write().await;
        let session = sessions.entry(session_id.to_string()).or_insert_with(|| {
            UserSession {
                session_id: session_id.to_string(),
                permission_level: PermissionLevel::Read,
                operations_count: 0,
                last_operation: Instant::now(),
                rate_state: RateLimitState::default(),
            }
        });
        
        session.permission_level = permission_level;
        Ok(())
    }
    
    /// Get component registry for external updates
    pub fn get_component_registry(&self) -> Arc<RwLock<ComponentRegistry>> {
        self.component_registry.clone()
    }
    
    /// Get entity tracker for external updates
    pub fn get_entity_tracker(&self) -> Arc<RwLock<EntityTracker>> {
        self.entity_tracker.clone()
    }
}

impl Default for BrpValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brp_messages::{BrpRequest, QueryFilter};
    use std::collections::HashMap;
    
    #[tokio::test]
    async fn test_basic_validation() {
        let validator = BrpValidator::new();
        let request = BrpRequest::ListComponents;
        
        // Should pass basic validation
        let result = validator.validate_request(&request, "test_session", 100).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_request_size_limit() {
        let validator = BrpValidator::new();
        let request = BrpRequest::ListComponents;
        
        // Should fail with oversized request
        let result = validator.validate_request(&request, "test_session", usize::MAX).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds maximum allowed size"));
    }
    
    #[tokio::test]
    async fn test_rate_limiting() {
        let mut config = ValidationConfig::default();
        config.rate_limit = 2; // Very low limit for testing
        let validator = BrpValidator::with_config(config);
        let request = BrpRequest::ListComponents;
        
        // First two requests should pass
        assert!(validator.validate_request(&request, "test_session", 100).await.is_ok());
        assert!(validator.validate_request(&request, "test_session", 100).await.is_ok());
        
        // Third request should fail
        let result = validator.validate_request(&request, "test_session", 100).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Rate limit exceeded"));
    }
    
    #[tokio::test]
    async fn test_component_registry() {
        let validator = BrpValidator::new();
        let registry = validator.get_component_registry();
        let registry = registry.read().await;
        
        // Common types should be registered
        assert!(registry.is_registered("Transform"));
        assert!(registry.is_registered("Name"));
        assert!(!registry.is_registered("NonexistentComponent"));
    }
    
    #[tokio::test]
    async fn test_permission_validation() {
        let validator = BrpValidator::new();
        
        // Set up a read-only session
        validator.update_session_permissions("readonly_session", PermissionLevel::Read).await.unwrap();
        
        // Read operation should pass
        let read_request = BrpRequest::ListComponents;
        assert!(validator.validate_request(&read_request, "readonly_session", 100).await.is_ok());
        
        // Write operation should fail
        let write_request = BrpRequest::Destroy { entity: 123 };
        let result = validator.validate_request(&write_request, "readonly_session", 100).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Insufficient permissions"));
    }
}