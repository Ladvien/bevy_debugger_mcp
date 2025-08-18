/// BRP (Bevy Remote Protocol) message types and serialization
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Unique identifier for an entity in the Bevy ECS world
pub type EntityId = u64;

/// Unique identifier for a component type
pub type ComponentTypeId = String;

/// Raw JSON value for flexible component data
pub type ComponentValue = serde_json::Value;

/// BRP request message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
#[non_exhaustive]
pub enum BrpRequest {
    /// Query entities matching certain criteria
    #[serde(rename = "bevy/query")]
    Query {
        /// Optional filter to limit results
        filter: Option<QueryFilter>,
        /// Maximum number of results to return
        limit: Option<usize>,
    },

    /// Get specific entity's components
    #[serde(rename = "bevy/get")]
    Get {
        /// Entity ID to retrieve
        entity: EntityId,
        /// Optional list of component types to include
        components: Option<Vec<ComponentTypeId>>,
    },

    /// Set component values on an entity
    #[serde(rename = "bevy/set")]
    Set {
        /// Target entity ID
        entity: EntityId,
        /// Component type and value pairs to set
        components: HashMap<ComponentTypeId, ComponentValue>,
    },

    /// Spawn a new entity with components
    #[serde(rename = "bevy/spawn")]
    Spawn {
        /// Initial components for the new entity
        components: HashMap<ComponentTypeId, ComponentValue>,
    },

    /// Destroy an entity
    #[serde(rename = "bevy/destroy")]
    Destroy {
        /// Entity ID to destroy
        entity: EntityId,
    },

    /// List all available component types
    #[serde(rename = "bevy/list_components")]
    ListComponents,

    /// List all entities (optionally filtered)
    #[serde(rename = "bevy/list_entities")]
    ListEntities {
        /// Optional filter criteria
        filter: Option<QueryFilter>,
    },

    /// Take a screenshot of the primary window
    #[serde(rename = "bevy_debugger/screenshot")]
    Screenshot {
        /// Path where to save the screenshot (optional)
        path: Option<String>,
        /// Time in milliseconds to wait before capture (game warmup)
        warmup_duration: Option<u64>,
        /// Additional delay in milliseconds before capture
        capture_delay: Option<u64>,
        /// Whether to wait for at least one frame to render
        wait_for_render: Option<bool>,
        /// Optional description for logging/debugging
        description: Option<String>,
    },

    /// Spawn a new entity (for experiment system)
    SpawnEntity {
        components: Vec<(ComponentTypeId, ComponentValue)>,
    },

    /// Modify an existing entity (for experiment system)
    ModifyEntity {
        entity_id: EntityId,
        components: Vec<(ComponentTypeId, ComponentValue)>,
    },

    /// Delete an entity (for experiment system)
    DeleteEntity { entity_id: EntityId },

    /// Query a specific entity (for experiment system)
    QueryEntity { entity_id: EntityId },
}

/// Query filter for selecting entities
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct QueryFilter {
    /// Entities must have all of these components
    pub with: Option<Vec<ComponentTypeId>>,
    /// Entities must not have any of these components
    pub without: Option<Vec<ComponentTypeId>>,
    /// Component value filters
    pub where_clause: Option<Vec<ComponentFilter>>,
}

/// Filter for component values
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ComponentFilter {
    /// Component type to filter on
    pub component: ComponentTypeId,
    /// Field path within the component (e.g., "position.x")
    pub field: Option<String>,
    /// Filter operation
    pub op: FilterOp,
    /// Value to compare against
    pub value: ComponentValue,
}

/// Filter operations for component values
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FilterOp {
    #[serde(rename = "eq")]
    Equal,
    #[serde(rename = "ne")]
    NotEqual,
    #[serde(rename = "gt")]
    GreaterThan,
    #[serde(rename = "gte")]
    GreaterThanOrEqual,
    #[serde(rename = "lt")]
    LessThan,
    #[serde(rename = "lte")]
    LessThanOrEqual,
    #[serde(rename = "contains")]
    Contains,
    #[serde(rename = "regex")]
    Regex,
}

/// BRP response message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BrpResponse {
    /// Successful response
    Success(BrpResult),
    /// Error response
    Error(BrpError),
}

/// Successful BRP operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[non_exhaustive]
pub enum BrpResult {
    /// Entity data response
    #[serde(rename = "entities")]
    Entities(Vec<EntityData>),

    /// Single entity response
    #[serde(rename = "entity")]
    Entity(EntityData),

    /// Entity ID response (for spawn operations)
    #[serde(rename = "entity_id")]
    EntityId(EntityId),

    /// Component types list
    #[serde(rename = "component_types")]
    ComponentTypes(Vec<ComponentTypeInfo>),

    /// Simple success confirmation
    #[serde(rename = "success")]
    Success,

    /// Entity spawned successfully
    EntitySpawned(EntityId),

    /// Entity modified successfully
    EntityModified,

    /// Entity deleted successfully
    EntityDeleted,

    /// Screenshot taken successfully
    #[serde(rename = "screenshot")]
    Screenshot {
        /// Path where the screenshot was saved
        path: String,
        /// Success status
        success: bool,
    },
}

/// Entity data with components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityData {
    /// Entity identifier
    pub id: EntityId,
    /// Component data by type
    pub components: HashMap<ComponentTypeId, ComponentValue>,
}

/// Information about a component type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentTypeInfo {
    /// Component type identifier
    pub id: ComponentTypeId,
    /// Human-readable name
    pub name: String,
    /// JSON schema for the component structure
    pub schema: Option<serde_json::Value>,
}

