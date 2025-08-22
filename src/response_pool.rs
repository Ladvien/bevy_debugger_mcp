use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::resource_manager::ObjectPool;
use crate::error::Result;

/// Pre-allocated response buffer for memory efficiency
#[derive(Debug, Clone)]
pub struct ResponseBuffer {
    /// The JSON data buffer
    pub data: Vec<u8>,
    /// Current capacity
    pub capacity: usize,
    /// Creation timestamp for tracking
    pub created_at: std::time::Instant,
}

impl ResponseBuffer {
    /// Create a new response buffer with initial capacity
    pub fn new(initial_capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(initial_capacity),
            capacity: initial_capacity,
            created_at: std::time::Instant::now(),
        }
    }
    
    /// Clear the buffer and prepare for reuse
    /// Security: Ensures sensitive data is properly cleared
    pub fn clear(&mut self) {
        // Zero out buffer contents for security
        self.data.fill(0);
        self.data.clear();
        // Don't shrink capacity unless it's excessive
        if self.data.capacity() > self.capacity * 4 {
            self.data.shrink_to(self.capacity);
        }
    }
    
    /// Serialize a JSON value into this buffer
    pub fn serialize_json(&mut self, value: &Value) -> Result<&[u8]> {
        self.clear();
        serde_json::to_writer(&mut self.data, value)
            .map_err(|e| crate::error::Error::Validation(format!("Failed to serialize JSON: {}", e)))?;
        Ok(&self.data)
    }
    
    /// Get the current size of the buffer
    pub fn len(&self) -> usize {
        self.data.len()
    }
    
    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    
    /// Get buffer utilization percentage
    pub fn utilization(&self) -> f32 {
        if self.capacity == 0 {
            return 0.0;
        }
        self.data.len() as f32 / self.capacity as f32
    }
}

/// Configuration for response pooling
#[derive(Debug, Clone)]
pub struct ResponsePoolConfig {
    /// Maximum number of small response buffers (< 1KB)
    pub max_small_buffers: usize,
    /// Maximum number of medium response buffers (1KB - 32KB)
    pub max_medium_buffers: usize,
    /// Maximum number of large response buffers (32KB - 512KB)
    pub max_large_buffers: usize,
    /// Initial capacity for small buffers
    pub small_buffer_capacity: usize,
    /// Initial capacity for medium buffers
    pub medium_buffer_capacity: usize,
    /// Initial capacity for large buffers
    pub large_buffer_capacity: usize,
    /// Enable buffer utilization tracking
    pub track_utilization: bool,
    /// Cleanup interval for unused buffers
    pub cleanup_interval: std::time::Duration,
}

impl Default for ResponsePoolConfig {
    fn default() -> Self {
        Self {
            max_small_buffers: 100,
            max_medium_buffers: 50,
            max_large_buffers: 20,
            small_buffer_capacity: 1024,      // 1KB
            medium_buffer_capacity: 32768,    // 32KB
            large_buffer_capacity: 524288,    // 512KB
            track_utilization: true,
            cleanup_interval: std::time::Duration::from_secs(60),
        }
    }
}

/// Statistics about response pool usage
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResponsePoolStats {
    pub small_buffers_allocated: u64,
    pub medium_buffers_allocated: u64,
    pub large_buffers_allocated: u64,
    pub small_buffers_pooled: usize,
    pub medium_buffers_pooled: usize,
    pub large_buffers_pooled: usize,
    pub total_serializations: u64,
    pub total_bytes_serialized: u64,
    pub average_response_size: f64,
    pub pool_hit_rate: f64,
    pub memory_savings_bytes: u64,
}

/// Response pooling system for memory optimization
pub struct ResponsePool {
    /// Pool for small responses (< 1KB)
    small_pool: Arc<ObjectPool<ResponseBuffer>>,
    /// Pool for medium responses (1KB - 32KB)
    medium_pool: Arc<ObjectPool<ResponseBuffer>>,
    /// Pool for large responses (32KB - 512KB)
    large_pool: Arc<ObjectPool<ResponseBuffer>>,
    
    /// Configuration
    config: ResponsePoolConfig,
    
    /// Statistics tracking
    stats: Arc<RwLock<ResponsePoolStats>>,
    
    /// Allocation counters
    small_allocations: AtomicU64,
    medium_allocations: AtomicU64,
    large_allocations: AtomicU64,
    pool_hits: AtomicU64,
    pool_misses: AtomicU64,
}

