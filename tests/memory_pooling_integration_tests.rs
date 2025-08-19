/// Memory Pooling Integration Tests
/// 
/// Tests the response pooling system to validate memory allocation optimization,
/// buffer reuse effectiveness, and security considerations.

use std::sync::Arc;
use std::time::{Duration, Instant};
use serde_json::{json, Value};

use bevy_debugger_mcp::{
    response_pool::{ResponsePool, ResponsePoolConfig, ResponseBuffer, PooledResponseBuffer},
};

mod fixtures;
mod helpers;

use helpers::MemoryUsageTracker;

/// Test basic pooling functionality and performance
#[tokio::test]
async fn test_basic_pooling_performance() {
    let pool_config = ResponsePoolConfig {
        max_small_buffers: 10,
        max_medium_buffers: 5,
        max_large_buffers: 2,
        small_buffer_capacity: 1024,
        medium_buffer_capacity: 32768,
        large_buffer_capacity: 524288,
        track_utilization: true,
        cleanup_interval: Duration::from_secs(60),
    };

    let pool = ResponsePool::new(pool_config);

    // Test small buffer allocation and reuse
    let test_data_small = json!({"message": "small test data", "size": "small"});
    
    let mut allocation_times = Vec::new();
    let mut serialization_times = Vec::new();

    // First allocation (pool miss)
    for i in 0..10 {
        let start_alloc = Instant::now();
        let buffer = pool.acquire_buffer(512).await;
        allocation_times.push(start_alloc.elapsed());

        let start_serialize = Instant::now();
        let serialized = pool.serialize_json(&test_data_small).await.unwrap();
        serialization_times.push(start_serialize.elapsed());
        
        assert!(!serialized.is_empty(), "Should serialize successfully");
        
        // Buffer should be automatically returned to pool when dropped
    }

    // Second round - should hit pool more often
    let mut reuse_allocation_times = Vec::new();
    let mut reuse_serialization_times = Vec::new();

    for _i in 0..10 {
        let start_alloc = Instant::now();
        let buffer = pool.acquire_buffer(512).await;
        reuse_allocation_times.push(start_alloc.elapsed());

        let start_serialize = Instant::now();
        let serialized = pool.serialize_json(&test_data_small).await.unwrap();
        reuse_serialization_times.push(start_serialize.elapsed());
        
        assert!(!serialized.is_empty(), "Should serialize successfully");
    }

    // Analyze performance
    let avg_first_alloc: Duration = allocation_times.iter().sum::<Duration>() / allocation_times.len() as u32;
    let avg_reuse_alloc: Duration = reuse_allocation_times.iter().sum::<Duration>() / reuse_allocation_times.len() as u32;
    let avg_first_serialize: Duration = serialization_times.iter().sum::<Duration>() / serialization_times.len() as u32;
    let avg_reuse_serialize: Duration = reuse_serialization_times.iter().sum::<Duration>() / reuse_serialization_times.len() as u32;

    println!("Average first allocation: {:?}", avg_first_alloc);
    println!("Average reuse allocation: {:?}", avg_reuse_alloc);
    println!("Average first serialization: {:?}", avg_first_serialize);
    println!("Average reuse serialization: {:?}", avg_reuse_serialize);

    // Check pool statistics
    let stats = pool.get_statistics().await;
    println!("Pool statistics: {:?}", stats);

    assert!(stats.total_serializations >= 20, "Should track all serializations");
    assert!(stats.pool_hit_rate > 0.0, "Should achieve some pool hits");
}

