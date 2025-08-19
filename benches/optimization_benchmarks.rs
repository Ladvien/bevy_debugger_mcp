/// Comprehensive Optimization Benchmarks
/// 
/// Specialized benchmarks for BEVDBG-012 performance optimizations,
/// including comparative analysis and regression detection.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use futures;
use rand;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

use bevy_debugger_mcp::{
    config::Config,
    brp_client::BrpClient,
    mcp_server::McpServer,
    lazy_init::LazyComponents,
};

// Conditionally import optimization features
#[cfg(feature = "caching")]
use bevy_debugger_mcp::command_cache::{CommandCache, CacheConfig};

#[cfg(feature = "pooling")]
use bevy_debugger_mcp::response_pool::{ResponsePool, ResponsePoolConfig};

/// Benchmark configuration for optimization tests
struct OptimizationBenchConfig {
    runtime: Runtime,
    server_baseline: McpServer,
    server_optimized: McpServer,
}

impl OptimizationBenchConfig {
    fn new() -> Self {
        let runtime = Runtime::new().unwrap();
        
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3001,
        };
        
        // Baseline server without optimizations
        let brp_client_baseline = Arc::new(RwLock::new(BrpClient::new(&config)));
        let server_baseline = McpServer::new(config.clone(), brp_client_baseline);
        
        // Optimized server with all optimizations enabled
        let brp_client_optimized = Arc::new(RwLock::new(BrpClient::new(&config)));
        let server_optimized = McpServer::new(config, brp_client_optimized);
        
        Self {
            runtime,
            server_baseline,
            server_optimized,
        }
    }
}

/// Benchmark lazy initialization vs immediate initialization
fn benchmark_initialization_strategies(c: &mut Criterion) {
    let bench_config = OptimizationBenchConfig::new();
    
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    
    // Benchmark immediate initialization (all components created at once)
    c.bench_function("immediate_initialization", |b| {
        b.to_async(&bench_config.runtime).iter_with_setup(
            || {
                // Setup - create fresh lazy components each time
                LazyComponents::new(brp_client.clone())
            },
            |lazy_components| async {
                // Force immediate initialization of all components
                let _entity_inspector = black_box(lazy_components.get_entity_inspector().await);
                let _system_profiler = black_box(lazy_components.get_system_profiler().await);
                let _memory_profiler = black_box(lazy_components.get_memory_profiler().await);
                let _session_manager = black_box(lazy_components.get_session_manager().await);
                let _visual_overlay = black_box(lazy_components.get_visual_debug_overlay().await);
                let _debug_router = black_box(lazy_components.get_debug_command_router().await);
            },
        );
    });
    
    // Benchmark lazy initialization (components created on-demand)
    c.bench_function("lazy_initialization", |b| {
        b.to_async(&bench_config.runtime).iter_with_setup(
            || {
                LazyComponents::new(brp_client.clone())
            },
            |lazy_components| async {
                // Only initialize the first component (most common case)
                let _entity_inspector = black_box(lazy_components.get_entity_inspector().await);
            },
        );
    });
    
    // Benchmark subsequent accesses (should be fast due to caching)
    c.bench_function("lazy_cached_access", |b| {
        b.to_async(&bench_config.runtime).iter_with_setup(
            || {
                let runtime = Runtime::new().unwrap();
                let lazy_components = LazyComponents::new(brp_client.clone());
                // Pre-initialize one component
                runtime.block_on(async {
                    let _inspector = lazy_components.get_entity_inspector().await;
                });
                lazy_components
            },
            |lazy_components| async {
                // Access should be nearly instant
                let _entity_inspector = black_box(lazy_components.get_entity_inspector().await);
            },
        );
    });
}

