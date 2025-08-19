/// ECS Query Builder and Validator for safe query construction
/// 
/// Provides a fluent interface for building complex ECS queries with validation,
/// optimization suggestions, and performance estimation.
use crate::brp_messages::{
    ValidatedQuery, QueryFilter, QueryCost, ComponentFilter, 
    DebugCommand
};
use crate::error::{Error, Result};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Maximum number of components allowed in a query
pub const MAX_COMPONENTS_PER_QUERY: usize = 20;

/// Maximum estimated entities to scan before requiring pagination
pub const MAX_ENTITIES_SCAN_THRESHOLD: usize = 10_000;

/// Performance budget for query execution (in microseconds)
pub const QUERY_PERFORMANCE_BUDGET_US: u64 = 10_000; // 10ms

/// Builder for creating type-safe ECS queries
#[derive(Debug, Clone)]
pub struct QueryBuilder {
    /// Components that entities must have
    with_components: Vec<String>,
    /// Components that entities must not have
    without_components: Vec<String>,
    /// Component value filters
    component_filters: Vec<ComponentFilter>,
    /// Query limit
    limit: Option<usize>,
    /// Offset for pagination
    offset: Option<usize>,
    /// Cache key for query reuse
    cache_key: Option<String>,
    /// Performance estimation cache
    cost_cache: Option<QueryCost>,
}

impl QueryBuilder {
    /// Create a new query builder
    pub fn new() -> Self {
        Self {
            with_components: Vec::new(),
            without_components: Vec::new(),
            component_filters: Vec::new(),
            limit: None,
            offset: None,
            cache_key: None,
            cost_cache: None,
        }
    }

    /// Add a required component to the query
    pub fn with_component<T: AsRef<str>>(mut self, component_type: T) -> Self {
        self.with_components.push(component_type.as_ref().to_string());
        self.invalidate_cache();
        self
    }

    /// Add multiple required components to the query
    pub fn with_components<I, T>(mut self, components: I) -> Self 
    where
        I: IntoIterator<Item = T>,
        T: AsRef<str>,
    {
        for component in components {
            self.with_components.push(component.as_ref().to_string());
        }
        self.invalidate_cache();
        self
    }

    /// Add a component that entities must not have
    pub fn without_component<T: AsRef<str>>(mut self, component_type: T) -> Self {
        self.without_components.push(component_type.as_ref().to_string());
        self.invalidate_cache();
        self
    }

    /// Add multiple components that entities must not have
    pub fn without_components<I, T>(mut self, components: I) -> Self 
    where
        I: IntoIterator<Item = T>,
        T: AsRef<str>,
    {
        for component in components {
            self.without_components.push(component.as_ref().to_string());
        }
        self.invalidate_cache();
        self
    }

    /// Add a component value filter
    pub fn where_component(mut self, filter: ComponentFilter) -> Self {
        self.component_filters.push(filter);
        self.invalidate_cache();
        self
    }