impl ResponsePool {
    /// Create a new response pool
    pub fn new(config: ResponsePoolConfig) -> Self {
        let small_pool = Arc::new(ObjectPool::new(
            {
                let capacity = config.small_buffer_capacity;
                move || ResponseBuffer::new(capacity)
            },
            config.max_small_buffers,
        ));
        
        let medium_pool = Arc::new(ObjectPool::new(
            {
                let capacity = config.medium_buffer_capacity;
                move || ResponseBuffer::new(capacity)
            },
            config.max_medium_buffers,
        ));
        
        let large_pool = Arc::new(ObjectPool::new(
            {
                let capacity = config.large_buffer_capacity;
                move || ResponseBuffer::new(capacity)
            },
            config.max_large_buffers,
        ));
        
        let stats = Arc::new(RwLock::new(ResponsePoolStats {
            small_buffers_allocated: 0,
            medium_buffers_allocated: 0,
            large_buffers_allocated: 0,
            small_buffers_pooled: 0,
            medium_buffers_pooled: 0,
            large_buffers_pooled: 0,
            total_serializations: 0,
            total_bytes_serialized: 0,
            average_response_size: 0.0,
            pool_hit_rate: 0.0,
            memory_savings_bytes: 0,
        }));
        
        let track_utilization = config.track_utilization;
        
        let pool = Self {
            small_pool,
            medium_pool,
            large_pool,
            config,
            stats,
            small_allocations: AtomicU64::new(0),
            medium_allocations: AtomicU64::new(0),
            large_allocations: AtomicU64::new(0),
            pool_hits: AtomicU64::new(0),
            pool_misses: AtomicU64::new(0),
        };
        
        // Start cleanup task if enabled
        if track_utilization {
            pool.start_cleanup_task();
        }
        
        pool
    }
    
    /// Get an appropriate response buffer based on estimated size
    pub async fn acquire_buffer(&self, estimated_size: usize) -> PooledResponseBuffer {
        let (pool, buffer_type) = if estimated_size <= self.config.small_buffer_capacity {
            self.small_allocations.fetch_add(1, Ordering::Relaxed);
            (self.small_pool.clone(), BufferType::Small)
        } else if estimated_size <= self.config.medium_buffer_capacity {
            self.medium_allocations.fetch_add(1, Ordering::Relaxed);
            (self.medium_pool.clone(), BufferType::Medium)
        } else {
            self.large_allocations.fetch_add(1, Ordering::Relaxed);
            (self.large_pool.clone(), BufferType::Large)
        };
        
        let buffer = pool.acquire().await;
        
        // Track pool hit/miss more accurately
        // A buffer from the pool should have a reasonable capacity and some age
        if buffer.data.capacity() >= estimated_size && 
           buffer.created_at.elapsed().as_millis() > 10 {
            self.pool_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.pool_misses.fetch_add(1, Ordering::Relaxed);
        }
        
        PooledResponseBuffer {
            buffer,
            pool,
            buffer_type,
            pool_ref: Arc::downgrade(&self.stats),
        }
    }
    
    /// Serialize a JSON value using pooled buffer
    pub async fn serialize_json(&self, value: &Value) -> Result<Vec<u8>> {
        // Estimate size based on JSON complexity
        let estimated_size = self.estimate_json_size(value);
        
        let mut pooled_buffer = self.acquire_buffer(estimated_size).await;
        let serialized = pooled_buffer.buffer.serialize_json(value)?;
        
        // Track statistics
        let actual_size = serialized.len();
        let mut stats = self.stats.write().await;
        stats.total_serializations += 1;
        stats.total_bytes_serialized += actual_size as u64;
        stats.average_response_size = stats.total_bytes_serialized as f64 / stats.total_serializations as f64;
        
        // Calculate memory savings from pooling
        if actual_size < estimated_size {
            stats.memory_savings_bytes += (estimated_size - actual_size) as u64;
        }
        
        debug!("Serialized JSON response: {} bytes (estimated: {})", actual_size, estimated_size);
        
        Ok(serialized.to_vec())
    }
    
    /// Estimate JSON serialization size
    fn estimate_json_size(&self, value: &Value) -> usize {
        match value {
            Value::Null => 4,
            Value::Bool(_) => 5,
            Value::Number(_) => 20,
            Value::String(s) => s.len() + 2,
            Value::Array(arr) => {
                2 + arr.iter().map(|v| self.estimate_json_size(v) + 1).sum::<usize>()
            }
            Value::Object(obj) => {
                2 + obj.iter().map(|(k, v)| k.len() + 3 + self.estimate_json_size(v) + 1).sum::<usize>()
            }
        }
    }
    
    /// Get pool statistics
    pub async fn get_statistics(&self) -> ResponsePoolStats {
        let mut stats = self.stats.write().await;
        
        // Update real-time counts
        stats.small_buffers_pooled = self.small_pool.size();
        stats.medium_buffers_pooled = self.medium_pool.size();
        stats.large_buffers_pooled = self.large_pool.size();
        
        // Calculate hit rate
        let total_requests = self.pool_hits.load(Ordering::Relaxed) + self.pool_misses.load(Ordering::Relaxed);
        stats.pool_hit_rate = if total_requests > 0 {
            self.pool_hits.load(Ordering::Relaxed) as f64 / total_requests as f64
        } else {
            0.0
        };
        
        stats.clone()
    }
    
