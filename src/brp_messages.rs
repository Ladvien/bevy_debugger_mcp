/// BRP (Bevy Remote Protocol) message types and serialization
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Unique identifier for an entity in the Bevy ECS world
/// In Bevy 0.16, this represents both the entity index and generation
pub type EntityId = u64;

/// Extended entity representation with generation field for Bevy 0.16 compatibility
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct EntityWithGeneration {
    /// Entity index
    pub index: u32,
    /// Entity generation (for reuse detection)
    pub generation: u32,
}

impl EntityWithGeneration {
    /// Create a new entity with generation
    pub fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }
    
    /// Convert to combined u64 entity ID for backward compatibility
    pub fn to_entity_id(self) -> EntityId {
        ((self.generation as u64) << 32) | (self.index as u64)
    }
    
    /// Extract entity with generation from u64 entity ID
    pub fn from_entity_id(entity_id: EntityId) -> Self {
        Self {
            index: entity_id as u32,
            generation: (entity_id >> 32) as u32,
        }
    }
}

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
        /// Bevy 0.16: Strict mode for component validation (defaults to false)
        /// When false, missing/invalid components are skipped instead of causing errors
        strict: Option<bool>,
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

    /// Insert components into an entity (Bevy 0.16)
    #[serde(rename = "bevy/insert")]
    Insert {
        /// Target entity ID
        entity: EntityId,
        /// Component type and value pairs to insert
        components: HashMap<ComponentTypeId, ComponentValue>,
    },

    /// Remove components from an entity (Bevy 0.16)
    #[serde(rename = "bevy/remove")]
    Remove {
        /// Target entity ID
        entity: EntityId,
        /// Component types to remove
        components: Vec<ComponentTypeId>,
    },

    /// Change entity parent-child relationships (Bevy 0.16)
    #[serde(rename = "bevy/reparent")]
    Reparent {
        /// Child entity ID
        entity: EntityId,
        /// New parent entity ID (None to remove from parent)
        parent: Option<EntityId>,
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

    /// Debug command wrapper for extensible debugging operations
    #[serde(rename = "bevy_debugger/debug")]
    Debug {
        /// Debug command to execute
        command: DebugCommand,
        /// Unique correlation ID for tracking responses
        correlation_id: String,
        /// Priority level (higher values = higher priority)
        priority: Option<u8>,
    },
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

/// Debug command types for extensible debugging operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "params")]
#[non_exhaustive]
pub enum DebugCommand {
    /// Inspect entity with detailed component information
    InspectEntity {
        entity_id: EntityId,
        /// Include component metadata
        include_metadata: Option<bool>,
        /// Include parent/child relationships
        include_relationships: Option<bool>,
    },

    /// Inspect multiple entities in a single batch operation
    InspectBatch {
        entity_ids: Vec<EntityId>,
        /// Include component metadata for all entities
        include_metadata: Option<bool>,
        /// Include parent/child relationships for all entities
        include_relationships: Option<bool>,
        /// Maximum number of entities to inspect (default: 100)
        limit: Option<usize>,
    },

    /// Profile system performance
    ProfileSystem {
        /// System name to profile
        system_name: String,
        /// Duration in milliseconds
        duration_ms: Option<u64>,
        /// Include memory allocations
        track_allocations: Option<bool>,
    },

    /// Enable/disable visual debugging overlays
    SetVisualDebug {
        /// Type of overlay
        overlay_type: DebugOverlayType,
        /// Enable or disable
        enabled: bool,
        /// Optional configuration
        config: Option<serde_json::Value>,
    },

    /// Execute a validated ECS query
    ExecuteQuery {
        /// Validated query structure
        query: ValidatedQuery,
        /// Pagination offset
        offset: Option<usize>,
        /// Results limit
        limit: Option<usize>,
    },

    /// Validate a query without executing it
    ValidateQuery {
        /// Query parameters as JSON
        params: serde_json::Value,
    },

    /// Estimate the cost of executing a query
    EstimateCost {
        /// Query parameters as JSON
        params: serde_json::Value,
    },

    /// Get optimization suggestions for a query
    GetQuerySuggestions {
        /// Query parameters as JSON
        params: serde_json::Value,
    },

    /// Build and execute a query using the query builder
    BuildAndExecuteQuery {
        /// Query parameters as JSON
        params: serde_json::Value,
    },

    /// Memory profiling command
    ProfileMemory {
        /// Capture allocation backtraces
        capture_backtraces: Option<bool>,
        /// Track specific systems
        target_systems: Option<Vec<String>>,
        /// Duration in seconds for profiling session
        duration_seconds: Option<u64>,
    },

    /// Stop memory profiling session
    StopMemoryProfiling {
        /// Session ID to stop (None for default session)
        session_id: Option<String>,
    },

    /// Get current memory profile
    GetMemoryProfile,

    /// Detect memory leaks
    DetectMemoryLeaks {
        /// Target systems to check for leaks
        target_systems: Option<Vec<String>>,
    },

    /// Analyze memory usage trends
    AnalyzeMemoryTrends {
        /// Target systems to analyze
        target_systems: Option<Vec<String>>,
    },

    /// Take a memory snapshot
    TakeMemorySnapshot,

    /// Get memory profiler statistics
    GetMemoryStatistics,

    /// Session management
    SessionControl {
        /// Session operation
        operation: SessionOperation,
        /// Session ID
        session_id: Option<String>,
    },

    /// Get debug system status
    GetStatus,

    /// Get entity hierarchy information
    GetHierarchy {
        /// Optional root entity to start from
        root_entity: Option<EntityId>,
        /// Maximum depth to traverse
        max_depth: Option<usize>,
    },

    /// Get system information and metadata
    GetSystemInfo {
        /// Optional system name filter
        system_name: Option<String>,
        /// Include scheduling information
        include_scheduling: Option<bool>,
    },

    /// Start automated issue detection monitoring
    StartIssueDetection,

    /// Stop automated issue detection monitoring
    StopIssueDetection,

    /// Get detected issues/alerts
    GetDetectedIssues {
        /// Maximum number of issues to return
        limit: Option<usize>,
    },

    /// Acknowledge an issue alert
    AcknowledgeIssue {
        /// Alert ID to acknowledge
        alert_id: String,
    },

    /// Report an alert as a false positive
    ReportFalsePositive {
        /// Alert ID to mark as false positive
        alert_id: String,
    },

    /// Get issue detection statistics
    GetIssueDetectionStats,

    /// Update a detection rule configuration
    UpdateDetectionRule {
        /// Rule name to update
        name: String,
        /// Enable/disable the rule
        enabled: Option<bool>,
        /// Sensitivity level (0.0 to 1.0)
        sensitivity: Option<f32>,
    },

    /// Clear issue alert history
    ClearIssueHistory,

    /// Start performance budget monitoring
    StartBudgetMonitoring,

    /// Stop performance budget monitoring
    StopBudgetMonitoring,

    /// Set performance budget configuration
    SetPerformanceBudget {
        /// Budget configuration as JSON
        config: serde_json::Value,
    },

    /// Get current performance budget configuration
    GetPerformanceBudget,

    /// Check for current budget violations
    CheckBudgetViolations,

    /// Get budget violation history
    GetBudgetViolationHistory {
        /// Maximum number of violations to return
        limit: Option<usize>,
    },

    /// Generate compliance report for specified duration
    GenerateComplianceReport {
        /// Duration in seconds (default: 3600 = 1 hour)
        duration_seconds: Option<u64>,
    },

    /// Get budget recommendations based on historical data
    GetBudgetRecommendations,

    /// Clear budget violation history
    ClearBudgetHistory,

    /// Get budget monitoring statistics
    GetBudgetStatistics,

    /// Custom debug command for extensions
    Custom {
        /// Command name
        name: String,
        /// Command parameters
        params: serde_json::Value,
    },
}

/// Debug overlay types for visual debugging
#[derive(Debug, Clone, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DebugOverlayType {
    /// Entity highlight overlay
    EntityHighlight,
    /// Physics collider visualization
    Colliders,
    /// Physics collider visualization (alternative name for compatibility)
    ColliderVisualization,
    /// Transform gizmos
    Transforms,
    /// Transform gizmos (alternative name for compatibility)
    TransformGizmos,
    /// System execution flow
    SystemFlow,
    /// Performance metrics
    PerformanceMetrics,
    /// Debug markers
    DebugMarkers,
    /// Custom overlay
    Custom(String),
}

