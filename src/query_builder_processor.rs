/// Query Builder Processor for handling ECS query building and validation commands
/// 
/// This processor provides safe query construction capabilities through the debug command system,
/// integrating with the QueryBuilder to offer validation, optimization, and performance estimation.
use crate::brp_messages::{DebugCommand, DebugResponse, ValidatedQuery};
use crate::brp_client::BrpClient;
use crate::debug_command_processor::DebugCommandProcessor;
use crate::query_builder::{QueryBuilder, QueryValidator, QueryOptimizer};
use crate::error::{Error, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Cache for query results and validations
#[derive(Debug, Clone)]
struct QueryCache {
    /// Validated queries by cache key
    validated_queries: HashMap<String, CachedQuery>,
    /// Query patterns that have been suggested for optimization
    optimization_suggestions: HashMap<String, Vec<String>>,
    /// Performance statistics for query patterns
    performance_stats: HashMap<String, QueryPerformanceStats>,
}

#[derive(Debug, Clone)]
struct CachedQuery {
    /// The validated query
    query: ValidatedQuery,
    /// When it was cached
    cached_at: Instant,
    /// TTL for cache entry
    ttl: Duration,
    /// Hit count for this cache entry
    hit_count: usize,
}

#[derive(Debug, Clone)]
struct QueryPerformanceStats {
    /// Total executions
    execution_count: usize,
    /// Average execution time in microseconds
    avg_execution_time_us: u64,
    /// Success rate
    success_rate: f64,
    /// Last updated
    last_updated: Instant,
}

impl QueryCache {
    fn new() -> Self {
        Self {
            validated_queries: HashMap::new(),
            optimization_suggestions: HashMap::new(),
            performance_stats: HashMap::new(),
        }
    }

    /// Get a cached validated query if still valid
    fn get_validated(&mut self, cache_key: &str) -> Option<ValidatedQuery> {
        if let Some(cached) = self.validated_queries.get_mut(cache_key) {
            if cached.cached_at.elapsed() < cached.ttl {
                cached.hit_count += 1;
                return Some(cached.query.clone());
            } else {
                // Remove expired entry
                self.validated_queries.remove(cache_key);
            }
        }
        None
    }

    /// Cache a validated query
    fn put_validated(&mut self, cache_key: String, query: ValidatedQuery) {
        let cached = CachedQuery {
            query,
            cached_at: Instant::now(),
            ttl: Duration::from_secs(300), // 5 minutes TTL
            hit_count: 0,
        };
        self.validated_queries.insert(cache_key, cached);
    }

    /// Get cached optimization suggestions
    fn get_optimization_suggestions(&self, cache_key: &str) -> Option<&Vec<String>> {
        self.optimization_suggestions.get(cache_key)
    }

    /// Cache optimization suggestions
    fn put_optimization_suggestions(&mut self, cache_key: String, suggestions: Vec<String>) {
        self.optimization_suggestions.insert(cache_key, suggestions);
    }

    /// Update performance statistics for a query pattern
    fn update_performance_stats(&mut self, cache_key: String, execution_time_us: u64, success: bool) {
        let stats = self.performance_stats.entry(cache_key).or_insert_with(|| {
            QueryPerformanceStats {
                execution_count: 0,
                avg_execution_time_us: 0,
                success_rate: 1.0,
                last_updated: Instant::now(),
            }
        });

        stats.execution_count += 1;
        
        // Update average execution time (exponential moving average)
        if stats.execution_count == 1 {
            stats.avg_execution_time_us = execution_time_us;
        } else {
            let alpha = 0.2; // Smoothing factor
            stats.avg_execution_time_us = ((1.0 - alpha) * stats.avg_execution_time_us as f64 + 
                                         alpha * execution_time_us as f64) as u64;
        }

        // Update success rate (exponential moving average)
        let success_value = if success { 1.0 } else { 0.0 };
        if stats.execution_count == 1 {
            stats.success_rate = success_value;
        } else {
            let alpha = 0.1; // Smoother for success rate
            stats.success_rate = (1.0 - alpha) * stats.success_rate + alpha * success_value;
        }

        stats.last_updated = Instant::now();
    }

    /// Clean up expired cache entries
    fn cleanup_expired(&mut self) {
        let now = Instant::now();
        
        // Remove expired validated queries
        self.validated_queries.retain(|_, cached| {
            now.duration_since(cached.cached_at) < cached.ttl
        });

        // Remove old optimization suggestions (1 hour TTL)
        let _suggestion_ttl = Duration::from_secs(3600);
        // Note: For simplicity, we're not tracking timestamps for suggestions
        // In a production system, you'd want proper expiration tracking
    }
}

/// Query Builder Processor for debug commands
pub struct QueryBuilderProcessor {
    /// BRP client for communication with Bevy
    brp_client: Arc<RwLock<BrpClient>>,
    /// Query cache for performance optimization
    cache: Arc<RwLock<QueryCache>>,
    /// Query validator
    validator: QueryValidator,
    /// Query optimizer
    optimizer: QueryOptimizer,
}

impl QueryBuilderProcessor {
    /// Create a new query builder processor
    pub fn new(brp_client: Arc<RwLock<BrpClient>>) -> Self {
        Self {
            brp_client,
            cache: Arc::new(RwLock::new(QueryCache::new())),
            validator: QueryValidator::new(),
            optimizer: QueryOptimizer::new(),
        }
    }

    /// Build a query from JSON parameters
    async fn build_query_from_params(&self, params: &Value) -> Result<QueryBuilder> {
        let mut builder = QueryBuilder::new();

        // Handle 'with' components
        if let Some(with_components) = params.get("with").and_then(|v| v.as_array()) {
            let components: Result<Vec<String>> = with_components
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(|s| s.to_string())
                        .ok_or_else(|| Error::Validation("Invalid component name in 'with'".to_string()))
                })
                .collect();
            builder = builder.with_components(components?);
        }

        // Handle 'without' components
        if let Some(without_components) = params.get("without").and_then(|v| v.as_array()) {
            let components: Result<Vec<String>> = without_components
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(|s| s.to_string())
                        .ok_or_else(|| Error::Validation("Invalid component name in 'without'".to_string()))
                })
                .collect();
            builder = builder.without_components(components?);
        }

        // Handle limit
        if let Some(limit) = params.get("limit").and_then(|v| v.as_u64()) {
            builder = builder.limit(limit as usize);
        }

        // Handle offset
        if let Some(offset) = params.get("offset").and_then(|v| v.as_u64()) {
            builder = builder.offset(offset as usize);
        }

        Ok(builder)
    }

    /// Validate a query and return validation result
    async fn handle_validate_query(&self, params: Value) -> Result<DebugResponse> {
        debug!("Handling validate query request: {:?}", params);
        
        let mut builder = self.build_query_from_params(&params).await?;
        let cache_key = builder.cache_key();
        
        // Check cache first
        {
            let mut cache = self.cache.write().await;
            if let Some(cached_query) = cache.get_validated(&cache_key) {
                debug!("Returning cached validated query for key: {}", cache_key);
                return Ok(DebugResponse::QueryValidation {
                    valid: true,
                    query: Some(cached_query),
                    errors: Vec::new(),
                    suggestions: cache.get_optimization_suggestions(&cache_key)
                        .cloned()
                        .unwrap_or_default(),
                });
            }
        }

        // Validate the query
        let validation_start = Instant::now();
        let validation_result = self.validator.validate(&builder);
        let validation_time = validation_start.elapsed();

        debug!("Query validation took: {:?}", validation_time);

        match validation_result {
            Ok(validated_query) => {
                // Get optimization suggestions
                let suggestions = self.optimizer.analyze(&builder);
                
                // Cache the results
                {
                    let mut cache = self.cache.write().await;
                    cache.put_validated(cache_key.clone(), validated_query.clone());
                    cache.put_optimization_suggestions(cache_key.clone(), suggestions.clone());
                    cache.update_performance_stats(
                        cache_key, 
                        validation_time.as_micros() as u64, 
                        true
                    );
                }

                info!("Successfully validated query with {} suggestions", suggestions.len());

                Ok(DebugResponse::QueryValidation {
                    valid: true,
                    query: Some(validated_query),
                    errors: Vec::new(),
                    suggestions,
                })
            }
            Err(error) => {
                // Get suggestions even for invalid queries
                let suggestions = self.optimizer.analyze(&builder);
                
                // Update performance stats for failed validation
                {
                    let mut cache = self.cache.write().await;
                    cache.update_performance_stats(
                        cache_key, 
                        validation_time.as_micros() as u64, 
                        false
                    );
                }

                warn!("Query validation failed: {}", error);

                Ok(DebugResponse::QueryValidation {
                    valid: false,
                    query: None,
                    errors: vec![error.to_string()],
                    suggestions,
                })
            }
        }
    }

    /// Estimate query performance cost
    async fn handle_estimate_cost(&self, params: Value) -> Result<DebugResponse> {
        debug!("Handling estimate cost request: {:?}", params);
        
        let mut builder = self.build_query_from_params(&params).await?;
        let cost = builder.estimate_cost();
        let performance_budget_exceeded = cost.estimated_time_us > crate::query_builder::QUERY_PERFORMANCE_BUDGET_US;
        let suggestions = self.optimizer.analyze(&builder);
        
        debug!("Estimated query cost: {:?}", cost);

        Ok(DebugResponse::QueryCost {
            cost,
            performance_budget_exceeded,
            suggestions,
        })
    }

    /// Get optimization suggestions for a query
    async fn handle_get_suggestions(&self, params: Value) -> Result<DebugResponse> {
        debug!("Handling get suggestions request: {:?}", params);
        
        let builder = self.build_query_from_params(&params).await?;
        let suggestions = self.optimizer.analyze(&builder);
        
        debug!("Generated {} optimization suggestions", suggestions.len());

        Ok(DebugResponse::QuerySuggestions {
            suggestions,
            query_complexity: self.calculate_query_complexity(&builder),
        })
    }

    /// Build and execute a query using the QueryBuilder
    async fn handle_build_and_execute(&self, params: Value) -> Result<DebugResponse> {
        debug!("Handling build and execute request: {:?}", params);
        
        let builder = self.build_query_from_params(&params).await?;
        
        // Build the command
        let command = builder.build()?;
        
        // Execute via BRP client
        let execution_start = Instant::now();
        let mut client = self.brp_client.write().await;
        
        if !client.is_connected() {
            return Err(Error::Connection("BRP client not connected".to_string()));
        }

        // Convert to BRP request
        let brp_request = match command {
            DebugCommand::ExecuteQuery { query, limit, offset: _ } => {
                crate::brp_messages::BrpRequest::Query {
                    filter: Some(query.filter),
                    limit,
                    strict: Some(false),
                }
            }
            _ => return Err(Error::DebugError("Invalid command type".to_string())),
        };

        match client.send_request(&brp_request).await {
            Ok(response) => {
                let execution_time = execution_start.elapsed();
                debug!("Query executed in {:?}", execution_time);
                
                // Update performance statistics
                {
                    let mut cache = self.cache.write().await;
                    let mut builder_for_key = self.build_query_from_params(&params).await?;
                    let cache_key = builder_for_key.cache_key();
                    cache.update_performance_stats(
                        cache_key, 
                        execution_time.as_micros() as u64, 
                        true
                    );
                }

                Ok(DebugResponse::QueryExecution {
                    success: true,
                    result: Some(Box::new(response)),
                    execution_time_us: execution_time.as_micros() as u64,
                    entities_processed: None, // Would be filled by actual execution
                })
            }
            Err(error) => {
                let execution_time = execution_start.elapsed();
                warn!("Query execution failed: {}", error);

                // Update performance statistics for failure
                {
                    let mut cache = self.cache.write().await;
                    let mut builder_for_key = self.build_query_from_params(&params).await?;
                    let cache_key = builder_for_key.cache_key();
                    cache.update_performance_stats(
                        cache_key, 
                        execution_time.as_micros() as u64, 
                        false
                    );
                }

                Ok(DebugResponse::QueryExecution {
                    success: false,
                    result: None,
                    execution_time_us: execution_time.as_micros() as u64,
                    entities_processed: None,
                })
            }
        }
    }

    /// Calculate query complexity score
    fn calculate_query_complexity(&self, builder: &QueryBuilder) -> u32 {
        let mut complexity = 0u32;
        
        // Add complexity for each component
        complexity += (builder.get_with_components().len() * 2) as u32;
        complexity += (builder.get_without_components().len() * 1) as u32;
        complexity += (builder.get_component_filters().len() * 3) as u32;
        
        // Reduce complexity if query has good selectivity (limit specified)
        if builder.get_limit().is_some() {
            complexity = complexity.saturating_sub(5);
        }

        complexity
    }

    /// Clean up cache periodically
    pub async fn cleanup_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.cleanup_expired();
        debug!("Cache cleanup completed");
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> Value {
        let cache = self.cache.read().await;
        serde_json::json!({
            "validated_queries_cached": cache.validated_queries.len(),
            "optimization_suggestions_cached": cache.optimization_suggestions.len(),
            "performance_stats_tracked": cache.performance_stats.len(),
            "total_hit_count": cache.validated_queries.values().map(|c| c.hit_count).sum::<usize>(),
        })
    }
}