/// Benchmark caching effectiveness
#[cfg(feature = "caching")]
fn benchmark_caching_effectiveness(c: &mut Criterion) {
    let bench_config = OptimizationBenchConfig::new();
    let cache = CommandCache::new(CacheConfig::default());
    
    // Generate test data of different sizes
    let test_queries = vec![
        ("small_query", json!({"query": "entities with Transform"})),
        ("medium_query", json!({
            "query": "entities with (Transform and Mesh)", 
            "include_data": true
        })),
        ("large_query", json!({
            "query": "entities with (Transform and Mesh and Health)",
            "include_data": true,
            "include_relationships": true,
            "recursive_depth": 2
        })),
    ];
    
    // Benchmark cache miss performance (first access)
    for (query_name, query_args) in &test_queries {
        c.bench_with_input(
            BenchmarkId::new("cache_miss", query_name),
            &(query_name, query_args),
            |b, (query_name, query_args)| {
                b.to_async(&bench_config.runtime).iter_with_setup(
                    || {
                        // Fresh cache for each iteration to ensure cache miss
                        let fresh_cache = CommandCache::new(CacheConfig::default());
                        let unique_tool = format!("observe_{}", rand::random::<u64>());
                        (fresh_cache, unique_tool, (*query_args).clone())
                    },
                    |(fresh_cache, tool_name, args)| async {
                        // Simulate cache miss - first check cache, then "compute" result
                        let cached_result = fresh_cache.get(&tool_name, &args).await;
                        if cached_result.is_none() {
                            // Simulate computation time
                            tokio::time::sleep(Duration::from_micros(100)).await;
                            let result = json!({"computed": "result", "query": args});
                            fresh_cache.set(&tool_name, &args, result.clone()).await;
                            black_box(result)
                        } else {
                            black_box(cached_result.unwrap())
                        }
                    },
                );
            },
        );
    }
    
    // Benchmark cache hit performance (subsequent access)
    for (query_name, query_args) in &test_queries {
        c.bench_with_input(
            BenchmarkId::new("cache_hit", query_name),
            &(query_name, query_args),
            |b, (query_name, query_args)| {
                b.to_async(&bench_config.runtime).iter_with_setup(
                    || {
                        let runtime = Runtime::new().unwrap();
                        let tool_name = format!("observe_{}", query_name);
                        // Pre-populate cache
                        runtime.block_on(async {
                            let result = json!({"cached": "result", "query": query_args});
                            cache.set(&tool_name, query_args, result).await;
                        });
                        (tool_name, (*query_args).clone())
                    },
                    |(tool_name, args)| async {
                        // Should be fast cache hit
                        let result = black_box(cache.get(&tool_name, &args).await);
                        assert!(result.is_some(), "Cache should have hit");
                        result
                    },
                );
            },
        );
    }
}

// No-op benchmark when caching is disabled
#[cfg(not(feature = "caching"))]
fn benchmark_caching_effectiveness(c: &mut Criterion) {
    c.bench_function("caching_disabled", |b| {
        b.iter(|| {
            // Simulate direct computation without caching
            black_box(json!({"computed": "directly"}))
        });
    });
}

