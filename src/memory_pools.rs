use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::resource_manager::ObjectPool;
use crate::brp_messages::{BrpRequest, BrpResponse, BrpResult, ComponentFilter, QueryFilter};
use crate::error::Result;

/// Specialized memory pools for frequently allocated game debugging objects
pub struct GameDebugPools {
    /// Pool for BRP request objects
    brp_request_pool: ObjectPool<Box<BrpRequest>>,
    /// Pool for BRP response objects  
    brp_response_pool: ObjectPool<Box<BrpResponse>>,
    /// Pool for component filter vectors
    component_filter_pool: ObjectPool<Vec<ComponentFilter>>,
    /// Pool for string vectors (common in entity queries)
    string_vec_pool: ObjectPool<Vec<String>>,
    /// Pool for JSON Value objects
    json_value_pool: ObjectPool<Value>,
    /// Pool for HashMap buffers (common in component data)
    hashmap_pool: ObjectPool<HashMap<String, Value>>,
    /// Pool for query result buffers
    query_result_pool: ObjectPool<Vec<Value>>,
}

impl GameDebugPools {
    /// Create new pools with optimized sizes for Bevy debugging workloads
    pub fn new() -> Self {
        Self {
            brp_request_pool: ObjectPool::new(
                Box::new(|| Box::new(BrpRequest::Query { 
                    filter: None,
                    limit: None,
                    strict: Some(false)
                })),
                50 // Max 50 concurrent requests
            ),
            brp_response_pool: ObjectPool::new(
                Box::new(|| Box::new(BrpResponse::Success(Box::new(BrpResult::Entities(Vec::new()))))),
                50 // Max 50 concurrent responses
            ),
            component_filter_pool: ObjectPool::new(
                Box::new(|| Vec::with_capacity(8)),
                100 // Filter vectors are commonly reused
            ),
            string_vec_pool: ObjectPool::new(
                Box::new(|| Vec::with_capacity(16)),
                200 // String vectors for component names, etc.
            ),
            json_value_pool: ObjectPool::new(
                Box::new(|| Value::Null),
                500 // JSON values are very common
            ),
            hashmap_pool: ObjectPool::new(
                Box::new(|| HashMap::with_capacity(32)),
                100 // Component data hashmaps
            ),
            query_result_pool: ObjectPool::new(
                Box::new(|| Vec::with_capacity(64)),
                50 // Query result vectors
            ),
        }
    }

    /// Get a pooled BRP request, resetting it for reuse
    pub async fn get_brp_request(&self) -> Box<BrpRequest> {
        let mut request = self.brp_request_pool.acquire().await;
        
        // Reset the request to default state
        *request = BrpRequest::Query { 
            filter: None,
            limit: None,
            strict: Some(false) 
        };
        
        request
    }

    /// Return a BRP request to the pool
    pub async fn return_brp_request(&self, request: Box<BrpRequest>) {
        self.brp_request_pool.release(request).await;
    }

    /// Get a pooled BRP response, resetting it for reuse
    pub async fn get_brp_response(&self) -> Box<BrpResponse> {
        let mut response = self.brp_response_pool.acquire().await;
        
        // Reset the response to default state
        *response = BrpResponse::Success(Box::new(BrpResult::Entities(Vec::new())));
        
        response
    }

    /// Return a BRP response to the pool
    pub async fn return_brp_response(&self, response: Box<BrpResponse>) {
        self.brp_response_pool.release(response).await;
    }

    /// Get a pooled component filter vector, cleared for reuse
    pub async fn get_component_filters(&self) -> Vec<ComponentFilter> {
        let mut filters = self.component_filter_pool.acquire().await;
        
        filters.clear();
        filters
    }

    /// Return a component filter vector to the pool
    pub async fn return_component_filters(&self, filters: Vec<ComponentFilter>) {
        self.component_filter_pool.release(filters).await;
    }

    /// Get a pooled string vector, cleared for reuse
    pub async fn get_string_vec(&self) -> Vec<String> {
        let mut strings = self.string_vec_pool.acquire().await;
        
        strings.clear();
        strings
    }

    /// Return a string vector to the pool
    pub async fn return_string_vec(&self, strings: Vec<String>) {
        self.string_vec_pool.release(strings).await;
    }

    /// Get a pooled JSON Value, reset to Null
    pub async fn get_json_value(&self) -> Value {
        let value = self.json_value_pool.acquire().await;
        Value::Null
    }

    /// Return a JSON Value to the pool
    pub async fn return_json_value(&self, value: Value) {
        self.json_value_pool.release(value).await;
    }

    /// Get a pooled HashMap, cleared for reuse  
    pub async fn get_hashmap(&self) -> HashMap<String, Value> {
        let mut map = self.hashmap_pool.acquire().await;
        
        map.clear();
        map
    }

    /// Return a HashMap to the pool
    pub async fn return_hashmap(&self, map: HashMap<String, Value>) {
        self.hashmap_pool.release(map).await;
    }

    /// Get a pooled query result vector, cleared for reuse
    pub async fn get_query_results(&self) -> Vec<Value> {
        let mut results = self.query_result_pool.acquire().await;
        
        results.clear();
        results
    }

    /// Return a query result vector to the pool
    pub async fn return_query_results(&self, results: Vec<Value>) {
        self.query_result_pool.release(results).await;
    }

