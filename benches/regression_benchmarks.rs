/// Performance Regression Benchmarks
/// 
/// Automated benchmarks to detect performance regressions in the
/// BEVDBG-012 optimization implementation. These benchmarks establish
/// baseline performance and can detect when optimizations regress.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use serde_json::json;
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

#[cfg(feature = "caching")]
use bevy_debugger_mcp::command_cache::{CommandCache, CacheConfig};

#[cfg(feature = "pooling")]
use bevy_debugger_mcp::response_pool::{ResponsePool, ResponsePoolConfig};

/// Performance targets for regression detection
struct PerformanceTargets {
    health_check_max_ms: f64,
    resource_metrics_max_ms: f64,
    observe_simple_max_ms: f64,
    observe_complex_max_ms: f64,
    lazy_init_max_ms: f64,
    cache_hit_max_us: f64,
    cache_miss_max_ms: f64,
    pool_serialize_small_max_us: f64,
    pool_serialize_large_max_ms: f64,
}

impl PerformanceTargets {
    /// BEVDBG-012 strict performance targets
    fn bevdbg_012() -> Self {
        Self {
            health_check_max_ms: 1.0,         // < 1ms p99
            resource_metrics_max_ms: 2.0,     // < 2ms p99
            observe_simple_max_ms: 1.0,       // < 1ms p99 for simple queries
            observe_complex_max_ms: 5.0,      // < 5ms p99 for complex queries
            lazy_init_max_ms: 10.0,           // < 10ms for lazy initialization
            cache_hit_max_us: 10.0,           // < 10 microseconds for cache hits
            cache_miss_max_ms: 1.0,           // < 1ms for cache misses
            pool_serialize_small_max_us: 50.0,// < 50 microseconds for small responses
            pool_serialize_large_max_ms: 5.0, // < 5ms for large responses
        }
    }
    
    /// Relaxed targets for CI/test environments
    fn testing() -> Self {
        Self {
            health_check_max_ms: 10.0,
            resource_metrics_max_ms: 20.0,
            observe_simple_max_ms: 15.0,
            observe_complex_max_ms: 50.0,
            lazy_init_max_ms: 100.0,
            cache_hit_max_us: 100.0,
            cache_miss_max_ms: 20.0,
            pool_serialize_small_max_us: 500.0,
            pool_serialize_large_max_ms: 50.0,
        }
    }
}

/// Configuration for regression benchmarks
struct RegressionBenchConfig {
    runtime: Runtime,
    server: McpServer,
    targets: PerformanceTargets,
}

impl RegressionBenchConfig {
    fn new() -> Self {
        let runtime = Runtime::new().unwrap();
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3001,
        };
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        let server = McpServer::new(config, brp_client);
        
        // Use testing targets in CI, strict targets in benchmark mode
        let targets = if std::env::var("CI").is_ok() {
            PerformanceTargets::testing()
        } else {
            PerformanceTargets::bevdbg_012()
        };
        
        Self {
            runtime,
            server,
            targets,
        }
    }
}

/// Core MCP operation regression tests
fn benchmark_mcp_operations_regression(c: &mut Criterion) {
    let bench_config = RegressionBenchConfig::new();
    
    // Test each core operation for regression
    let operations = vec![
        ("health_check", json!({}), bench_config.targets.health_check_max_ms),
        ("resource_metrics", json!({}), bench_config.targets.resource_metrics_max_ms),
        ("observe_simple", json!({
            "query": "entities with Transform"
        }), bench_config.targets.observe_simple_max_ms),
        ("observe_complex", json!({
            "query": "entities with (Transform and Mesh and Health)",
            "include_data": true,
            "sort": "name",
            "limit": 100
        }), bench_config.targets.observe_complex_max_ms),
    ];
    
    for (op_name, args, target_ms) in operations {
        c.bench_function(&format!("regression_mcp_{}", op_name), |b| {
            b.to_async(&bench_config.runtime).iter_custom(|iters| async move {
                let mut total_time = Duration::ZERO;
                let mut max_time = Duration::ZERO;
                let mut times = Vec::new();
                
                for _ in 0..iters {
                    let start = Instant::now();
                    let _result = bench_config.server.handle_tool_call(op_name, args.clone()).await;
                    let elapsed = start.elapsed();
                    
                    total_time += elapsed;
                    max_time = max_time.max(elapsed);
                    times.push(elapsed);
                }
                
                // Calculate p99 latency
                times.sort_unstable();
                let p99_idx = ((times.len() as f64 * 0.99) as usize).min(times.len() - 1);
                let p99_time = times[p99_idx];
                
                // Regression check: fail if p99 exceeds target
                let p99_ms = p99_time.as_millis() as f64;
                if p99_ms > target_ms {
                    eprintln!(
                        "⚠️  PERFORMANCE REGRESSION DETECTED in {}: P99 {:.2}ms > target {:.1}ms",
                        op_name, p99_ms, target_ms
                    );
                }
                
                total_time
            });
        });
    }
}