    /// Set result limit for pagination
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self.invalidate_cache();
        self
    }

    /// Set offset for pagination
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self.invalidate_cache();
        self
    }

    /// Validate the query and return a ValidatedQuery if successful
    pub fn validate(&self) -> Result<ValidatedQuery> {
        let validator = QueryValidator::new();
        validator.validate(self)
    }

    /// Estimate the performance cost of this query
    pub fn estimate_cost(&mut self) -> QueryCost {
        if let Some(cached_cost) = &self.cost_cache {
            return cached_cost.clone();
        }

        let estimator = QueryCostEstimator::new();
        let cost = estimator.estimate(self);
        self.cost_cache = Some(cost.clone());
        cost
    }

    /// Get optimization suggestions for this query
    pub fn get_optimization_hints(&self) -> Vec<String> {
        let optimizer = QueryOptimizer::new();
        optimizer.analyze(self)
    }

    /// Build the query into a DebugCommand
    pub fn build(self) -> Result<DebugCommand> {
        let validated_query = self.validate()?;
        
        Ok(DebugCommand::ExecuteQuery {
            query: validated_query,
            limit: self.limit,
            offset: self.offset,
        })
    }

    /// Get the query components for internal use
    pub fn get_with_components(&self) -> &[String] {
        &self.with_components
    }

    pub fn get_without_components(&self) -> &[String] {
        &self.without_components
    }

    pub fn get_component_filters(&self) -> &[ComponentFilter] {
        &self.component_filters
    }

    pub fn get_limit(&self) -> Option<usize> {
        self.limit
    }

    pub fn get_offset(&self) -> Option<usize> {
        self.offset
    }

    /// Invalidate cached values when query changes
    fn invalidate_cache(&mut self) {
        self.cache_key = None;
        self.cost_cache = None;
    }

    /// Generate a cache key for this query
    pub fn cache_key(&mut self) -> String {
        if let Some(ref key) = self.cache_key {
            return key.clone();
        }

        // Create deterministic cache key from query components
        let mut key_parts = Vec::new();
        
        // Sort components for deterministic key generation
        let mut with_sorted = self.with_components.clone();
        with_sorted.sort();
        key_parts.push(format!("with:{}", with_sorted.join(",")));
        
        let mut without_sorted = self.without_components.clone();
        without_sorted.sort();
        key_parts.push(format!("without:{}", without_sorted.join(",")));
        
        if !self.component_filters.is_empty() {
            key_parts.push(format!("filters:{}", self.component_filters.len()));
        }
        
        if let Some(limit) = self.limit {
            key_parts.push(format!("limit:{}", limit));
        }
        
        if let Some(offset) = self.offset {
            key_parts.push(format!("offset:{}", offset));
        }

        let key = format!("qb_{}", sha256_hash(&key_parts.join("|")));
        self.cache_key = Some(key.clone());
        key
    }
}

impl Default for QueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for ECS queries
#[derive(Debug)]
pub struct QueryValidator {
    /// Known component types (would be populated from type registry)
    known_components: HashSet<String>,
}

impl QueryValidator {
    /// Create a new query validator
    pub fn new() -> Self {
        // In a real implementation, this would be populated from Bevy's type registry
        let mut known_components = HashSet::new();
        
        // Common Bevy components for validation
        known_components.insert("Transform".to_string());
        known_components.insert("GlobalTransform".to_string());
        known_components.insert("Translation".to_string());
        known_components.insert("Rotation".to_string());
        known_components.insert("Scale".to_string());
        known_components.insert("Velocity".to_string());
        known_components.insert("Acceleration".to_string());
        known_components.insert("Handle<Mesh>".to_string());
        known_components.insert("Handle<StandardMaterial>".to_string());
        known_components.insert("Camera".to_string());
        known_components.insert("DirectionalLight".to_string());
        known_components.insert("PointLight".to_string());
        known_components.insert("SpotLight".to_string());
        known_components.insert("Name".to_string());
        known_components.insert("Parent".to_string());
        known_components.insert("Children".to_string());
        known_components.insert("Visibility".to_string());
        known_components.insert("ComputedVisibility".to_string());
        
        Self {
            known_components,
        }
    }

    /// Validate a query builder and return a ValidatedQuery
    pub fn validate(&self, builder: &QueryBuilder) -> Result<ValidatedQuery> {
        // Check component count limits
        let total_components = builder.with_components.len() + builder.without_components.len();
        if total_components > MAX_COMPONENTS_PER_QUERY {
            return Err(Error::Validation(format!(
                "Query has too many components: {} (max: {})",
                total_components,
                MAX_COMPONENTS_PER_QUERY
            )));
        }

        // Validate that components exist
        self.validate_component_types(builder)?;

        // Check for contradictory filters
        self.validate_logical_consistency(builder)?;

        // Create the validated query
        let filter = QueryFilter {
            with: if builder.with_components.is_empty() {
                None
            } else {
                Some(builder.with_components.clone())
            },
            without: if builder.without_components.is_empty() {
                None
            } else {
                Some(builder.without_components.clone())
            },
            where_clause: if builder.component_filters.is_empty() {
                None
            } else {
                Some(builder.component_filters.clone())
            },
        };

        // Estimate cost
        let cost_estimator = QueryCostEstimator::new();
        let mut builder_mut = builder.clone();
        let estimated_cost = cost_estimator.estimate(&mut builder_mut);

        // Get optimization hints
        let optimizer = QueryOptimizer::new();
        let hints = optimizer.analyze(builder);

        let query_id = Uuid::new_v4().to_string();

        Ok(ValidatedQuery {
            id: query_id,
            filter,
            estimated_cost,
            hints,
        })
    }