/// Validated query structure for safe ECS queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedQuery {
    /// Query ID for caching
    pub id: String,
    /// Validated filter
    pub filter: QueryFilter,
    /// Estimated cost
    pub estimated_cost: QueryCost,
    /// Optimization hints
    pub hints: Vec<String>,
}

/// Query cost estimation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCost {
    /// Estimated entities to scan
    pub estimated_entities: usize,
    /// Estimated time in microseconds
    pub estimated_time_us: u64,
    /// Memory usage estimate in bytes
    pub estimated_memory: usize,
}

/// Session operations for debug session management
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SessionOperation {
    /// Create new session
    Create,
    /// Resume existing session
    Resume,
    /// Checkpoint current state
    Checkpoint,
    /// Restore from checkpoint
    Restore { checkpoint_id: String },
    /// End session
    End,
}

/// Debug response types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[non_exhaustive]
pub enum DebugResponse {
    /// Entity inspection result
    EntityInspection {
        entity: EntityData,
        metadata: Option<EntityMetadata>,
        relationships: Option<EntityRelationships>,
    },

    /// Batch entity inspection result
    BatchEntityInspection {
        entities: Vec<EntityInspectionResult>,
        /// Total entities requested
        requested_count: usize,
        /// Successfully inspected entities
        found_count: usize,
        /// Entities that were not found (despawned)
        missing_entities: Vec<EntityId>,
        /// Total inspection time in microseconds
        inspection_time_us: u64,
    },