#[async_trait]
impl DebugCommandProcessor for QueryBuilderProcessor {
    async fn process(&self, command: DebugCommand) -> Result<DebugResponse> {
        match command {
            DebugCommand::ValidateQuery { params } => {
                self.handle_validate_query(params).await
            }
            DebugCommand::EstimateCost { params } => {
                self.handle_estimate_cost(params).await
            }
            DebugCommand::GetQuerySuggestions { params } => {
                self.handle_get_suggestions(params).await
            }
            DebugCommand::BuildAndExecuteQuery { params } => {
                self.handle_build_and_execute(params).await
            }
            _ => Err(Error::DebugError(
                "Unsupported command for query builder processor".to_string(),
            )),
        }
    }

    async fn validate(&self, command: &DebugCommand) -> Result<()> {
        match command {
            DebugCommand::ValidateQuery { params } |
            DebugCommand::EstimateCost { params } |
            DebugCommand::GetQuerySuggestions { params } |
            DebugCommand::BuildAndExecuteQuery { params } => {
                // Validate that params is a proper object
                if !params.is_object() {
                    return Err(Error::Validation("Parameters must be an object".to_string()));
                }

                // Basic validation - more specific validation happens in individual handlers
                if let Some(with_components) = params.get("with") {
                    if !with_components.is_array() {
                        return Err(Error::Validation("'with' parameter must be an array".to_string()));
                    }
                }

                if let Some(without_components) = params.get("without") {
                    if !without_components.is_array() {
                        return Err(Error::Validation("'without' parameter must be an array".to_string()));
                    }
                }

                Ok(())
            }
            _ => Err(Error::DebugError("Command not supported by query builder processor".to_string())),
        }
    }

