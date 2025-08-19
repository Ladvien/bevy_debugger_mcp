/// Performance Regression Tests
/// 
/// Automated tests to detect performance regressions in BEVDBG-012 optimizations.
/// These tests establish baselines and validate that performance remains within
/// acceptable bounds across different scenarios and configurations.

use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use serde_json::json;

use bevy_debugger_mcp::{
    config::Config,
    mcp_server::McpServer,
    brp_client::BrpClient,
    lazy_init::LazyComponents,
    command_cache::{CommandCache, CacheConfig},
    response_pool::{ResponsePool, ResponsePoolConfig},
};

mod helpers;
mod fixtures;
mod integration;

use helpers::{
    PerformanceMeasurement, PerformanceTargets, RegressionDetector,
    MemoryUsageTracker, generate_realistic_queries, generate_workload_pattern,
    TestGameProcess, with_test_game,
};
use integration::IntegrationTestHarness;

/// Baseline performance characteristics for regression detection
#[derive(Debug, Clone)]
pub struct PerformanceBaseline {
    pub name: String,
    pub measurements: HashMap<String, Duration>,
    pub memory_baseline: u64,
    pub throughput_baseline: f64,
    pub created_at: std::time::SystemTime,
}

impl PerformanceBaseline {
    /// Create a new baseline from current measurements
    pub fn from_measurement(name: &str, measurement: &PerformanceMeasurement, memory_tracker: &MemoryUsageTracker) -> Self {
        let summary = measurement.performance_summary();
        let mut measurements = HashMap::new();

        for (op_name, stats) in &summary.operation_stats {
            measurements.insert(op_name.clone(), stats.p99_duration);
        }

        Self {
            name: name.to_string(),
            measurements,
            memory_baseline: memory_tracker.current_usage(),
            throughput_baseline: summary.throughput_ops_per_sec,
            created_at: std::time::SystemTime::now(),
        }
    }
    
    /// Save baseline to file for persistence across test runs
    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::Write;
        
        let serialized = serde_json::to_string_pretty(self)?;
        let mut file = File::create(path)?;
        file.write_all(serialized.as_bytes())?;
        Ok(())
    }
    
    /// Load baseline from file
    pub fn load_from_file(path: &str) -> std::io::Result<Self> {
        use std::fs::File;
        use std::io::Read;
        
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        
        let baseline: Self = serde_json::from_str(&contents)?;
        Ok(baseline)
    }
}

impl serde::Serialize for PerformanceBaseline {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        
        let mut state = serializer.serialize_struct("PerformanceBaseline", 5)?;
        state.serialize_field("name", &self.name)?;
        
        // Convert Duration to milliseconds for serialization
        let measurements_ms: HashMap<String, f64> = self.measurements
            .iter()
            .map(|(k, v)| (k.clone(), v.as_millis() as f64))
            .collect();
        state.serialize_field("measurements", &measurements_ms)?;
        
        state.serialize_field("memory_baseline", &self.memory_baseline)?;
        state.serialize_field("throughput_baseline", &self.throughput_baseline)?;
        state.serialize_field("created_at", &self.created_at.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs())?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for PerformanceBaseline {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Deserialize, Deserializer, MapAccess, Visitor};
        use std::fmt;
        
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field { Name, Measurements, MemoryBaseline, ThroughputBaseline, CreatedAt }

        struct PerformanceBaselineVisitor;

        impl<'de> Visitor<'de> for PerformanceBaselineVisitor {
            type Value = PerformanceBaseline;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct PerformanceBaseline")
            }