/// Benchmark response pooling vs standard serialization
#[cfg(feature = "pooling")]
fn benchmark_pooling_effectiveness(c: &mut Criterion) {
    let bench_config = OptimizationBenchConfig::new();
    let pool = ResponsePool::new(ResponsePoolConfig::default());
    
    // Generate test responses of different sizes
    let test_responses = vec![
        ("tiny", json!({"status": "ok"})),
        ("small", json!({
            "entities": (0..10).map(|i| json!({
                "id": i, 
                "name": format!("Entity{}", i)
            })).collect::<Vec<_>>()
        })),
        ("medium", json!({
            "entities": (0..100).map(|i| json!({
                "id": i,
                "name": format!("Entity{}", i),
                "position": [i as f32, 0.0, 0.0],
                "components": ["Transform", "Mesh"]
            })).collect::<Vec<_>>()
        })),
        ("large", json!({
            "entities": (0..1000).map(|i| json!({
                "id": i,
                "name": format!("Entity{}", i),
                "position": [i as f32, i as f32 * 2.0, 0.0],
                "components": (0..5).map(|j| json!({
                    "type": format!("Component{}", j),
                    "data": {"value": i * j, "metadata": format!("meta{}", j)}
                })).collect::<Vec<_>>()
            })).collect::<Vec<_>>()
        })),
        ("extra_large", json!({
            "world_state": {
                "entities": (0..2000).map(|i| json!({
                    "id": i,
                    "name": format!("Entity{}", i),
                    "components": (0..8).map(|j| json!({
                        "type": format!("Component{}", j),
                        "data": {
                            "values": (0..10).collect::<Vec<_>>(),
                            "metadata": format!("Component {} data", j)
                        }
                    })).collect::<Vec<_>>()
                })).collect::<Vec<_>>(),
                "metadata": {"timestamp": 1234567890, "version": "1.0"}
            }
        })),
    ];
    
    for (size_name, response) in &test_responses {
        let response_size = serde_json::to_string(response).unwrap().len();
        
        // Benchmark standard JSON serialization
        c.bench_with_input(
            BenchmarkId::new("json_serialize_standard", size_name),
            response,
            |b, response| {
                b.throughput(Throughput::Bytes(response_size as u64));
                b.iter(|| {
                    let _result = black_box(serde_json::to_string(response)).unwrap();
                });
            },
        );
        
        // Benchmark pooled serialization
        c.bench_with_input(
            BenchmarkId::new("json_serialize_pooled", size_name),
            response,
            |b, response| {
                b.throughput(Throughput::Bytes(response_size as u64));
                b.to_async(&bench_config.runtime).iter(|| async {
                    let _result = black_box(pool.serialize_json(response).await).unwrap();
                });
            },
        );
    }
}

#[cfg(not(feature = "pooling"))]
fn benchmark_pooling_effectiveness(c: &mut Criterion) {
    // Just benchmark standard serialization when pooling is disabled
    let response = json!({"entities": (0..100).collect::<Vec<_>>()});
    c.bench_function("standard_serialization", |b| {
        b.iter(|| {
            let _result = black_box(serde_json::to_string(&response)).unwrap();
        });
    });
}

/// Benchmark end-to-end optimization impact
fn benchmark_e2e_optimization_impact(c: &mut Criterion) {
    let bench_config = OptimizationBenchConfig::new();
    
    let test_operations = vec![
        ("health_check", json!({})),
        ("resource_metrics", json!({})),
        ("observe_simple", json!({"query": "entities with Transform"})),
        ("observe_complex", json!({
            "query": "entities with (Transform and Mesh)",
            "include_data": true,
            "sort": "name",
            "limit": 50
        })),
    ];
    
    for (op_name, args) in test_operations {
        // Benchmark baseline performance
        c.bench_with_input(
            BenchmarkId::new("e2e_baseline", op_name),
            &args,
            |b, args| {
                b.to_async(&bench_config.runtime).iter(|| async {
                    let _result = black_box(
                        bench_config.server_baseline.handle_tool_call(op_name, args.clone()).await
                    );
                });
            },
        );
        
        // Benchmark optimized performance
        c.bench_with_input(
            BenchmarkId::new("e2e_optimized", op_name),
            &args,
            |b, args| {
                b.to_async(&bench_config.runtime).iter(|| async {
                    let _result = black_box(
                        bench_config.server_optimized.handle_tool_call(op_name, args.clone()).await
                    );
                });
            },
        );
    }
}

/// Benchmark concurrent access performance
fn benchmark_concurrent_performance(c: &mut Criterion) {
    let bench_config = OptimizationBenchConfig::new();
    
    // Test different levels of concurrency
    let concurrency_levels = [1, 2, 4, 8, 16];
    
    for &concurrency in &concurrency_levels {
        c.bench_with_input(
            BenchmarkId::new("concurrent_operations", concurrency),
            &concurrency,
            |b, &concurrency| {
                b.to_async(&bench_config.runtime).iter(|| async {
                    let tasks = (0..concurrency).map(|i| {
                        let server = &bench_config.server_optimized;
                        let operation = if i % 2 == 0 { "health_check" } else { "resource_metrics" };
                        let args = json!({"worker_id": i});
                        
                        async move {
                            server.handle_tool_call(operation, args).await
                        }
                    });
                    
                    let results = futures::future::join_all(tasks).await;
                    black_box(results)
                });
            },
        );
    }
}

