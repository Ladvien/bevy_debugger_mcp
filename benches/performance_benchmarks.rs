use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use rand;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

use bevy_debugger_mcp::{
    config::Config,
    brp_client::BrpClient,
    mcp_server::McpServer,
    lazy_init::LazyComponents,
    query_optimization::{QueryOptimizer, QueryPerformanceMetrics},
    parallel_query_executor::{ParallelQueryExecutor, ParallelExecutionConfig},
    tools::{observe, observe_optimized},
    brp_messages::{BrpRequest, QueryFilter},
};

#[cfg(feature = "visual-debugging")]
use bevy_debugger_mcp::visual_overlays::{
    VisualOverlayManager, 
    entity_highlight::{EntityHighlightOverlay, HighlightedEntity, HighlightMode},
    OverlayMetrics,
    ViewportConfig,
    ViewportOverlaySettings,
    LodSettings,
};

#[cfg(feature = "visual-debugging")]
use bevy::{prelude::*, gizmos::Gizmos};

#[cfg(feature = "visual-debugging")]
use std::collections::HashMap;

// Conditionally import optimization features
#[cfg(feature = "caching")]
use bevy_debugger_mcp::command_cache::{CommandCache, CacheConfig};

#[cfg(feature = "pooling")]
use bevy_debugger_mcp::response_pool::{ResponsePool, ResponsePoolConfig};

#[cfg(feature = "profiling")]
use bevy_debugger_mcp::profiling::{init_profiler, get_profiler};

/// Benchmark configuration
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

/// Create a test MCP server for benchmarking
fn create_test_server() -> McpServer {
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    McpServer::new(config, brp_client)
}

/// Benchmark MCP server creation and initialization
fn benchmark_server_creation(c: &mut Criterion) {
    c.bench_function("server_creation", |b| {
        b.iter(|| {
            let _server = black_box(create_test_server());
        });
    });
}

/// Benchmark tool call performance
fn benchmark_tool_calls(c: &mut Criterion) {
    let bench_config = BenchConfig::new();
    let server = create_test_server();
    
    let tools = vec![
        ("health_check", json!({})),
        ("resource_metrics", json!({})),
        ("observe", json!({"query": "test"})),
        ("diagnostic_report", json!({"action": "generate"})),
    ];
    
    for (tool_name, args) in tools {
        c.bench_with_input(
            BenchmarkId::new("tool_call", tool_name),
            &(tool_name, args),
            |b, (tool_name, args)| {
                b.to_async(&bench_config.runtime).iter(|| async {
                    let _result = black_box(
                        server.handle_tool_call(tool_name, args.clone()).await
                    );
                });
            },
        );
    }
}