/// BRP error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrpError {
    /// Error code
    pub code: BrpErrorCode,
    /// Human-readable error message
    pub message: String,
    /// Optional additional error details
    pub details: Option<serde_json::Value>,
}

/// BRP error codes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum BrpErrorCode {
    /// Entity not found
    #[serde(rename = "entity_not_found")]
    EntityNotFound,

    /// Component type not found
    #[serde(rename = "component_not_found")]
    ComponentNotFound,

    /// Invalid component data
    #[serde(rename = "invalid_component_data")]
    InvalidComponentData,

    /// Query syntax error
    #[serde(rename = "invalid_query")]
    InvalidQuery,

    /// Permission denied
    #[serde(rename = "permission_denied")]
    PermissionDenied,

    /// Internal server error
    #[serde(rename = "internal_error")]
    InternalError,

    /// Request timeout
    #[serde(rename = "timeout")]
    Timeout,
}

impl fmt::Display for BrpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl fmt::Display for BrpErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EntityNotFound => write!(f, "Entity not found"),
            Self::ComponentNotFound => write!(f, "Component not found"),
            Self::InvalidComponentData => write!(f, "Invalid component data"),
            Self::InvalidQuery => write!(f, "Invalid query"),
            Self::PermissionDenied => write!(f, "Permission denied"),
            Self::InternalError => write!(f, "Internal error"),
            Self::Timeout => write!(f, "Request timeout"),
        }
    }
}

/// Common Bevy component type wrappers
pub mod components {
    use serde::{Deserialize, Serialize};

    /// 3D Transform component
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Transform {
        pub translation: Vec3,
        pub rotation: Quat,
        pub scale: Vec3,
    }

    /// 3D Vector
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Vec3 {
        pub x: f32,
        pub y: f32,
        pub z: f32,
    }

    /// Quaternion rotation
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Quat {
        pub x: f32,
        pub y: f32,
        pub z: f32,
        pub w: f32,
    }

    /// Velocity component
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Velocity {
        pub linear: Vec3,
        pub angular: Vec3,
    }

    /// Name component
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Name {
        pub name: String,
    }

    /// Visibility component
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Visibility {
        pub is_visible: bool,
    }
}

/// Validation utilities for BRP messages
pub mod validation {
    use super::{BrpRequest, EntityId};

    /// Validate entity ID format
    pub fn validate_entity_id(id: EntityId) -> Result<(), String> {
        if id == 0 {
            Err("Entity ID cannot be zero".to_string())
        } else {
            Ok(())
        }
    }

    /// Validate component type ID format
    pub fn validate_component_type_id(type_id: &str) -> Result<(), String> {
        if type_id.is_empty() {
            return Err("Component type ID cannot be empty".to_string());
        }

        if !type_id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == ':')
        {
            return Err("Component type ID contains invalid characters".to_string());
        }

        Ok(())
    }

    /// Validate BRP request
    pub fn validate_request(request: &BrpRequest) -> Result<(), String> {
        match request {
            BrpRequest::Get { entity, .. } | BrpRequest::Destroy { entity } => {
                validate_entity_id(*entity)
            }
            BrpRequest::Set { entity, components } => {
                validate_entity_id(*entity)?;
                for type_id in components.keys() {
                    validate_component_type_id(type_id)?;
                }
                Ok(())
            }
            BrpRequest::Spawn { components } => {
                for type_id in components.keys() {
                    validate_component_type_id(type_id)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Conversion utilities between MCP JSON and BRP messages
pub mod conversion {
    use super::{BrpError, BrpErrorCode, BrpRequest, BrpResponse};
    use crate::error::{Error, Result};

    /// Convert MCP JSON arguments to BRP request
    pub fn mcp_to_brp_request(method: &str, args: &serde_json::Value) -> Result<BrpRequest> {
        let request_json = serde_json::json!({
            "method": method,
            "params": args
        });

        serde_json::from_value(request_json).map_err(Error::Json)
    }

    /// Convert BRP response to MCP JSON
    pub fn brp_to_mcp_response(response: &BrpResponse) -> Result<serde_json::Value> {
        serde_json::to_value(response).map_err(Error::Json)
    }

    /// Helper to create BRP error response
    #[must_use]
    pub fn create_brp_error(code: BrpErrorCode, message: String) -> BrpResponse {
        BrpResponse::Error(BrpError {
            code,
            message,
            details: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brp_request_serialization() {
        let request = BrpRequest::Query {
            filter: Some(QueryFilter {
                with: Some(vec!["Transform".to_string(), "Velocity".to_string()]),
                without: None,
                where_clause: None,
            }),
            limit: Some(10),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: BrpRequest = serde_json::from_str(&json).unwrap();

        match deserialized {
            BrpRequest::Query { filter, limit } => {
                assert_eq!(limit, Some(10));
                assert!(filter.is_some());
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_entity_validation() {
        use validation::*;

        assert!(validate_entity_id(1).is_ok());
        assert!(validate_entity_id(0).is_err());

        assert!(validate_component_type_id("Transform").is_ok());
        assert!(validate_component_type_id("core::Transform").is_ok());
        assert!(validate_component_type_id("").is_err());
        assert!(validate_component_type_id("invalid-name").is_err());
    }

    #[test]
    fn test_component_types() {
        use components::*;

        let transform = Transform {
            translation: Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            rotation: Quat {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            scale: Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
        };

        let json = serde_json::to_string(&transform).unwrap();
        let _deserialized: Transform = serde_json::from_str(&json).unwrap();
    }
}