/// Test buffer size selection and utilization
#[tokio::test]
async fn test_buffer_size_selection() {
    let pool = ResponsePool::new(ResponsePoolConfig::default());

    // Test small data
    let small_data = json!({"id": 1, "name": "small"});
    let small_serialized = pool.serialize_json(&small_data).await.unwrap();
    
    // Test medium data
    let medium_data = json!({
        "entities": (0..100).map(|i| json!({
            "id": i,
            "name": format!("entity_{}", i),
            "position": {"x": i as f32, "y": 0.0, "z": 0.0}
        })).collect::<Vec<_>>()
    });
    let medium_serialized = pool.serialize_json(&medium_data).await.unwrap();

    // Test large data
    let large_data = json!({
        "entities": (0..5000).map(|i| json!({
            "id": i,
            "name": format!("entity_with_long_name_{}", i),
            "position": {"x": i as f32, "y": i as f32, "z": i as f32},
            "components": (0..10).map(|c| json!({
                "type": format!("Component{}", c),
                "data": format!("component_data_{}", c)
            })).collect::<Vec<_>>()
        })).collect::<Vec<_>>()
    });
    let large_serialized = pool.serialize_json(&large_data).await.unwrap();

    println!("Small data size: {} bytes", small_serialized.len());
    println!("Medium data size: {} bytes", medium_serialized.len());
    println!("Large data size: {} bytes", large_serialized.len());

    let stats = pool.get_statistics().await;
    println!("Pool utilization stats: {:?}", stats);

    // Verify different buffer types were used
    assert!(stats.small_buffers_allocated > 0, "Should use small buffers");
    assert!(stats.medium_buffers_allocated > 0, "Should use medium buffers");
    assert!(stats.large_buffers_allocated > 0, "Should use large buffers");
    
    // Verify serialization produced correct results
    assert!(small_serialized.len() < 1000, "Small data should be small");
    assert!(medium_serialized.len() > 1000 && medium_serialized.len() < 50000, 
            "Medium data should be medium-sized");
    assert!(large_serialized.len() > 50000, "Large data should be large");
}

/// Test memory usage under sustained load
#[tokio::test]
async fn test_memory_usage_under_load() {
    let mut memory_tracker = MemoryUsageTracker::new();
    let initial_memory = memory_tracker.current_usage();

    let pool = ResponsePool::new(ResponsePoolConfig {
        max_small_buffers: 50,
        max_medium_buffers: 20,
        max_large_buffers: 5,
        track_utilization: true,
        ..Default::default()
    });

    // Sustained load test
    let test_data = json!({
        "response": "load test",
        "data": (0..1000).collect::<Vec<_>>()
    });

    println!("Starting sustained load test...");
    
    let mut peak_memory = initial_memory;
    let iterations = 1000;
    
    for i in 0..iterations {
        let _serialized = pool.serialize_json(&test_data).await.unwrap();
        
        if i % 100 == 0 {
            let current_memory = memory_tracker.current_usage();
            peak_memory = peak_memory.max(current_memory);
            println!("Iteration {}: {} bytes", i, current_memory);
        }
    }

    let final_memory = memory_tracker.current_usage();
    let memory_overhead = final_memory.saturating_sub(initial_memory);
    
    println!("Initial memory: {} bytes", initial_memory);
    println!("Peak memory: {} bytes", peak_memory);
    println!("Final memory: {} bytes", final_memory);
    println!("Memory overhead: {} bytes", memory_overhead);

    let stats = pool.get_statistics().await;
    println!("Final pool stats: {:?}", stats);

    // Verify memory usage is reasonable
    assert!(memory_overhead < 50_000_000, // 50MB limit from BEVDBG-012
            "Memory overhead should be less than 50MB, got {} bytes", memory_overhead);
    
    // Hit rate should be good under sustained load
    assert!(stats.pool_hit_rate > 0.8, 
            "Hit rate should be above 80% under sustained load, got {:.2}%", 
            stats.pool_hit_rate * 100.0);
}