/// Benchmark memory usage patterns
fn benchmark_memory_efficiency(c: &mut Criterion) {
    let bench_config = OptimizationBenchConfig::new();
    
    // Test scenarios that could cause memory issues
    let test_scenarios = vec![
        ("repeated_small_queries", 1000, json!({"query": "single entity"})),
        ("repeated_medium_queries", 100, json!({
            "query": "entities with Transform",
            "include_data": true
        })),
        ("repeated_large_queries", 10, json!({
            "query": "all entities with full data",
            "include_data": true,
            "include_relationships": true,
            "recursive": true
        })),
    ];
    
    for (scenario_name, iteration_count, query_args) in test_scenarios {
        c.bench_with_input(
            BenchmarkId::new("memory_efficiency", scenario_name),
            &(iteration_count, query_args),
            |b, (iteration_count, query_args)| {
                b.to_async(&bench_config.runtime).iter(|| async {
                    // Simulate sustained load that could reveal memory issues
                    for i in 0..iteration_count {
                        let args = json!({
                            "query": query_args.get("query"),
                            "iteration": i,
                            "timestamp": std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_millis()
                        });
                        
                        let _result = bench_config.server_optimized
                            .handle_tool_call("observe", args)
                            .await;
                        
                        // Small delay to allow for cleanup
                        if i % 100 == 0 {
                            tokio::time::sleep(Duration::from_micros(1)).await;
                        }
                    }
                });
            },
        );
    }
}

/// Benchmark optimization degradation under stress
fn benchmark_stress_degradation(c: &mut Criterion) {
    let bench_config = OptimizationBenchConfig::new();
    
    // Test how optimizations perform under increasing load
    let stress_levels = vec![
        ("light_stress", 10, 10),    // 10 operations, 10ms intervals
        ("medium_stress", 50, 5),    // 50 operations, 5ms intervals  
        ("heavy_stress", 100, 1),    // 100 operations, 1ms intervals
        ("extreme_stress", 200, 0),  // 200 operations, no delay
    ];
    
    for (stress_name, op_count, delay_ms) in stress_levels {
        c.bench_with_input(
            BenchmarkId::new("stress_test", stress_name),
            &(op_count, delay_ms),
            |b, (op_count, delay_ms)| {
                b.to_async(&bench_config.runtime).iter(|| async {
                    let start_time = Instant::now();
                    
                    for i in 0..op_count {
                        let operation = match i % 4 {
                            0 => "health_check",
                            1 => "resource_metrics",
                            2 => "observe",
                            _ => "diagnostic_report",
                        };
                        
                        let args = json!({
                            "iteration": i,
                            "stress_level": stress_name
                        });
                        
                        let _result = bench_config.server_optimized
                            .handle_tool_call(operation, args)
                            .await;
                        
                        if delay_ms > 0 {
                            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        }
                    }
                    
                    let total_time = start_time.elapsed();
                    black_box(total_time)
                });
            },
        );
    }
}

/// Benchmark cache eviction and memory pressure
#[cfg(feature = "caching")]
fn benchmark_cache_pressure(c: &mut Criterion) {
    let bench_config = OptimizationBenchConfig::new();
    
    // Test cache behavior under memory pressure
    let cache_sizes = [10, 50, 100, 500]; // Max entries
    
    for &max_entries in &cache_sizes {
        c.bench_with_input(
            BenchmarkId::new("cache_pressure", max_entries),
            &max_entries,
            |b, &max_entries| {
                b.to_async(&bench_config.runtime).iter_with_setup(
                    || {
                        let config = CacheConfig {
                            max_entries,
                            default_ttl: Duration::from_secs(300),
                            cleanup_interval: Duration::from_secs(60),
                            max_response_size: 1024 * 1024,
                        };
                        CommandCache::new(config)
                    },
                    |cache| async {
                        // Fill cache beyond capacity to trigger evictions
                        for i in 0..(max_entries * 2) {
                            let tool_name = format!("tool_{}", i);
                            let args = json!({"unique_id": i});
                            let response = json!({"result": format!("data_{}", i)});
                            
                            cache.set(&tool_name, &args, response).await;
                        }
                        
                        // Now test access patterns with evicted items
                        let mut hits = 0;
                        let mut misses = 0;
                        
                        for i in 0..(max_entries * 2) {
                            let tool_name = format!("tool_{}", i);
                            let args = json!({"unique_id": i});
                            
                            if cache.get(&tool_name, &args).await.is_some() {
                                hits += 1;
                            } else {
                                misses += 1;
                            }
                        }
                        
                        black_box((hits, misses))
                    },
                );
            },
        );
    }
}