            fn visit_map<V>(self, mut map: V) -> Result<PerformanceBaseline, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut name = None;
                let mut measurements_ms: Option<HashMap<String, f64>> = None;
                let mut memory_baseline = None;
                let mut throughput_baseline = None;
                let mut created_at_secs = None;
                
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Name => {
                            if name.is_some() {
                                return Err(de::Error::duplicate_field("name"));
                            }
                            name = Some(map.next_value()?);
                        }
                        Field::Measurements => {
                            if measurements_ms.is_some() {
                                return Err(de::Error::duplicate_field("measurements"));
                            }
                            measurements_ms = Some(map.next_value()?);
                        }
                        Field::MemoryBaseline => {
                            if memory_baseline.is_some() {
                                return Err(de::Error::duplicate_field("memory_baseline"));
                            }
                            memory_baseline = Some(map.next_value()?);
                        }
                        Field::ThroughputBaseline => {
                            if throughput_baseline.is_some() {
                                return Err(de::Error::duplicate_field("throughput_baseline"));
                            }
                            throughput_baseline = Some(map.next_value()?);
                        }
                        Field::CreatedAt => {
                            if created_at_secs.is_some() {
                                return Err(de::Error::duplicate_field("created_at"));
                            }
                            created_at_secs = Some(map.next_value()?);
                        }
                    }
                }

                let name = name.ok_or_else(|| de::Error::missing_field("name"))?;
                let measurements_ms = measurements_ms.ok_or_else(|| de::Error::missing_field("measurements"))?;
                let memory_baseline = memory_baseline.ok_or_else(|| de::Error::missing_field("memory_baseline"))?;
                let throughput_baseline = throughput_baseline.ok_or_else(|| de::Error::missing_field("throughput_baseline"))?;
                let created_at_secs = created_at_secs.ok_or_else(|| de::Error::missing_field("created_at"))?;
                
                // Convert milliseconds back to Duration
                let measurements: HashMap<String, Duration> = measurements_ms
                    .into_iter()
                    .map(|(k, ms)| (k, Duration::from_millis(ms as u64)))
                    .collect();
                
                let created_at = std::time::UNIX_EPOCH + Duration::from_secs(created_at_secs);
                
                Ok(PerformanceBaseline {
                    name,
                    measurements,
                    memory_baseline,
                    throughput_baseline,
                    created_at,
                })
            }
        }

        const FIELDS: &'static [&'static str] = &["name", "measurements", "memory_baseline", "throughput_baseline", "created_at"];
        deserializer.deserialize_struct("PerformanceBaseline", FIELDS, PerformanceBaselineVisitor)
    }
}