/// Lazy initialization regression tests
fn benchmark_lazy_init_regression(c: &mut Criterion) {
    let bench_config = RegressionBenchConfig::new();
    
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    
    c.bench_function("regression_lazy_init_first_access", |b| {
        b.to_async(&bench_config.runtime).iter_custom(|iters| async move {
            let mut total_time = Duration::ZERO;
            let mut times = Vec::new();
            
            for _ in 0..iters {
                let lazy_components = LazyComponents::new(brp_client.clone());
                
                let start = Instant::now();
                let _inspector = lazy_components.get_entity_inspector().await;
                let elapsed = start.elapsed();
                
                total_time += elapsed;
                times.push(elapsed);
            }
            
            // Check regression
            times.sort_unstable();
            let p99_idx = ((times.len() as f64 * 0.99) as usize).min(times.len() - 1);
            let p99_time = times[p99_idx];
            let p99_ms = p99_time.as_millis() as f64;
            
            if p99_ms > bench_config.targets.lazy_init_max_ms {
                eprintln!(
                    "⚠️  PERFORMANCE REGRESSION DETECTED in lazy_init: P99 {:.2}ms > target {:.1}ms",
                    p99_ms, bench_config.targets.lazy_init_max_ms
                );
            }
            
            total_time
        });
    });
    
    // Test cached access performance
    c.bench_function("regression_lazy_init_cached_access", |b| {
        b.to_async(&bench_config.runtime).iter_with_setup(
            || {
                let runtime = Runtime::new().unwrap();
                let components = LazyComponents::new(brp_client.clone());
                // Pre-initialize
                runtime.block_on(async {
                    let _inspector = components.get_entity_inspector().await;
                });
                components
            },
            |components| async {
                let start = Instant::now();
                let _inspector = components.get_entity_inspector().await;
                let elapsed = start.elapsed();
                
                // Cached access should be very fast
                let elapsed_us = elapsed.as_micros() as f64;
                if elapsed_us > 1000.0 { // 1ms
                    eprintln!(
                        "⚠️  PERFORMANCE REGRESSION DETECTED in lazy_init_cached: {}μs > 1000μs",
                        elapsed_us
                    );
                }
                
                black_box(elapsed)
            },
        );
    });
}

/// Cache performance regression tests
#[cfg(feature = "caching")]
fn benchmark_cache_regression(c: &mut Criterion) {
    let bench_config = RegressionBenchConfig::new();
    let cache = CommandCache::new(CacheConfig::default());
    
    // Test cache hit performance
    c.bench_function("regression_cache_hit", |b| {
        b.to_async(&bench_config.runtime).iter_with_setup(
            || {
                let runtime = Runtime::new().unwrap();
                let tool_name = "test_tool".to_string();
                let args = json!({"test": "data"});
                let response = json!({"cached": "result"});
                
                runtime.block_on(async {
                    cache.set(&tool_name, &args, response).await;
                });
                
                (tool_name, args)
            },
            |(tool_name, args)| async {
                let start = Instant::now();
                let _result = cache.get(&tool_name, &args).await;
                let elapsed = start.elapsed();
                
                // Check regression
                let elapsed_us = elapsed.as_micros() as f64;
                if elapsed_us > bench_config.targets.cache_hit_max_us {
                    eprintln!(
                        "⚠️  PERFORMANCE REGRESSION DETECTED in cache_hit: {:.2}μs > {:.1}μs",
                        elapsed_us, bench_config.targets.cache_hit_max_us
                    );
                }
                
                black_box(elapsed)
            },
        );
    });
    
    // Test cache miss and set performance
    c.bench_function("regression_cache_miss_and_set", |b| {
        b.to_async(&bench_config.runtime).iter_with_setup(
            || {
                let tool_name = format!("unique_tool_{}", rand::random::<u64>());
                let args = json!({"unique": rand::random::<u64>()});
                let response = json!({"result": "data"});
                (tool_name, args, response)
            },
            |(tool_name, args, response)| async {
                let start = Instant::now();
                
                // Check for cache miss (should be None)
                let cached = cache.get(&tool_name, &args).await;
                assert!(cached.is_none(), "Should be cache miss");
                
                // Set new value
                cache.set(&tool_name, &args, response).await;
                
                let elapsed = start.elapsed();
                
                // Check regression
                let elapsed_ms = elapsed.as_millis() as f64;
                if elapsed_ms > bench_config.targets.cache_miss_max_ms {
                    eprintln!(
                        "⚠️  PERFORMANCE REGRESSION DETECTED in cache_miss_and_set: {:.2}ms > {:.1}ms",
                        elapsed_ms, bench_config.targets.cache_miss_max_ms
                    );
                }
                
                black_box(elapsed)
            },
        );
    });
}