    /// System profiling result
    SystemProfile(SystemProfile),
    
    /// Profiling started response
    ProfilingStarted {
        system_name: String,
        duration_ms: Option<u64>,
    },
    
    /// Profiling history response
    ProfileHistory {
        system_name: String,
        samples: Vec<ProfileSample>,
        frame_count: usize,
    },
    
    /// Performance anomalies response
    PerformanceAnomalies {
        count: usize,
        anomalies: Vec<serde_json::Value>,
    },

    /// Visual debug status
    VisualDebugStatus {
        overlay_type: DebugOverlayType,
        enabled: bool,
        config: Option<serde_json::Value>,
    },

    /// Query execution result
    QueryResult {
        entities: Vec<EntityData>,
        total_count: usize,
        execution_time_us: u64,
        has_more: bool,
    },

    /// Memory profile result
    MemoryProfile {
        total_allocated: usize,
        allocations_per_system: HashMap<String, usize>,
        top_allocations: Vec<AllocationInfo>,
    },

    /// Session control result
    SessionStatus {
        session_id: String,
        state: SessionState,
        command_count: usize,
        checkpoints: Vec<CheckpointInfo>,
    },

    /// Debug system status
    Status {
        version: String,
        active_sessions: usize,
        command_queue_size: usize,
        performance_overhead_percent: f32,
    },

    /// Query validation result
    QueryValidation {
        valid: bool,
        query: Option<ValidatedQuery>,
        errors: Vec<String>,
        suggestions: Vec<String>,
    },

    /// Query cost estimation result
    QueryCost {
        cost: QueryCost,
        performance_budget_exceeded: bool,
        suggestions: Vec<String>,
    },

    /// Query optimization suggestions
    QuerySuggestions {
        suggestions: Vec<String>,
        query_complexity: u32,
    },

    /// Query execution result
    QueryExecution {
        success: bool,
        result: Option<Box<BrpResponse>>,
        execution_time_us: u64,
        entities_processed: Option<usize>,
    },

    /// Generic success response
    Success {
        message: String,
        data: Option<serde_json::Value>,
    },

    /// Custom debug response
    Custom(serde_json::Value),
}

