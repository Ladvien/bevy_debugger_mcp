/// End-to-End Performance Tests with Real Bevy Games
/// 
/// Comprehensive E2E testing of performance optimizations using actual Bevy games
/// to validate that all BEVDBG-012 optimizations work correctly in realistic scenarios.

use std::sync::Arc;
use std::time::Duration;
use serde_json::json;

use bevy_debugger_mcp::{
    config::Config,
    mcp_server::McpServer,
    brp_client::BrpClient,
    lazy_init::LazyComponents,
    command_cache::CommandCache,
    response_pool::ResponsePool,
};

mod fixtures;
mod helpers;
mod integration;

use fixtures::{performance_test_game, complex_ecs_game};
use helpers::{
    TestGameProcess, TestScenario, 
    PerformanceMeasurement, PerformanceTargets, RegressionDetector,
    MemoryUsageTracker,
    generate_realistic_queries, generate_workload_pattern,
    with_test_game,
};
use integration::IntegrationTestHarness;

/// Test performance optimizations with the performance test game
#[tokio::test]
async fn test_optimizations_with_performance_game() {
    let result = with_test_game("performance_test_game", |mut game_process| async move {
        let mut performance = PerformanceMeasurement::with_targets(
            "E2E Performance Test",
            PerformanceTargets::bevdbg_012_targets()
        );

        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3001,
        };

        // Initialize all optimization systems
        let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
        let lazy_components = LazyComponents::new(brp_client.clone());
        let mcp_server = Arc::new(McpServer::new(config, brp_client));

        // Test 1: Lazy initialization performance
        performance.measure_async("lazy_init_entity_inspector", || async {
            let _inspector = lazy_components.get_entity_inspector().await;
        }).await;

        performance.measure_async("lazy_init_system_profiler", || async {
            let _profiler = lazy_components.get_system_profiler().await;
        }).await;

        performance.measure_async("lazy_init_debug_router", || async {
            let _router = lazy_components.get_debug_command_router().await;
        }).await;

        // Test 2: MCP server with optimizations
        let realistic_queries = generate_realistic_queries();
        for (tool_name, args) in realistic_queries.iter().take(20) {
            performance.measure_async(&format!("mcp_tool_{}", tool_name), || {
                let server = mcp_server.clone();
                let tool_name = tool_name.clone();
                let args = args.clone();
                async move {
                    server.handle_tool_call(&tool_name, args).await
                }
            }).await;
        }

        // Test 3: Game stress testing with optimizations
        performance.measure_async("spawn_entities_stress_test", || {
            game_process.run_stress_test("entity_spawn", 3)
        }).await;

        performance.measure_async("continuous_spawn_stress_test", || {
            game_process.run_stress_test("continuous_spawn", 2)
        }).await;

        // Test 4: Complex queries during high load
        tokio::spawn(async move {
            // Background load
            let _ = game_process.run_stress_test("continuous_spawn", 1).await;
        });

        tokio::time::sleep(Duration::from_secs(2)).await;

        let complex_queries = [
            ("observe", json!({"query": "entities with (Transform and Mesh and Health)"})),
            ("observe", json!({"query": "entities in combat with Health < 30"})),
            ("experiment", json!({"type": "performance", "duration": 3000})),
            ("diagnostic_report", json!({"action": "generate", "include_performance": true})),
        ];

        for (tool_name, args) in complex_queries {
            performance.measure_async(&format!("complex_query_{}", tool_name), || {
                let server = mcp_server.clone();
                async move {
                    server.handle_tool_call(tool_name, args).await
                }
            }).await;
        }

        let summary = performance.performance_summary();
        let report = performance.generate_report();

        println!("=== E2E Performance Test Results ===");
        println!("{}", report);

        // Validate performance targets
        assert!(performance.meets_targets(), 
                "Performance optimizations should meet BEVDBG-012 targets");

        // Specific target validations
        if let Some(mcp_stats) = summary.operation_stats.get("mcp_tool_observe") {
            assert!(mcp_stats.p99_duration.as_millis() < 1, 
                    "MCP observe commands should be < 1ms p99");
        }

        if let Some(lazy_stats) = summary.operation_stats.get("lazy_init_entity_inspector") {
            assert!(lazy_stats.avg_duration.as_millis() < 100, 
                    "Lazy initialization should be < 100ms average");
        }

        Ok(summary)
    }).await;

    assert!(result.is_ok(), "E2E performance test should complete successfully");
}