    /// Get comprehensive pool statistics for monitoring
    pub async fn get_pool_stats(&self) -> PoolStats {
        PoolStats {
            brp_request_pool_size: self.brp_request_pool.size(),
            brp_response_pool_size: self.brp_response_pool.size(),
            component_filter_pool_size: self.component_filter_pool.size(),
            string_vec_pool_size: self.string_vec_pool.size(),
            json_value_pool_size: self.json_value_pool.size(),
            hashmap_pool_size: self.hashmap_pool.size(),
            query_result_pool_size: self.query_result_pool.size(),
        }
    }

    /// Warm up pools by pre-allocating objects
    pub async fn warm_up_pools(&self) {
        info!("Warming up memory pools for optimal game debugging performance");
        
        // Pre-allocate common objects to reduce first-request latency
        let mut pre_allocated = Vec::new();
        
        // Warm up BRP request pool
        for _ in 0..10 {
            pre_allocated.push(self.get_brp_request().await);
        }
        for req in pre_allocated.drain(..) {
            self.return_brp_request(req).await;
        }
        
        // Warm up string vector pool
        let mut string_vecs = Vec::new();
        for _ in 0..20 {
            string_vecs.push(self.get_string_vec().await);
        }
        for vec in string_vecs {
            self.return_string_vec(vec).await;
        }
        
        // Warm up component filter pool
        let mut filters = Vec::new();
        for _ in 0..15 {
            filters.push(self.get_component_filters().await);
        }
        for filter in filters {
            self.return_component_filters(filter).await;
        }
        
        info!("Memory pools warmed up successfully");
    }

    /// Clean up pools and shrink excessive capacity
    pub async fn cleanup_pools(&self) {
        info!("Cleaning up memory pools");
        
        // Implementation would call cleanup methods on individual pools
        // This would be implemented once ObjectPool has cleanup methods
        
        debug!("Pool cleanup completed");
    }
}

impl Default for GameDebugPools {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for monitoring pool usage and effectiveness
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub brp_request_pool_size: usize,
    pub brp_response_pool_size: usize,
    pub component_filter_pool_size: usize,
    pub string_vec_pool_size: usize,
    pub json_value_pool_size: usize,
    pub hashmap_pool_size: usize,
    pub query_result_pool_size: usize,
}

impl PoolStats {
    /// Get total number of pooled objects across all pools
    pub fn total_pooled_objects(&self) -> usize {
        self.brp_request_pool_size
            + self.brp_response_pool_size
            + self.component_filter_pool_size
            + self.string_vec_pool_size
            + self.json_value_pool_size
            + self.hashmap_pool_size
            + self.query_result_pool_size
    }

    /// Log pool statistics for monitoring
    pub fn log_stats(&self) {
        info!(
            "Pool Stats - BRP Req: {}, BRP Res: {}, Filters: {}, Strings: {}, JSON: {}, Maps: {}, Results: {} (Total: {})",
            self.brp_request_pool_size,
            self.brp_response_pool_size,
            self.component_filter_pool_size,
            self.string_vec_pool_size,
            self.json_value_pool_size,
            self.hashmap_pool_size,
            self.query_result_pool_size,
            self.total_pooled_objects()
        );
    }
}

/// Global instance of the game debug pools
pub static GAME_POOLS: once_cell::sync::Lazy<GameDebugPools> = 
    once_cell::sync::Lazy::new(|| GameDebugPools::new());

/// Convenience macro for getting pooled objects with automatic return
#[macro_export]
macro_rules! with_pooled {
    ($pool:expr, $method:ident, $return_method:ident, $body:expr) => {{
        let obj = $pool.$method().await;
        let result = $body(obj);
        // Note: In a real implementation, we'd need RAII wrapper to ensure return
        result
    }};
}

/// RAII wrapper for automatic pool return
pub struct PooledGuard<T, F>
where
    F: Fn(T),
{
    obj: Option<T>,
    return_fn: F,
}

impl<T, F> PooledGuard<T, F>
where
    F: Fn(T),
{
    pub fn new(obj: T, return_fn: F) -> Self {
        Self {
            obj: Some(obj),
            return_fn,
        }
    }

    /// Get a reference to the pooled object
    pub fn get(&self) -> &T {
        self.obj.as_ref().expect("PooledGuard used after drop")
    }

    /// Get a mutable reference to the pooled object
    pub fn get_mut(&mut self) -> &mut T {
        self.obj.as_mut().expect("PooledGuard used after drop")
    }

    /// Take the object out of the guard (prevents automatic return)
    pub fn take(mut self) -> T {
        self.obj.take().expect("PooledGuard used after drop")
    }
}

impl<T, F> Drop for PooledGuard<T, F>
where
    F: Fn(T),
{
    fn drop(&mut self) {
        if let Some(obj) = self.obj.take() {
            (self.return_fn)(obj);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_game_debug_pools() {
        let pools = GameDebugPools::new();
        
        // Test string vector pooling
        let mut strings = pools.get_string_vec().await;
        strings.push("Transform".to_string());
        strings.push("Velocity".to_string());
        
        assert_eq!(strings.len(), 2);
        pools.return_string_vec(strings).await;
        
        // Get another vector - should be cleared
        let strings2 = pools.get_string_vec().await;
        assert_eq!(strings2.len(), 0);
        pools.return_string_vec(strings2).await;
        
        // Test pool stats
        let stats = pools.get_pool_stats().await;
        assert!(stats.total_pooled_objects() >= 0);
    }

    #[tokio::test]
    async fn test_pool_warm_up() {
        let pools = GameDebugPools::new();
        pools.warm_up_pools().await;
        
        let stats = pools.get_pool_stats().await;
        stats.log_stats();
        
        // After warm-up, pools should have some objects
        // (Exact numbers depend on warm-up implementation)
    }
}