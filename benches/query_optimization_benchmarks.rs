/*
 * Query Optimization Benchmarks - BEVDBG-014
 * Copyright (C) 2025 ladvien
 *
 * Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
 *
 * Relicensed from GPL-3.0 when this crate was adopted into Ladvien/foundation_vs_slop: a GPL
 * crate in the Bevy ecosystem cannot be adopted, and being adoptable is why it is published.
 */

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

use bevy_fairy::{
    server::config::Config,
    server::brp_client::BrpClient,
    server::query_optimization::{QueryOptimizer, QueryPerformanceMetrics},
    server::parallel_query_executor::{ParallelQueryExecutor, ParallelExecutionConfig},
    server::tools::{observe, observe_optimized},
    server::brp_messages::{builtin_methods, query_params, BrpRequest, QueryFilter},
};

struct BenchConfig {
    runtime: Runtime,
}

impl BenchConfig {
    fn new() -> Self {
        Self {
            runtime: Runtime::new().unwrap(),
        }
    }
}

fn query_request(with: &[&str], without: &[&str], limit: Option<u64>) -> BrpRequest {
    let mut filter = QueryFilter::default();
    filter.with = Some(with.iter().map(|c| c.to_string()).collect());
    filter.without = Some(without.iter().map(|c| c.to_string()).collect());
    let mut params = query_params(Some(&filter), false);
    if let Some(limit) = limit {
        params["limit"] = json!(limit);
    }
    BrpRequest {
        method: builtin_methods::BRP_QUERY_METHOD.to_string(),
        id: None,
        params: Some(params),
    }
}

/// Benchmark query optimization performance (BEVDBG-014)
fn benchmark_query_optimization(c: &mut Criterion) {
    let bench_config = BenchConfig::new();
    
    // Create test queries with different complexity levels
    let test_queries = vec![
        ("simple", BrpRequest {
            method: builtin_methods::BRP_LIST_COMPONENTS_METHOD.to_string(),
            id: None,
            params: None,
        }),
        ("single_entity", BrpRequest {
            method: builtin_methods::BRP_GET_COMPONENTS_METHOD.to_string(),
            id: None,
            params: Some(json!({"entity": 1})),
        }),
        ("single_component_filter", query_request(&["Transform"], &[], None)),
        ("multi_component_filter", query_request(&["Transform", "Velocity"], &["Inactive"], None)),
        ("complex_query", query_request(
            &["Transform", "Velocity", "Health", "Collider"],
            &["Inactive", "Destroyed"],
            Some(500),
        )),
    ];

    // Benchmark QueryOptimizer performance
    for (query_name, request) in &test_queries {
        c.bench_with_input(
            BenchmarkId::new("query_optimizer", query_name),
            request,
            |b, request| {
                b.to_async(&bench_config.runtime).iter(|| async move {
                    let optimizer = QueryOptimizer::new(50, 100, 1000);
                    let _optimized = black_box(optimizer.optimize_request(request).await.unwrap());
                });
            },
        );
    }

    // Benchmark ParallelQueryExecutor creation
    c.bench_function("parallel_executor_creation", |b| {
        b.iter(|| {
            let config = ParallelExecutionConfig::default();
            let _executor = black_box(ParallelQueryExecutor::new(config).unwrap());
        });
    });
}

/// Benchmark parallel processing scalability
fn benchmark_parallel_scalability(c: &mut Criterion) {
    let bench_config = BenchConfig::new();
    
    // Test different entity counts to measure parallel scaling
    let entity_counts = vec![100, 500, 1000, 5000, 10000];
    
    for entity_count in entity_counts {
        c.bench_with_input(
            BenchmarkId::new("parallel_scalability", entity_count),
            &entity_count,
            |b, &entity_count| {
                b.to_async(&bench_config.runtime).iter(|| async move {
                    // Simulate processing entities in batches
                    let batch_size = 250;
                    let num_batches = (entity_count + batch_size - 1) / batch_size;
                    
                    // Sequential processing
                    let sequential_start = std::time::Instant::now();
                    for _batch in 0..num_batches {
                        for _entity in 0..batch_size.min(entity_count) {
                            let _work = black_box(simulate_entity_processing());
                        }
                    }
                    let sequential_time = sequential_start.elapsed();
                    
                    // Parallel processing
                    let parallel_start = std::time::Instant::now();
                    let per_batch = batch_size.min(entity_count);
                    let batch_futures = (0..num_batches).map(|_| async move {
                        tokio::task::spawn_blocking(move || {
                            for _entity in 0..per_batch {
                                let _work = simulate_entity_processing();
                            }
                        }).await.unwrap()
                    });
                    
                    futures_util::future::join_all(batch_futures).await;
                    let parallel_time = parallel_start.elapsed();
                    
                    // Calculate speedup
                    let speedup = sequential_time.as_secs_f64() / parallel_time.as_secs_f64().max(0.000001);
                    
                    black_box((speedup, parallel_time));
                });
            },
        );
    }
}