/// Test memory usage with complex ECS game
#[tokio::test]
async fn test_memory_optimization_with_complex_game() {
    let result = with_test_game("complex_ecs_game", |mut game_process| async move {
        let mut memory_tracker = MemoryUsageTracker::new();
        let baseline_memory = memory_tracker.baseline();

        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3001,
        };

        // Initialize systems with memory tracking
        let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
        let lazy_components = LazyComponents::new(brp_client.clone());
        let mcp_server = Arc::new(McpServer::new(config, brp_client));

        memory_tracker.record_measurement();

        // Spawn complex entities to stress memory system
        for _ in 0..5 {
            let _ = game_process.spawn_entities("complex", 50).await;
            memory_tracker.record_measurement();
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        // Execute memory-intensive operations
        let memory_intensive_operations = [
            ("observe", json!({
                "query": "all entities with detailed components",
                "include_all_data": true
            })),
            ("experiment", json!({
                "type": "memory_analysis",
                "duration": 5000,
                "deep_analysis": true
            })),
            ("diagnostic_report", json!({
                "action": "full_export",
                "include_history": true
            })),
        ];

        for (tool_name, args) in memory_intensive_operations {
            let _ = mcp_server.handle_tool_call(tool_name, args).await;
            memory_tracker.record_measurement();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Run sustained load test
        for i in 0..20 {
            let _ = mcp_server.handle_tool_call(
                "observe",
                json!({"query": format!("entities iteration {}", i)})
            ).await;
            
            if i % 5 == 0 {
                memory_tracker.record_measurement();
            }
        }

        let final_memory = memory_tracker.current_usage();
        let memory_overhead = memory_tracker.peak_overhead();
        
        println!("=== Memory Optimization Test Results ===");
        println!("{}", memory_tracker.generate_report());

        // Validate BEVDBG-012 memory targets
        assert!(memory_overhead < 50_000_000, // 50MB limit
                "Memory overhead should be < 50MB, got {} bytes", memory_overhead);

        // Check memory growth rate is reasonable
        assert!(memory_tracker.is_memory_stable(1_000_000.0), // 1MB/sec max growth
                "Memory growth should be stable");

        Ok(memory_tracker)
    }).await;

    assert!(result.is_ok(), "Memory optimization test should complete successfully");
}

/// Test all optimization features working together
#[tokio::test]
async fn test_full_optimization_stack_integration() {
    let mut performance = PerformanceMeasurement::with_targets(
        "Full Stack Integration",
        PerformanceTargets::bevdbg_012_targets()
    );
    
    let mut memory_tracker = MemoryUsageTracker::new();

    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };

    // Initialize full optimization stack
    let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
    let lazy_components = LazyComponents::new(brp_client.clone());
    let cache = CommandCache::new(bevy_debugger_mcp::command_cache::CacheConfig::default());
    let pool = ResponsePool::new(bevy_debugger_mcp::response_pool::ResponsePoolConfig::default());
    let mcp_server = Arc::new(McpServer::new(config, brp_client));

    memory_tracker.record_measurement();

    // Test lazy initialization cascade
    performance.measure_async("full_lazy_init_cascade", || async {
        let _router = lazy_components.get_debug_command_router().await;
        let _entity_processor = lazy_components.get_entity_processor().await;
        let _profiler_processor = lazy_components.get_profiler_processor().await;
        let _session_processor = lazy_components.get_session_processor().await;
    }).await;

    memory_tracker.record_measurement();

    // Test caching with various query types
    let cache_test_queries = [
        ("observe", json!({"query": "entities with Transform"})),
        ("observe", json!({"query": "entities with Mesh"})),
        ("experiment", json!({"type": "performance", "duration": 1000})),
    ];

    // First pass - populate cache
    for (tool_name, args) in &cache_test_queries {
        performance.measure_async(&format!("cache_miss_{}", tool_name), || {
            let server = mcp_server.clone();
            let tool_name = tool_name.clone();
            let args = args.clone();
            async move {
                server.handle_tool_call(tool_name, args).await
            }
        }).await;
    }

    memory_tracker.record_measurement();

    // Second pass - should hit cache
    for (tool_name, args) in &cache_test_queries {
        performance.measure_async(&format!("cache_hit_{}", tool_name), || {
            let server = mcp_server.clone();
            let tool_name = tool_name.clone();
            let args = args.clone();
            async move {
                server.handle_tool_call(tool_name, args).await
            }
        }).await;
    }

    // Test response pooling with different data sizes
    let test_data_sizes = [
        ("small", json!({"small": "data"})),
        ("medium", json!({"data": (0..100).collect::<Vec<_>>()})),
        ("large", json!({"data": (0..5000).collect::<Vec<_>>()})),
    ];

    for (size_name, data) in test_data_sizes {
        performance.measure_async(&format!("response_pool_{}", size_name), || async {
            pool.serialize_json(&data).await
        }).await;
    }

    memory_tracker.record_measurement();

    // Stress test all systems together
    performance.measure_async("integrated_stress_test", || async {
        let stress_queries = generate_workload_pattern("debugging_session");
        
        for (tool_name, args) in stress_queries {
            let _ = mcp_server.handle_tool_call(&tool_name, args).await;
        }
    }).await;

    let final_memory = memory_tracker.current_usage();
    let memory_overhead = memory_tracker.peak_overhead();

    // Generate comprehensive report
    let performance_summary = performance.performance_summary();
    let performance_report = performance.generate_report();
    let memory_report = memory_tracker.generate_report();

    println!("=== Full Stack Integration Test Results ===");
    println!("{}", performance_report);
    println!("\n{}", memory_report);

    // Validate integrated performance
    assert!(performance.meets_targets(), 
            "Integrated optimization stack should meet all targets");

    // Validate cache effectiveness
    let cache_miss_times: Vec<_> = performance_summary.operation_stats
        .iter()
        .filter(|(name, _)| name.contains("cache_miss"))
        .collect();
    
    let cache_hit_times: Vec<_> = performance_summary.operation_stats
        .iter()
        .filter(|(name, _)| name.contains("cache_hit"))
        .collect();

    if !cache_miss_times.is_empty() && !cache_hit_times.is_empty() {
        for (miss_name, miss_stats) in &cache_miss_times {
            let tool_name = miss_name.replace("cache_miss_", "");
            let hit_name = format!("cache_hit_{}", tool_name);
            
            if let Some(hit_stats) = performance_summary.operation_stats.get(&hit_name) {
                let speedup = miss_stats.avg_duration.as_millis() as f64 / 
                             hit_stats.avg_duration.as_millis() as f64;
                
                assert!(speedup > 5.0, 
                        "Cache should provide at least 5x speedup for {}, got {:.2}x", 
                        tool_name, speedup);
                
                println!("Cache speedup for {}: {:.1}x", tool_name, speedup);
            }
        }
    }

    // Validate memory usage
    assert!(memory_overhead < 50_000_000, 
            "Total memory overhead should be < 50MB");

    // Validate response pooling effectiveness
    let pool_stats = pool.get_statistics().await;
    assert!(pool_stats.pool_hit_rate > 0.5, 
            "Response pool hit rate should be > 50%");

    println!("=== Integration Test Summary ===");
    println!("Performance targets met: ✓");
    println!("Memory targets met: ✓");
    println!("Cache effectiveness verified: ✓");
    println!("Response pooling effective: ✓");
    println!("All optimization systems integrated successfully: ✓");
}

