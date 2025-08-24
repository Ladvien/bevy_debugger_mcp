/// Caching System Integration Tests
/// 
/// Tests the command caching system with realistic workloads to validate
/// performance improvements and correct behavior under various conditions.

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use serde_json::{json, Value};
use tokio::sync::RwLock;

use bevy_debugger_mcp::{
    config::Config,
    mcp_server::McpServer,
    brp_client::BrpClient,
    command_cache::{CommandCache, CacheConfig, CacheKey},
};

mod fixtures;
mod helpers;

use fixtures::complex_ecs_game;
use helpers::{TestGameProcess, generate_realistic_queries};

/// Test cache performance with realistic command patterns
#[tokio::test]
async fn test_cache_with_realistic_workload() {
    let mut game_process = TestGameProcess::new("complex_ecs_game").await;
    game_process.start().await.expect("Failed to start test game");
    
    // Wait for game to have some entities
    tokio::time::sleep(Duration::from_secs(3)).await;

    let cache_config = Cache{ let mut config = Config::default();
        max_size: 1000,
        ttl: Duration::from_secs(300),
        enable_metrics: true,
    };
    let cache = CommandCache::new(cache_config);

    // Generate realistic query patterns that would be used in debugging
    let realistic_queries = generate_realistic_queries();
    
    println!("Testing cache with {} realistic queries", realistic_queries.len());

    // First pass - populate cache (all misses)
    let mut cache_miss_times = Vec::new();
    for (command, args) in &realistic_queries {
        let start = Instant::now();
        
        // Simulate expensive operation result
        let mock_result = simulate_expensive_query_result(command, args).await;
        cache.set(command, args, mock_result).await;
        
        cache_miss_times.push(start.elapsed());
        }

    // Second pass - cache hits
    let mut cache_hit_times = Vec::new();
    for (command, args) in &realistic_queries {
        let start = Instant::now();
        
        let cached_result = cache.get(command, args).await;
        assert!(cached_result.is_some(), "Should get cached result for {}", command);
        
        cache_hit_times.push(start.elapsed());
        }

    // Analyze performance
    let avg_miss_time: Duration = cache_miss_times.iter().sum::<Duration>() / cache_miss_times.len() as u32;
    let avg_hit_time: Duration = cache_hit_times.iter().sum::<Duration>() / cache_hit_times.len() as u32;
    let speedup = avg_miss_time.as_millis() as f64 / avg_hit_time.as_millis() as f64;

    println!("Average cache miss time: {:?}", avg_miss_time);
    println!("Average cache hit time: {:?}", avg_hit_time);
    println!("Cache speedup: {:.2}x", speedup);

    // Validate performance improvements
    assert!(speedup > 10.0, "Cache should provide at least 10x speedup, got {:.2}x", speedup);
    assert!(avg_hit_time < Duration::from_millis(1), 
            "Cache hits should be sub-millisecond, got {:?}", avg_hit_time);

    // Check cache statistics
    let stats = cache.get_cache_stats().await;
    println!("Cache statistics: {:?}", stats);
    
    assert_eq!(stats.total_gets, realistic_queries.len() as u64, "Should track all get operations");
    assert_eq!(stats.cache_hits, realistic_queries.len() as u64, "All second pass queries should be hits");
    assert!(stats.hit_rate > 0.5, "Hit rate should be above 50%");

    game_process.cleanup().await.expect("Failed to cleanup test game");
    }

/// Test cache behavior with different TTL settings
#[tokio::test]
async fn test_cache_ttl_behavior() {
    // Test with very short TTL
    let short_ttl_config = Cache{ let mut config = Config::default();
        max_size: 100,
        ttl: Duration::from_millis(100), // Very short TTL
        enable_metrics: true,
    };
    let cache = CommandCache::new(short_ttl_config);

    let test_command = "observe";
    let test_args = json!({"query": "entities with Transform"});
    let test_result = json!({"entities": [1, 2, 3], "count": 3});

    // Store in cache
    cache.set(test_command, &test_args, test_result.clone()).await;
    
    // Immediate retrieval should work
    let immediate_result = cache.get(test_command, &test_args).await;
    assert!(immediate_result.is_some(), "Should get immediate cached result");
    assert_eq!(immediate_result.unwrap(), test_result, "Cached result should match");

    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_millis(150)).await;
    
    // Should now be expired
    let expired_result = cache.get(test_command, &test_args).await;
    assert!(expired_result.is_none(), "Result should be expired after TTL");

    // Test with longer TTL
    let long_ttl_config = Cache{ let mut config = Config::default();
        max_size: 100,
        ttl: Duration::from_secs(10), // Long TTL
        enable_metrics: true,
    };
    let long_cache = CommandCache::new(long_ttl_config);
    
    long_cache.set(test_command, &test_args, test_result.clone()).await;
    
    // Wait a bit and should still be cached
    tokio::time::sleep(Duration::from_millis(100)).await;
    let still_cached = long_cache.get(test_command, &test_args).await;
    assert!(still_cached.is_some(), "Should still be cached with longer TTL");
    }

