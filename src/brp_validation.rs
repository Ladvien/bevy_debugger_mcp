/*
 * Bevy Debugger MCP Server - BRP Command Validation
 * Copyright (C) 2025 ladvien
 *
 * Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
 *
 * Relicensed from GPL-3.0 when this crate was adopted into Ladvien/foundation_vs_slop: a GPL
 * crate in the Bevy ecosystem cannot be adopted, and being adoptable is why it is published.
 */

use crate::brp::{builtin_methods, BrpRequest, ComponentTypeId, EntityId};
use crate::error::{Error, ErrorContext, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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

/// All real Bevy 0.19 BRP method names that this validator recognizes.
const KNOWN_METHODS: &[&str] = &[
    builtin_methods::BRP_QUERY_METHOD,
    builtin_methods::BRP_GET_COMPONENTS_METHOD,
    builtin_methods::BRP_SPAWN_ENTITY_METHOD,
    builtin_methods::BRP_INSERT_COMPONENTS_METHOD,
    builtin_methods::BRP_REMOVE_COMPONENTS_METHOD,
    builtin_methods::BRP_DESPAWN_COMPONENTS_METHOD,
    builtin_methods::BRP_REPARENT_ENTITIES_METHOD,
    builtin_methods::BRP_LIST_COMPONENTS_METHOD,
    builtin_methods::BRP_MUTATE_COMPONENTS_METHOD,
    builtin_methods::BRP_GET_COMPONENTS_AND_WATCH_METHOD,
    builtin_methods::BRP_LIST_COMPONENTS_AND_WATCH_METHOD,
    builtin_methods::BRP_GET_RESOURCE_METHOD,
    builtin_methods::BRP_INSERT_RESOURCE_METHOD,
    builtin_methods::BRP_REMOVE_RESOURCE_METHOD,
    builtin_methods::BRP_MUTATE_RESOURCE_METHOD,
    builtin_methods::BRP_LIST_RESOURCES_METHOD,
    builtin_methods::BRP_TRIGGER_EVENT_METHOD,
    builtin_methods::BRP_REGISTRY_SCHEMA_METHOD,
    builtin_methods::RPC_DISCOVER_METHOD,
];

/// Validate that `method` is a real Bevy 0.19 BRP method name.
pub fn validate_method_name(method: &str) -> std::result::Result<(), String> {
    if KNOWN_METHODS.contains(&method) {
        Ok(())
    } else {
        Err(format!(
            "Unknown BRP method '{method}'. Known methods: {}",
            KNOWN_METHODS.join(", ")
        ))
    }
}

/// BRP command validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    /// Maximum request size in bytes
    pub max_request_size: usize,

    /// Maximum entities per query.
    ///
    /// **Currently enforced by nothing.** `validate_request_specifics` deliberately bounds nothing for
    /// `world.query` — the server owns pagination, being the only side that knows how many entities
    /// match. The field is kept because it is part of the serialized config shape, but setting it has
    /// no effect; do not read a limit here as a guarantee.
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
    /// Read-only access (world.query, world.get_components, world.list_components)
    Read = 1,

    /// Write access (spawn/despawn/insert/remove/mutate/reparent/trigger)
    Write = 2,

    /// Administrative access (registry.schema, rpc.discover, watches)
    Admin = 3,
}