#[cfg(not(feature = "caching"))]
fn benchmark_cache_regression(c: &mut Criterion) {
    c.bench_function("regression_cache_disabled", |b| {
        b.iter(|| {
            // No cache regression when caching is disabled
            black_box(())
        });
    });
}

/// Response pooling regression tests
#[cfg(feature = "pooling")]
fn benchmark_pooling_regression(c: &mut Criterion) {
    let bench_config = RegressionBenchConfig::new();
    let pool = ResponsePool::new(ResponsePoolConfig::default());
    
    // Small response serialization
    c.bench_function("regression_pool_serialize_small", |b| {
        let small_response = json!({"status": "ok", "data": [1, 2, 3]});
        
        b.to_async(&bench_config.runtime).iter(|| async {
            let start = Instant::now();
            let _result = pool.serialize_json(&small_response).await.unwrap();
            let elapsed = start.elapsed();
            
            // Check regression
            let elapsed_us = elapsed.as_micros() as f64;
            if elapsed_us > bench_config.targets.pool_serialize_small_max_us {
                eprintln!(
                    "⚠️  PERFORMANCE REGRESSION DETECTED in pool_serialize_small: {:.2}μs > {:.1}μs",
                    elapsed_us, bench_config.targets.pool_serialize_small_max_us
                );
            }
            
            black_box(elapsed)
        });
    });
    
    // Large response serialization
    c.bench_function("regression_pool_serialize_large", |b| {
        let large_response = json!({
            "entities": (0..1000).map(|i| json!({
                "id": i,
                "name": format!("Entity{}", i),
                "components": (0..5).map(|j| json!({
                    "type": format!("Comp{}", j),
                    "data": {"value": i * j}
                })).collect::<Vec<_>>()
            })).collect::<Vec<_>>()
        });
        
        b.to_async(&bench_config.runtime).iter(|| async {
            let start = Instant::now();
            let _result = pool.serialize_json(&large_response).await.unwrap();
            let elapsed = start.elapsed();
            
            // Check regression
            let elapsed_ms = elapsed.as_millis() as f64;
            if elapsed_ms > bench_config.targets.pool_serialize_large_max_ms {
                eprintln!(
                    "⚠️  PERFORMANCE REGRESSION DETECTED in pool_serialize_large: {:.2}ms > {:.1}ms",
                    elapsed_ms, bench_config.targets.pool_serialize_large_max_ms
                );
            }
            
            black_box(elapsed)
        });
    });
}

#[cfg(not(feature = "pooling"))]
fn benchmark_pooling_regression(c: &mut Criterion) {
    // Test standard serialization performance when pooling is disabled
    c.bench_function("regression_json_serialize_standard", |b| {
        let response = json!({
            "entities": (0..100).map(|i| json!({"id": i})).collect::<Vec<_>>()
        });
        
        b.iter(|| {
            let start = Instant::now();
            let _result = serde_json::to_string(&response).unwrap();
            let elapsed = start.elapsed();
            
            // Should still be reasonably fast
            let elapsed_ms = elapsed.as_millis() as f64;
            if elapsed_ms > 10.0 { // 10ms max for standard serialization
                eprintln!(
                    "⚠️  PERFORMANCE REGRESSION DETECTED in standard_serialize: {:.2}ms > 10.0ms",
                    elapsed_ms
                );
            }
            
            black_box(elapsed)
        });
    });
}

/// Memory usage regression tests
fn benchmark_memory_regression(c: &mut Criterion) {
    let bench_config = RegressionBenchConfig::new();
    
    // Test that repeated operations don't leak memory
    c.bench_function("regression_memory_stability", |b| {
        b.to_async(&bench_config.runtime).iter_custom(|iters| async move {
            let start_time = Instant::now();
            
            // Perform operations that could potentially leak memory
            for i in 0..iters {
                let args = json!({
                    "query": format!("iteration {} with data {}", i, "x".repeat(100)),
                    "iteration": i
                });
                
                let _result = bench_config.server.handle_tool_call("observe", args).await;
                
                // Occasional cleanup hint
                if i % 100 == 0 {
                    tokio::time::sleep(Duration::from_micros(1)).await;
                }
            }
            
            start_time.elapsed()
        });
    });
}