/// Test performance regression detection
#[tokio::test]
async fn test_performance_regression_detection() {
    // Create baseline measurements
    let mut baseline_measurement = PerformanceMeasurement::new("baseline");
    
    // Simulate baseline performance (good performance)
    for i in 0..10 {
        baseline_measurement.record(&format!("operation_{}", i), Duration::from_millis(5));
    }
    baseline_measurement.record("critical_operation", Duration::from_millis(2));
    
    let baseline_summary = baseline_measurement.performance_summary();

    // Create current measurements with some regressions
    let mut current_measurement = PerformanceMeasurement::new("current");
    
    // Most operations remain the same
    for i in 0..8 {
        current_measurement.record(&format!("operation_{}", i), Duration::from_millis(5));
    }
    
    // Some operations have regressions
    current_measurement.record("operation_8", Duration::from_millis(15)); // 3x slower
    current_measurement.record("operation_9", Duration::from_millis(12)); // 2.4x slower
    current_measurement.record("critical_operation", Duration::from_millis(8)); // 4x slower
    
    let current_summary = current_measurement.performance_summary();

    // Test regression detection
    let mut detector = RegressionDetector::new(20.0); // 20% threshold
    detector.set_baseline(&baseline_summary);
    
    let regression_report = detector.check_regression(&current_summary);
    
    println!("=== Regression Detection Test ===");
    println!("{}", regression_report.generate_report());

    // Validate regression detection
    assert!(regression_report.has_regressions(), 
            "Should detect performance regressions");
    
    assert!(regression_report.regressions.len() >= 2, 
            "Should detect at least 2 regressions");

    // Check that critical operation regression is detected
    let critical_regression = regression_report.regressions
        .iter()
        .find(|r| r.operation_name == "critical_operation");
    
    assert!(critical_regression.is_some(), 
            "Should detect critical operation regression");
    
    let critical_regression = critical_regression.unwrap();
    assert!(critical_regression.change_percent > 200.0, 
            "Critical operation regression should be > 200%");

    println!("Regression detection working correctly: ✓");
}