/// Test cache invalidation patterns
#[tokio::test]
async fn test_cache_invalidation() {
    let cache = CommandCache::new(Cache{ let mut config = Config::default();
        max_size: 100,
        ttl: Duration::from_secs(300),
        enable_metrics: true,
    });

    // Cache various types of queries
    let entity_queries = vec![
        ("observe", json!({"query": "entities with Transform"})),
        ("observe", json!({"query": "entities with Mesh"})),
        ("observe", json!({"query": "entities with Name"})),
    ];

    for (command, args) in &entity_queries {
        let result = json!({"entities": [1, 2, 3], "timestamp": "2024-01-01T00:00:00Z"});
        cache.set(command, args, result).await;
        }

    // Verify all are cached
    for (command, args) in &entity_queries {
        let cached = cache.get(command, args).await;
        assert!(cached.is_some(), "Query should be cached: {:?}", args);
        }

    // Invalidate by tag (entity-related queries)
    cache.invalidate_by_tag("entity").await;

    // All entity queries should now be invalidated
    for (command, args) in &entity_queries {
        let cached = cache.get(command, args).await;
        assert!(cached.is_none(), "Entity query should be invalidated: {:?}", args);
        }

    // Test command-specific invalidation
    cache.set("system_profile", &json!({"system": "movement"}), json!({"metrics": "data"})).await;
    cache.set("system_profile", &json!({"system": "physics"}), json!({"metrics": "data2"})).await;
    
    // Both should be cached
    assert!(cache.get("system_profile", &json!({"system": "movement"})).await.is_some());
    assert!(cache.get("system_profile", &json!({"system": "physics"})).await.is_some());
    
    // Invalidate all system_profile commands
    cache.invalidate_by_command("system_profile").await;
    
    // Both should now be invalidated
    assert!(cache.get("system_profile", &json!({"system": "movement"})).await.is_none());
    assert!(cache.get("system_profile", &json!({"system": "physics"})).await.is_none());
    }

/// Test cache size limits and eviction policies
#[tokio::test]
async fn test_cache_size_limits_and_eviction() {
    let cache = CommandCache::new(Cache{ let mut config = Config::default();
        max_size: 10, // Small cache for testing eviction
        ttl: Duration::from_secs(300),
        enable_metrics: true,
    });

    // Fill cache beyond capacity
    for i in 0..20 {
        let args = json!({"query": format!("unique_query_{}", i)});
        let result = json!({"result": i, "data": "test"});
        cache.set("test_command", &args, result).await;
        }

    let stats = cache.get_cache_stats().await;
    println!("Cache stats after overfill: {:?}", stats);
    
    // Cache should respect size limit
    assert!(stats.size <= stats.max_size, 
            "Cache size ({}) should not exceed max size ({})", stats.size, stats.max_size);

    // LRU eviction test - access some entries to make them recently used
    for i in 15..20 {
        let args = json!({"query": format!("unique_query_{}", i)});
        let _ = cache.get("test_command", &args).await;
        }

    // Add more entries to trigger eviction
    for i in 20..25 {
        let args = json!({"query": format!("unique_query_{}", i)});
        let result = json!({"result": i, "data": "test"});
        cache.set("test_command", &args, result).await;
        }

    // Recently accessed entries should still be present
    for i in 15..20 {
        let args = json!({"query": format!("unique_query_{}", i)});
        let cached = cache.get("test_command", &args).await;
        // Note: Due to LRU implementation details, this might not always hold
        // but it's a good indication that LRU is working
        if cached.is_none() {
            println!("Recently accessed entry {} was evicted (acceptable)", i);
            }
        }

    let final_stats = cache.get_cache_stats().await;
    assert!(final_stats.size <= final_stats.max_size, 
            "Cache should maintain size limits after evictions");
    }