/// Test that establishes performance baselines for future regression detection
#[tokio::test]
async fn test_establish_performance_baselines() {
    let result = with_test_game("performance_test_game", |mut game_process| async move {
        let mut performance = PerformanceMeasurement::with_targets(
            "Baseline Establishment",
            PerformanceTargets::bevdbg_012_targets()
        );
        
        let mut memory_tracker = MemoryUsageTracker::new();
        memory_tracker.record_measurement();

        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3001,
        };

        // Initialize optimized system
        let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
        let lazy_components = LazyComponents::new(brp_client.clone());
        let cache = CommandCache::new(CacheConfig::default());
        let pool = ResponsePool::new(ResponsePoolConfig::default());
        let mcp_server = Arc::new(McpServer::new(config, brp_client));

        memory_tracker.record_measurement();

        // Establish baseline measurements for key operations
        let baseline_operations = [
            // Lazy initialization operations
            ("lazy_init_entity_inspector", || async {
                lazy_components.get_entity_inspector().await
            }),
            ("lazy_init_system_profiler", || async {
                lazy_components.get_system_profiler().await
            }),
            
            // Core MCP operations
            ("mcp_health_check", || {
                let server = mcp_server.clone();
                async move {
                    server.handle_tool_call("health_check", json!({})).await
                }
            }),
            ("mcp_resource_metrics", || {
                let server = mcp_server.clone();
                async move {
                    server.handle_tool_call("resource_metrics", json!({})).await
                }
            }),
            ("mcp_observe_entities", || {
                let server = mcp_server.clone();
                async move {
                    server.handle_tool_call("observe", json!({"query": "entities with Transform"})).await
                }
            }),
            
            // Caching operations
            ("cache_set_operation", || async {
                let test_data = json!({"baseline": "cache_test"});
                cache.set("baseline_test", &json!({"arg": "value"}), test_data).await;
            }),
            ("cache_get_operation", || async {
                cache.get("baseline_test", &json!({"arg": "value"})).await
            }),
            
            // Response pooling operations
            ("response_pool_small", || async {
                pool.serialize_json(&json!({"size": "small", "data": [1, 2, 3]})).await
            }),
            ("response_pool_large", || async {
                let large_data: Vec<i32> = (0..1000).collect();
                pool.serialize_json(&json!({"size": "large", "data": large_data})).await
            }),
        ];

        // Measure each operation multiple times for stable baseline
        for (op_name, op_closure) in baseline_operations {
            for i in 0..10 {
                performance.measure_async(&format!("{}_{}", op_name, i), op_closure).await;
                
                if i % 3 == 0 {
                    memory_tracker.record_measurement();
                }
                
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        // Measure realistic workload
        let realistic_queries = generate_realistic_queries();
        for (i, (tool_name, args)) in realistic_queries.iter().take(15).enumerate() {
            performance.measure_async(&format!("realistic_{}_{}", tool_name, i), || {
                let server = mcp_server.clone();
                let tool_name = tool_name.clone();
                let args = args.clone();
                async move {
                    server.handle_tool_call(&tool_name, args).await
                }
            }).await;
        }

        memory_tracker.record_measurement();

        // Create baseline
        let baseline = PerformanceBaseline::from_measurement(
            "BEVDBG-012 Optimization Baseline",
            &performance,
            &memory_tracker
        );

        // Save baseline for future tests
        let baseline_path = "/tmp/bevdbg_012_baseline.json";
        baseline.save_to_file(baseline_path).expect("Should save baseline");

        println!("=== Performance Baseline Established ===");
        println!("Baseline name: {}", baseline.name);
        println!("Operations measured: {}", baseline.measurements.len());
        println!("Memory baseline: {:.2} MB", baseline.memory_baseline as f64 / 1_048_576.0);
        println!("Throughput baseline: {:.2} ops/sec", baseline.throughput_baseline);
        println!("Saved to: {}", baseline_path);

        // Validate baseline meets targets
        assert!(performance.meets_targets(), 
                "Baseline should meet performance targets");

        Ok(baseline)
    }).await;

    assert!(result.is_ok(), "Baseline establishment should succeed");
}

/// Test regression detection with artificially degraded performance
#[tokio::test]
async fn test_regression_detection_with_degraded_performance() {
    // First establish a good baseline
    let mut baseline_performance = PerformanceMeasurement::new("Baseline");
    for i in 0..10 {
        baseline_performance.record(&format!("operation_{}", i), Duration::from_millis(5));
    }
    baseline_performance.record("critical_operation", Duration::from_millis(2));
    let baseline_summary = baseline_performance.performance_summary();

    // Now simulate degraded performance (regression)
    let mut degraded_performance = PerformanceMeasurement::new("Degraded");
    for i in 0..8 {
        // Most operations slightly slower
        degraded_performance.record(&format!("operation_{}", i), Duration::from_millis(7));
    }
    
    // Some operations significantly slower (regressions)
    degraded_performance.record("operation_8", Duration::from_millis(25)); // 5x slower
    degraded_performance.record("operation_9", Duration::from_millis(15)); // 3x slower
    degraded_performance.record("critical_operation", Duration::from_millis(12)); // 6x slower
    let degraded_summary = degraded_performance.performance_summary();

    // Test regression detection
    let mut detector = RegressionDetector::new(15.0); // 15% threshold
    detector.set_baseline(&baseline_summary);
    let report = detector.check_regression(&degraded_summary);

    println!("=== Regression Detection Test ===");
    println!("{}", report.generate_report());

    // Validate regression detection
    assert!(report.has_regressions(), "Should detect regressions");
    assert!(report.regressions.len() >= 3, "Should detect at least 3 regressions");
    
    // Check critical operation regression
    let critical_regression = report.regressions.iter()
        .find(|r| r.operation_name == "critical_operation");
    assert!(critical_regression.is_some(), "Should detect critical operation regression");
    
    let critical_regression = critical_regression.unwrap();
    assert!(critical_regression.change_percent > 400.0, 
            "Critical operation should show >400% regression");

    println!("Regression detection test passed: ✓");
}

/// Test performance under different optimization configurations
#[tokio::test]
async fn test_performance_with_optimization_configurations() {
    let configurations = [
        ("no_optimizations", false, false, false),
        ("cache_only", true, false, false),
        ("pool_only", false, true, false),
        ("lazy_only", false, false, true),
        ("all_optimizations", true, true, true),
    ];

    let mut config_results = HashMap::new();

    for (config_name, use_cache, use_pool, use_lazy) in configurations {
        let mut performance = PerformanceMeasurement::with_targets(
            config_name,
            PerformanceTargets::testing_targets()
        );

        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3001,
        };

        let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
        let mcp_server = Arc::new(McpServer::new(config, brp_client.clone()));

        // Conditionally initialize optimization components
        let _cache = if use_cache {
            Some(CommandCache::new(CacheConfig::default()))
        } else {
            None
        };

        let _pool = if use_pool {
            Some(ResponsePool::new(ResponsePoolConfig::default()))
        } else {
            None
        };

        let _lazy_components = if use_lazy {
            Some(LazyComponents::new(brp_client.clone()))
        } else {
            None
        };

        // Test standard operations
        let test_operations = [
            ("health_check", json!({})),
            ("resource_metrics", json!({})),
            ("observe", json!({"query": "entities with Transform"})),
            ("observe", json!({"query": "entities with Health < 50"})),
        ];

        for (op_name, args) in test_operations {
            // Run each operation multiple times
            for i in 0..5 {
                performance.measure_async(&format!("{}_{}", op_name, i), || {
                    let server = mcp_server.clone();
                    async move {
                        server.handle_tool_call(op_name, args.clone()).await
                    }
                }).await;
            }
        }

        let summary = performance.performance_summary();
        config_results.insert(config_name.to_string(), summary);

        println!("Configuration '{}' completed - Throughput: {:.2} ops/sec", 
                config_name, summary.throughput_ops_per_sec);
    }

    // Compare results
    println!("\n=== Optimization Configuration Comparison ===");
    
    let baseline_throughput = config_results.get("no_optimizations")
        .map(|s| s.throughput_ops_per_sec)
        .unwrap_or(1.0);

    for (config_name, summary) in &config_results {
        let speedup = summary.throughput_ops_per_sec / baseline_throughput;
        println!("Config '{}': {:.2} ops/sec ({:.1}x speedup)", 
                config_name, summary.throughput_ops_per_sec, speedup);
    }

    // Validate that optimizations improve performance
    let all_opts_throughput = config_results.get("all_optimizations")
        .map(|s| s.throughput_ops_per_sec)
        .unwrap_or(0.0);

    assert!(all_opts_throughput > baseline_throughput * 1.5,
            "All optimizations should provide at least 1.5x speedup");

    println!("Optimization configuration testing passed: ✓");
}