/// Test optimization effectiveness under realistic load
#[tokio::test]
async fn test_optimization_effectiveness_under_load() {
    let result = with_test_game("performance_test_game", |mut game_process| async move {
        let mut performance = PerformanceMeasurement::with_targets(
            "Load Test",
            PerformanceTargets::testing_targets() // Use relaxed targets for load testing
        );

        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3001,
        };

        let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
        let mcp_server = Arc::new(McpServer::new(config, brp_client));

        // Start background load
        let game_handle = tokio::spawn(async move {
            for i in 0..10 {
                let _ = game_process.run_stress_test("entity_spawn", 2).await;
                tokio::time::sleep(Duration::from_secs(3)).await;
                
                if i % 3 == 0 {
                    let _ = game_process.spawn_entities("complex", 20).await;
                }
            }
        });

        // Execute queries under load
        let load_test_queries = generate_workload_pattern("performance_analysis");
        
        for round in 0..5 {
            println!("Load test round {}/5", round + 1);
            
            for (tool_name, args) in &load_test_queries {
                performance.measure_async(&format!("load_{}_{}", round, tool_name), || {
                    let server = mcp_server.clone();
                    let tool_name = tool_name.clone();
                    let args = args.clone();
                    async move {
                        server.handle_tool_call(&tool_name, args).await
                    }
                }).await;
                
                // Small delay between operations
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        // Wait for background load to complete
        let _ = game_handle.await;

        let summary = performance.performance_summary();
        println!("=== Load Test Results ===");
        println!("{}", performance.generate_report());

        // Validate performance under load
        assert!(summary.throughput_ops_per_sec > 20.0, 
                "Should maintain reasonable throughput under load");

        // Check that performance degradation is reasonable
        for (op_name, stats) in &summary.operation_stats {
            assert!(stats.p99_duration.as_millis() < 500, 
                    "Operation {} should complete within 500ms under load", op_name);
        }

        println!("Performance under load test passed: ✓");

        Ok(summary)
    }).await;

    assert!(result.is_ok(), "Load test should complete successfully");
}

/// Test long-running stability of optimizations
#[tokio::test]
#[ignore] // Long-running test, enable manually
async fn test_optimization_stability_long_term() {
    let result = with_test_game("complex_ecs_game", |mut game_process| async move {
        let mut memory_tracker = MemoryUsageTracker::new();
        let mut performance = PerformanceMeasurement::with_targets(
            "Stability Test",
            PerformanceTargets::testing_targets()
        );

        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3001,
        };

        let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
        let mcp_server = Arc::new(McpServer::new(config, brp_client));

        // Run for 5 minutes with continuous activity
        let test_duration = Duration::from_secs(300);
        let start_time = std::time::Instant::now();

        let mut iteration = 0;
        while start_time.elapsed() < test_duration {
            iteration += 1;
            
            // Vary the workload to simulate real usage
            let workload_pattern = match iteration % 4 {
                0 => "debugging_session",
                1 => "performance_analysis",
                2 => "stress_testing",
                _ => "cache_warming",
            };

            let queries = generate_workload_pattern(workload_pattern);
            
            for (tool_name, args) in queries {
                performance.measure_async(&format!("stability_{}_{}", iteration, tool_name), || {
                    let server = mcp_server.clone();
                    async move {
                        server.handle_tool_call(&tool_name, args).await
                    }
                }).await;
            }

            // Periodic stress testing
            if iteration % 10 == 0 {
                let _ = game_process.run_stress_test("entity_spawn", 1).await;
                memory_tracker.record_measurement();
            }

            // Memory measurement every minute
            if iteration % 60 == 0 {
                memory_tracker.record_measurement();
                
                let current_overhead = memory_tracker.current_overhead();
                println!("Iteration {}: Memory overhead: {:.2} MB", 
                        iteration, current_overhead as f64 / 1_048_576.0);
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        println!("=== Long-term Stability Test Results ===");
        println!("{}", performance.generate_report());
        println!("\n{}", memory_tracker.generate_report());

        // Validate long-term stability
        assert!(memory_tracker.is_memory_stable(500_000.0), // 500KB/sec max growth
                "Memory should remain stable over long term");

        let summary = performance.performance_summary();
        assert!(summary.throughput_ops_per_sec > 10.0, 
                "Should maintain throughput over long term");

        // Check that memory overhead didn't grow excessively
        let final_overhead = memory_tracker.current_overhead();
        assert!(final_overhead < 100_000_000, // 100MB max after long run
                "Long-term memory overhead should be reasonable");

        println!("Long-term stability test passed: ✓");
        
        Ok((performance.performance_summary(), memory_tracker))
    }).await;

    assert!(result.is_ok(), "Stability test should complete successfully");
}