/// Entity metadata for inspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMetadata {
    /// Component count
    pub component_count: usize,
    /// Total memory size in bytes
    pub memory_size: usize,
    /// Last modified timestamp
    pub last_modified: Option<u64>,
    /// Entity generation (Bevy 0.16 compatibility)
    pub generation: u32,
    /// Entity index (Bevy 0.16 compatibility)
    pub index: u32,
    /// Component type information
    pub component_types: Vec<DetailedComponentTypeInfo>,
    /// Which components have been modified
    pub modified_components: Vec<String>,
    /// Entity archetype information
    pub archetype_id: Option<u32>,
    /// Entity location in world storage
    pub location_info: Option<EntityLocationInfo>,
}

/// Detailed component type information with reflection data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedComponentTypeInfo {
    /// Component type identifier
    pub type_id: String,
    /// Human-readable type name
    pub type_name: String,
    /// Size in bytes
    pub size_bytes: usize,
    /// Whether component has reflection data
    pub is_reflected: bool,
    /// Type schema if available
    pub schema: Option<serde_json::Value>,
    /// Whether component was modified this frame
    pub is_modified: bool,
}

/// Entity location information in Bevy's storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityLocationInfo {
    /// Archetype ID
    pub archetype_id: u32,
    /// Index within archetype
    pub index: u32,
    /// Table ID
    pub table_id: Option<u32>,
    /// Row in table
    pub table_row: Option<u32>,
}

/// Entity relationships
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRelationships {
    /// Parent entity if exists
    pub parent: Option<EntityId>,
    /// Child entities
    pub children: Vec<EntityId>,
    /// Related entities (custom relationships)
    pub related: HashMap<String, Vec<EntityId>>,
}

/// System performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Total execution time in microseconds
    pub total_time_us: u64,
    /// Minimum execution time in microseconds
    pub min_time_us: u64,
    /// Maximum execution time in microseconds
    pub max_time_us: u64,
    /// Average execution time in microseconds
    pub avg_time_us: u64,
    /// Median execution time in microseconds
    pub median_time_us: u64,
    /// 95th percentile time
    pub p95_time_us: u64,
    /// 99th percentile time
    pub p99_time_us: u64,
    /// Total memory allocations
    pub total_allocations: usize,
    /// Allocation rate per invocation
    pub allocation_rate: f32,
    /// Overhead percentage
    pub overhead_percent: f32,
}

/// Profile sample point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSample {
    /// Timestamp in microseconds
    pub timestamp: u64,
    /// Execution time in microseconds
    pub duration_us: u64,
    /// Memory allocations during execution
    pub allocations: Option<usize>,
}

/// System profile data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemProfile {
    /// System name
    pub system_name: String,
    /// Performance metrics
    pub metrics: SystemMetrics,
    /// Sample timeline
    pub samples: Vec<ProfileSample>,
    /// System dependencies
    pub dependencies: Vec<String>,
}

/// Memory allocation information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationInfo {
    /// Size in bytes
    pub size: usize,
    /// Allocation site (function name)
    pub location: String,
    /// Backtrace if available
    pub backtrace: Option<Vec<String>>,
    /// Allocation count
    pub count: usize,
}

/// Debug session state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SessionState {
    /// Session is active
    Active,
    /// Session is paused
    Paused,
    /// Session is replaying commands
    Replaying,
    /// Session ended
    Ended,
}

/// Checkpoint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    /// Checkpoint ID
    pub id: String,
    /// Creation timestamp
    pub timestamp: u64,
    /// Description
    pub description: Option<String>,
    /// Size in bytes
    pub size: usize,
}

/// Individual entity inspection result for batch operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityInspectionResult {
    /// Entity ID
    pub entity_id: EntityId,
    /// Whether the entity was found
    pub found: bool,
    /// Entity data if found
    pub entity: Option<EntityData>,
    /// Entity metadata if requested and found
    pub metadata: Option<EntityMetadata>,
    /// Entity relationships if requested and found
    pub relationships: Option<EntityRelationships>,
    /// Error message if inspection failed
    pub error: Option<String>,
}