    fn estimate_processing_time(&self, command: &DebugCommand) -> Duration {
        match command {
            DebugCommand::ValidateQuery { .. } => Duration::from_millis(10),
            DebugCommand::EstimateCost { .. } => Duration::from_millis(5),
            DebugCommand::GetQuerySuggestions { .. } => Duration::from_millis(15),
            DebugCommand::BuildAndExecuteQuery { .. } => Duration::from_millis(100), // Actual BRP execution
            _ => Duration::from_millis(1),
        }
    }

    fn supports_command(&self, command: &DebugCommand) -> bool {
        matches!(
            command,
            DebugCommand::ValidateQuery { .. } |
            DebugCommand::EstimateCost { .. } |
            DebugCommand::GetQuerySuggestions { .. } |
            DebugCommand::BuildAndExecuteQuery { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use serde_json::json;

    async fn create_test_processor() -> QueryBuilderProcessor {
        let mut config = Config::default();
        config.bevy_brp_host = "localhost".to_string();
        config.bevy_brp_port = 15702;
        config.mcp_port = 3000;
        let brp_client = Arc::new(RwLock::new(crate::brp_client::BrpClient::new(&config)));
        QueryBuilderProcessor::new(brp_client)
    }

    #[tokio::test]
    async fn test_build_query_from_params() {
        let processor = create_test_processor().await;
        
        let params = json!({
            "with": ["Transform", "Velocity"],
            "without": ["Camera"],
            "limit": 10,
            "offset": 5
        });

        let builder = processor.build_query_from_params(&params).await.unwrap();
        
        assert_eq!(builder.get_with_components(), &["Transform", "Velocity"]);
        assert_eq!(builder.get_without_components(), &["Camera"]);
        assert_eq!(builder.get_limit(), Some(10));
        assert_eq!(builder.get_offset(), Some(5));
    }

    #[tokio::test]
    async fn test_supports_query_commands() {
        let processor = create_test_processor().await;
        
        let validate_cmd = DebugCommand::ValidateQuery { 
            params: json!({"with": ["Transform"]})
        };
        let cost_cmd = DebugCommand::EstimateCost { 
            params: json!({"with": ["Transform"]})
        };
        
        assert!(processor.supports_command(&validate_cmd));
        assert!(processor.supports_command(&cost_cmd));
    }

    #[tokio::test]
    async fn test_validate_query_success() {
        let processor = create_test_processor().await;
        
        let params = json!({
            "with": ["Transform", "Velocity"]
        });

        let result = processor.handle_validate_query(params).await;
        assert!(result.is_ok());

        match result.unwrap() {
            DebugResponse::QueryValidation { valid, query, errors, .. } => {
                assert!(valid);
                assert!(query.is_some());
                assert!(errors.is_empty());
            }
            _ => panic!("Expected QueryValidation response"),
        }
    }

    #[tokio::test]
    async fn test_validate_query_invalid_component() {
        let processor = create_test_processor().await;
        
        let params = json!({
            "with": ["UnknownComponent"]
        });

        let result = processor.handle_validate_query(params).await;
        assert!(result.is_ok());

        match result.unwrap() {
            DebugResponse::QueryValidation { valid, query, errors, .. } => {
                assert!(!valid);
                assert!(query.is_none());
                assert!(!errors.is_empty());
                assert!(errors[0].contains("Unknown component"));
            }
            _ => panic!("Expected QueryValidation response"),
        }
    }

    #[tokio::test]
    async fn test_estimate_cost() {
        let processor = create_test_processor().await;
        
        let params = json!({
            "with": ["Transform"],
            "limit": 100
        });

        let result = processor.handle_estimate_cost(params).await;
        assert!(result.is_ok());

        match result.unwrap() {
            DebugResponse::QueryCost { cost, performance_budget_exceeded, .. } => {
                assert!(cost.estimated_entities > 0);
                assert!(cost.estimated_time_us > 0);
                // With limit of 100, should be within budget
                assert!(!performance_budget_exceeded || cost.estimated_entities <= 100);
            }
            _ => panic!("Expected QueryCost response"),
        }
    }

    #[tokio::test]
    async fn test_cache_functionality() {
        let processor = create_test_processor().await;
        
        let params = json!({
            "with": ["Transform", "Velocity"]
        });

        // First request should miss cache
        let result1 = processor.handle_validate_query(params.clone()).await;
        assert!(result1.is_ok());

        // Second identical request should hit cache
        let result2 = processor.handle_validate_query(params).await;
        assert!(result2.is_ok());

        // Check cache stats
        let stats = processor.get_cache_stats().await;
        assert!(stats["validated_queries_cached"].as_u64().unwrap() > 0);
    }
}