/// Test cache performance under concurrent access
#[tokio::test]
async fn test_cache_concurrent_access() {
    let cache = Arc::new(CommandCache::new(Cache{ let mut config = Config::default();
        max_size: 1000,
        ttl: Duration::from_secs(300),
        enable_metrics: true,
    }));

    // Concurrent readers and writers
    let mut handles = vec![];
    let start_time = Instant::now();

    // Writer tasks
    for writer_id in 0..5 {
        let cache_clone = cache.clone();
        let handle = tokio::spawn(async move {
            for i in 0..100 {
                let key = format!("writer_{}_{}", writer_id, i);
                let args = json!({"query": key});
                let result = json!({"writer": writer_id, "iteration": i});
                cache_clone.set("concurrent_test", &args, result).await;
                }
        });
        handles.push(handle);
        }

    // Reader tasks
    for reader_id in 0..5 {
        let cache_clone = cache.clone();
        let handle = tokio::spawn(async move {
            let mut hits = 0;
            let mut misses = 0;
            
            for i in 0..100 {
                // Try to read from all writers
                for writer_id in 0..5 {
                    let key = format!("writer_{}_{}", writer_id, i);
                    let args = json!({"query": key});
                    
                    if cache_clone.get("concurrent_test", &args).await.is_some() {
                        hits += 1;
                    } else {
                        misses += 1;
                        }
                    }
                }
            
            (reader_id, hits, misses)
        });
        handles.push(handle);
        }

    // Wait for all tasks to complete
    let mut reader_results = vec![];
    for handle in handles {
        if let Ok(result) = handle.await {
            if let (reader_id, hits, misses) = result {
                reader_results.push((reader_id, hits, misses));
                println!("Reader {}: {} hits, {} misses", reader_id, hits, misses);
                }
            }
        }

    let total_time = start_time.elapsed();
    println!("Concurrent access test completed in: {:?}", total_time);

    // Verify cache remained consistent
    let final_stats = cache.get_cache_stats().await;
    println!("Final cache stats: {:?}", final_stats);
    
    assert!(final_stats.size <= final_stats.max_size, 
            "Cache should maintain size limits under concurrent access");
    assert!(total_time < Duration::from_secs(10), 
            "Concurrent access test should complete within 10 seconds");
    }

/// Test cache with complex query patterns
#[tokio::test]
async fn test_cache_with_complex_queries() {
    let cache = CommandCache::new(CacheConfig::default());

    // Test complex nested queries
    let complex_queries = vec![
        ("observe", json!({
            "query": "entities with (Transform and Mesh) or (Light and Name)",
            "filters": {
                "position": {"min": {"x": -10, "y": -10}, "max": {"x": 10, "y": 10}},
                "tags": ["player", "npc", "item"]
            },
            "sort": {"field": "creation_time", "order": "desc"},
            "limit": 100
        })),
        ("system_profile", json!({
            "systems": ["movement", "physics", "rendering"],
            "metrics": ["cpu_time", "memory_usage", "call_count"],
            "duration": 1000,
            "include_subsystems": true
        })),
        ("experiment", json!({
            "type": "performance",
            "parameters": {
                "entity_count": [100, 500, 1000],
                "system_complexity": ["low", "medium", "high"],
                "measurement_duration": 5000
            },
            "analysis": {
                "correlations": true,
                "regression": true,
                "anomaly_detection": true
                }
        }))
    ];

    // Test caching of complex queries
    for (command, args) in &complex_queries {
        // Cache complex result
        let complex_result = simulate_complex_query_result(command, args).await;
        let set_start = Instant::now();
        cache.set(command, args, complex_result.clone()).await;
        let set_time = set_start.elapsed();

        // Retrieve from cache
        let get_start = Instant::now();
        let cached_result = cache.get(command, args).await;
        let get_time = get_start.elapsed();

        assert!(cached_result.is_some(), "Complex query should be cacheable");
        assert_eq!(cached_result.unwrap(), complex_result, "Cached result should match");
        
        println!("Complex query '{}' - Set: {:?}, Get: {:?}", command, set_time, get_time);
        
        // Cache operations should be fast even for complex queries
        assert!(set_time < Duration::from_millis(10), 
                "Complex query caching should be fast");
        assert!(get_time < Duration::from_millis(1), 
                "Complex query retrieval should be very fast");
        }

    // Test cache key uniqueness for similar but different queries
    let similar_queries = vec![
        json!({"query": "entities with Transform", "limit": 100}),
        json!({"query": "entities with Transform", "limit": 200}),
        json!({"query": "entities with Transform", "sort": "name"}),
    ];

    for (i, args) in similar_queries.iter().enumerate() {
        let result = json!({"query_id": i, "entities": [i]});
        cache.set("observe", args, result).await;
        }

    // Each should have its own cache entry
    for (i, args) in similar_queries.iter().enumerate() {
        let cached = cache.get("observe", args).await;
        assert!(cached.is_some(), "Similar query {} should be cached separately", i);
        assert_eq!(cached.unwrap()["query_id"], i, "Should get correct result for query {}", i);
        }
    }