/// User session for permission tracking
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct RateLimitState {
    /// Operations in current window
    pub operations_in_window: u32,

    /// Window start time
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

    /// Update entity existence (called with world.query results)
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

        // 0. Method must be a real Bevy 0.19 BRP method
        validate_method_name(&request.method).map_err(Error::Validation)?;

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
        _context: &ErrorContext,
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
    async fn validate_rate_limit(&self, session_id: &str, _context: &ErrorContext) -> Result<()> {
        let mut sessions = self.user_sessions.write().await;
        let session = sessions.entry(session_id.to_string()).or_insert_with(|| {
            UserSession {
                session_id: session_id.to_string(),
                permission_level: PermissionLevel::Admin, // Local debugger: full access
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
        _context: &ErrorContext,
    ) -> Result<()> {
        if !self.config.enforce_permissions {
            return Ok(());
        }

        let required_permission = required_permission_for_method(&request.method);

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

    /// Validate request-specific constraints from the JSON params
    async fn validate_request_specifics(
        &self,
        request: &BrpRequest,
        _context: &ErrorContext,
    ) -> Result<()> {
        let Some(params) = &request.params else {
            return Ok(());
        };

        match request.method.as_str() {
            builtin_methods::BRP_QUERY_METHOD => {
                // Nothing world.query-specific to bound client-side; the
                // server owns pagination.
            }
            builtin_methods::BRP_SPAWN_ENTITY_METHOD
            | builtin_methods::BRP_INSERT_COMPONENTS_METHOD => {
                if let Some(components) = params.get("components").and_then(Value::as_object) {
                    self.validate_component_value_sizes(components.iter())?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Validate the serialized size of each component value
    fn validate_component_value_sizes<'a>(
        &self,
        components: impl Iterator<Item = (&'a String, &'a Value)>,
    ) -> Result<()> {
        for (type_id, value) in components {
            let value_size = serde_json::to_vec(value)
                .map_err(|e| Error::Validation(format!("Invalid component value: {e}")))?
                .len();

            if value_size > self.config.limits.max_component_value_size {
                return Err(Error::Validation(format!(
                    "Component '{}' value size {} exceeds maximum {}",
                    type_id, value_size, self.config.limits.max_component_value_size
                )));
            }
        }
        Ok(())
    }

    /// Validate entity existence
    async fn validate_entity_existence(
        &self,
        request: &BrpRequest,
        _context: &ErrorContext,
    ) -> Result<()> {
        let entity_tracker = self.entity_tracker.read().await;

        let entities_to_check: Vec<EntityId> = match request.method.as_str() {
            builtin_methods::BRP_GET_COMPONENTS_METHOD
            | builtin_methods::BRP_DESPAWN_COMPONENTS_METHOD
            | builtin_methods::BRP_INSERT_COMPONENTS_METHOD
            | builtin_methods::BRP_REMOVE_COMPONENTS_METHOD
            | builtin_methods::BRP_MUTATE_COMPONENTS_METHOD => request
                .params
                .as_ref()
                .and_then(|p| p.get("entity"))
                .and_then(Value::as_u64)
                .into_iter()
                .collect(),
            builtin_methods::BRP_REPARENT_ENTITIES_METHOD => request
                .params
                .as_ref()
                .and_then(|p| p.get("entities"))
                .and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(Value::as_u64).collect())
                .unwrap_or_default(),
            _ => Vec::new(),
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
        _context: &ErrorContext,
    ) -> Result<()> {
        let component_registry = self.component_registry.read().await;

        let component_types: Vec<ComponentTypeId> = match request.method.as_str() {
            builtin_methods::BRP_SPAWN_ENTITY_METHOD
            | builtin_methods::BRP_INSERT_COMPONENTS_METHOD => request
                .params
                .as_ref()
                .and_then(|p| p.get("components"))
                .and_then(Value::as_object)
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default(),
            builtin_methods::BRP_GET_COMPONENTS_METHOD
            | builtin_methods::BRP_REMOVE_COMPONENTS_METHOD => request
                .params
                .as_ref()
                .and_then(|p| p.get("components"))
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        };

        for type_id in &component_types {
            if !component_registry.is_registered(type_id) {
                return Err(Error::Validation(format!(
                    "Component type '{}' is not registered. Available types can be retrieved using world.list_components or the registry.schema method.",
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

/// Map a real BRP method name to the permission it requires.
pub fn required_permission_for_method(method: &str) -> PermissionLevel {
    match method {
        builtin_methods::BRP_QUERY_METHOD
        | builtin_methods::BRP_GET_COMPONENTS_METHOD
        | builtin_methods::BRP_LIST_COMPONENTS_METHOD
        | builtin_methods::BRP_GET_RESOURCE_METHOD
        | builtin_methods::BRP_LIST_RESOURCES_METHOD => PermissionLevel::Read,

        builtin_methods::BRP_SPAWN_ENTITY_METHOD
        | builtin_methods::BRP_DESPAWN_COMPONENTS_METHOD
        | builtin_methods::BRP_INSERT_COMPONENTS_METHOD
        | builtin_methods::BRP_REMOVE_COMPONENTS_METHOD
        | builtin_methods::BRP_MUTATE_COMPONENTS_METHOD
        | builtin_methods::BRP_REPARENT_ENTITIES_METHOD
        | builtin_methods::BRP_INSERT_RESOURCE_METHOD
        | builtin_methods::BRP_REMOVE_RESOURCE_METHOD
        | builtin_methods::BRP_MUTATE_RESOURCE_METHOD
        | builtin_methods::BRP_TRIGGER_EVENT_METHOD => PermissionLevel::Write,

        _ => PermissionLevel::Admin,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(method: &str, params: Option<Value>) -> BrpRequest {
        BrpRequest {
            method: method.to_string(),
            id: None,
            params,
        }
    }

    #[tokio::test]
    async fn test_basic_validation() {
        let validator = BrpValidator::new();
        let request = request(builtin_methods::BRP_LIST_COMPONENTS_METHOD, None);

        // Should pass basic validation
        let result = validator.validate_request(&request, "test_session", 100).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_unknown_method_rejected() {
        let validator = BrpValidator::new();
        // Fictional legacy name must be rejected
        let request = request("bevy/list_components", None);

        let result = validator.validate_request(&request, "test_session", 100).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown BRP method"));
    }

    #[tokio::test]
    async fn test_request_size_limit() {
        let validator = BrpValidator::new();
        let request = request(builtin_methods::BRP_QUERY_METHOD, None);

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
        let request = request(builtin_methods::BRP_QUERY_METHOD, None);

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
        // `ComponentTypeId` is a `String` alias, so these need owned values rather than `&str`.
        assert!(registry.is_registered(&"Transform".to_string()));
        assert!(registry.is_registered(&"Name".to_string()));
        assert!(!registry.is_registered(&"NonexistentComponent".to_string()));
    }

    #[tokio::test]
    async fn test_permission_validation() {
        let validator = BrpValidator::new();

        // Set up a read-only session
        validator.update_session_permissions("readonly_session", PermissionLevel::Read).await.unwrap();

        // Read operation should pass
        let read_request = request(builtin_methods::BRP_QUERY_METHOD, None);
        assert!(validator.validate_request(&read_request, "readonly_session", 100).await.is_ok());

        // Write operation should fail
        let write_request = request(
            builtin_methods::BRP_DESPAWN_COMPONENTS_METHOD,
            Some(json!({ "entity": 123 })),
        );
        let result = validator.validate_request(&write_request, "readonly_session", 100).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Insufficient permissions"));
    }
}