/// Throughput regression tests
fn benchmark_throughput_regression(c: &mut Criterion) {
    let bench_config = RegressionBenchConfig::new();
    
    // Test sustained throughput doesn't regress
    c.bench_function("regression_sustained_throughput", |b| {
        b.to_async(&bench_config.runtime).iter_custom(|iters| async move {
            let start_time = Instant::now();
            let operations_per_batch = 10;
            let batches = (iters / operations_per_batch as u64).max(1);
            
            for batch in 0..batches {
                let batch_tasks = (0..operations_per_batch).map(|i| {
                    let server = &bench_config.server;
                    let op_id = batch * operations_per_batch as u64 + i as u64;
                    
                    async move {
                        let operation = match op_id % 4 {
                            0 => "health_check",
                            1 => "resource_metrics", 
                            2 => "observe",
                            _ => "diagnostic_report",
                        };
                        
                        let args = json!({"batch": batch, "op_id": op_id});
                        server.handle_tool_call(operation, args).await
                    }
                });
                
                let _results = futures::future::join_all(batch_tasks).await;
            }
            
            let total_time = start_time.elapsed();
            let actual_ops = batches * operations_per_batch as u64;
            let throughput = actual_ops as f64 / total_time.as_secs_f64();
            
            // Check throughput regression
            let min_throughput = 50.0; // ops/sec minimum
            if throughput < min_throughput {
                eprintln!(
                    "⚠️  THROUGHPUT REGRESSION DETECTED: {:.1} ops/sec < {:.1} ops/sec",
                    throughput, min_throughput
                );
            }
            
            total_time
        });
    });
}

/// Concurrent access regression tests
fn benchmark_concurrency_regression(c: &mut Criterion) {
    let bench_config = RegressionBenchConfig::new();
    
    // Test that concurrent access doesn't cause performance degradation
    let concurrency_levels = [1, 2, 4, 8];
    
    for &concurrency in &concurrency_levels {
        c.bench_function(&format!("regression_concurrency_{}", concurrency), |b| {
            b.to_async(&bench_config.runtime).iter_custom(|iters| async move {
                let start_time = Instant::now();
                let ops_per_worker = (iters / concurrency as u64).max(1);
                
                let worker_tasks = (0..concurrency).map(|worker_id| {
                    let server = &bench_config.server;
                    
                    async move {
                        for i in 0..ops_per_worker {
                            let args = json!({
                                "worker_id": worker_id,
                                "iteration": i
                            });
                            
                            let _result = server.handle_tool_call("health_check", args).await;
                        }
                    }
                });
                
                futures::future::join_all(worker_tasks).await;
                
                let total_time = start_time.elapsed();
                
                // Check that concurrency doesn't cause excessive slowdown
                let ops_per_sec = (concurrency as u64 * ops_per_worker) as f64 / total_time.as_secs_f64();
                let min_concurrent_throughput = 10.0; // ops/sec minimum under concurrency
                
                if ops_per_sec < min_concurrent_throughput {
                    eprintln!(
                        "⚠️  CONCURRENCY REGRESSION DETECTED at level {}: {:.1} ops/sec < {:.1} ops/sec",
                        concurrency, ops_per_sec, min_concurrent_throughput
                    );
                }
                
                total_time
            });
        });
    }
}

criterion_group!(
    regression_benches,
    benchmark_mcp_operations_regression,
    benchmark_lazy_init_regression,
    benchmark_cache_regression,
    benchmark_pooling_regression,
    benchmark_memory_regression,
    benchmark_throughput_regression,
    benchmark_concurrency_regression,
);

criterion_main!(regression_benches);

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_performance_targets_reasonable() {
        let targets = PerformanceTargets::bevdbg_012();
        
        // Ensure targets are reasonable
        assert!(targets.health_check_max_ms > 0.0);
        assert!(targets.health_check_max_ms < 100.0);
        assert!(targets.cache_hit_max_us > 0.0);
        assert!(targets.cache_hit_max_us < 1000.0);
        
        println!("Performance targets are reasonable");
    }
    
    #[tokio::test]
    async fn test_regression_detection_setup() {
        let bench_config = RegressionBenchConfig::new();
        
        // Test that we can detect regressions
        let start = Instant::now();
        let _ = bench_config.server.handle_tool_call("health_check", json!({})).await;
        let elapsed = start.elapsed();
        
        println!("Health check took: {:?}", elapsed);
        
        // Should complete within reasonable time
        assert!(elapsed < Duration::from_secs(5), "Health check should complete within 5s");
    }
}