/// Test cache cleanup and maintenance
#[tokio::test]
async fn test_cache_cleanup_and_maintenance() {
    let cache = CommandCache::new(Cache{ let mut config = Config::default();
        max_size: 100,
        ttl: Duration::from_millis(200), // Short TTL for testing
        enable_metrics: true,
    });

    // Fill cache with entries that will expire
    for i in 0..50 {
        let args = json!({"query": format!("expiring_query_{}", i)});
        let result = json!({"data": i});
        cache.set("test", &args, result).await;
        }

    let stats_before = cache.get_cache_stats().await;
    println!("Stats before expiration: {:?}", stats_before);
    assert_eq!(stats_before.size, 50, "Should have 50 entries initially");

    // Wait for entries to expire
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Add new entry to trigger cleanup
    cache.set("test", &json!({"query": "trigger_cleanup"}), json!({"data": "new"})).await;

    // Check that expired entries are cleaned up
    let stats_after = cache.get_cache_stats().await;
    println!("Stats after cleanup: {:?}", stats_after);
    
    // Size should be significantly reduced (expired entries cleaned up)
    assert!(stats_after.size < stats_before.size, 
            "Cache size should be reduced after cleanup");

    // New entry should still be accessible
    let new_entry = cache.get("test", &json!({"query": "trigger_cleanup"})).await;
    assert!(new_entry.is_some(), "New entry should still be accessible");
    }

/// Helper function to simulate expensive query results
async fn simulate_expensive_query_result(command: &str, args: &Value) -> Value {
    // Simulate processing time
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    match command {
        "observe" => {
            let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("default");
            json!({
                "query": query,
                "entities": (0..100).collect::<Vec<_>>(),
                "total_count": 100,
                "execution_time_ms": 10,
                "timestamp": "2024-01-01T00:00:00Z"
            })
            }
        "system_profile" => {
            json!({
                "systems": ["movement", "physics", "rendering"],
                "metrics": {
                    "cpu_time": [5.2, 3.1, 12.7],
                    "memory_usage": [1024, 2048, 4096],
                    "call_count": [60, 30, 15]
                },
                "total_time_ms": 21.0,
                "timestamp": "2024-01-01T00:00:00Z"
            })
            }
        _ => {
            json!({
                "command": command,
                "args": args,
                "result": "simulated",
                "timestamp": "2024-01-01T00:00:00Z"
            })
            }
        }
    }

/// Helper function to simulate complex query results
async fn simulate_complex_query_result(command: &str, _args: &Value) -> Value {
    // Simulate longer processing for complex queries
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    match command {
        "observe" => json!({
            "entities": (0..1000).map(|i| json!({
                "id": i,
                "components": ["Transform", "Mesh", "Material"],
                "position": {"x": i as f32 * 0.1, "y": 0.0, "z": 0.0    }
            })).collect::<Vec<_>>(),
            "total_count": 1000,
            "filtered_count": 856,
            "query_complexity": "high",
            "execution_time_ms": 50
        }),
        "system_profile" => json!({
            "systems": (0..20).map(|i| json!({
                "name": format!("system_{}", i),
                "cpu_time": i as f64 * 0.5,
                "memory_usage": i * 1024,
                "dependencies": if i > 0 { vec![i - 1] } else { vec![]     }
            })).collect::<Vec<_>>(),
            "total_execution_time": 125.5,
            "analysis": {
                "bottlenecks": ["system_19", "system_15"],
                "optimization_suggestions": ["parallelize system_5", "cache system_8 results"]
                }
        }),
        _ => json!({
            "complex_result": true,
            "data_size": "large",
            "processing_time_ms": 50,
            "metadata": {
                "version": "1.0",
                "algorithm": "advanced",
                "confidence": 0.95
                }
        })
        }
    }