/// BRP response message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BrpResponse {
    /// Successful response
    Success(Box<BrpResult>),
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

    /// Components inserted successfully (Bevy 0.16)
    ComponentsInserted,

    /// Components removed successfully (Bevy 0.16)
    ComponentsRemoved,

    /// Entity reparented successfully (Bevy 0.16)
    EntityReparented,

    /// Screenshot taken successfully
    #[serde(rename = "screenshot")]
    Screenshot {
        /// Path where the screenshot was saved
        path: String,
        /// Success status
        success: bool,
    },

    /// Debug command response
    #[serde(rename = "debug")]
    Debug(Box<DebugResponse>),
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

    /// Debug command not supported
    #[serde(rename = "debug_not_supported")]
    DebugNotSupported,

    /// Debug session error
    #[serde(rename = "debug_session_error")]
    DebugSessionError,

    /// Debug command validation failed
    #[serde(rename = "debug_validation_error")]
    DebugValidationError,

    /// Component insertion failed (Bevy 0.16)
    #[serde(rename = "component_insertion_error")]
    ComponentInsertionError,

    /// Component removal failed (Bevy 0.16)
    #[serde(rename = "component_removal_error")]
    ComponentRemovalError,

    /// Entity reparenting failed (Bevy 0.16)
    #[serde(rename = "reparenting_error")]
    ReparentingError,

    /// Strict mode validation failed (Bevy 0.16)
    #[serde(rename = "strict_validation_error")]
    StrictValidationError,
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
            Self::DebugNotSupported => write!(f, "Debug command not supported"),
            Self::DebugSessionError => write!(f, "Debug session error"),
            Self::DebugValidationError => write!(f, "Debug command validation error"),
            Self::ComponentInsertionError => write!(f, "Component insertion failed"),
            Self::ComponentRemovalError => write!(f, "Component removal failed"),
            Self::ReparentingError => write!(f, "Entity reparenting failed"),
            Self::StrictValidationError => write!(f, "Strict mode validation failed"),
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
            BrpRequest::Get { entity, .. } 
            | BrpRequest::Destroy { entity }
            | BrpRequest::Insert { entity, .. }
            | BrpRequest::Remove { entity, .. }
            | BrpRequest::Reparent { entity, .. } => {
                validate_entity_id(*entity)
            }
            BrpRequest::Set { entity, components }
            | BrpRequest::Insert { entity, components } => {
                validate_entity_id(*entity)?;
                for type_id in components.keys() {
                    validate_component_type_id(type_id)?;
                }
                Ok(())
            }
            BrpRequest::Remove { entity, components } => {
                validate_entity_id(*entity)?;
                for type_id in components {
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
            BrpRequest::Query { strict, .. } => {
                // Validate strict parameter if provided
                if let Some(_strict_mode) = strict {
                    // Strict mode is valid - no additional validation needed
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
            strict: Some(true),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: BrpRequest = serde_json::from_str(&json).unwrap();

        match deserialized {
            BrpRequest::Query { filter, limit, strict } => {
                assert_eq!(limit, Some(10));
                assert_eq!(strict, Some(true));
                assert!(filter.is_some());
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_bevy_16_new_methods() {
        // Test Insert request
        let insert_request = BrpRequest::Insert {
            entity: 123,
            components: {
                let mut components = HashMap::new();
                components.insert("Transform".to_string(), serde_json::json!({"x": 1.0}));
                components
            },
        };
        let json = serde_json::to_string(&insert_request).unwrap();
        assert!(json.contains("bevy/insert"));

        // Test Remove request
        let remove_request = BrpRequest::Remove {
            entity: 123,
            components: vec!["Transform".to_string()],
        };
        let json = serde_json::to_string(&remove_request).unwrap();
        assert!(json.contains("bevy/remove"));

        // Test Reparent request
        let reparent_request = BrpRequest::Reparent {
            entity: 123,
            parent: Some(456),
        };
        let json = serde_json::to_string(&reparent_request).unwrap();
        assert!(json.contains("bevy/reparent"));
    }

    #[test]
    fn test_entity_with_generation() {
        let entity = EntityWithGeneration::new(123, 5);
        assert_eq!(entity.index, 123);
        assert_eq!(entity.generation, 5);

        let entity_id = entity.to_entity_id();
        let restored = EntityWithGeneration::from_entity_id(entity_id);
        assert_eq!(restored.index, 123);
        assert_eq!(restored.generation, 5);
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