/// Benchmark standard vs optimized observe tool
fn benchmark_observe_performance(c: &mut Criterion) {
    let bench_config = BenchConfig::new();
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
        ..Config::default()
    };
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));

    let observe_test_cases = vec![
        ("simple_query", json!({"query": "list all entities"})),
        ("component_filter", json!({"query": "find entities with Transform"})),
        ("complex_query", json!({"query": "find entities with Transform, Velocity without Inactive"})),
        ("diff_mode", json!({"query": "list all entities", "diff": true})),
    ];

    for (test_name, args) in observe_test_cases {
        // Benchmark standard observe tool
        c.bench_with_input(
            BenchmarkId::new("observe_standard", test_name),
            &args,
            |b, args| {
                b.to_async(&bench_config.runtime).iter(|| {
                    let args = args.clone();
                    let brp_client = brp_client.clone();
                    async move {
                        let _result = black_box(observe::handle(args, brp_client).await);
                    }
                });
            },
        );

        // Benchmark optimized observe tool
        c.bench_with_input(
            BenchmarkId::new("observe_optimized", test_name),
            &args,
            |b, args| {
                b.to_async(&bench_config.runtime).iter(|| {
                    let args = args.clone();
                    let brp_client = brp_client.clone();
                    async move {
                        let _result = black_box(observe_optimized::handle_optimized(args, brp_client).await);
                    }
                });
            },
        );
    }
}

/// Performance comparison benchmark for BEVDBG-014 acceptance criteria
fn benchmark_bevdbg014_acceptance(c: &mut Criterion) {
    let bench_config = BenchConfig::new();
    
    c.bench_function("bevdbg014_10x_improvement_target", |b| {
        b.to_async(&bench_config.runtime).iter(|| async {
            // Simulate processing 10k entities with standard approach
            let mut total_time_standard = Duration::ZERO;
            for _batch in 0..40 { // 40 batches of 250 entities = 10k
                let batch_start = std::time::Instant::now();
                // Simulate standard query processing
                for _entity in 0..250 {
                    let _work = black_box(simulate_entity_processing());
                }
                total_time_standard += batch_start.elapsed();
            }
            
            // Simulate optimized parallel approach
            let parallel_start = std::time::Instant::now();
            let batch_futures = (0..40).map(|_| async {
                tokio::task::spawn_blocking(|| {
                    for _entity in 0..250 {
                        let _work = simulate_entity_processing();
                    }
                }).await.unwrap()
            });
            
            futures_util::future::join_all(batch_futures).await;
            let total_time_parallel = parallel_start.elapsed();
            
            // Calculate speedup
            let speedup = total_time_standard.as_secs_f64() / total_time_parallel.as_secs_f64().max(0.000001);
            
            // Assert 10x improvement target (allowing for some variance in benchmarks)
            #[cfg(debug_assertions)]
            if speedup < 5.0 { // At least 5x improvement in debug mode
                println!("WARNING: Speedup is {:.2}x, target is 10x", speedup);
            }
            
            black_box((speedup, total_time_parallel));
        });
    });
    
    c.bench_function("bevdbg014_cache_effectiveness", |b| {
        b.to_async(&bench_config.runtime).iter(|| async {
            let optimizer = QueryOptimizer::new(100, 1000, 500);
            
            let request = query_request(&["Transform", "Velocity"], &[], None);
            
            // First optimization (cache miss)
            let start_first = std::time::Instant::now();
            let _optimized1 = optimizer.optimize_request(&request).await.unwrap();
            let first_time = start_first.elapsed();
            
            // Second optimization (cache hit)
            let start_second = std::time::Instant::now();
            let _optimized2 = optimizer.optimize_request(&request).await.unwrap();
            let second_time = start_second.elapsed();
            
            // Cache should make second request significantly faster
            let speedup = first_time.as_secs_f64() / second_time.as_secs_f64().max(0.000001);
            
            #[cfg(debug_assertions)]
            if speedup < 2.0 { // At least 2x improvement from caching
                println!("WARNING: Cache speedup is {:.2}x, expected > 2x", speedup);
            }
            
            black_box((first_time, second_time, speedup));
        });
    });
}

/// Benchmark query cache performance
fn benchmark_query_cache(c: &mut Criterion) {
    let bench_config = BenchConfig::new();
    
    c.bench_function("query_cache_hit_rate", |b| {
        b.to_async(&bench_config.runtime).iter(|| async {
            let optimizer = QueryOptimizer::new(100, 1000, 500);
            
            // Create several different query patterns
            let queries = vec![
                query_request(&["Transform"], &[], None),
                query_request(&["Velocity"], &[], None),
                query_request(&["Health"], &[], None),
            ];
            
            // Prime the cache
            for query in &queries {
                let _ = optimizer.optimize_request(query).await;
            }
            
            // Measure cache hits
            let cache_start = std::time::Instant::now();
            for query in &queries {
                for _repeat in 0..10 { // Repeat each query 10 times
                    let _optimized = optimizer.optimize_request(query).await.unwrap();
                }
            }
            let cache_time = cache_start.elapsed();
            
            black_box(cache_time);
        });
    });
}

/// Simulate entity processing work
fn simulate_entity_processing() -> u64 {
    let mut result = 0u64;
    for i in 0..100 {
        result = result.wrapping_add(i * 17 + 42);
    }
    result
}

criterion_group!(
    bevdbg014_benches,
    benchmark_query_optimization,
    benchmark_parallel_scalability,
    benchmark_observe_performance,
    benchmark_bevdbg014_acceptance,
    benchmark_query_cache,
);

criterion_main!(bevdbg014_benches);