    /// Validate that all referenced component types exist
    fn validate_component_types(&self, builder: &QueryBuilder) -> Result<()> {
        for component in &builder.with_components {
            if !self.known_components.contains(component) {
                return Err(Error::Validation(format!(
                    "Unknown component type: '{}'. Available components: {}",
                    component,
                    self.get_component_suggestions(component)
                )));
            }
        }

        for component in &builder.without_components {
            if !self.known_components.contains(component) {
                return Err(Error::Validation(format!(
                    "Unknown component type: '{}'. Available components: {}",
                    component,
                    self.get_component_suggestions(component)
                )));
            }
        }

        Ok(())
    }

    /// Validate logical consistency (no contradictory filters)
    fn validate_logical_consistency(&self, builder: &QueryBuilder) -> Result<()> {
        // Check for components that are both required and excluded
        for with_component in &builder.with_components {
            if builder.without_components.contains(with_component) {
                return Err(Error::Validation(format!(
                    "Component '{}' is both required (with) and excluded (without)",
                    with_component
                )));
            }
        }

        // Check for empty query
        if builder.with_components.is_empty() && builder.without_components.is_empty() && builder.component_filters.is_empty() {
            return Err(Error::Validation(
                "Query is empty - must specify at least one component filter".to_string()
            ));
        }

        Ok(())
    }

    /// Get suggestions for similar component names
    fn get_component_suggestions(&self, input: &str) -> String {
        let mut suggestions = Vec::new();
        let input_lower = input.to_lowercase();

        for component in &self.known_components {
            let component_lower = component.to_lowercase();
            if component_lower.contains(&input_lower) || input_lower.contains(&component_lower) {
                suggestions.push(component.clone());
            }
        }

        if suggestions.is_empty() {
            // Return first few components as fallback
            suggestions.extend(self.known_components.iter().take(5).cloned());
        }

        suggestions.join(", ")
    }
}

/// Query cost estimator for performance prediction
#[derive(Debug)]
pub struct QueryCostEstimator {
    /// Base cost per component check (in microseconds)
    component_check_cost_us: u64,
    /// Cost multiplier for component filters
    filter_cost_multiplier: f64,
}

impl QueryCostEstimator {
    /// Create a new cost estimator
    pub fn new() -> Self {
        Self {
            component_check_cost_us: 10, // 10 microseconds per component check
            filter_cost_multiplier: 1.5,
        }
    }

    /// Estimate the cost of executing a query
    pub fn estimate(&self, builder: &mut QueryBuilder) -> QueryCost {
        // Estimate number of entities to scan based on component selectivity
        let estimated_entities = self.estimate_entity_count(builder);
        
        // Calculate time estimate
        let component_checks = builder.with_components.len() + builder.without_components.len();
        let base_time = estimated_entities as u64 * component_checks as u64 * self.component_check_cost_us;
        
        // Add filter overhead
        let filter_overhead = if !builder.component_filters.is_empty() {
            (base_time as f64 * self.filter_cost_multiplier) as u64
        } else {
            0
        };

        let estimated_time_us = base_time + filter_overhead;

        // Estimate memory usage (rough approximation)
        let entity_size_estimate = 128; // bytes per entity result
        let estimated_memory = estimated_entities * entity_size_estimate;

        QueryCost {
            estimated_entities,
            estimated_time_us,
            estimated_memory,
        }
    }

    /// Estimate the number of entities that will be scanned
    fn estimate_entity_count(&self, builder: &QueryBuilder) -> usize {
        // This is a simplified estimation - in reality would use statistics from the ECS world
        let base_entity_count = 10_000; // Assume 10k entities in world
        
        // Apply selectivity based on component requirements
        let selectivity = self.calculate_selectivity(builder);
        
        let estimated = (base_entity_count as f64 * selectivity) as usize;
        
        // Apply limit if specified
        if let Some(limit) = builder.limit {
            estimated.min(limit)
        } else {
            estimated
        }
    }