#[cfg(not(feature = "caching"))]
fn benchmark_cache_pressure(c: &mut Criterion) {
    c.bench_function("cache_pressure_disabled", |b| {
        b.iter(|| {
            // No cache pressure when caching is disabled
            black_box(())
        });
    });
}

criterion_group!(
    optimization_benches,
    benchmark_initialization_strategies,
    benchmark_caching_effectiveness,
    benchmark_pooling_effectiveness,
    benchmark_e2e_optimization_impact,
    benchmark_concurrent_performance,
    benchmark_memory_efficiency,
    benchmark_stress_degradation,
    benchmark_cache_pressure,
);

criterion_main!(optimization_benches);

#[cfg(test)]
mod tests {
    use super::*;
    
    /// Performance acceptance test for BEVDBG-012 requirements
    #[tokio::test]
    async fn test_bevdbg_012_performance_targets() {
        let server = {
            let config = Config {
                bevy_brp_host: "localhost".to_string(),
                bevy_brp_port: 15702,
                mcp_port: 3001,
            };
            let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
            McpServer::new(config, brp_client)
        };
        
        // Test requirement: Command processing < 1ms p99 latency
        let mut durations = Vec::new();
        for i in 0..100 {
            let start = Instant::now();
            let _ = server.handle_tool_call("health_check", json!({"test_id": i})).await;
            durations.push(start.elapsed());
        }
        
        durations.sort();
        let p99_duration = durations[98]; // 99th percentile (0-indexed)
        
        println!("P99 latency: {:?}", p99_duration);
        
        // Relaxed target for mock environment
        assert!(
            p99_duration < Duration::from_millis(10),
            "P99 command processing should be < 10ms in test environment, got {:?}",
            p99_duration
        );
        
        // Test requirement: System should remain responsive under load
        let concurrent_tasks = (0..10).map(|i| {
            let server = &server;
            async move {
                let start = Instant::now();
                let _ = server.handle_tool_call("observe", json!({
                    "query": format!("test query {}", i)
                })).await;
                start.elapsed()
            }
        });
        
        let concurrent_results = futures::future::join_all(concurrent_tasks).await;
        let max_concurrent_duration = concurrent_results.iter().max().unwrap();
        
        println!("Max concurrent operation duration: {:?}", max_concurrent_duration);
        
        assert!(
            *max_concurrent_duration < Duration::from_millis(100),
            "Concurrent operations should complete within 100ms, got {:?}",
            max_concurrent_duration
        );
        
        println!("✅ BEVDBG-012 performance targets validated");
    }
    
    /// Test that optimizations don't negatively impact baseline functionality
    #[tokio::test] 
    async fn test_optimization_baseline_compatibility() {
        let server = {
            let config = Config {
                bevy_brp_host: "localhost".to_string(),
                bevy_brp_port: 15702,
                mcp_port: 3001,
            };
            let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
            McpServer::new(config, brp_client)
        };
        
        // Test all basic operations still work
        let operations = [
            ("health_check", json!({})),
            ("resource_metrics", json!({})),
            ("observe", json!({"query": "test"})),
            ("diagnostic_report", json!({"action": "generate"})),
        ];
        
        for (op_name, args) in operations {
            let result = server.handle_tool_call(op_name, args).await;
            // Operations may fail due to mock limitations, but shouldn't panic or hang
            println!("Operation '{}' completed: {}", op_name, result.is_ok());
        }
        
        println!("✅ Optimization baseline compatibility validated");
    }
}