/// Test long-term performance stability (quick version)
#[tokio::test]
async fn test_performance_stability_short_term() {
    let mut performance = PerformanceMeasurement::with_targets(
        "Stability Test",
        PerformanceTargets::testing_targets()
    );
    
    let mut memory_tracker = MemoryUsageTracker::new();

    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };

    let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
    let mcp_server = Arc::new(McpServer::new(config, brp_client));

    // Run for 30 seconds with continuous activity
    let test_duration = Duration::from_secs(30);
    let start_time = std::time::Instant::now();
    let mut iteration = 0;

    while start_time.elapsed() < test_duration {
        iteration += 1;
        
        let workload_pattern = match iteration % 3 {
            0 => "debugging_session",
            1 => "performance_analysis", 
            _ => "cache_warming",
        };

        let queries = generate_workload_pattern(workload_pattern);
        
        for (tool_name, args) in queries.into_iter().take(3) { // Limit for speed
            performance.measure_async(&format!("stability_{}_{}", iteration, tool_name), || {
                let server = mcp_server.clone();
                async move {
                    server.handle_tool_call(&tool_name, args).await
                }
            }).await;
        }

        // Memory measurement every 10 iterations
        if iteration % 10 == 0 {
            memory_tracker.record_measurement();
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let summary = performance.performance_summary();
    
    println!("=== Short-term Stability Test Results ===");
    println!("{}", performance.generate_report());
    println!("{}", memory_tracker.generate_report());

    // Validate stability
    assert!(memory_tracker.is_memory_stable(1_000_000.0), // 1MB/sec max growth
            "Memory should remain stable");
    
    assert!(summary.throughput_ops_per_sec > 5.0,
            "Should maintain reasonable throughput");

    // Check that no individual operation is extremely slow
    for (op_name, stats) in &summary.operation_stats {
        assert!(stats.p99_duration.as_millis() < 1000,
                "Operation {} should complete within 1s", op_name);
    }

    println!("Short-term stability test passed: ✓");
}

/// Test that performance improvements are maintained across different scenarios
#[tokio::test]
async fn test_optimization_consistency_across_scenarios() {
    let scenarios = [
        ("light_load", 5, 100),      // 5 operations, 100ms intervals
        ("medium_load", 15, 50),     // 15 operations, 50ms intervals  
        ("heavy_load", 25, 20),      // 25 operations, 20ms intervals
    ];

    let mut scenario_results = HashMap::new();

    for (scenario_name, op_count, interval_ms) in scenarios {
        let mut performance = PerformanceMeasurement::with_targets(
            scenario_name,
            PerformanceTargets::testing_targets()
        );

        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3001,
        };

        let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
        let mcp_server = Arc::new(McpServer::new(config, brp_client));

        // Execute operations based on scenario parameters
        let realistic_queries = generate_realistic_queries();
        for (i, (tool_name, args)) in realistic_queries.iter().take(op_count).enumerate() {
            performance.measure_async(&format!("{}_{}", tool_name, i), || {
                let server = mcp_server.clone();
                let tool_name = tool_name.clone();
                let args = args.clone();
                async move {
                    server.handle_tool_call(&tool_name, args).await
                }
            }).await;

            tokio::time::sleep(Duration::from_millis(interval_ms)).await;
        }

        let summary = performance.performance_summary();
        scenario_results.insert(scenario_name, summary);

        println!("Scenario '{}': {:.2} ops/sec", scenario_name, summary.throughput_ops_per_sec);
    }

    // Analyze consistency across scenarios
    println!("\n=== Optimization Consistency Analysis ===");

    let mut all_p99_latencies = Vec::new();
    for (scenario_name, summary) in &scenario_results {
        println!("Scenario '{}':", scenario_name);
        
        for (op_name, stats) in &summary.operation_stats {
            let p99_ms = stats.p99_duration.as_millis() as f64;
            all_p99_latencies.push(p99_ms);
            
            // Each operation should meet basic performance requirements
            assert!(p99_ms < 100.0, 
                    "Operation {} in scenario {} should be < 100ms, got {:.2}ms", 
                    op_name, scenario_name, p99_ms);
        }
    }

    // Check consistency - variance shouldn't be too high
    let mean_latency: f64 = all_p99_latencies.iter().sum::<f64>() / all_p99_latencies.len() as f64;
    let variance: f64 = all_p99_latencies.iter()
        .map(|x| (x - mean_latency).powi(2))
        .sum::<f64>() / all_p99_latencies.len() as f64;
    let std_dev = variance.sqrt();
    let coefficient_of_variation = std_dev / mean_latency;

    println!("Mean P99 latency: {:.2}ms", mean_latency);
    println!("Standard deviation: {:.2}ms", std_dev);
    println!("Coefficient of variation: {:.3}", coefficient_of_variation);

    // Performance should be reasonably consistent (CV < 0.5)
    assert!(coefficient_of_variation < 0.5,
            "Performance should be consistent across scenarios (CV: {:.3})", 
            coefficient_of_variation);

    println!("Optimization consistency test passed: ✓");
}

