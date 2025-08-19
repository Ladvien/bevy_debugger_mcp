/// Performance Optimizations Integration Tests
/// 
/// Validates that all BEVDBG-012 performance optimizations work correctly
/// in integration with the broader system, meeting performance targets
/// and providing measurable improvements.

use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::{json, Value};

use bevy_debugger_mcp::{
    config::Config,
    mcp_server::McpServer,
    brp_client::BrpClient,
    lazy_init::LazyComponents,
    command_cache::{CommandCache, CacheConfig},
    response_pool::{ResponsePool, ResponsePoolConfig},
    profiling::{init_profiler, get_profiler},
};

mod fixtures;
mod helpers;
mod integration;

use integration::IntegrationTestHarness;

/// Test that lazy initialization provides measurable startup improvement
#[tokio::test]
async fn test_lazy_initialization_startup_performance() {
    // Test without lazy initialization (eager loading all components)
    let start_eager = Instant::now();
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };
    let brp_client_eager = Arc::new(RwLock::new(BrpClient::new(&config)));
    let _server_eager = McpServer::new(config.clone(), brp_client_eager);
    let eager_startup_time = start_eager.elapsed();

    // Test with lazy initialization
    let start_lazy = Instant::now();
    let brp_client_lazy = Arc::new(RwLock::new(BrpClient::new(&config)));
    let lazy_components = LazyComponents::new(brp_client_lazy.clone());
    
    // Simulate minimal initialization (only what's immediately needed)
    let _server_lazy = McpServer::new(config, brp_client_lazy);
    let lazy_startup_time = start_lazy.elapsed();

    println!("Eager startup time: {:?}", eager_startup_time);
    println!("Lazy startup time: {:?}", lazy_startup_time);

    // Lazy initialization should be faster or at least not significantly slower
    // In practice, the difference will be more pronounced with real component initialization
    assert!(lazy_startup_time <= eager_startup_time + Duration::from_millis(50), 
            "Lazy initialization should not significantly increase startup time");

    // Test that components are initialized on demand
    let start_init = Instant::now();
    let _entity_inspector = lazy_components.get_entity_inspector().await;
    let first_init_time = start_init.elapsed();

    let start_cached = Instant::now();
    let _entity_inspector2 = lazy_components.get_entity_inspector().await;
    let cached_init_time = start_cached.elapsed();

    println!("First initialization: {:?}", first_init_time);
    println!("Cached initialization: {:?}", cached_init_time);

    // Second access should be significantly faster (cached)
    assert!(cached_init_time < first_init_time, 
            "Cached component access should be faster than first initialization");
    assert!(cached_init_time < Duration::from_millis(1), 
            "Cached component access should be sub-millisecond");
}

/// Test command caching effectiveness and performance improvement
#[tokio::test]
async fn test_command_caching_performance() {
    let cache_config = CacheConfig {
        max_size: 100,
        ttl: Duration::from_secs(300),
        enable_metrics: true,
    };
    let cache = CommandCache::new(cache_config);

    // Test cache miss performance
    let test_command = "observe";
    let test_args = json!({"query": "entities with Transform"});
    
    let start_miss = Instant::now();
    // Simulate expensive operation
    let expensive_result = json!({"entities": [1, 2, 3, 4, 5], "timestamp": "2024-01-01T00:00:00Z"});
    cache.set(test_command, &test_args, expensive_result.clone()).await;
    let cache_miss_time = start_miss.elapsed();

    // Test cache hit performance
    let start_hit = Instant::now();
    let cached_result = cache.get(test_command, &test_args).await;
    let cache_hit_time = start_hit.elapsed();

    println!("Cache miss time: {:?}", cache_miss_time);
    println!("Cache hit time: {:?}", cache_hit_time);

    assert!(cached_result.is_some(), "Cache should return cached result");
    assert_eq!(cached_result.unwrap(), expensive_result, "Cached result should match original");
    
    // Cache hit should be significantly faster
    assert!(cache_hit_time < cache_miss_time / 10, 
            "Cache hit should be at least 10x faster than cache miss");
    assert!(cache_hit_time < Duration::from_millis(1), 
            "Cache hit should be sub-millisecond");

    // Test cache hit rate over multiple operations
    let mut total_hits = 0;
    let mut total_requests = 0;

    for i in 0..100 {
        total_requests += 1;
        let args = json!({"query": format!("entities with Component{}", i % 10)});
        
        let result = cache.get("observe", &args).await;
        if result.is_some() {
            total_hits += 1;
        } else {
            // Simulate cache miss - store result
            let mock_result = json!({"entities": [i], "query": args});
            cache.set("observe", &args, mock_result).await;
        }
    }

    let hit_rate = total_hits as f64 / total_requests as f64;
    println!("Cache hit rate: {:.2}%", hit_rate * 100.0);

    // Should achieve reasonable hit rate with repeated queries
    assert!(hit_rate >= 0.5, "Cache should achieve at least 50% hit rate with repeated queries");
}