    /// Start background cleanup task
    fn start_cleanup_task(&self) {
        let small_pool = self.small_pool.clone();
        let medium_pool = self.medium_pool.clone();
        let large_pool = self.large_pool.clone();
        let cleanup_interval = self.config.cleanup_interval;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            
            loop {
                interval.tick().await;
                
                debug!("Response pool cleanup: small={}, medium={}, large={}", 
                       small_pool.size(), medium_pool.size(), large_pool.size());
                
                // Note: ObjectPool doesn't currently support removing old objects
                // This is a placeholder for future enhancement
            }
        });
    }
}

/// Type of buffer from the pool
#[derive(Debug, Clone, Copy)]
enum BufferType {
    Small,
    Medium,
    Large,
}

/// A response buffer acquired from the pool
pub struct PooledResponseBuffer {
    pub buffer: ResponseBuffer,
    pool: Arc<ObjectPool<ResponseBuffer>>,
    buffer_type: BufferType,
    pool_ref: std::sync::Weak<RwLock<ResponsePoolStats>>,
}

impl Drop for PooledResponseBuffer {
    fn drop(&mut self) {
        let pool = self.pool.clone();
        let mut buffer = std::mem::replace(&mut self.buffer, ResponseBuffer::new(0));
        
        // Clear buffer before returning to pool - ensure sensitive data is zeroed
        buffer.clear();
        
        // Note: Async operations in Drop are problematic.
        // This should be redesigned to use a background cleanup task
        // or synchronous pool operations for simple objects like buffers.
        // For now, we'll attempt synchronous return if possible.
        std::thread::spawn(move || {
            // Use runtime handle if available, otherwise buffer is dropped
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    pool.release(buffer).await;
                });
            }
            // If no runtime available, buffer is simply dropped
        });
    }
}

impl std::fmt::Debug for PooledResponseBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PooledResponseBuffer")
            .field("buffer_type", &self.buffer_type)
            .field("buffer_len", &self.buffer.len())
            .field("buffer_capacity", &self.buffer.capacity)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_response_buffer_basic_operations() {
        let mut buffer = ResponseBuffer::new(1024);
        let test_json = json!({"test": "data", "number": 42});
        
        let serialized = buffer.serialize_json(&test_json).unwrap();
        let serialized_len = serialized.len();
        assert!(!serialized.is_empty());
        let buffer_len = buffer.len();
        assert_eq!(buffer_len, serialized_len);
        
        buffer.clear();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.data.capacity() >= 1024);
    }
    
    #[tokio::test]
    async fn test_response_pool_acquisition() {
        let config = ResponsePoolConfig::default();
        let pool = ResponsePool::new(config);
        
        // Test small buffer acquisition
        let buffer1 = pool.acquire_buffer(500).await;
        assert!(buffer1.buffer.capacity >= 500);
        
        // Test medium buffer acquisition  
        let buffer2 = pool.acquire_buffer(5000).await;
        assert!(buffer2.buffer.capacity >= 5000);
        
        // Test large buffer acquisition
        let buffer3 = pool.acquire_buffer(100000).await;
        assert!(buffer3.buffer.capacity >= 100000);
    }
    
    #[tokio::test]
    async fn test_json_serialization_pooling() {
        let config = ResponsePoolConfig::default();
        let pool = ResponsePool::new(config);
        
        let test_data = json!({
            "entities": [
                {"id": 1, "name": "Entity1"},
                {"id": 2, "name": "Entity2"}
            ],
            "metadata": {
                "count": 2,
                "timestamp": "2024-01-01T00:00:00Z"
            }
        });
        
        let serialized = pool.serialize_json(&test_data).await.unwrap();
        assert!(!serialized.is_empty());
        
        // Test multiple serializations
        for _ in 0..10 {
            let _serialized = pool.serialize_json(&test_data).await.unwrap();
        }
        
        let stats = pool.get_statistics().await;
        assert_eq!(stats.total_serializations, 11);
    }
    
    #[tokio::test]
    async fn test_buffer_type_selection() {
        let config = ResponsePoolConfig::default();
        let pool = ResponsePool::new(config.clone());
        
        // Small buffer
        let small_buffer = pool.acquire_buffer(512).await;
        assert!(small_buffer.buffer.capacity <= config.small_buffer_capacity);
        
        // Medium buffer
        let medium_buffer = pool.acquire_buffer(16384).await;
        assert!(medium_buffer.buffer.capacity >= config.small_buffer_capacity);
        assert!(medium_buffer.buffer.capacity <= config.medium_buffer_capacity);
        
        // Large buffer
        let large_buffer = pool.acquire_buffer(262144).await;
        assert!(large_buffer.buffer.capacity >= config.medium_buffer_capacity);
    }
}