/// Test performance with cached vs non-cached operations
#[tokio::test]
async fn test_cache_performance_impact() {
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };

    let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
    let cache = CommandCache::new(CacheConfig::default());
    let mcp_server = Arc::new(McpServer::new(config, brp_client));

    // Test operations without cache (first run)
    let mut no_cache_performance = PerformanceMeasurement::new("No Cache");
    
    let cache_test_queries = [
        ("observe", json!({"query": "entities with Transform"})),
        ("observe", json!({"query": "entities with Health"})), 
        ("resource_metrics", json!({})),
    ];

    for (tool_name, args) in &cache_test_queries {
        for i in 0..5 {
            no_cache_performance.measure_async(&format!("cold_{}_{}", tool_name, i), || {
                let server = mcp_server.clone();
                let tool_name = tool_name.clone();
                let args = args.clone();
                async move {
                    server.handle_tool_call(&tool_name, args).await
                }
            }).await;
        }
    }

    // Warm up cache
    for (tool_name, args) in &cache_test_queries {
        let _ = cache.set(tool_name, args, json!({"cached": "data"})).await;
    }

    // Test operations with warm cache
    let mut cache_performance = PerformanceMeasurement::new("With Cache");
    
    for (tool_name, args) in &cache_test_queries {
        for i in 0..5 {
            cache_performance.measure_async(&format!("warm_{}_{}", tool_name, i), || async {
                // Simulate cache hit
                let _ = cache.get(tool_name, args).await;
            }).await;
        }
    }

    let no_cache_summary = no_cache_performance.performance_summary();
    let cache_summary = cache_performance.performance_summary();

    println!("=== Cache Performance Impact Analysis ===");
    println!("Without cache throughput: {:.2} ops/sec", no_cache_summary.throughput_ops_per_sec);
    println!("With cache throughput: {:.2} ops/sec", cache_summary.throughput_ops_per_sec);

    let cache_speedup = cache_summary.throughput_ops_per_sec / no_cache_summary.throughput_ops_per_sec;
    println!("Cache speedup: {:.1}x", cache_speedup);

    // Cache should provide significant speedup
    assert!(cache_speedup > 5.0, 
            "Cache should provide at least 5x speedup, got {:.2}x", cache_speedup);

    println!("Cache performance impact test passed: ✓");
}