/// Test concurrent access and thread safety
#[tokio::test]
async fn test_concurrent_access() {
    let pool = Arc::new(ResponsePool::new(ResponsePoolConfig::default()));
    
    let mut handles = vec![];
    let start_time = Instant::now();

    // Spawn multiple concurrent serialization tasks
    for task_id in 0..20 {
        let pool_clone = pool.clone();
        let handle = tokio::spawn(async move {
            let mut task_stats = (0u64, 0u64); // (successes, failures)
            
            for i in 0..100 {
                let test_data = json!({
                    "task_id": task_id,
                    "iteration": i,
                    "data": (0..(i % 50 + 10)).collect::<Vec<_>>()
                });
                
                match pool_clone.serialize_json(&test_data).await {
                    Ok(serialized) => {
                        assert!(!serialized.is_empty(), "Should produce non-empty result");
                        task_stats.0 += 1;
                    }
                    Err(_) => {
                        task_stats.1 += 1;
                    }
                }
            }
            
            task_stats
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    let mut total_successes = 0;
    let mut total_failures = 0;
    
    for handle in handles {
        let (successes, failures) = handle.await.unwrap();
        total_successes += successes;
        total_failures += failures;
    }

    let total_time = start_time.elapsed();
    println!("Concurrent test completed in: {:?}", total_time);
    println!("Total successes: {}, Total failures: {}", total_successes, total_failures);

    let stats = pool.get_statistics().await;
    println!("Concurrent access stats: {:?}", stats);

    // Verify results
    assert_eq!(total_failures, 0, "No failures should occur during concurrent access");
    assert_eq!(total_successes, 2000, "Should complete all 2000 operations");
    assert!(total_time < Duration::from_secs(10), 
            "Concurrent access should complete within 10 seconds");
    
    // Pool should maintain consistency
    assert!(stats.total_serializations >= 2000, "Should track all serializations");
}

/// Test buffer cleanup and security (no data leakage)
#[tokio::test]
async fn test_buffer_security_and_cleanup() {
    let pool = ResponsePool::new(ResponsePoolConfig::default());

    // Test with sensitive data
    let sensitive_data = json!({
        "secret": "this_is_sensitive_data_12345",
        "api_key": "sk-abcdef1234567890",
        "password": "super_secret_password",
        "user_data": {
            "email": "user@example.com",
            "ssn": "123-45-6789"
        }
    });

    // Serialize sensitive data
    let serialized1 = pool.serialize_json(&sensitive_data).await.unwrap();
    assert!(std::str::from_utf8(&serialized1).unwrap().contains("sensitive_data_12345"),
            "First serialization should contain sensitive data");

    // Drop the first serialized data and let buffer return to pool
    drop(serialized1);

    // Force some buffer reuse by doing many small allocations
    for _ in 0..10 {
        let dummy_data = json!({"dummy": "data"});
        let _dummy_serialized = pool.serialize_json(&dummy_data).await.unwrap();
    }

    // Try to get a buffer that might have been reused
    let non_sensitive_data = json!({
        "normal": "data",
        "nothing": "secret here"
    });
    
    let serialized2 = pool.serialize_json(&non_sensitive_data).await.unwrap();
    let serialized2_str = std::str::from_utf8(&serialized2).unwrap();
    
    // Verify no sensitive data leaked into new buffer
    assert!(!serialized2_str.contains("sensitive_data_12345"),
            "Sensitive data should not leak into reused buffer");
    assert!(!serialized2_str.contains("sk-abcdef1234567890"),
            "API key should not leak into reused buffer");
    assert!(!serialized2_str.contains("super_secret_password"),
            "Password should not leak into reused buffer");
    
    println!("Buffer security test passed - no data leakage detected");
}

/// Test pool behavior under memory pressure
#[tokio::test]
async fn test_pool_under_memory_pressure() {
    let config = ResponsePoolConfig {
        max_small_buffers: 5,    // Small limits to force pressure
        max_medium_buffers: 3,
        max_large_buffers: 1,
        track_utilization: true,
        ..Default::default()
    };
    
    let pool = ResponsePool::new(config);

    // Try to allocate more buffers than pool can hold
    let large_data = json!({
        "large_array": (0..10000).map(|i| json!({
            "id": i,
            "data": format!("item_{}", i)
        })).collect::<Vec<_>>()
    });

    let mut serialized_results = Vec::new();
    
    // This should exceed pool capacity and force allocation of new buffers
    for i in 0..10 {
        let start = Instant::now();
        let result = pool.serialize_json(&large_data).await;
        let duration = start.elapsed();
        
        println!("Large allocation {}: {:?} ({:?})", i, result.is_ok(), duration);
        
        match result {
            Ok(serialized) => {
                serialized_results.push(serialized);
                // Even under pressure, operations should complete reasonably quickly
                assert!(duration < Duration::from_millis(100), 
                        "Operations should complete quickly even under memory pressure");
            }
            Err(e) => {
                println!("Allocation {} failed (acceptable under pressure): {}", i, e);
            }
        }
    }

    let stats = pool.get_statistics().await;
    println!("Stats under memory pressure: {:?}", stats);

    // Some operations should succeed even under pressure
    assert!(serialized_results.len() >= 5, 
            "At least some operations should succeed under memory pressure");
    
    // Pool should handle pressure gracefully
    assert!(stats.large_buffers_allocated >= 5, 
            "Should attempt multiple large buffer allocations");
}

/// Test pool statistics accuracy
#[tokio::test]
async fn test_pool_statistics_accuracy() {
    let pool = ResponsePool::new(ResponsePoolConfig {
        track_utilization: true,
        ..Default::default()
    });

    // Perform known operations and verify statistics
    let small_data = json!({"type": "small"});
    let medium_data = json!({"data": (0..500).collect::<Vec<_>>()});
    let large_data = json!({"data": (0..5000).collect::<Vec<_>>()});

    // 5 small, 3 medium, 2 large operations
    for _ in 0..5 {
        let _ = pool.serialize_json(&small_data).await.unwrap();
    }
    
    for _ in 0..3 {
        let _ = pool.serialize_json(&medium_data).await.unwrap();
    }
    
    for _ in 0..2 {
        let _ = pool.serialize_json(&large_data).await.unwrap();
    }

    let stats = pool.get_statistics().await;
    println!("Detailed statistics: {:?}", stats);

    // Verify statistics accuracy
    assert_eq!(stats.total_serializations, 10, "Should track exactly 10 serializations");
    assert!(stats.small_buffers_allocated >= 5, "Should track small buffer allocations");
    assert!(stats.medium_buffers_allocated >= 3, "Should track medium buffer allocations");
    assert!(stats.large_buffers_allocated >= 2, "Should track large buffer allocations");
    
    // Calculate expected total bytes (approximate)
    assert!(stats.total_bytes_serialized > 0, "Should track total bytes serialized");
    assert!(stats.average_response_size > 0.0, "Should calculate average response size");
}

/// Test pool cleanup and resource management
#[tokio::test]
async fn test_pool_cleanup() {
    let pool = ResponsePool::new(ResponsePoolConfig {
        max_small_buffers: 10,
        cleanup_interval: Duration::from_millis(100), // Frequent cleanup for testing
        track_utilization: true,
        ..Default::default()
    });

    // Fill pool with buffers
    let mut buffers = Vec::new();
    for i in 0..15 {
        let data = json!({"buffer": i});
        let serialized = pool.serialize_json(&data).await.unwrap();
        buffers.push(serialized);
    }

    let stats_before = pool.get_statistics().await;
    println!("Stats before cleanup: {:?}", stats_before);

    // Drop all buffers to return them to pool
    drop(buffers);
    
    // Wait for cleanup cycle
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Force some activity to trigger cleanup
    let dummy_data = json!({"cleanup": "trigger"});
    let _ = pool.serialize_json(&dummy_data).await.unwrap();

    let stats_after = pool.get_statistics().await;
    println!("Stats after cleanup: {:?}", stats_after);

    // Pool should maintain reasonable size
    assert!(stats_after.small_buffers_pooled <= stats_before.small_buffers_allocated,
            "Pool should not grow unbounded");
}

/// Test error conditions and edge cases
#[tokio::test]
async fn test_error_conditions() {
    let pool = ResponsePool::new(ResponsePoolConfig::default());

    // Test serialization of problematic data
    let problematic_data = vec![
        // NaN values
        json!({"value": std::f64::NAN}),
        // Infinite values  
        json!({"value": std::f64::INFINITY}),
        // Very deeply nested structures
        create_deep_json_structure(100),
    ];

    for (i, data) in problematic_data.iter().enumerate() {
        let result = pool.serialize_json(data).await;
        println!("Problematic data {}: {:?}", i, result.is_ok());
        
        match result {
            Ok(_) => {
                // Some might succeed depending on serde_json behavior
                println!("Data {} serialized successfully", i);
            }
            Err(e) => {
                // Should handle errors gracefully
                println!("Data {} failed gracefully: {}", i, e);
                assert!(!e.to_string().contains("panic"), "Should not panic on invalid data");
            }
        }
    }

    // Pool should remain functional after errors
    let valid_data = json!({"valid": "data"});
    let valid_result = pool.serialize_json(&valid_data).await;
    assert!(valid_result.is_ok(), "Pool should remain functional after errors");
}

/// Helper function to create deeply nested JSON for testing
fn create_deep_json_structure(depth: usize) -> Value {
    let mut current = json!("deep");
    for _ in 0..depth {
        current = json!({"nested": current});
    }
    current
}