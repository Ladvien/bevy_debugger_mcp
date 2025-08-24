/// Entity inspection system for detailed debugging and analysis
use crate::brp_messages::{
    BrpRequest, BrpResponse, BrpResult, EntityData, EntityId, EntityMetadata, 
    EntityRelationships, EntityInspectionResult, DetailedComponentTypeInfo, 
    EntityLocationInfo, DebugResponse, ComponentValue,
};
use crate::brp_client::BrpClient;
use crate::error::{Error, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Maximum number of entities that can be inspected in a single batch
pub const MAX_BATCH_SIZE: usize = 100;

/// Entity inspector with advanced capabilities
pub struct EntityInspector {
    brp_client: Arc<RwLock<BrpClient>>,
    /// Cache of recently inspected entities to improve performance
    entity_cache: Arc<RwLock<HashMap<EntityId, CachedEntityData>>>,
    /// Component change tracking
    change_tracker: Arc<RwLock<ComponentChangeTracker>>,
}

/// Cached entity data to avoid repeated BRP calls
#[derive(Debug, Clone)]
struct CachedEntityData {
    entity: EntityData,
    metadata: EntityMetadata,
    relationships: Option<EntityRelationships>,
    cached_at: Instant,
    /// Cache TTL (5 seconds for active debugging)
    ttl: std::time::Duration,
}

/// Component change tracking system
#[derive(Debug, Default)]
struct ComponentChangeTracker {
    /// Track component modifications by entity
    modifications: HashMap<EntityId, HashMap<String, u64>>, // component -> last_modified_frame
    /// Current frame number
    current_frame: u64,
}

impl EntityInspector {
    /// Create new entity inspector
    pub fn new(brp_client: Arc<RwLock<BrpClient>>) -> Self {
        Self {
            brp_client,
            entity_cache: Arc::new(RwLock::new(HashMap::new())),
            change_tracker: Arc::new(RwLock::new(ComponentChangeTracker::default())),
        }
    }

    /// Inspect a single entity with comprehensive data
    pub async fn inspect_entity(
        &self,
        entity_id: EntityId,
        include_metadata: bool,
        include_relationships: bool,
    ) -> Result<DebugResponse> {
        debug!("Inspecting entity {} (metadata: {}, relationships: {})", 
               entity_id, include_metadata, include_relationships);

        let start_time = Instant::now();

        // Check cache first
        if let Some(cached) = self.get_cached_entity(entity_id).await {
            if !cached.ttl_expired() {
                debug!("Using cached data for entity {}", entity_id);
                return Ok(DebugResponse::EntityInspection {
                    entity: cached.entity,
                    metadata: if include_metadata { Some(cached.metadata) } else { None },
                    relationships: if include_relationships { cached.relationships } else { None },
                });
            }
        }

        // Fetch entity data from Bevy via BRP
        let entity_data = match self.fetch_entity_from_brp(entity_id).await {
            Ok(data) => data,
            Err(e) => {
                warn!("Failed to fetch entity {}: {}", entity_id, e);
                return Err(Error::DebugError(format!("Entity {} not found or inaccessible: {}", entity_id, e)));
            }
        };

        let metadata = if include_metadata {
            Some(self.build_entity_metadata(&entity_data, entity_id).await?)
        } else {
            None
        };

        let relationships = if include_relationships {
            self.build_entity_relationships(&entity_data, entity_id).await.ok()
        } else {
            None
        };

        // Cache the result
        self.cache_entity(entity_id, &entity_data, metadata.as_ref(), relationships.as_ref()).await;

        // Update change tracking
        self.update_change_tracking(entity_id, &entity_data).await;

        debug!("Entity {} inspection completed in {:?}", entity_id, start_time.elapsed());

        Ok(DebugResponse::EntityInspection {
            entity: entity_data,
            metadata,
            relationships,
        })
    }

    /// Inspect multiple entities in an optimized batch operation
    pub async fn inspect_batch(
        &self,
        entity_ids: Vec<EntityId>,
        include_metadata: bool,
        include_relationships: bool,
        limit: Option<usize>,
    ) -> Result<DebugResponse> {
        let start_time = Instant::now();
        
        // Apply limit (default to MAX_BATCH_SIZE)
        let actual_limit = limit.unwrap_or(MAX_BATCH_SIZE).min(MAX_BATCH_SIZE);
        let entities_to_inspect: Vec<EntityId> = entity_ids.into_iter().take(actual_limit).collect();
        
        debug!("Batch inspecting {} entities", entities_to_inspect.len());

        let mut results = Vec::with_capacity(entities_to_inspect.len());
        let mut missing_entities = Vec::new();
        let mut found_count = 0;

        // Process entities in parallel batches for better performance
        let chunk_size = 10; // Process 10 entities at a time
        for chunk in entities_to_inspect.chunks(chunk_size) {
            let mut chunk_futures = Vec::new();
            
            for &entity_id in chunk {
                let inspector = self.clone();
                let future = async move {
                    inspector.inspect_single_for_batch(entity_id, include_metadata, include_relationships).await
                };
                chunk_futures.push((entity_id, future));
            }

            // Wait for chunk to complete
            for (entity_id, future) in chunk_futures {
                match future.await {
                    Ok(result) => {
                        results.push(result);
                        found_count += 1;
                    }
                    Err(_) => {
                        missing_entities.push(entity_id);
                        results.push(EntityInspectionResult {
                            entity_id,
                            found: false,
                            entity: None,
                            metadata: None,
                            relationships: None,
                            error: Some("Entity not found or inaccessible".to_string()),
                        });
                    }
                }
            }
        }

        let inspection_time = start_time.elapsed();
        debug!("Batch inspection of {} entities completed in {:?}", 
               entities_to_inspect.len(), inspection_time);

        Ok(DebugResponse::BatchEntityInspection {
            entities: results,
            requested_count: entities_to_inspect.len(),
            found_count,
            missing_entities,
            inspection_time_us: inspection_time.as_micros() as u64,
        })
    }

    /// Inspect a single entity for batch operations (simplified result)
    async fn inspect_single_for_batch(
        &self,
        entity_id: EntityId,
        include_metadata: bool,
        include_relationships: bool,
    ) -> Result<EntityInspectionResult> {
        // Try to fetch entity data
        let entity_data = self.fetch_entity_from_brp(entity_id).await?;

        let metadata = if include_metadata {
            self.build_entity_metadata(&entity_data, entity_id).await.ok()
        } else {
            None
        };

        let relationships = if include_relationships {
            self.build_entity_relationships(&entity_data, entity_id).await.ok()
        } else {
            None
        };

        Ok(EntityInspectionResult {
            entity_id,
            found: true,
            entity: Some(entity_data),
            metadata,
            relationships,
            error: None,
        })
    }

    /// Fetch entity data from Bevy via BRP client
    async fn fetch_entity_from_brp(&self, entity_id: EntityId) -> Result<EntityData> {
        let mut brp_client = self.brp_client.write().await;
        
        // Use BRP Get command to fetch entity with all components
        let request = BrpRequest::Get {
            entity: entity_id,
            components: None, // Fetch all components
        };

        let response = brp_client.send_request(&request).await?;

        match response {
            BrpResponse::Success(boxed_result) => {
                if let BrpResult::Entity(entity_data) = boxed_result.as_ref() {
                    Ok(entity_data.clone())
                } else {
                    Err(Error::Brp("Expected entity data".to_string()))
                }
            }
            BrpResponse::Error(err) => {
                Err(Error::DebugError(format!("BRP error: {}", err.message)))
            }
            _ => Err(Error::DebugError("Unexpected BRP response type".to_string())),
        }
    }

    /// Build comprehensive entity metadata
    async fn build_entity_metadata(&self, entity_data: &EntityData, entity_id: EntityId) -> Result<EntityMetadata> {
        let component_count = entity_data.components.len();
        let mut total_memory_size = 0usize;
        let mut component_types = Vec::new();
        let mut modified_components = Vec::new();

        // Analyze each component
        for (component_type, component_value) in &entity_data.components {
            let size_estimate = self.estimate_component_size(component_value);
            total_memory_size += size_estimate;

            let is_modified = self.is_component_modified(entity_id, component_type).await;
            if is_modified {
                modified_components.push(component_type.clone());
            }

            component_types.push(DetailedComponentTypeInfo {
                type_id: component_type.clone(),
                type_name: self.get_friendly_type_name(component_type),
                size_bytes: size_estimate,
                is_reflected: self.has_reflection_data(component_type),
                schema: self.get_component_schema(component_type, component_value).await,
                is_modified,
            });
        }

        // Get entity generation and archetype info (simulated for now)
        let generation = self.get_entity_generation(entity_id).await.unwrap_or(0);
        let archetype_id = self.get_entity_archetype(entity_id).await;
        let location_info = self.get_entity_location(entity_id).await;

        Ok(EntityMetadata {
            component_count,
            memory_size: total_memory_size,
            last_modified: Some(chrono::Utc::now().timestamp_micros() as u64),
            generation,
            index: entity_id as u32, // Extract index from entity_id
            component_types,
            modified_components,
            archetype_id,
            location_info,
        })
    }

    /// Build entity relationship information
    async fn build_entity_relationships(&self, entity_data: &EntityData, _entity_id: EntityId) -> Result<EntityRelationships> {
        let mut parent = None;
        let mut children = Vec::new();
        let mut related = HashMap::new();

        // Look for Parent component
        if let Some(parent_value) = entity_data.components.get("bevy_hierarchy::components::parent::Parent") {
            if let Ok(parent_id) = self.extract_entity_id_from_component(parent_value) {
                parent = Some(parent_id);
            }
        }

        // Look for Children component
        if let Some(children_value) = entity_data.components.get("bevy_hierarchy::components::children::Children") {
            children = self.extract_entity_ids_from_component(children_value)
                .unwrap_or_default();
        }

        // Look for other relationship components (Transform, GlobalTransform, etc.)
        for (component_type, component_value) in &entity_data.components {
            if self.is_relationship_component(component_type) {
                if let Ok(entity_ids) = self.extract_entity_ids_from_component(component_value) {
                    if !entity_ids.is_empty() {
                        related.insert(component_type.clone(), entity_ids);
                    }
                }
            }
        }

        Ok(EntityRelationships {
            parent,
            children,
            related,
        })
    }

    /// Check if entity is in cache and not expired
    async fn get_cached_entity(&self, entity_id: EntityId) -> Option<CachedEntityData> {
        let cache = self.entity_cache.read().await;
        cache.get(&entity_id).cloned().filter(|cached| !cached.ttl_expired())
    }

    /// Cache entity data
    async fn cache_entity(
        &self,
        entity_id: EntityId,
        entity_data: &EntityData,
        metadata: Option<&EntityMetadata>,
        relationships: Option<&EntityRelationships>,
    ) {
        let cached_data = CachedEntityData {
            entity: entity_data.clone(),
            metadata: metadata.cloned().unwrap_or_else(|| EntityMetadata {
                component_count: entity_data.components.len(),
                memory_size: 0,
                last_modified: None,
                generation: 0,
                index: entity_data.entity as u32, // Extract index from entity ID
                component_types: Vec::new(),
                modified_components: Vec::new(),
                archetype_id: None,
                location_info: None,
            }),
            relationships: relationships.cloned(),
            cached_at: Instant::now(),
            ttl: std::time::Duration::from_secs(5),
        };

        let mut cache = self.entity_cache.write().await;
        cache.insert(entity_id, cached_data);

        // Cleanup old cache entries (keep max 1000) - more efficient cleanup
        if cache.len() > 1000 {
            let cutoff = Instant::now() - std::time::Duration::from_secs(30);
            let initial_len = cache.len();
            cache.retain(|_, cached| cached.cached_at > cutoff);
            let cleaned_count = initial_len - cache.len();
            if cleaned_count > 0 {
                debug!("Cleaned {} expired cache entries", cleaned_count);
            }
        }
    }

    /// Update component change tracking
    async fn update_change_tracking(&self, entity_id: EntityId, entity_data: &EntityData) {
        let mut tracker = self.change_tracker.write().await;
        tracker.current_frame += 1;
        let current_frame = tracker.current_frame;

        let entity_mods = tracker.modifications.entry(entity_id).or_insert_with(HashMap::new);
        for component_type in entity_data.components.keys() {
            entity_mods.insert(component_type.clone(), current_frame);
        }
    }

    /// Check if a component has been modified recently
    async fn is_component_modified(&self, entity_id: EntityId, component_type: &str) -> bool {
        let tracker = self.change_tracker.read().await;
        if let Some(entity_mods) = tracker.modifications.get(&entity_id) {
            if let Some(&last_modified) = entity_mods.get(component_type) {
                return tracker.current_frame - last_modified <= 60; // Modified within last 60 frames
            }
        }
        false
    }

    /// Estimate component memory size with depth protection
    fn estimate_component_size(&self, component_value: &ComponentValue) -> usize {
        self.estimate_component_size_with_depth(component_value, 0)
    }
    
    /// Estimate component memory size with depth limit to prevent stack overflow
    fn estimate_component_size_with_depth(&self, component_value: &ComponentValue, depth: usize) -> usize {
        const MAX_DEPTH: usize = 32; // Prevent infinite recursion
        if depth > MAX_DEPTH {
            return 1024; // Default size for deeply nested structures
        }
        
        match component_value {
            Value::Null => 0,
            Value::Bool(_) => 1,
            Value::Number(_) => 8, // Assume f64
            Value::String(s) => s.len() + 24, // String overhead
            Value::Array(arr) => {
                arr.iter().map(|v| self.estimate_component_size_with_depth(v, depth + 1)).sum::<usize>() + 24
            }
            Value::Object(obj) => {
                obj.values().map(|v| self.estimate_component_size_with_depth(v, depth + 1)).sum::<usize>() + obj.len() * 32
            }
        }
    }

    /// Get friendly type name from component type ID
    fn get_friendly_type_name(&self, type_id: &str) -> String {
        // Extract the last part of the type path for readability
        if let Some(last_part) = type_id.split("::").last() {
            last_part.to_string()
        } else {
            type_id.to_string()
        }
    }

    /// Check if component has reflection data
    fn has_reflection_data(&self, component_type: &str) -> bool {
        // Common Bevy components that have reflection
        matches!(component_type,
            "bevy_transform::components::transform::Transform" |
            "bevy_transform::components::global_transform::GlobalTransform" |
            "bevy_core::name::Name" |
            "bevy_render::view::visibility::Visibility" |
            "bevy_hierarchy::components::parent::Parent" |
            "bevy_hierarchy::components::children::Children"
        )
    }

    /// Get component schema if available
    async fn get_component_schema(&self, component_type: &str, _component_value: &ComponentValue) -> Option<Value> {
        // Return basic schemas for known types
        match component_type {
            "bevy_transform::components::transform::Transform" => Some(json!({
                "type": "object",
                "properties": {
                    "translation": {"type": "object", "properties": {"x": {"type": "number"}, "y": {"type": "number"}, "z": {"type": "number"}}},
                    "rotation": {"type": "object", "properties": {"x": {"type": "number"}, "y": {"type": "number"}, "z": {"type": "number"}, "w": {"type": "number"}}},
                    "scale": {"type": "object", "properties": {"x": {"type": "number"}, "y": {"type": "number"}, "z": {"type": "number"}}}
                }
            })),
            "bevy_core::name::Name" => Some(json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"}
                }
            })),
            _ => None,
        }
    }

    /// Get entity generation (simulated)
    async fn get_entity_generation(&self, _entity_id: EntityId) -> Option<u32> {
        // In a real implementation, this would query Bevy's entity metadata
        Some(1) // Placeholder
    }

    /// Get entity archetype ID (simulated)
    async fn get_entity_archetype(&self, _entity_id: EntityId) -> Option<u32> {
        // In a real implementation, this would query Bevy's archetype system
        Some(0) // Placeholder
    }

    /// Get entity location info (simulated)
    async fn get_entity_location(&self, _entity_id: EntityId) -> Option<EntityLocationInfo> {
        // In a real implementation, this would query Bevy's entity storage
        Some(EntityLocationInfo {
            archetype_id: 0,
            index: 0,
            table_id: Some(0),
            table_row: Some(0),
        })
    }

    /// Extract entity ID from component value
    fn extract_entity_id_from_component(&self, component_value: &ComponentValue) -> Result<EntityId> {
        match component_value {
            Value::Number(n) => {
                if let Some(id) = n.as_u64() {
                    Ok(id)
                } else {
                    Err(Error::DebugError("Invalid entity ID in component".to_string()))
                }
            }
            Value::Object(obj) => {
                // Look for common entity ID field names
                for field_name in &["entity", "id", "Entity", "target"] {
                    if let Some(Value::Number(n)) = obj.get(*field_name) {
                        if let Some(id) = n.as_u64() {
                            return Ok(id);
                        }
                    }
                }
                Err(Error::DebugError("No entity ID found in component".to_string()))
            }
            _ => Err(Error::DebugError("Invalid component format for entity ID".to_string())),
        }
    }

    /// Extract entity IDs from component value (for collections)
    fn extract_entity_ids_from_component(&self, component_value: &ComponentValue) -> Result<Vec<EntityId>> {
        match component_value {
            Value::Array(arr) => {
                let mut entity_ids = Vec::new();
                for item in arr {
                    if let Ok(id) = self.extract_entity_id_from_component(item) {
                        entity_ids.push(id);
                    }
                }
                Ok(entity_ids)
            }
            _ => {
                // Try to extract single entity ID
                self.extract_entity_id_from_component(component_value)
                    .map(|id| vec![id])
            }
        }
    }

    /// Check if component type represents a relationship
    fn is_relationship_component(&self, component_type: &str) -> bool {
        component_type.contains("parent") ||
        component_type.contains("child") ||
        component_type.contains("Parent") ||
        component_type.contains("Children") ||
        component_type.contains("Relation")
    }

    /// Clear entity from cache (useful when entity is despawned)
    pub async fn invalidate_cache(&self, entity_id: EntityId) {
        let mut cache = self.entity_cache.write().await;
        cache.remove(&entity_id);
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> (usize, usize) {
        let cache = self.entity_cache.read().await;
        let total_entries = cache.len();
        let expired_entries = cache.values()
            .filter(|cached| cached.ttl_expired())
            .count();
        (total_entries, expired_entries)
    }
}