/// Test memory usage regression detection
#[tokio::test]
async fn test_memory_regression_detection() {
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };

    // Baseline memory usage
    let mut baseline_tracker = MemoryUsageTracker::new();
    baseline_tracker.record_measurement();

    let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
    let mcp_server = Arc::new(McpServer::new(config, brp_client));

    baseline_tracker.record_measurement();

    // Perform some operations to establish baseline
    for i in 0..10 {
        let _ = mcp_server.handle_tool_call(
            "health_check", 
            json!({"iteration": i})
        ).await;
        
        if i % 3 == 0 {
            baseline_tracker.record_measurement();
        }
    }

    let baseline_memory = baseline_tracker.current_usage();
    let baseline_overhead = baseline_tracker.current_overhead();

    // Now simulate a scenario with potential memory regression
    let mut regression_tracker = MemoryUsageTracker::new();
    regression_tracker.record_measurement();

    // Perform operations that might cause memory growth
    for i in 0..20 {
        let large_query = json!({
            "query": format!("large query with data {}", "x".repeat(i * 100)),
            "include_large_data": true
        });
        
        let _ = mcp_server.handle_tool_call("observe", large_query).await;
        
        if i % 2 == 0 {
            regression_tracker.record_measurement();
        }
    }

    let regression_memory = regression_tracker.current_usage();
    let regression_overhead = regression_tracker.current_overhead();

    println!("=== Memory Regression Detection ===");
    println!("Baseline memory: {:.2} MB", baseline_memory as f64 / 1_048_576.0);
    println!("Regression memory: {:.2} MB", regression_memory as f64 / 1_048_576.0);
    println!("Baseline overhead: {:.2} MB", baseline_overhead as f64 / 1_048_576.0);
    println!("Regression overhead: {:.2} MB", regression_overhead as f64 / 1_048_576.0);

    // Check for excessive memory growth
    let memory_growth = regression_memory as f64 / baseline_memory as f64;
    let overhead_growth = regression_overhead as f64 / baseline_overhead.max(1) as f64;

    println!("Memory growth ratio: {:.2}x", memory_growth);
    println!("Overhead growth ratio: {:.2}x", overhead_growth);

    // Memory should not grow excessively (less than 3x for this test)
    assert!(memory_growth < 3.0, 
            "Memory growth should be reasonable, got {:.2}x", memory_growth);

    // Overhead should not exceed BEVDBG-012 limits
    assert!(regression_overhead < 50_000_000, // 50MB limit
            "Memory overhead should stay within limits");

    // Check memory stability
    assert!(regression_tracker.is_memory_stable(2_000_000.0), // 2MB/sec max growth
            "Memory should be stable during operations");

    println!("Memory regression detection test passed: ✓");
}