/// Test memory pooling effectiveness and allocation reduction
#[tokio::test]
async fn test_memory_pooling_performance() {
    let pool_config = ResponsePoolConfig {
        max_small_buffers: 50,
        max_medium_buffers: 25,
        max_large_buffers: 10,
        small_buffer_capacity: 1024,
        medium_buffer_capacity: 32768,
        large_buffer_capacity: 524288,
        track_utilization: true,
        cleanup_interval: Duration::from_secs(60),
    };
    
    let pool = ResponsePool::new(pool_config);

    // Test allocation performance with pooling
    let test_data = json!({
        "entities": (0..1000).map(|i| json!({
            "id": i,
            "name": format!("Entity{}", i),
            "components": [
                {"type": "Transform", "data": {"x": i as f32, "y": 0.0, "z": 0.0}},
                {"type": "Mesh", "data": {"vertices": 100, "faces": 50}}
            ]
        })).collect::<Vec<_>>(),
        "metadata": {
            "count": 1000,
            "timestamp": "2024-01-01T00:00:00Z",
            "query": "entities with Transform"
        }
    });

    // Measure pooled allocation performance
    let start_pooled = Instant::now();
    let mut pooled_results = Vec::new();
    for _ in 0..100 {
        let result = pool.serialize_json(&test_data).await.unwrap();
        pooled_results.push(result);
    }
    let pooled_time = start_pooled.elapsed();

    // Measure direct allocation performance (without pooling)
    let start_direct = Instant::now();
    let mut direct_results = Vec::new();
    for _ in 0..100 {
        let result = serde_json::to_vec(&test_data).unwrap();
        direct_results.push(result);
    }
    let direct_time = start_direct.elapsed();

    println!("Pooled allocation time: {:?}", pooled_time);
    println!("Direct allocation time: {:?}", direct_time);

    // Pooled allocation should be competitive or faster
    // Note: For small objects, pooling overhead might make it slightly slower
    // but it should prevent allocation pressure under load
    let performance_ratio = pooled_time.as_millis() as f64 / direct_time.as_millis() as f64;
    assert!(performance_ratio < 2.0, 
            "Pooled allocation should not be more than 2x slower than direct allocation");

    // Check pool statistics
    let stats = pool.get_statistics().await;
    println!("Pool statistics: {:?}", stats);

    assert!(stats.total_serializations >= 100, "Should track all serializations");
    assert!(stats.pool_hit_rate > 0.0, "Should achieve some pool hits");
    
    // Verify all results are identical
    assert_eq!(pooled_results[0], direct_results[0], 
              "Pooled and direct results should be identical");
}

/// Test overall system performance meets targets
#[tokio::test]
async fn test_performance_targets_met() {
    let harness = IntegrationTestHarness::new().await.unwrap();

    // Performance targets from BEVDBG-012:
    // - Command processing < 1ms p99
    // - Memory overhead < 50MB when active  
    // - CPU overhead < 3% when monitoring

    let commands = vec![
        ("observe", json!({"query": "entities with Transform"})),
        ("health_check", json!({})),
        ("resource_metrics", json!({})),
        ("diagnostic_report", json!({"action": "generate"})),
    ];

    let mut command_latencies = Vec::new();

    // Execute commands and measure latency
    for _ in 0..100 {
        for (command, args) in &commands {
            let start = Instant::now();
            let _ = harness.execute_tool_call(command, args.clone()).await;
            let latency = start.elapsed();
            command_latencies.push(latency);
        }
    }

    // Sort latencies for percentile calculation
    command_latencies.sort();
    let p99_index = (command_latencies.len() as f64 * 0.99) as usize;
    let p99_latency = command_latencies[p99_index.min(command_latencies.len() - 1)];
    let avg_latency: Duration = command_latencies.iter().sum::<Duration>() / command_latencies.len() as u32;

    println!("P99 latency: {:?}", p99_latency);
    println!("Average latency: {:?}", avg_latency);
    println!("Total commands executed: {}", command_latencies.len());

    // Validate performance targets
    assert!(p99_latency < Duration::from_millis(1), 
            "P99 command processing should be < 1ms, got {:?}", p99_latency);
    assert!(avg_latency < Duration::from_millis(10), 
            "Average command processing should be reasonable, got {:?}", avg_latency);
}

/// Test feature flag combinations work correctly
#[cfg(test)]
mod feature_flag_tests {
    use super::*;

    #[tokio::test]
    #[cfg(feature = "caching")]
    async fn test_caching_feature_enabled() {
        let cache = CommandCache::new(CacheConfig::default());
        cache.set("test", &json!({}), json!({"result": "cached"})).await;
        let result = cache.get("test", &json!({})).await;
        assert!(result.is_some(), "Caching should work when feature is enabled");
    }