    /// Calculate query selectivity (fraction of entities that will match)
    fn calculate_selectivity(&self, builder: &QueryBuilder) -> f64 {
        // Base selectivity for common components
        let component_selectivity: HashMap<&str, f64> = [
            ("Transform", 0.8),
            ("GlobalTransform", 0.8),
            ("Name", 0.3),
            ("Velocity", 0.1),
            ("Camera", 0.001),
            ("DirectionalLight", 0.0001),
        ].iter().cloned().collect();

        let mut selectivity = 1.0;

        // Apply selectivity reduction for each required component
        for component in &builder.with_components {
            let component_sel = component_selectivity.get(component.as_str()).unwrap_or(&0.1);
            selectivity *= component_sel;
        }

        // Without components increase selectivity (fewer exclusions)
        for _component in &builder.without_components {
            selectivity *= 0.95; // Slight reduction for each exclusion
        }

        selectivity.max(0.001) // Minimum 0.1% selectivity
    }
}

/// Query optimizer that provides suggestions for better performance
#[derive(Debug)]
pub struct QueryOptimizer;

impl QueryOptimizer {
    /// Create a new query optimizer
    pub fn new() -> Self {
        Self
    }

    /// Analyze a query and provide optimization hints
    pub fn analyze(&self, builder: &QueryBuilder) -> Vec<String> {
        let mut hints = Vec::new();

        // Check if query might be too broad
        self.check_query_selectivity(builder, &mut hints);

        // Suggest adding limits for potentially expensive queries
        self.suggest_pagination(builder, &mut hints);

        // Suggest component ordering optimization
        self.suggest_component_ordering(builder, &mut hints);

        // Check for unnecessary complexity
        self.check_query_complexity(builder, &mut hints);

        hints
    }

    /// Check if query might return too many results
    fn check_query_selectivity(&self, builder: &QueryBuilder, hints: &mut Vec<String>) {
        let estimator = QueryCostEstimator::new();
        let selectivity = estimator.calculate_selectivity(builder);

        if selectivity > 0.5 {
            hints.push(
                "Query may be too broad - consider adding more specific component filters to reduce results".to_string()
            );
        }

        if builder.with_components.is_empty() && builder.component_filters.is_empty() {
            hints.push(
                "Query with only exclusions (without) may be inefficient - consider adding positive filters (with)".to_string()
            );
        }
    }

    /// Suggest pagination for expensive queries
    fn suggest_pagination(&self, builder: &QueryBuilder, hints: &mut Vec<String>) {
        let mut builder_mut = builder.clone();
        let estimator = QueryCostEstimator::new();
        let cost = estimator.estimate(&mut builder_mut);

        if cost.estimated_entities > 1000 && builder.limit.is_none() {
            hints.push(
                format!("Large result set expected ({} entities) - consider adding .limit() for better performance", 
                    cost.estimated_entities)
            );
        }

        if cost.estimated_time_us > QUERY_PERFORMANCE_BUDGET_US {
            hints.push(
                format!("Query may exceed performance budget ({:.1}ms > {:.1}ms) - consider pagination or more selective filters", 
                    cost.estimated_time_us as f64 / 1000.0,
                    QUERY_PERFORMANCE_BUDGET_US as f64 / 1000.0)
            );
        }
    }

    /// Suggest optimal component ordering
    fn suggest_component_ordering(&self, builder: &QueryBuilder, hints: &mut Vec<String>) {
        // Suggest putting most selective components first
        let selective_components = ["Camera", "DirectionalLight", "PointLight"];
        let broad_components = ["Transform", "GlobalTransform"];

        let has_selective = builder.with_components.iter()
            .any(|c| selective_components.contains(&c.as_str()));
        let has_broad = builder.with_components.iter()
            .any(|c| broad_components.contains(&c.as_str()));

        if has_broad && !has_selective && builder.with_components.len() == 1 {
            hints.push(
                "Query uses only broad components (Transform, GlobalTransform) - consider adding more specific components for better performance".to_string()
            );
        }
    }