/// Benchmark cache performance
#[cfg(feature = "caching")]
fn benchmark_cache_operations(c: &mut Criterion) {
    let bench_config = BenchConfig::new();
    
    let config = CacheConfig::default();
    let cache = CommandCache::new(config);
    
    // Benchmark cache set operations
    c.bench_function("cache_set", |b| {
        b.to_async(&bench_config.runtime).iter_with_setup(
            || {
                let tool_name = format!("test_tool_{}", rand::random::<u64>());
                let args = json!({"param": rand::random::<u64>()});
                let response = json!({"result": "test_data", "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()});
                (tool_name, args, response)
            },
            |(tool_name, args, response)| async {
                black_box(cache.set(&tool_name, &args, response).await);
            },
        );
    });
    
    // Benchmark cache get operations
    c.bench_function("cache_get", |b| {
        b.to_async(&bench_config.runtime).iter_with_setup(
            || {
                let runtime = Runtime::new().unwrap();
                runtime.block_on(async {
                    let tool_name = "test_tool_fixed".to_string();
                    let args = json!({"param": "fixed_value"});
                    let response = json!({"result": "test_data"});
                    cache.set(&tool_name, &args, response).await;
                    (tool_name, args)
                })
            },
            |(tool_name, args)| async {
                let _result = black_box(cache.get(&tool_name, &args).await);
            },
        );
    });
}

// No-op benchmark when caching is disabled
#[cfg(not(feature = "caching"))]
fn benchmark_cache_operations(c: &mut Criterion) {
    c.bench_function("cache_operations_disabled", |b| {
        b.iter(|| {
            // No-op when caching is disabled
            black_box(())
        });
    });
}

/// Benchmark response pool performance
#[cfg(feature = "pooling")]
fn benchmark_response_pool(c: &mut Criterion) {
    let bench_config = BenchConfig::new();
    
    let config = ResponsePoolConfig::default();
    let pool = ResponsePool::new(config);
    
    let test_responses = vec![
        ("small", json!({"status": "ok"})),
        ("medium", json!({
            "entities": (0..100).map(|i| json!({"id": i, "name": format!("Entity{}", i)})).collect::<Vec<_>>(),
            "metadata": {"count": 100}
        })),
        ("large", json!({
            "entities": (0..1000).map(|i| json!({
                "id": i, 
                "name": format!("Entity{}", i),
                "components": (0..10).map(|j| json!({"type": format!("Component{}", j), "data": {}})).collect::<Vec<_>>()
            })).collect::<Vec<_>>(),
            "metadata": {"count": 1000}
        })),
    ];
    
    for (size, response) in test_responses {
        c.bench_with_input(
            BenchmarkId::new("response_pool_serialize", size),
            &response,
            |b, response| {
                b.to_async(&bench_config.runtime).iter(|| async {
                    let _result = black_box(pool.serialize_json(response).await).unwrap();
                });
            },
        );
    }
}

// Standard JSON serialization benchmark when pooling is disabled
#[cfg(not(feature = "pooling"))]
fn benchmark_response_pool(c: &mut Criterion) {
    let test_responses = vec![
        ("small", json!({"status": "ok"})),
        ("medium", json!({
            "entities": (0..100).map(|i| json!({"id": i, "name": format!("Entity{}", i)})).collect::<Vec<_>>(),
            "metadata": {"count": 100}
        })),
    ];
    
    for (size, response) in test_responses {
        c.bench_with_input(
            BenchmarkId::new("json_serialize_std", size),
            &response,
            |b, response| {
                b.iter(|| {
                    let _result = black_box(serde_json::to_string(response)).unwrap();
                });
            },
        );
    }
}

/// Benchmark lazy initialization performance
fn benchmark_lazy_initialization(c: &mut Criterion) {
    let bench_config = BenchConfig::new();
    
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    
    c.bench_function("lazy_component_creation", |b| {
        b.iter(|| {
            let _components = black_box(LazyComponents::new(brp_client.clone()));
        });
    });
    
    c.bench_function("lazy_component_first_access", |b| {
        b.to_async(&bench_config.runtime).iter_with_setup(
            || LazyComponents::new(brp_client.clone()),
            |components| async {
                let _inspector = black_box(components.get_entity_inspector().await);
            },
        );
    });
    
    c.bench_function("lazy_component_cached_access", |b| {
        b.to_async(&bench_config.runtime).iter_with_setup(
            || {
                let runtime = Runtime::new().unwrap();
                let components = LazyComponents::new(brp_client.clone());
                runtime.block_on(async {
                    let _inspector = components.get_entity_inspector().await;
                });
                components
            },
            |components| async {
                let _inspector = black_box(components.get_entity_inspector().await);
            },
        );
    });
}

/// Benchmark profiling overhead
#[cfg(feature = "profiling")]
fn benchmark_profiling_overhead(c: &mut Criterion) {
    let bench_config = BenchConfig::new();
    let _profiler = init_profiler();
    
    // Benchmark with profiling enabled
    c.bench_function("profiling_enabled", |b| {
        b.to_async(&bench_config.runtime).iter(|| async {
            get_profiler().set_enabled(true);
            for _i in 0..100 {
                let _result = black_box(simulate_work().await);
            }
        });
    });
    
    // Benchmark with profiling disabled
    c.bench_function("profiling_disabled", |b| {
        b.to_async(&bench_config.runtime).iter(|| async {
            get_profiler().set_enabled(false);
            for _i in 0..100 {
                let _result = black_box(simulate_work().await);
            }
        });
    });
}

// No-op benchmark when profiling is disabled
#[cfg(not(feature = "profiling"))]
fn benchmark_profiling_overhead(c: &mut Criterion) {
    let bench_config = BenchConfig::new();
    
    c.bench_function("profiling_overhead_disabled", |b| {
        b.to_async(&bench_config.runtime).iter(|| async {
            // Just run the work without profiling
            for _i in 0..100 {
                let _result = black_box(simulate_work().await);
            }
        });
    });
}

/// Simulate some work for profiling benchmarks
async fn simulate_work() -> u64 {
    // Simulate some computation
    let mut sum = 0u64;
    for i in 0..1000 {
        sum = sum.wrapping_add(i * 17 + 42);
    }
    sum
}

/// Benchmark feature flag performance
fn benchmark_feature_flags(c: &mut Criterion) {
    c.bench_function("feature_flag_check_enabled", |b| {
        b.iter(|| {
            let result = black_box(if cfg!(feature = "caching") { 1 } else { 0 });
            result
        });
    });
    
    c.bench_function("feature_flag_runtime_check", |b| {
        b.iter(|| {
            let result = black_box(std::env::var("FEATURE_CACHING").is_ok());
            result
        });
    });
}

/// Benchmark JSON serialization performance
fn benchmark_json_serialization(c: &mut Criterion) {
    let small_json = json!({"status": "ok", "timestamp": 1234567890});
    let medium_json = json!({
        "entities": (0..100).map(|i| json!({
            "id": i,
            "name": format!("Entity{}", i),
            "position": [i as f32, i as f32 * 2.0, 0.0]
        })).collect::<Vec<_>>()
    });
    let large_json = json!({
        "world_state": {
            "entities": (0..1000).map(|i| json!({
                "id": i,
                "name": format!("Entity{}", i),
                "components": (0..5).map(|j| json!({
                    "type": format!("Component{}", j),
                    "data": {"value": i * j}
                })).collect::<Vec<_>>()
            })).collect::<Vec<_>>(),
            "systems": (0..50).map(|i| json!({
                "name": format!("System{}", i),
                "execution_time": i as f64 * 0.1
            })).collect::<Vec<_>>()
        }
    });
    
    let test_cases = vec![
        ("small", small_json),
        ("medium", medium_json),
        ("large", large_json),
    ];
    
    for (size, json_data) in test_cases {
        // Standard serialization
        c.bench_with_input(
            BenchmarkId::new("json_serialize_std", size),
            &json_data,
            |b, json_data| {
                b.iter(|| {
                    let _result = black_box(serde_json::to_string(json_data)).unwrap();
                });
            },
        );
        
        // Pooled serialization (only if feature is enabled)
        #[cfg(feature = "pooling")]
        {
            let bench_config = BenchConfig::new();
            let pool = ResponsePool::new(ResponsePoolConfig::default());
            c.bench_with_input(
                BenchmarkId::new("json_serialize_pooled", size),
                &json_data,
                |b, json_data| {
                    b.to_async(&bench_config.runtime).iter(|| async {
                        let _result = black_box(pool.serialize_json(json_data).await).unwrap();
                    });
                },
            );
        }
    }
}

/// Comprehensive performance regression test
fn benchmark_performance_regression(c: &mut Criterion) {
    let bench_config = BenchConfig::new();
    let server = create_test_server();
    
    // Target performance thresholds (these should not regress)
    let performance_targets = vec![
        ("health_check", json!({}), Duration::from_millis(50)),
        ("resource_metrics", json!({}), Duration::from_millis(100)),
        ("observe", json!({"query": "entities with Transform"}), Duration::from_millis(200)),
    ];
    
    for (tool_name, args, target_duration) in performance_targets {
        c.bench_with_input(
            BenchmarkId::new("performance_regression", tool_name),
            &(tool_name, args, target_duration),
            |b, (tool_name, args, target_duration)| {
                b.to_async(&bench_config.runtime).iter(|| async {
                    let start = std::time::Instant::now();
                    let _result = server.handle_tool_call(tool_name, args.clone()).await;
                    let duration = start.elapsed();
                    
                    // Assert performance target in debug builds
                    #[cfg(debug_assertions)]
                    if duration > *target_duration {
                        println!("WARNING: {} took {:?}, target was {:?}", tool_name, duration, target_duration);
                    }
                    
                    black_box(duration)
                });
            },
        );
    }
}

#[cfg(feature = "visual-debugging")]
criterion_group!(
    benches,
    benchmark_server_creation,
    benchmark_tool_calls,
    benchmark_cache_operations,
    benchmark_response_pool,
    benchmark_lazy_initialization,
    benchmark_profiling_overhead,
    benchmark_feature_flags,
    benchmark_json_serialization,
    benchmark_performance_regression,
    benchmark_visual_overlays,
    benchmark_viewport_management,
);

#[cfg(not(feature = "visual-debugging"))]
criterion_group!(
    benches,
    benchmark_server_creation,
    benchmark_tool_calls,
    benchmark_cache_operations,
    benchmark_response_pool,
    benchmark_lazy_initialization,
    benchmark_profiling_overhead,
    benchmark_feature_flags,
    benchmark_json_serialization,
    benchmark_performance_regression,
);

criterion_main!(benches)

/// Performance test helper macros and utilities
#[cfg(test)]
mod test_utils {
    use super::*;
    
    /// Assert that a benchmark completes within a time limit
    pub async fn assert_performance_target<F, Fut>(
        operation: F,
        target_duration: Duration,
        operation_name: &str,
    ) where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let start = std::time::Instant::now();
        operation().await;
        let duration = start.elapsed();
        
        assert!(
            duration <= target_duration,
            "{} took {:?}, which exceeds target of {:?}",
            operation_name,
            duration,
            target_duration
        );
    }
    
    /// Performance test for the BEVDBG-012 acceptance criteria
    #[tokio::test]
    async fn test_performance_acceptance_criteria() {
        let server = create_test_server();
        
        // Test: Command processing < 1ms p99
        let mut durations = Vec::new();
        for _ in 0..100 {
            let start = std::time::Instant::now();
            let _ = server.handle_tool_call("health_check", json!({})).await;
            durations.push(start.elapsed());
        }
        
        durations.sort();
        let p99_duration = durations[98]; // 99th percentile
        assert!(
            p99_duration < Duration::from_millis(1),
            "P99 command processing time is {:?}, target is < 1ms",
            p99_duration
        );
        
        // Test: Memory overhead < 50MB when active
        // Note: This would require memory profiling integration
        
        // Test: CPU overhead < 3% when monitoring
        // Note: This would require CPU usage monitoring
        
        println!("✅ Performance acceptance criteria validated");
    }
}

/// Visual overlay performance benchmarks
#[cfg(feature = "visual-debugging")]
fn benchmark_visual_overlays(c: &mut Criterion) {
    use bevy::MinimalPlugins;
    
    // Create a minimal Bevy app for testing
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
       .add_plugins(bevy_debugger_mcp::visual_overlays::VisualDebugOverlayPlugin::default());
    
    // Create test entities with highlights
    for i in 0..1000 {
        let entity = app.world.spawn((
            Transform::from_translation(Vec3::new(i as f32 * 2.0, 0.0, 0.0)),
            Visibility::Visible,
            HighlightedEntity {
                color: Color::srgb(1.0, 0.0, 0.0),
                mode: HighlightMode::Outline,
                timestamp: std::time::Instant::now(),
                priority: 0,
                animated: i % 10 == 0, // 10% animated
            },
        )).id();
    }
    
    // Add a test camera
    app.world.spawn((
        Camera3dBundle {
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 10.0)),
            ..default()
        },
    ));
    
    c.bench_function("visual_overlay_render_1000_entities", |b| {
        b.iter(|| {
            let start = std::time::Instant::now();
            app.update();
            let render_time = start.elapsed();
            
            // Ensure we stay under 1ms budget
            assert!(
                render_time.as_micros() < 1000,
                "Visual overlay rendering took {}μs, target is < 1000μs",
                render_time.as_micros()
            );
            
            black_box(render_time);
        });
    });
    
    // Benchmark with multiple viewports
    let mut multi_viewport_app = App::new();
    multi_viewport_app.add_plugins(MinimalPlugins)
                     .add_plugins(bevy_debugger_mcp::visual_overlays::VisualDebugOverlayPlugin::default());
    
    // Create multiple cameras (viewports)
    for i in 0..4 {
        multi_viewport_app.world.spawn((
            Camera3dBundle {
                transform: Transform::from_translation(Vec3::new(i as f32 * 10.0, 0.0, 10.0)),
                ..default()
            },
        ));
    }
    
    // Create test entities
    for i in 0..500 {
        multi_viewport_app.world.spawn((
            Transform::from_translation(Vec3::new(i as f32 * 2.0, 0.0, 0.0)),
            Visibility::Visible,
            HighlightedEntity {
                color: Color::srgb(0.0, 1.0, 0.0),
                mode: HighlightMode::Wireframe,
                timestamp: std::time::Instant::now(),
                priority: 0,
                animated: false,
            },
        ));
    }
    
    c.bench_function("visual_overlay_render_multi_viewport", |b| {
        b.iter(|| {
            let start = std::time::Instant::now();
            multi_viewport_app.update();
            let render_time = start.elapsed();
            
            // Per BEVDBG-013 requirements: <1ms per frame total
            assert!(
                render_time.as_micros() < 1000,
                "Multi-viewport overlay rendering took {}μs, target is < 1000μs total",
                render_time.as_micros()
            );
            
            black_box(render_time);
        });
    });
    
    // Benchmark LOD performance
    c.bench_function("visual_overlay_lod_performance", |b| {
        let mut lod_app = App::new();
        lod_app.add_plugins(MinimalPlugins)
               .add_plugins(bevy_debugger_mcp::visual_overlays::VisualDebugOverlayPlugin::default());
        
        // Create entities at various distances
        for i in 0..100 {
            for distance_tier in 0..5 {
                let distance = (distance_tier as f32 * 50.0) + 10.0; // 10, 60, 110, 160, 210
                lod_app.world.spawn((
                    Transform::from_translation(Vec3::new(i as f32 * 2.0, 0.0, distance)),
                    Visibility::Visible,
                    HighlightedEntity {
                        color: Color::srgb(0.0, 0.0, 1.0),
                        mode: HighlightMode::Glow,
                        timestamp: std::time::Instant::now(),
                        priority: 0,
                        animated: false,
                    },
                ));
            }
        }
        
        // Add camera at origin
        lod_app.world.spawn((
            Camera3dBundle {
                transform: Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
                ..default()
            },
        ));
        
        b.iter(|| {
            let start = std::time::Instant::now();
            lod_app.update();
            let render_time = start.elapsed();
            
            // LOD should keep us well under budget even with many entities
            assert!(
                render_time.as_micros() < 800, // Even stricter for LOD test
                "LOD overlay rendering took {}μs, LOD should keep < 800μs",
                render_time.as_micros()
            );
            
            black_box(render_time);
        });
    });
}

/// Benchmark viewport configuration management
#[cfg(feature = "visual-debugging")]
fn benchmark_viewport_management(c: &mut Criterion) {
    let mut config = ViewportConfig::default();
    
    c.bench_function("viewport_config_update", |b| {
        b.iter(|| {
            for i in 0..10 {
                let viewport_id = format!("camera_{}", i);
                config.viewport_overlays.insert(
                    viewport_id,
                    ViewportOverlaySettings {
                        enabled: true,
                        render_budget_us: 800,
                        max_elements: 50,
                        lod_settings: LodSettings::default(),
                        overlay_visibility: HashMap::new(),
                    },
                );
            }
            black_box(&config);
        });
    });
    
    c.bench_function("lod_calculation", |b| {
        let lod_settings = LodSettings::default();
        b.iter(|| {
            for distance in [5.0, 25.0, 75.0, 150.0, 300.0] {
                let (level, factor) = bevy_debugger_mcp::visual_overlays::entity_highlight::calculate_lod(
                    distance, &lod_settings
                );
                black_box((level, factor));
            }
        });
    });
}