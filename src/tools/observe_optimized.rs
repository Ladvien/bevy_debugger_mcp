/*
 * Bevy Debugger MCP Server - Optimized Observe Tool
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

use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn, instrument};

use crate::brp_client::BrpClient;
use crate::brp_messages::{BrpResponse, BrpResult, EntityData};
use crate::error::{Error, Result};
use crate::query_parser::{QueryCache, QueryMetrics, QueryParser, RegexQueryParser};
use crate::query_optimization::{QueryOptimizer, QueryPerformanceMetrics};
use crate::parallel_query_executor::{ParallelQueryExecutor, ParallelExecutionConfig, QueryExecutionResult};
use crate::state_diff::{FuzzyCompareConfig, GameRules, StateDiff, StateDiffResult, StateSnapshot};

/// Optimized observe state with performance tracking
pub struct OptimizedObserveState {
    parser: RegexQueryParser,
    cache: QueryCache,
    diff_engine: StateDiff,
    last_snapshot: Option<StateSnapshot>,
    snapshots_history: Vec<StateSnapshot>,
    max_history_size: usize,
    query_optimizer: QueryOptimizer,
    parallel_executor: ParallelQueryExecutor,
    performance_metrics: Vec<QueryPerformanceMetrics>,
    max_metrics_history: usize,
}

impl OptimizedObserveState {
    /// Create new optimized observe state
    /// 
    /// # Errors
    /// Returns error if initialization fails
    pub fn new() -> Result<Self> {
        let query_optimizer = QueryOptimizer::new(100, 1000, 1000);
        let parallel_config = ParallelExecutionConfig {
            max_concurrent_queries: num_cpus::get(),
            parallel_threshold: 500,
            batch_size: 250,
            max_worker_threads: num_cpus::get(),
            query_timeout: std::time::Duration::from_secs(30),
            enable_cpu_affinity: false,
        };
        let parallel_executor = ParallelQueryExecutor::new(parallel_config)?;

        Ok(Self {
            parser: RegexQueryParser::new()?,
            cache: QueryCache::new(300), // 5 minute TTL
            diff_engine: StateDiff::new(),
            last_snapshot: None,
            snapshots_history: Vec::new(),
            max_history_size: 10,
            query_optimizer,
            parallel_executor,
            performance_metrics: Vec::new(),
            max_metrics_history: 100,
        })
    }

    /// Create with custom configuration
    /// 
    /// # Errors
    /// Returns error if initialization fails
    pub fn with_config(
        fuzzy_config: FuzzyCompareConfig,
        game_rules: GameRules,
        parallel_config: ParallelExecutionConfig,
    ) -> Result<Self> {
        let query_optimizer = QueryOptimizer::new(100, 1000, parallel_config.parallel_threshold);
        let parallel_executor = ParallelQueryExecutor::new(parallel_config)?;

        Ok(Self {
            parser: RegexQueryParser::new()?,
            cache: QueryCache::new(300),
            diff_engine: StateDiff::with_config(fuzzy_config, game_rules),
            last_snapshot: None,
            snapshots_history: Vec::new(),
            max_history_size: 10,
            query_optimizer,
            parallel_executor,
            performance_metrics: Vec::new(),
            max_metrics_history: 100,
        })
    }

    /// Record performance metrics
    pub fn record_metrics(&mut self, metrics: QueryPerformanceMetrics) {
        self.performance_metrics.push(metrics);
        if self.performance_metrics.len() > self.max_metrics_history {
            self.performance_metrics.remove(0);
        }
    }

    /// Get recent performance statistics
    pub fn get_performance_stats(&self) -> Value {
        if self.performance_metrics.is_empty() {
            return json!({
                "total_queries": 0,
                "avg_execution_time_ms": 0,
                "parallel_queries": 0,
                "cache_hit_rate": 0.0
            });
        }

        let total_queries = self.performance_metrics.len();
        let parallel_queries = self.performance_metrics.iter()
            .filter(|m| m.parallel_processing)
            .count();
        
        let avg_execution_time = self.performance_metrics.iter()
            .map(|m| m.execution_time_ms as f64)
            .sum::<f64>() / total_queries as f64;

        let cache_hits = self.performance_metrics.iter()
            .filter(|m| m.cache_hit)
            .count();
        let cache_hit_rate = cache_hits as f64 / total_queries as f64;

        json!({
            "total_queries": total_queries,
            "avg_execution_time_ms": avg_execution_time,
            "parallel_queries": parallel_queries,
            "cache_hit_rate": cache_hit_rate,
            "recent_metrics": self.performance_metrics.iter().rev().take(10).collect::<Vec<_>>()
        })
    }

    /// Add a new snapshot and maintain history
    pub fn add_snapshot(&mut self, entities: Vec<EntityData>) -> StateSnapshot {
        let snapshot = self.diff_engine.create_snapshot(entities);

        // Add to history
        self.snapshots_history.push(snapshot.clone());
        if self.snapshots_history.len() > self.max_history_size {
            self.snapshots_history.remove(0);
        }

        self.last_snapshot = Some(snapshot.clone());
        snapshot
    }

    /// Get diff against last snapshot
    pub fn diff_against_last(&self, current_snapshot: &StateSnapshot) -> Option<StateDiffResult> {
        self.last_snapshot
            .as_ref()
            .map(|last| self.diff_engine.diff_snapshots(last, current_snapshot))
    }

    /// Clear all state and metrics
    pub fn clear(&mut self) {
        self.snapshots_history.clear();
        self.last_snapshot = None;
        self.performance_metrics.clear();
    }
}

// Global optimized observe state
static OPTIMIZED_OBSERVE_STATE: std::sync::OnceLock<Arc<RwLock<OptimizedObserveState>>> = std::sync::OnceLock::new();

fn get_optimized_observe_state() -> Arc<RwLock<OptimizedObserveState>> {
    OPTIMIZED_OBSERVE_STATE
        .get_or_init(|| Arc::new(RwLock::new(
            OptimizedObserveState::new().expect("Default optimized observe state should initialize successfully")
        )))
        .clone()
}

/// Optimized observe handler with performance tracking
///
/// # Errors
/// Returns error if query parsing fails, BRP communication fails, or response formatting fails
#[instrument(skip(brp_client), fields(query = %arguments.get("query").and_then(|q| q.as_str()).unwrap_or("unknown")))]
pub async fn handle_optimized(arguments: Value, brp_client: Arc<RwLock<BrpClient>>) -> Result<Value> {
    debug!("Optimized observe tool called with arguments: {}", arguments);

    let query = arguments
        .get("query")
        .and_then(|q| q.as_str())
        .unwrap_or("list all entities");

    let diff_mode = arguments
        .get("diff")
        .and_then(|d| d.as_bool())
        .unwrap_or(false);

    let detailed = arguments
        .get("detailed")
        .and_then(|d| d.as_bool())
        .unwrap_or(false);

    info!(
        "Processing optimized observe query: {} (diff_mode: {}, detailed: {})",
        query, diff_mode, detailed
    );

    let start_time = Instant::now();
    let state = get_optimized_observe_state();

    // Handle diff clear command
    if diff_mode && query == "clear" {
        let mut state_guard = state.write().await;
        state_guard.clear();
        return Ok(json!({
            "message": "Diff history and metrics cleared",
            "diff_mode": true,
            "action": "clear"
        }));
    }

    let (brp_request, semantic_info) = {
        let state_guard = state.read().await;
        
        // Check cache first (skip cache for diff mode)
        if !diff_mode {
            if let Some((cached_result, entity_count)) = state_guard.cache.get(query) {
                info!("Cache hit for optimized query: {}", query);
                let metrics = QueryMetrics {
                    query: query.to_string(),
                    execution_time_ms: start_time.elapsed().as_millis() as u64,
                    entity_count,
                    cache_hit: true,
                    timestamp: chrono::Utc::now(),
                };

                return Ok(json!({
                    "result": cached_result,
                    "metadata": {
                        "query": metrics.query,
                        "execution_time_ms": metrics.execution_time_ms,
                        "entity_count": metrics.entity_count,
                        "cache_hit": metrics.cache_hit,
                        "timestamp": metrics.timestamp.to_rfc3339(),
                        "optimization_used": true,
                        "parallel_processing": false,
                    }
                }));
            }
        }

        // Parse query
        match state_guard.parser.parse_semantic(query) {
            Ok(semantic_result) => {
                info!("Parsed as semantic query with {} explanations", semantic_result.explanations.len());
                (semantic_result.request.clone(), Some(semantic_result))
            }
            Err(_) => {
                match state_guard.parser.parse(query) {
                    Ok(request) => (request, None),
                    Err(e) => {
                        warn!("Query parsing failed: {}", e);
                        return Ok(json!({
                            "error": "Query parsing failed",
                            "message": e.to_string(),
                            "help": state_guard.parser.help()
                        }));
                    }
                }
            }
        }
    };

    // Check BRP connection
    let client_connected = {
        let client = brp_client.read().await;
        client.is_connected()
    };

    if !client_connected {
        warn!("BRP client not connected");
        return Ok(json!({
            "error": "BRP client not connected",
            "message": "Cannot execute query - not connected to Bevy game",
            "brp_connected": false
        }));
    }

    // Optimize query for performance
    let optimized_query = {
        let state_guard = state.read().await;
        match state_guard.query_optimizer.optimize_request(&brp_request).await {
            Ok(optimized) => optimized,
            Err(e) => {
                warn!("Query optimization failed, falling back to standard execution: {}", e);
                // Continue with standard execution
                return execute_fallback_query(&brp_request, brp_client, query, start_time, diff_mode).await;
            }
        }
    };

    // Execute optimized query
    let execution_result = {
        let state_guard = state.read().await;
        match state_guard.parallel_executor.execute_query(&optimized_query, brp_client.clone()).await {
            Ok(result) => result,
            Err(e) => {
                error!("Optimized query execution failed: {}", e);
                return execute_fallback_query(&brp_request, brp_client, query, start_time, diff_mode).await;
            }
        }
    };

    // Record performance metrics
    let performance_metrics = execution_result.to_performance_metrics(query.to_string());
    
    {
        let mut state_guard = state.write().await;
        state_guard.record_metrics(performance_metrics.clone());
        
        // Record in query optimizer for continuous improvement
        let _ = state_guard.query_optimizer.record_execution_metrics(performance_metrics.clone()).await;
    }

    // Handle diff mode
    let diff_result = if diff_mode {
        let mut state_guard = state.write().await;
        let current_snapshot = state_guard.add_snapshot(execution_result.entities.clone());
        state_guard.diff_against_last(&current_snapshot)
    } else {
        None
    };

    // Cache results if beneficial
    if optimized_query.should_cache_results() && !diff_mode {
        let state_guard = state.read().await;
        let result_json = serde_json::to_value(&execution_result.entities).map_err(Error::Json)?;
        state_guard.cache.set(query.to_string(), result_json.clone(), execution_result.entity_count);
    }

    // Build response
    let mut response = json!({
        "result": {
            "entities": execution_result.entities,
            "entity_count": execution_result.entity_count
        },
        "metadata": {
            "query": query,
            "execution_time_ms": execution_result.execution_time.as_millis() as u64,
            "entity_count": execution_result.entity_count,
            "cache_hit": false,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "optimization_used": true,
            "parallel_processing": execution_result.parallel_processing_used,
            "speedup_achieved": execution_result.speedup_achieved,
            "memory_usage_bytes": execution_result.memory_usage_bytes,
            "diff_mode": diff_mode,
        },
        "performance": performance_metrics
    });

    // Add diff information if available
    if let Some(diff_result) = diff_result {
        let state_guard = state.read().await;
        let grouped_changes = state_guard.diff_engine.group_changes(&diff_result.changes);

        response["diff"] = json!({
            "summary": diff_result.summary,
            "changes": diff_result.changes,
            "grouped_changes": grouped_changes,
            "before_timestamp": diff_result.before_snapshot.timestamp.to_rfc3339(),
            "after_timestamp": diff_result.after_snapshot.timestamp.to_rfc3339(),
            "colored_output": diff_result.format_colored(),
            "unexpected_changes_count": diff_result.unexpected_changes().len(),
        });
    }

    // Add semantic analysis if available
    if let Some(semantic_result) = semantic_info {
        response["semantic_analysis"] = json!({
            "explanations": semantic_result.explanations,
            "suggestions": semantic_result.suggestions,
            "is_semantic_query": true
        });
    } else {
        response["semantic_analysis"] = json!({
            "is_semantic_query": false
        });
    }

    // Add detailed performance info if requested
    if detailed {
        let state_guard = state.read().await;
        response["detailed_performance"] = state_guard.get_performance_stats();
    }

    info!(
        "Optimized query '{}' completed in {}ms, {} entities processed (parallel: {}, speedup: {:.2}x)",
        query,
        execution_result.execution_time.as_millis(),
        execution_result.entity_count,
        execution_result.parallel_processing_used,
        execution_result.speedup_achieved
    );

    Ok(response)
}

/// Fallback to standard query execution if optimization fails
async fn execute_fallback_query(
    brp_request: &crate::brp_messages::BrpRequest,
    brp_client: Arc<RwLock<BrpClient>>,
    query: &str,
    start_time: Instant,
    diff_mode: bool,
) -> Result<Value> {
    warn!("Falling back to standard query execution");
    
    let brp_response = {
        let mut client = brp_client.write().await;
        match client.send_request(brp_request).await {
            Ok(response) => response,
            Err(e) => {
                error!("BRP request failed: {}", e);
                return Ok(json!({
                    "error": "BRP request failed",
                    "message": e.to_string(),
                    "query": query
                }));
            }
        }
    };

    let (result_json, entity_count) = match brp_response {
        BrpResponse::Success(result) => {
            let entity_count = match result.as_ref() {
                BrpResult::Entities(entities) => entities.len(),
                BrpResult::Entity(_) => 1,
                BrpResult::ComponentTypes(types) => types.len(),
                _ => 0,
            };
            let result_json = serde_json::to_value(&result).map_err(Error::Json)?;
            (result_json, entity_count)
        }
        BrpResponse::Error(error) => {
            warn!("BRP returned error: {}", error);
            return Ok(json!({
                "error": "BRP error",
                "code": error.code,
                "message": error.message,
                "details": error.details
            }));
        }
    };

    let execution_time = start_time.elapsed().as_millis() as u64;

    Ok(json!({
        "result": result_json,
        "metadata": {
            "query": query,
            "execution_time_ms": execution_time,
            "entity_count": entity_count,
            "cache_hit": false,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "optimization_used": false,
            "parallel_processing": false,
            "fallback_execution": true,
            "diff_mode": diff_mode,
        }
    }))
}

/// Get performance statistics for the optimized observe system
pub async fn get_performance_stats() -> Value {
    let state = get_optimized_observe_state();
    let state_guard = state.read().await;
    state_guard.get_performance_stats()
}

/// Clear performance metrics and cache
pub async fn clear_performance_data() {
    let state = get_optimized_observe_state();
    let mut state_guard = state.write().await;
    state_guard.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[tokio::test]
    async fn test_optimized_observe_creation() {
        let state = OptimizedObserveState::new().unwrap();
        assert_eq!(state.performance_metrics.len(), 0);
    }

    #[tokio::test]
    async fn test_performance_stats() {
        let mut state = OptimizedObserveState::new().unwrap();
        
        let metrics = QueryPerformanceMetrics {
            query_id: "test_query".to_string(),
            query: "list all entities".to_string(),
            execution_time_ms: 100,
            entities_processed: 1000,
            components_accessed: 3000,
            cache_hit: false,
            parallel_processing: true,
            memory_usage_bytes: 64000,
            timestamp: chrono::Utc::now(),
            archetype_pattern: crate::query_optimization::ArchetypeAccessPattern {
                required_components: Default::default(),
                excluded_components: Default::default(),
                matching_archetypes: 5,
                parallel_friendly: true,
            },
        };
        
        state.record_metrics(metrics);
        let stats = state.get_performance_stats();
        
        assert_eq!(stats["total_queries"], 1);
        assert_eq!(stats["parallel_queries"], 1);
    }
}