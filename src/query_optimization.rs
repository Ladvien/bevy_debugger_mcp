/*
 * Bevy Debugger MCP Server - Query Performance Optimization
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
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use ahash::AHashMap;
use std::collections::HashSet;
use serde::{Deserialize, Serialize};

use crate::brp_messages::{BrpRequest, QueryFilter};
use crate::error::{Error, Result};

/// Performance metrics for query execution
#[derive(Debug, Clone, Serialize)]
pub struct QueryPerformanceMetrics {
    /// Unique identifier for the query pattern
    pub query_id: String,
    /// Original query string
    pub query: String,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Number of entities processed
    pub entities_processed: usize,
    /// Number of components accessed
    pub components_accessed: usize,
    /// Whether this was a cache hit
    pub cache_hit: bool,
    /// Whether parallel processing was used
    pub parallel_processing: bool,
    /// Memory usage in bytes (estimated)
    pub memory_usage_bytes: usize,
    /// Timestamp of execution
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Archetype access pattern
    pub archetype_pattern: ArchetypeAccessPattern,
}

/// Represents how a query accesses Bevy's archetype storage
#[derive(Debug, Clone, Serialize)]
pub struct ArchetypeAccessPattern {
    /// Components required for query
    pub required_components: HashSet<String>,
    /// Components excluded from query
    pub excluded_components: HashSet<String>,
    /// Estimated number of archetypes that match
    pub matching_archetypes: usize,
    /// Whether query benefits from parallel iteration
    pub parallel_friendly: bool,
}

/// Cached query state for Bevy ECS queries
#[derive(Debug, Clone)]
pub struct CachedQueryState {
    /// The compiled query pattern
    pub query_pattern: QueryPattern,
    /// Performance characteristics
    pub perf_characteristics: QueryPerformanceCharacteristics,
    /// Last used timestamp for LRU eviction
    pub last_used: Instant,
    /// Number of times this query has been executed
    pub usage_count: u64,
    /// Average execution time
    pub avg_execution_time_ms: f64,
}

/// Optimized query pattern for Bevy ECS
#[derive(Debug, Clone)]
pub struct QueryPattern {
    /// Unique hash of the query
    pub query_hash: u64,
    /// Original BRP request
    pub original_request: BrpRequest,
    /// Optimized component access strategy
    pub access_strategy: ComponentAccessStrategy,
    /// Estimated performance impact
    pub performance_impact: PerformanceImpact,
}

/// Strategy for accessing components efficiently
#[derive(Debug, Clone)]
pub enum ComponentAccessStrategy {
    /// Direct archetype iteration (fastest)
    DirectArchetype {
        components: Vec<String>,
        parallel_threshold: usize,
    },
    /// Filtered iteration with component checks
    FilteredIteration {
        with_components: Vec<String>,
        without_components: Vec<String>,
        filter_early: bool,
    },
    /// Complex query requiring multiple passes
    MultiPass {
        passes: Vec<QueryPass>,
        parallel_passes: Vec<usize>,
    },
}

/// Individual pass in a multi-pass query
#[derive(Debug, Clone)]
pub struct QueryPass {
    pub components: Vec<String>,
    pub filter: Option<QueryFilter>,
    pub parallel_safe: bool,
    pub estimated_entity_count: usize,
}

/// Performance characteristics of a query
#[derive(Debug, Clone)]
pub struct QueryPerformanceCharacteristics {
    /// Whether this query benefits from parallel processing
    pub parallel_friendly: bool,
    /// Estimated number of entities this query will process
    pub estimated_entity_count: usize,
    /// Memory overhead per entity
    pub memory_overhead_per_entity: usize,
    /// CPU complexity score (1-10, higher = more expensive)
    pub cpu_complexity: u8,
    /// Whether query results should be cached
    pub cacheable: bool,
}

/// Performance impact classification
#[derive(Debug, Clone, Serialize)]
pub enum PerformanceImpact {
    /// Low impact - can run frequently
    Low,
    /// Medium impact - should be throttled
    Medium,
    /// High impact - requires careful scheduling
    High,
    /// Critical impact - may cause frame drops
    Critical,
}

/// Query state cache with LRU eviction and performance tracking
pub struct QueryStateCache {
    /// Cached query states
    cache: AHashMap<u64, CachedQueryState>,
    /// Performance metrics history
    metrics_history: Vec<QueryPerformanceMetrics>,
    /// Maximum number of cached queries
    max_cache_size: usize,
    /// Maximum metrics history size
    max_metrics_history: usize,
    /// Cache hit/miss statistics
    cache_stats: CacheStatistics,
}

/// Cache performance statistics
#[derive(Debug, Clone, Default, Serialize)]
pub struct CacheStatistics {
    pub total_queries: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub evictions: u64,
    pub avg_build_time_ms: f64,
    pub avg_execution_time_ms: f64,
}

impl QueryStateCache {
    /// Create new query state cache with specified limits
    pub fn new(max_cache_size: usize, max_metrics_history: usize) -> Self {
        Self {
            cache: AHashMap::with_capacity(max_cache_size),
            metrics_history: Vec::with_capacity(max_metrics_history),
            max_cache_size,
            max_metrics_history,
            cache_stats: CacheStatistics::default(),
        }
    }

    /// Get cached query state or build new one
    pub async fn get_or_build_query_state(
        &mut self,
        request: &BrpRequest,
    ) -> Result<Arc<CachedQueryState>> {
        let query_hash = self.hash_request(request);
        
        // Check cache first
        if let Some(cached_state) = self.cache.get_mut(&query_hash) {
            cached_state.last_used = Instant::now();
            cached_state.usage_count += 1;
            self.cache_stats.cache_hits += 1;
            self.cache_stats.total_queries += 1;
            
            debug!("Query state cache hit for hash: {}", query_hash);
            return Ok(Arc::new(cached_state.clone()));
        }

        // Build new query state
        let build_start = Instant::now();
        let query_state = self.build_query_state(request).await?;
        let build_time = build_start.elapsed();

        // Update statistics
        self.cache_stats.cache_misses += 1;
        self.cache_stats.total_queries += 1;
        self.cache_stats.avg_build_time_ms = 
            (self.cache_stats.avg_build_time_ms * (self.cache_stats.cache_misses - 1) as f64 + 
             build_time.as_millis() as f64) / self.cache_stats.cache_misses as f64;

        // Cache the new state
        self.cache.insert(query_hash, query_state.clone());

        // Evict old entries if necessary
        self.evict_if_necessary();

        debug!(
            "Built and cached new query state in {}ms, hash: {}",
            build_time.as_millis(),
            query_hash
        );

        Ok(Arc::new(query_state))
    }

    /// Build optimized query state for a BRP request
    async fn build_query_state(&self, request: &BrpRequest) -> Result<CachedQueryState> {
        let query_pattern = self.analyze_query_pattern(request).await?;
        let perf_characteristics = self.analyze_performance_characteristics(&query_pattern);

        Ok(CachedQueryState {
            query_pattern,
            perf_characteristics,
            last_used: Instant::now(),
            usage_count: 1,
            avg_execution_time_ms: 0.0,
        })
    }

    /// Analyze query pattern and determine optimal access strategy
    async fn analyze_query_pattern(&self, request: &BrpRequest) -> Result<QueryPattern> {
        let query_hash = self.hash_request(request);
        
        let (access_strategy, performance_impact) = match request {
            BrpRequest::Query { filter, limit, strict: _ } => {
                self.analyze_query_request(filter.as_ref(), *limit).await?
            }
            BrpRequest::Get { entity: _, components } => {
                // Single entity access - always fast
                let strategy = ComponentAccessStrategy::DirectArchetype {
                    components: components.clone().unwrap_or_default(),
                    parallel_threshold: usize::MAX, // Never parallel for single entity
                };
                (strategy, PerformanceImpact::Low)
            }
            BrpRequest::ListEntities { filter } => {
                self.analyze_list_entities_request(filter.as_ref()).await?
            }
            BrpRequest::ListComponents => {
                // Component list access - medium cost
                let strategy = ComponentAccessStrategy::DirectArchetype {
                    components: vec![],
                    parallel_threshold: 1000,
                };
                (strategy, PerformanceImpact::Medium)
            }
            _ => {
                // For all other request types (Set, Spawn, Destroy, Insert, Remove, etc.)
                // Use a simple direct access strategy with medium impact
                let strategy = ComponentAccessStrategy::DirectArchetype {
                    components: vec![],
                    parallel_threshold: 500,
                };
                (strategy, PerformanceImpact::Medium)
            }
        };

        Ok(QueryPattern {
            query_hash,
            original_request: request.clone(),
            access_strategy,
            performance_impact,
        })
    }

    /// Analyze query request and determine optimal strategy
    async fn analyze_query_request(
        &self,
        filter: Option<&QueryFilter>,
        limit: Option<usize>,
    ) -> Result<(ComponentAccessStrategy, PerformanceImpact)> {
        let Some(filter) = filter else {
            // No filter - list all entities (expensive)
            let strategy = ComponentAccessStrategy::FilteredIteration {
                with_components: vec![],
                without_components: vec![],
                filter_early: false,
            };
            return Ok((strategy, PerformanceImpact::High));
        };

        let with_components = filter.with.clone().unwrap_or_default();
        let without_components = filter.without.clone().unwrap_or_default();

        // Determine parallel threshold based on complexity
        let parallel_threshold = match (with_components.len(), without_components.len()) {
            (0, 0) => 100,    // No filters - low threshold for parallelization
            (1..=2, 0) => 500, // Simple filters - medium threshold
            (_, _) => 1000,    // Complex filters - high threshold
        };

        let performance_impact = if with_components.is_empty() && without_components.is_empty() {
            PerformanceImpact::High
        } else if with_components.len() <= 2 && without_components.len() <= 1 {
            PerformanceImpact::Medium
        } else {
            PerformanceImpact::Low
        };

        let strategy = if with_components.len() <= 3 && without_components.is_empty() {
            // Simple case - direct archetype access
            ComponentAccessStrategy::DirectArchetype {
                components: with_components,
                parallel_threshold,
            }
        } else {
            // Complex case - filtered iteration
            ComponentAccessStrategy::FilteredIteration {
                with_components,
                without_components,
                filter_early: true,
            }
        };

        Ok((strategy, performance_impact))
    }

    /// Analyze list entities request
    async fn analyze_list_entities_request(
        &self,
        filter: Option<&QueryFilter>,
    ) -> Result<(ComponentAccessStrategy, PerformanceImpact)> {
        if let Some(filter) = filter {
            self.analyze_query_request(Some(filter), None).await
        } else {
            // List all entities - most expensive operation
            let strategy = ComponentAccessStrategy::FilteredIteration {
                with_components: vec![],
                without_components: vec![],
                filter_early: false,
            };
            Ok((strategy, PerformanceImpact::Critical))
        }
    }

    /// Analyze performance characteristics of a query pattern
    fn analyze_performance_characteristics(&self, pattern: &QueryPattern) -> QueryPerformanceCharacteristics {
        let (parallel_friendly, estimated_entity_count, cpu_complexity) = match &pattern.access_strategy {
            ComponentAccessStrategy::DirectArchetype { components, parallel_threshold } => {
                let entity_count = self.estimate_entity_count_for_components(components);
                (entity_count >= *parallel_threshold, entity_count, 2)
            }
            ComponentAccessStrategy::FilteredIteration { with_components, without_components, .. } => {
                let entity_count = self.estimate_filtered_entity_count(with_components, without_components);
                let complexity = 3 + without_components.len() as u8;
                (entity_count >= 500, entity_count, complexity.min(10))
            }
            ComponentAccessStrategy::MultiPass { passes, .. } => {
                let total_entities: usize = passes.iter().map(|p| p.estimated_entity_count).sum();
                let complexity = (5 + passes.len() as u8).min(10);
                (total_entities >= 1000, total_entities, complexity)
            }
        };

        let memory_overhead_per_entity = match &pattern.access_strategy {
            ComponentAccessStrategy::DirectArchetype { components, .. } => {
                // Estimate memory per component
                components.len() * 64 + 32 // Base entity overhead
            }
            ComponentAccessStrategy::FilteredIteration { .. } => {
                128 // Higher overhead for filtering
            }
            ComponentAccessStrategy::MultiPass { .. } => {
                256 // Highest overhead for multi-pass
            }
        };

        let cacheable = matches!(pattern.performance_impact, 
            PerformanceImpact::Medium | PerformanceImpact::High | PerformanceImpact::Critical
        );

        QueryPerformanceCharacteristics {
            parallel_friendly,
            estimated_entity_count,
            memory_overhead_per_entity,
            cpu_complexity,
            cacheable,
        }
    }

    /// Estimate entity count for specific components
    fn estimate_entity_count_for_components(&self, components: &[String]) -> usize {
        // This would be improved with actual archetype statistics from Bevy
        match components.len() {
            0 => 10000,      // All entities
            1 => 5000,       // Common components
            2 => 2000,       // Component pairs
            3 => 800,        // Triple components
            _ => 200,        // Complex queries
        }
    }

    /// Estimate entity count for filtered queries
    fn estimate_filtered_entity_count(&self, with: &[String], without: &[String]) -> usize {
        let base_count = self.estimate_entity_count_for_components(with);
        
        // Reduce count based on exclusions
        let exclusion_factor = 1.0 - (without.len() as f64 * 0.2).min(0.8);
        (base_count as f64 * exclusion_factor) as usize
    }

    /// Hash a BRP request for caching
    fn hash_request(&self, request: &BrpRequest) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        
        // Create a simplified hashable representation
        match request {
            BrpRequest::Query { filter, limit, strict: _ } => {
                "Query".hash(&mut hasher);
                // Hash filter components that can be hashed
                if let Some(filter) = filter {
                    filter.with.hash(&mut hasher);
                    filter.without.hash(&mut hasher);
                    // Skip where_clause as it contains ComponentValue which doesn't implement Hash
                } else {
                    None::<Vec<String>>.hash(&mut hasher);
                }
                limit.hash(&mut hasher);
            }
            BrpRequest::Get { entity, components } => {
                "Get".hash(&mut hasher);
                entity.hash(&mut hasher);
                components.hash(&mut hasher);
            }
            BrpRequest::ListEntities { filter } => {
                "ListEntities".hash(&mut hasher);
                // Hash filter components that can be hashed
                if let Some(filter) = filter {
                    filter.with.hash(&mut hasher);
                    filter.without.hash(&mut hasher);
                    // Skip where_clause as it contains ComponentValue which doesn't implement Hash
                } else {
                    None::<Vec<String>>.hash(&mut hasher);
                }
            }
            BrpRequest::ListComponents => {
                "ListComponents".hash(&mut hasher);
            }
            _ => {
                // For all other request types, use a generic hash
                std::mem::discriminant(request).hash(&mut hasher);
            }
        }
        
        hasher.finish()
    }

    /// Record query execution metrics
    pub fn record_metrics(&mut self, metrics: QueryPerformanceMetrics) {
        // Update cached query state with new metrics
        if let Some(cached_state) = self.cache.get_mut(&self.hash_request(&BrpRequest::Query { 
            filter: None, 
            limit: None,
            strict: Some(false),
        })) {
            cached_state.avg_execution_time_ms = 
                (cached_state.avg_execution_time_ms * (cached_state.usage_count - 1) as f64 + 
                 metrics.execution_time_ms as f64) / cached_state.usage_count as f64;
        }

        // Update global statistics
        self.cache_stats.avg_execution_time_ms = 
            (self.cache_stats.avg_execution_time_ms * (self.metrics_history.len() as f64) + 
             metrics.execution_time_ms as f64) / (self.metrics_history.len() + 1) as f64;

        // Store metrics
        self.metrics_history.push(metrics);
        
        // Trim history if necessary
        if self.metrics_history.len() > self.max_metrics_history {
            self.metrics_history.drain(0..self.metrics_history.len() - self.max_metrics_history);
        }
    }

    /// Evict least recently used entries if cache is full
    fn evict_if_necessary(&mut self) {
        if self.cache.len() <= self.max_cache_size {
            return;
        }

        // Find LRU entry
        let mut oldest_time = Instant::now();
        let mut oldest_hash = 0;
        
        for (&hash, state) in &self.cache {
            if state.last_used < oldest_time {
                oldest_time = state.last_used;
                oldest_hash = hash;
            }
        }

        if oldest_hash != 0 {
            self.cache.remove(&oldest_hash);
            self.cache_stats.evictions += 1;
            debug!("Evicted query state with hash: {}", oldest_hash);
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> &CacheStatistics {
        &self.cache_stats
    }

    /// Get recent performance metrics
    pub fn recent_metrics(&self, count: usize) -> Vec<&QueryPerformanceMetrics> {
        let start_idx = self.metrics_history.len().saturating_sub(count);
        self.metrics_history[start_idx..].iter().collect()
    }

    /// Clear all cached states and metrics
    pub fn clear(&mut self) {
        self.cache.clear();
        self.metrics_history.clear();
        self.cache_stats = CacheStatistics::default();
        info!("Query state cache cleared");
    }
}

/// Global query optimization system
pub struct QueryOptimizer {
    cache: Arc<RwLock<QueryStateCache>>,
    parallel_threshold: usize,
    metrics_enabled: bool,
}

impl QueryOptimizer {
    /// Create new query optimizer
    pub fn new(cache_size: usize, metrics_history_size: usize, parallel_threshold: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(QueryStateCache::new(cache_size, metrics_history_size))),
            parallel_threshold,
            metrics_enabled: true,
        }
    }

    /// Optimize a BRP request for performance
    pub async fn optimize_request(&self, request: &BrpRequest) -> Result<OptimizedQuery> {
        let mut cache = self.cache.write().await;
        let cached_state = cache.get_or_build_query_state(request).await?;
        
        let should_use_parallel = cached_state.perf_characteristics.parallel_friendly 
            && cached_state.perf_characteristics.estimated_entity_count >= self.parallel_threshold;
            
        Ok(OptimizedQuery {
            original_request: request.clone(),
            cached_state,
            should_use_parallel,
            optimization_applied: true,
        })
    }

    /// Record execution metrics for continuous optimization
    pub async fn record_execution_metrics(&self, metrics: QueryPerformanceMetrics) {
        if !self.metrics_enabled {
            return;
        }

        let mut cache = self.cache.write().await;
        cache.record_metrics(metrics);
    }

    /// Get optimization statistics
    pub async fn get_stats(&self) -> CacheStatistics {
        let cache = self.cache.read().await;
        cache.stats().clone()
    }

    /// Get recent performance metrics
    pub async fn get_recent_metrics(&self, count: usize) -> Vec<QueryPerformanceMetrics> {
        let cache = self.cache.read().await;
        cache.recent_metrics(count).into_iter().cloned().collect()
    }

    /// Clear optimization cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

/// Optimized query ready for execution
#[derive(Debug)]
pub struct OptimizedQuery {
    pub original_request: BrpRequest,
    pub cached_state: Arc<CachedQueryState>,
    pub should_use_parallel: bool,
    pub optimization_applied: bool,
}

impl OptimizedQuery {
    /// Get performance impact of this query
    pub fn performance_impact(&self) -> &PerformanceImpact {
        &self.cached_state.query_pattern.performance_impact
    }

    /// Get estimated execution time based on history
    pub fn estimated_execution_time_ms(&self) -> u64 {
        self.cached_state.avg_execution_time_ms as u64
    }

    /// Check if this query should be cached
    pub fn should_cache_results(&self) -> bool {
        self.cached_state.perf_characteristics.cacheable
    }

    /// Get recommended batch size for parallel processing
    pub fn recommended_batch_size(&self) -> Option<usize> {
        if !self.should_use_parallel {
            return None;
        }

        let entity_count = self.cached_state.perf_characteristics.estimated_entity_count;
        let cpu_count = num_cpus::get();
        
        Some((entity_count / (cpu_count * 2)).max(100).min(1000))
    }
}

// Make sure the module is thread-safe
unsafe impl Send for QueryStateCache {}
unsafe impl Sync for QueryStateCache {}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_query_state_cache_basic() {
        let mut cache = QueryStateCache::new(10, 100);
        
        let request = BrpRequest::ListComponents;
        let state = cache.get_or_build_query_state(&request).await.unwrap();
        
        assert_eq!(state.usage_count, 1);
        assert!(cache.stats().cache_misses == 1);
        
        // Second access should hit cache
        let state2 = cache.get_or_build_query_state(&request).await.unwrap();
        assert!(cache.stats().cache_hits == 1);
    }
    
    #[tokio::test]
    async fn test_query_optimizer() {
        let optimizer = QueryOptimizer::new(10, 100, 500);
        
        let request = BrpRequest::Query {
            filter: Some(QueryFilter {
                with: Some(vec!["Transform".to_string()]),
                without: None,
                where_clause: None,
            }),
            limit: None,
        };
        
        let optimized = optimizer.optimize_request(&request).await.unwrap();
        assert!(optimized.optimization_applied);
    }
}