    /// Check for unnecessary query complexity
    fn check_query_complexity(&self, builder: &QueryBuilder, hints: &mut Vec<String>) {
        if builder.component_filters.len() > 5 {
            hints.push(
                "Query has many component filters - consider simplifying or breaking into multiple queries".to_string()
            );
        }

        if builder.with_components.len() > 10 {
            hints.push(
                "Query requires many components - consider if all are necessary for your use case".to_string()
            );
        }
    }
}

/// Simple hash function for cache keys (simplified version)
fn sha256_hash(input: &str) -> String {
    // In a real implementation, you'd use a proper hash function
    // For now, we'll use a simple hash based on content
    let mut hash = 0u64;
    for byte in input.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    format!("{:x}", hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_builder_basic() {
        let query = QueryBuilder::new()
            .with_component("Transform")
            .with_component("Velocity")
            .limit(10);

        assert_eq!(query.get_with_components(), &["Transform", "Velocity"]);
        assert_eq!(query.get_limit(), Some(10));
    }

    #[test]
    fn test_query_builder_fluent_interface() {
        let query = QueryBuilder::new()
            .with_components(vec!["Transform", "Velocity"])
            .without_component("Camera")
            .limit(50)
            .offset(10);

        assert_eq!(query.get_with_components().len(), 2);
        assert_eq!(query.get_without_components(), &["Camera"]);
        assert_eq!(query.get_limit(), Some(50));
        assert_eq!(query.get_offset(), Some(10));
    }

    #[test]
    fn test_query_validation_success() {
        let query = QueryBuilder::new()
            .with_component("Transform")
            .with_component("Velocity");

        let result = query.validate();
        assert!(result.is_ok());

        let validated = result.unwrap();
        assert!(!validated.id.is_empty());
        assert!(validated.filter.with.is_some());
    }

    #[test]
    fn test_query_validation_unknown_component() {
        let query = QueryBuilder::new()
            .with_component("UnknownComponent");

        let result = query.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown component type"));
    }

    #[test]
    fn test_query_validation_contradictory() {
        let query = QueryBuilder::new()
            .with_component("Transform")
            .without_component("Transform");

        let result = query.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("both required"));
    }

    #[test]
    fn test_query_validation_empty() {
        let query = QueryBuilder::new();

        let result = query.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn test_query_cost_estimation() {
        let mut query = QueryBuilder::new()
            .with_component("Transform")
            .with_component("Velocity")
            .limit(100);

        let cost = query.estimate_cost();
        
        assert!(cost.estimated_entities > 0);
        assert!(cost.estimated_time_us > 0);
        assert!(cost.estimated_memory > 0);
        
        // Cost should be affected by limit
        assert!(cost.estimated_entities <= 100);
    }

    #[test]
    fn test_optimization_hints() {
        let query = QueryBuilder::new()
            .with_component("Transform"); // Broad component

        let hints = query.get_optimization_hints();
        assert!(!hints.is_empty());
        
        // Should suggest adding more specific components or limits
        assert!(hints.iter().any(|h| h.contains("broad") || h.contains("limit")));
    }

    #[test]
    fn test_query_cache_key_deterministic() {
        let mut query1 = QueryBuilder::new()
            .with_component("Transform")
            .with_component("Velocity")
            .limit(10);

        let mut query2 = QueryBuilder::new()
            .with_component("Velocity")
            .with_component("Transform")  // Different order
            .limit(10);

        let key1 = query1.cache_key();
        let key2 = query2.cache_key();
        
        assert_eq!(key1, key2); // Should be same despite different construction order
    }

    #[test]
    fn test_query_build_command() {
        let query = QueryBuilder::new()
            .with_component("Transform")
            .limit(10);

        let result = query.build();
        assert!(result.is_ok());

        match result.unwrap() {
            DebugCommand::ExecuteQuery { query, limit, .. } => {
                assert!(!query.id.is_empty());
                assert_eq!(limit, Some(10));
            }
            _ => panic!("Expected ExecuteQuery command"),
        }
    }

    #[test]
    fn test_too_many_components() {
        let mut query = QueryBuilder::new();
        
        // Add too many components
        for i in 0..=MAX_COMPONENTS_PER_QUERY {
            query = query.with_component(format!("Component{}", i));
        }

        let result = query.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too many components"));
    }
}