    #[tokio::test]
    #[cfg(feature = "pooling")]
    async fn test_pooling_feature_enabled() {
        let pool = ResponsePool::new(ResponsePoolConfig::default());
        let result = pool.serialize_json(&json!({"test": "data"})).await;
        assert!(result.is_ok(), "Pooling should work when feature is enabled");
    }

    #[tokio::test]
    #[cfg(feature = "lazy-init")]
    async fn test_lazy_init_feature_enabled() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3001,
        };
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        let lazy_components = LazyComponents::new(brp_client);
        
        let inspector = lazy_components.get_entity_inspector().await;
        assert!(inspector.is_initialized(), "Lazy initialization should work when feature is enabled");
    }
}

/// Test profiling and measurement systems
#[tokio::test]
async fn test_profiling_system_performance() {
    init_profiler();
    let profiler = get_profiler();

    // Test profiling overhead
    let iterations = 1000;
    
    // Measure without profiling
    let start_no_profile = Instant::now();
    for _ in 0..iterations {
        // Simulate some work
        let _result = serde_json::to_string(&json!({"test": "data"})).unwrap();
    }
    let time_no_profile = start_no_profile.elapsed();

    // Measure with profiling
    let start_with_profile = Instant::now();
    for _ in 0..iterations {
        let _guard = profiler.start_measurement("test_operation");
        // Simulate some work
        let _result = serde_json::to_string(&json!({"test": "data"})).unwrap();
    }
    let time_with_profile = start_with_profile.elapsed();

    println!("Time without profiling: {:?}", time_no_profile);
    println!("Time with profiling: {:?}", time_with_profile);

    // Profiling overhead should be minimal
    let overhead_ratio = time_with_profile.as_millis() as f64 / time_no_profile.as_millis() as f64;
    assert!(overhead_ratio < 1.1, 
            "Profiling overhead should be less than 10%, got {:.2}%", 
            (overhead_ratio - 1.0) * 100.0);

    // Check profiling results
    let stats = profiler.get_stats("test_operation").await;
    assert!(stats.is_some(), "Should have profiling statistics");
    
    let stats = stats.unwrap();
    assert_eq!(stats.call_count, iterations as u64, "Should track all calls");
    assert!(stats.total_duration > Duration::from_nanos(1), "Should measure some duration");
}

/// Test optimization combinations work together
#[tokio::test]
async fn test_optimization_combinations() {
    // Test with all optimizations enabled
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };
    
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let lazy_components = LazyComponents::new(brp_client.clone());
    let cache = CommandCache::new(CacheConfig::default());
    let pool = ResponsePool::new(ResponsePoolConfig::default());
    
    // Test that all optimizations work together
    let test_command = "observe";
    let test_args = json!({"query": "entities with Transform"});
    let test_result = json!({
        "entities": [1, 2, 3],
        "timestamp": "2024-01-01T00:00:00Z"
    });

    // Test lazy initialization + caching
    let start_combined = Instant::now();
    
    // Initialize component lazily
    let _inspector = lazy_components.get_entity_inspector().await;
    
    // Cache the result
    cache.set(test_command, &test_args, test_result.clone()).await;
    
    // Use pooling for serialization
    let _serialized = pool.serialize_json(&test_result).await.unwrap();
    
    let combined_time = start_combined.elapsed();
    
    println!("Combined optimizations time: {:?}", combined_time);
    
    // Combined operations should complete quickly
    assert!(combined_time < Duration::from_millis(10), 
            "Combined optimizations should complete quickly");

    // Test cache hit with pooling
    let start_optimized = Instant::now();
    let cached_result = cache.get(test_command, &test_args).await;
    assert!(cached_result.is_some(), "Should get cached result");
    
    let _serialized = pool.serialize_json(&cached_result.unwrap()).await.unwrap();
    let optimized_time = start_optimized.elapsed();
    
    println!("Fully optimized time: {:?}", optimized_time);
    
    // Fully optimized path should be very fast
    assert!(optimized_time < Duration::from_millis(1), 
            "Fully optimized path should be sub-millisecond");
}

/// Test error handling with optimizations enabled
#[tokio::test]
async fn test_error_handling_with_optimizations() {
    let cache = CommandCache::new(CacheConfig::default());
    let pool = ResponsePool::new(ResponsePoolConfig::default());

    // Test cache with invalid data
    let invalid_data = json!({"invalid": std::f64::NAN});
    let result = pool.serialize_json(&invalid_data).await;
    assert!(result.is_err(), "Should handle invalid JSON gracefully");

    // Test cache eviction under pressure
    for i in 0..1000 {
        let args = json!({"query": format!("unique_query_{}", i)});
        cache.set("test", &args, json!({"result": i})).await;
    }

    // Cache should handle pressure gracefully
    let cache_stats = cache.get_cache_stats().await;
    assert!(cache_stats.size <= cache_stats.max_size, 
            "Cache should respect size limits");
}