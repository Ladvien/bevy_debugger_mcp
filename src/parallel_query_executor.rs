/*
 * Bevy Debugger MCP Server - Parallel Query Executor
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

use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use futures_util::{stream, StreamExt};
use rayon::prelude::*;
use tracing::{debug, info, warn, error, instrument};

use crate::brp_messages::{BrpRequest, BrpResponse, BrpResult, EntityData, QueryFilter};
use crate::brp_client::BrpClient;
use crate::query_optimization::{OptimizedQuery, QueryPerformanceMetrics, ArchetypeAccessPattern};
use crate::error::{Error, Result};

/// Configuration for parallel query execution
#[derive(Debug, Clone)]
pub struct ParallelExecutionConfig {
    /// Maximum number of concurrent queries
    pub max_concurrent_queries: usize,
    /// Minimum entity count to trigger parallel processing
    pub parallel_threshold: usize,
    /// Batch size for parallel processing
    pub batch_size: usize,
    /// Maximum number of worker threads
    pub max_worker_threads: usize,
    /// Timeout for individual query operations
    pub query_timeout: Duration,
    /// Enable CPU affinity optimization
    pub enable_cpu_affinity: bool,
}

impl Default for ParallelExecutionConfig {
    fn default() -> Self {
        Self {
            max_concurrent_queries: num_cpus::get() * 2,
            parallel_threshold: 1000,
            batch_size: 500,
            max_worker_threads: num_cpus::get(),
            query_timeout: Duration::from_secs(30),
            enable_cpu_affinity: false,
        }
    }
}

/// Statistics for parallel execution
#[derive(Debug, Clone, Default)]
pub struct ParallelExecutionStats {
    /// Total queries executed
    pub total_queries: AtomicUsize,
    /// Queries executed in parallel
    pub parallel_queries: AtomicUsize,
    /// Total entities processed
    pub total_entities_processed: AtomicUsize,
    /// Average speedup achieved with parallel processing
    pub average_speedup: f64,
    /// Peak memory usage in bytes
    pub peak_memory_usage: AtomicUsize,
    /// Number of timeouts
    pub timeout_count: AtomicUsize,
}

/// Parallel query executor for Bevy ECS queries
pub struct ParallelQueryExecutor {
    config: ParallelExecutionConfig,
    stats: Arc<ParallelExecutionStats>,
    semaphore: Arc<Semaphore>,
    thread_pool: Arc<rayon::ThreadPool>,
}

impl ParallelQueryExecutor {
    /// Create new parallel query executor
    pub fn new(config: ParallelExecutionConfig) -> Result<Self> {
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(config.max_worker_threads)
            .thread_name(|i| format!("bevy-query-worker-{}", i))
            .build()
            .map_err(|e| Error::Validation(format!("Failed to create thread pool: {}", e)))?;

        Ok(Self {
            semaphore: Arc::new(Semaphore::new(config.max_concurrent_queries)),
            stats: Arc::new(ParallelExecutionStats::default()),
            config,
            thread_pool: Arc::new(thread_pool),
        })
    }

    /// Execute an optimized query with parallel processing if beneficial
    #[instrument(skip(self, brp_client), fields(parallel = optimized_query.should_use_parallel))]
    pub async fn execute_query(
        &self,
        optimized_query: &OptimizedQuery,
        brp_client: Arc<RwLock<BrpClient>>,
    ) -> Result<QueryExecutionResult> {
        let start_time = Instant::now();
        self.stats.total_queries.fetch_add(1, Ordering::Relaxed);

        // Acquire semaphore permit to limit concurrent queries
        let _permit = self.semaphore
            .acquire()
            .await
            .map_err(|_| Error::Validation("Failed to acquire semaphore permit".to_string()))?;

        if optimized_query.should_use_parallel {
            self.execute_parallel_query(optimized_query, brp_client, start_time).await
        } else {
            self.execute_sequential_query(optimized_query, brp_client, start_time).await
        }
    }

    /// Execute query using parallel processing
    async fn execute_parallel_query(
        &self,
        optimized_query: &OptimizedQuery,
        brp_client: Arc<RwLock<BrpClient>>,
        start_time: Instant,
    ) -> Result<QueryExecutionResult> {
        debug!("Executing query with parallel processing");
        self.stats.parallel_queries.fetch_add(1, Ordering::Relaxed);

        let batch_size = optimized_query.recommended_batch_size()
            .unwrap_or(self.config.batch_size);

        match &optimized_query.original_request {
            BrpRequest::Query { filter, limit } => {
                self.execute_parallel_filtered_query(
                    filter, 
                    *limit, 
                    batch_size, 
                    brp_client, 
                    start_time
                ).await
            }
            BrpRequest::ListEntities { filter } => {
                self.execute_parallel_list_entities(
                    filter, 
                    batch_size, 
                    brp_client, 
                    start_time
                ).await
            }
            _ => {
                // Fall back to sequential for non-parallelizable requests
                self.execute_sequential_query(optimized_query, brp_client, start_time).await
            }
        }
    }

    /// Execute parallel filtered query
    async fn execute_parallel_filtered_query(
        &self,
        filter: &Option<QueryFilter>,
        limit: Option<usize>,
        batch_size: usize,
        brp_client: Arc<RwLock<BrpClient>>,
        start_time: Instant,
    ) -> Result<QueryExecutionResult> {
        // First, get a rough count of entities to determine batching strategy
        let entity_count_request = BrpRequest::ListEntities { 
            filter: filter.clone()
        };

        let entities = {
            let mut client = brp_client.write().await;
            match client.send_request(&entity_count_request).await? {
                BrpResponse::Success(BrpResult::Entities(entities)) => entities,
                BrpResponse::Success(_) => return Err(Error::Brp("Unexpected response type".to_string())),
                BrpResponse::Error(e) => return Err(Error::Brp(format!("BRP error: {}", e.message))),
            }
        };

        let total_entities = entities.len();
        if total_entities < self.config.parallel_threshold {
            // Not enough entities to justify parallel processing
            return self.create_sequential_result(entities, start_time);
        }

        // Process entities in parallel batches
        let batches = self.create_entity_batches(&entities, batch_size);
        let results = self.process_batches_parallel(batches, filter, brp_client).await?;

        // Combine results
        let mut all_entities = Vec::new();
        for batch_result in results {
            all_entities.extend(batch_result);
        }

        // Apply limit if specified
        if let Some(limit) = limit {
            all_entities.truncate(limit);
        }

        self.create_parallel_result(all_entities, total_entities, start_time)
    }

    /// Execute parallel list entities query
    async fn execute_parallel_list_entities(
        &self,
        filter: &Option<QueryFilter>,
        batch_size: usize,
        brp_client: Arc<RwLock<BrpClient>>,
        start_time: Instant,
    ) -> Result<QueryExecutionResult> {
        // Similar to filtered query but without limit handling
        self.execute_parallel_filtered_query(filter, None, batch_size, brp_client, start_time).await
    }

    /// Create entity batches for parallel processing
    fn create_entity_batches(&self, entities: &[EntityData], batch_size: usize) -> Vec<Vec<EntityData>> {
        entities
            .chunks(batch_size)
            .map(|chunk| chunk.to_vec())
            .collect()
    }

    /// Process entity batches in parallel
    async fn process_batches_parallel(
        &self,
        batches: Vec<Vec<EntityData>>,
        filter: &Option<QueryFilter>,
        brp_client: Arc<RwLock<BrpClient>>,
    ) -> Result<Vec<Vec<EntityData>>> {
        let filter = filter.clone();
        let thread_pool = self.thread_pool.clone();
        
        // Process batches using async stream with parallel computation
        let results = stream::iter(batches)
            .map(move |batch| {
                let filter = filter.clone();
                let thread_pool = thread_pool.clone();
                
                tokio::task::spawn_blocking(move || {
                    thread_pool.install(|| {
                        // Use rayon for CPU-intensive filtering within each batch
                        batch.into_par_iter()
                            .filter(|entity| Self::entity_matches_filter(entity, &filter))
                            .collect::<Vec<_>>()
                    })
                })
            })
            .buffer_unordered(self.config.max_concurrent_queries)
            .collect::<Vec<_>>()
            .await;

        // Handle task join results
        let mut processed_batches = Vec::new();
        for result in results {
            match result {
                Ok(batch) => processed_batches.push(batch),
                Err(e) => {
                    warn!("Batch processing failed: {}", e);
                    return Err(Error::Validation(format!("Batch processing failed: {}", e)));
                }
            }
        }

        Ok(processed_batches)
    }

    /// Check if entity matches the filter criteria
    fn entity_matches_filter(entity: &EntityData, filter: &Option<QueryFilter>) -> bool {
        let Some(filter) = filter else {
            return true; // No filter means all entities match
        };

        // Check required components
        if let Some(with_components) = &filter.with {
            let entity_components: std::collections::HashSet<String> = 
                entity.components.keys().cloned().collect();
            
            for required in with_components {
                if !entity_components.contains(required) {
                    return false;
                }
            }
        }

        // Check excluded components
        if let Some(without_components) = &filter.without {
            let entity_components: std::collections::HashSet<String> = 
                entity.components.keys().cloned().collect();
            
            for excluded in without_components {
                if entity_components.contains(excluded) {
                    return false;
                }
            }
        }

        // TODO: Implement where_clause filtering when available
        true
    }

    /// Execute query sequentially (fallback)
    async fn execute_sequential_query(
        &self,
        optimized_query: &OptimizedQuery,
        brp_client: Arc<RwLock<BrpClient>>,
        start_time: Instant,
    ) -> Result<QueryExecutionResult> {
        debug!("Executing query sequentially");

        let response = {
            let mut client = brp_client.write().await;
            client.send_request(&optimized_query.original_request).await?
        };

        match response {
            BrpResponse::Success(result) => {
                let (entities, entity_count) = match result {
                    BrpResult::Entities(entities) => {
                        let count = entities.len();
                        (entities, count)
                    }
                    BrpResult::Entity(entity) => (vec![entity], 1),
                    _ => (vec![], 0),
                };

                self.create_sequential_result(entities, start_time)
            }
            BrpResponse::Error(e) => Err(Error::Brp(format!("BRP error: {}", e.message))),
        }
    }

    /// Create result for sequential execution
    fn create_sequential_result(
        &self,
        entities: Vec<EntityData>,
        start_time: Instant,
    ) -> Result<QueryExecutionResult> {
        let execution_time = start_time.elapsed();
        let entity_count = entities.len();
        
        self.stats.total_entities_processed.fetch_add(entity_count, Ordering::Relaxed);

        Ok(QueryExecutionResult {
            entities,
            entity_count,
            execution_time,
            parallel_processing_used: false,
            speedup_achieved: 1.0,
            memory_usage_bytes: entity_count * 256, // Estimate
            archetype_access_pattern: ArchetypeAccessPattern {
                required_components: Default::default(),
                excluded_components: Default::default(),
                matching_archetypes: 1,
                parallel_friendly: false,
            },
        })
    }

    /// Create result for parallel execution
    fn create_parallel_result(
        &self,
        entities: Vec<EntityData>,
        original_entity_count: usize,
        start_time: Instant,
    ) -> Result<QueryExecutionResult> {
        let execution_time = start_time.elapsed();
        let entity_count = entities.len();
        
        self.stats.total_entities_processed.fetch_add(entity_count, Ordering::Relaxed);

        // Estimate speedup (this would be more accurate with benchmarking)
        let estimated_sequential_time = original_entity_count as f64 * 0.001; // 1ms per 1000 entities
        let speedup = (estimated_sequential_time / execution_time.as_secs_f64()).max(1.0);

        Ok(QueryExecutionResult {
            entities,
            entity_count,
            execution_time,
            parallel_processing_used: true,
            speedup_achieved: speedup,
            memory_usage_bytes: entity_count * 512, // Higher estimate for parallel overhead
            archetype_access_pattern: ArchetypeAccessPattern {
                required_components: Default::default(),
                excluded_components: Default::default(),
                matching_archetypes: (original_entity_count / 1000).max(1),
                parallel_friendly: true,
            },
        })
    }

    /// Get execution statistics
    pub fn stats(&self) -> ParallelExecutionStats {
        ParallelExecutionStats {
            total_queries: AtomicUsize::new(self.stats.total_queries.load(Ordering::Relaxed)),
            parallel_queries: AtomicUsize::new(self.stats.parallel_queries.load(Ordering::Relaxed)),
            total_entities_processed: AtomicUsize::new(self.stats.total_entities_processed.load(Ordering::Relaxed)),
            average_speedup: self.stats.average_speedup,
            peak_memory_usage: AtomicUsize::new(self.stats.peak_memory_usage.load(Ordering::Relaxed)),
            timeout_count: AtomicUsize::new(self.stats.timeout_count.load(Ordering::Relaxed)),
        }
    }

    /// Clear execution statistics
    pub fn clear_stats(&self) {
        self.stats.total_queries.store(0, Ordering::Relaxed);
        self.stats.parallel_queries.store(0, Ordering::Relaxed);
        self.stats.total_entities_processed.store(0, Ordering::Relaxed);
        self.stats.peak_memory_usage.store(0, Ordering::Relaxed);
        self.stats.timeout_count.store(0, Ordering::Relaxed);
    }
}

/// Result of query execution with performance metrics
#[derive(Debug)]
pub struct QueryExecutionResult {
    /// The entities returned by the query
    pub entities: Vec<EntityData>,
    /// Number of entities processed
    pub entity_count: usize,
    /// Total execution time
    pub execution_time: Duration,
    /// Whether parallel processing was used
    pub parallel_processing_used: bool,
    /// Speedup achieved compared to estimated sequential execution
    pub speedup_achieved: f64,
    /// Estimated memory usage in bytes
    pub memory_usage_bytes: usize,
    /// Archetype access pattern used
    pub archetype_access_pattern: ArchetypeAccessPattern,
}

impl QueryExecutionResult {
    /// Convert to performance metrics
    pub fn to_performance_metrics(&self, query: String) -> QueryPerformanceMetrics {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        query.hash(&mut hasher);
        let query_hash = hasher.finish();
        
        QueryPerformanceMetrics {
            query_id: format!("query_{:x}", query_hash),
            query,
            execution_time_ms: self.execution_time.as_millis() as u64,
            entities_processed: self.entity_count,
            components_accessed: self.estimate_components_accessed(),
            cache_hit: false,
            parallel_processing: self.parallel_processing_used,
            memory_usage_bytes: self.memory_usage_bytes,
            timestamp: chrono::Utc::now(),
            archetype_pattern: self.archetype_access_pattern.clone(),
        }
    }

    fn estimate_components_accessed(&self) -> usize {
        // Rough estimate based on entities and typical component count
        self.entity_count * 3
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[tokio::test]
    async fn test_parallel_executor_creation() {
        let config = ParallelExecutionConfig::default();
        let executor = ParallelQueryExecutor::new(config).unwrap();
        
        let stats = executor.stats();
        assert_eq!(stats.total_queries.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_entity_filter_matching() {
        let entity = EntityData {
            entity: 1,
            components: [
                ("Transform".to_string(), serde_json::json!({"x": 1.0})),
                ("Velocity".to_string(), serde_json::json!({"x": 2.0}))
            ].iter().cloned().collect(),
        };

        let filter = Some(QueryFilter {
            with: Some(vec!["Transform".to_string()]),
            without: None,
            where_clause: None,
        });

        assert!(ParallelQueryExecutor::entity_matches_filter(&entity, &filter));

        let filter = Some(QueryFilter {
            with: Some(vec!["Health".to_string()]),
            without: None,
            where_clause: None,
        });

        assert!(!ParallelQueryExecutor::entity_matches_filter(&entity, &filter));
    }

    #[test]
    fn test_batch_creation() {
        let executor = ParallelQueryExecutor::new(ParallelExecutionConfig::default()).unwrap();
        
        let entities = vec![
            EntityData { entity: 1, components: Default::default() },
            EntityData { entity: 2, components: Default::default() },
            EntityData { entity: 3, components: Default::default() },
        ];

        let batches = executor.create_entity_batches(&entities, 2);
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].len(), 2);
        assert_eq!(batches[1].len(), 1);
    }
}