impl Clone for EntityInspector {
    fn clone(&self) -> Self {
        Self {
            brp_client: self.brp_client.clone(),
            entity_cache: self.entity_cache.clone(),
            change_tracker: self.change_tracker.clone(),
        }
    }
}

impl CachedEntityData {
    /// Check if cached data has expired
    fn ttl_expired(&self) -> bool {
        self.cached_at.elapsed() > self.ttl
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_component_size_estimation() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        let inspector = EntityInspector::new(brp_client);

        // Test different component value sizes
        assert_eq!(inspector.estimate_component_size(&Value::Null), 0);
        assert_eq!(inspector.estimate_component_size(&Value::Bool(true)), 1);
        assert_eq!(inspector.estimate_component_size(&Value::Number(serde_json::Number::from(42))), 8);
        assert!(inspector.estimate_component_size(&Value::String("test".to_string())) >= 4);
    }

    #[test]
    fn test_friendly_type_names() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        let inspector = EntityInspector::new(brp_client);

        assert_eq!(
            inspector.get_friendly_type_name("bevy_transform::components::transform::Transform"),
            "Transform"
        );
        assert_eq!(
            inspector.get_friendly_type_name("simple_name"),
            "simple_name"
        );
    }

    #[test]
    fn test_reflection_detection() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        let inspector = EntityInspector::new(brp_client);

        assert!(inspector.has_reflection_data("bevy_transform::components::transform::Transform"));
        assert!(inspector.has_reflection_data("bevy_core::name::Name"));
        assert!(!inspector.has_reflection_data("custom::unknown::Component"));
    }

    #[tokio::test]
    async fn test_cache_expiry() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        let inspector = EntityInspector::new(brp_client);

        let cached = CachedEntityData {
            entity: EntityData {
                id: 123,
                components: HashMap::new(),
            },
            metadata: EntityMetadata {
                component_count: 0,
                memory_size: 0,
                last_modified: None,
                generation: 0,
                component_types: Vec::new(),
                modified_components: Vec::new(),
                archetype_id: None,
                location_info: None,
            },
            relationships: None,
            cached_at: Instant::now() - std::time::Duration::from_secs(10),
            ttl: std::time::Duration::from_secs(5),
        };

        assert!(cached.ttl_expired());
    }
}