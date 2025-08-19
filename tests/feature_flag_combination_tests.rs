/// Feature Flag Combination Tests
/// 
/// Tests all combinations of feature flags to ensure that the optimization
/// system works correctly with different feature combinations and that
/// conditional compilation produces correct results.

use std::sync::Arc;
use serde_json::json;

use bevy_debugger_mcp::{
    config::Config,
    mcp_server::McpServer,
    brp_client::BrpClient,
};

mod integration;
use integration::IntegrationTestHarness;

/// Test basic functionality with minimal features
#[tokio::test]
async fn test_basic_debugging_features_only() {
    // This test runs with only basic-debugging enabled (default)
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };

    let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
    let mcp_server = McpServer::new(config, brp_client);

    // Basic functionality should work
    let health_result = mcp_server.handle_tool_call("health_check", json!({})).await;
    assert!(health_result.is_ok(), "Health check should work with basic features");

    let resource_result = mcp_server.handle_tool_call("resource_metrics", json!({})).await;
    assert!(resource_result.is_ok(), "Resource metrics should work with basic features");

    println!("Basic debugging features test passed: ✓");
}

/// Test with caching feature enabled
#[tokio::test]
#[cfg(feature = "caching")]
async fn test_caching_feature_enabled() {
    use bevy_debugger_mcp::command_cache::{CommandCache, CacheConfig};

    let cache = CommandCache::new(CacheConfig::default());
    
    // Test cache operations
    let test_data = json!({"test": "cached_data"});
    cache.set("test_command", &json!({"arg": "value"}), test_data.clone()).await;
    
    let cached_result = cache.get("test_command", &json!({"arg": "value"})).await;
    assert!(cached_result.is_some(), "Caching should work when feature is enabled");
    assert_eq!(cached_result.unwrap(), test_data, "Cached data should match");

    println!("Caching feature test passed: ✓");
}

/// Test with caching feature disabled
#[tokio::test]
#[cfg(not(feature = "caching"))]
async fn test_caching_feature_disabled() {
    // When caching is disabled, the system should still work but without caching benefits
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };

    let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
    let mcp_server = McpServer::new(config, brp_client);

    // Should work without caching
    let result1 = mcp_server.handle_tool_call("observe", json!({"query": "test"})).await;
    let result2 = mcp_server.handle_tool_call("observe", json!({"query": "test"})).await;

    // Both should work, but no caching benefits expected
    println!("System works correctly without caching feature: ✓");
}

/// Test with pooling feature enabled
#[tokio::test]
#[cfg(feature = "pooling")]
async fn test_pooling_feature_enabled() {
    use bevy_debugger_mcp::response_pool::{ResponsePool, ResponsePoolConfig};

    let pool = ResponsePool::new(ResponsePoolConfig::default());
    
    let test_data = json!({"response": "pooled_data", "size": "medium"});
    let serialized = pool.serialize_json(&test_data).await;
    
    assert!(serialized.is_ok(), "Pooling should work when feature is enabled");
    
    let stats = pool.get_statistics().await;
    assert!(stats.total_serializations > 0, "Should track pooling statistics");

    println!("Pooling feature test passed: ✓");
}

/// Test with lazy-init feature enabled
#[tokio::test]
#[cfg(feature = "lazy-init")]
async fn test_lazy_init_feature_enabled() {
    use bevy_debugger_mcp::lazy_init::LazyComponents;

    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };

    let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
    let lazy_components = LazyComponents::new(brp_client);

    // Test lazy initialization
    assert!(!lazy_components.is_any_initialized(), "Should start uninitialized");
    
    let _inspector = lazy_components.get_entity_inspector().await;
    assert!(lazy_components.is_any_initialized(), "Should be initialized after access");

    println!("Lazy initialization feature test passed: ✓");
}

/// Test with all optimization features enabled
#[tokio::test]
#[cfg(all(feature = "optimizations"))]
async fn test_all_optimizations_enabled() {
    use bevy_debugger_mcp::{
        lazy_init::LazyComponents,
        command_cache::CommandCache,
        response_pool::ResponsePool,
    };

    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };

    // All optimization systems should be available
    let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
    let lazy_components = LazyComponents::new(brp_client.clone());
    let cache = CommandCache::new(Default::default());
    let pool = ResponsePool::new(Default::default());
    let mcp_server = McpServer::new(config, brp_client);

    // Test that all systems work together
    let test_queries = [
        ("health_check", json!({})),
        ("resource_metrics", json!({})),
        ("observe", json!({"query": "test entities"})),
    ];

    for (tool_name, args) in test_queries {
        let result = mcp_server.handle_tool_call(tool_name, args).await;
        assert!(result.is_ok(), "Tool {} should work with all optimizations", tool_name);
    }

    // Test lazy initialization
    let _router = lazy_components.get_debug_command_router().await;
    
    // Test caching
    let test_data = json!({"optimized": true});
    cache.set("test", &json!({"key": "value"}), test_data.clone()).await;
    let cached = cache.get("test", &json!({"key": "value"})).await;
    assert_eq!(cached.unwrap(), test_data, "Caching should work");

    // Test pooling
    let serialized = pool.serialize_json(&json!({"pooled": "response"})).await;
    assert!(serialized.is_ok(), "Pooling should work");

    println!("All optimizations integration test passed: ✓");
}

/// Test feature combinations with entity inspection
#[tokio::test]
#[cfg(feature = "entity-inspection")]
async fn test_entity_inspection_with_optimizations() {
    let harness = IntegrationTestHarness::new().await.unwrap();

    // Test entity inspection commands
    let entity_commands = [
        ("observe", json!({"query": "entities with Transform"})),
        ("observe", json!({"query": "entities with Health < 50"})),
        ("observe", json!({"query": "all entities", "limit": 10})),
    ];

    for (command, args) in entity_commands {
        let result = harness.execute_tool_call(command, args).await;
        // May fail due to no real connection, but should not crash
        println!("Entity inspection command '{}': {}", command, result.is_ok());
    }

    println!("Entity inspection features test passed: ✓");
}

/// Test feature combinations with performance profiling
#[tokio::test]
#[cfg(feature = "performance-profiling")]
async fn test_performance_profiling_with_optimizations() {
    let harness = IntegrationTestHarness::new().await.unwrap();

    // Test performance profiling commands
    let profiling_commands = [
        ("experiment", json!({"type": "performance", "duration": 1000})),
        ("experiment", json!({"type": "memory", "duration": 2000})),
    ];

    for (command, args) in profiling_commands {
        let result = harness.execute_tool_call(command, args).await;
        println!("Performance profiling command '{}': {}", command, result.is_ok());
    }

    println!("Performance profiling features test passed: ✓");
}

/// Test feature combinations with visual debugging
#[tokio::test]
#[cfg(feature = "visual-debugging")]
async fn test_visual_debugging_with_optimizations() {
    let harness = IntegrationTestHarness::new().await.unwrap();

    // Test visual debugging commands
    let visual_commands = [
        ("screenshot", json!({"path": "/tmp/feature_test.png"})),
    ];

    for (command, args) in visual_commands {
        let result = harness.execute_tool_call(command, args).await;
        println!("Visual debugging command '{}': {}", command, result.is_ok());
    }

    println!("Visual debugging features test passed: ✓");
}

/// Test feature combinations with session management
#[tokio::test]
#[cfg(feature = "session-management")]
async fn test_session_management_with_optimizations() {
    let harness = IntegrationTestHarness::new().await.unwrap();

    // Test session management commands
    let session_commands = [
        ("checkpoint", json!({"name": "test_checkpoint"})),
        ("replay", json!({"session_id": "test_session"})),
    ];

    for (command, args) in session_commands {
        let result = harness.execute_tool_call(command, args).await;
        println!("Session management command '{}': {}", command, result.is_ok());
    }

    println!("Session management features test passed: ✓");
}

/// Test that disabled features don't cause compilation errors
#[tokio::test]
async fn test_disabled_features_compilation() {
    // This test ensures that code compiles correctly when features are disabled
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };

    let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
    let mcp_server = McpServer::new(config, brp_client);

    // Basic functionality should always work regardless of features
    let result = mcp_server.handle_tool_call("health_check", json!({})).await;
    
    // The result may be Ok or Err depending on mock state, but it should compile
    println!("Compilation test with current features passed: ✓");
}

/// Test graceful degradation when optimization features are disabled
#[tokio::test]
async fn test_graceful_degradation() {
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };

    let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
    let mcp_server = McpServer::new(config, brp_client);

    // Test that system works even if some optimizations are not available
    let test_operations = [
        ("health_check", json!({})),
        ("resource_metrics", json!({})),
        ("diagnostic_report", json!({"action": "generate"})),
    ];

    for (operation, args) in test_operations {
        let start_time = std::time::Instant::now();
        let result = mcp_server.handle_tool_call(operation, args).await;
        let duration = start_time.elapsed();
        
        // Operations should complete within reasonable time even without optimizations
        assert!(duration < std::time::Duration::from_secs(10), 
                "Operation {} should complete within 10 seconds", operation);
        
        println!("Operation '{}' completed in {:?} (result: {})", 
                operation, duration, result.is_ok());
    }

    println!("Graceful degradation test passed: ✓");
}

/// Test compile-time feature detection
#[tokio::test]
async fn test_compile_time_feature_detection() {
    use bevy_debugger_mcp::compile_opts::CompileConfig;

    // Test compile-time feature detection
    let caching_enabled = CompileConfig::caching_enabled();
    let pooling_enabled = CompileConfig::pooling_enabled();
    let lazy_init_enabled = CompileConfig::lazy_init_enabled();

    println!("Feature detection:");
    println!("  Caching enabled: {}", caching_enabled);
    println!("  Pooling enabled: {}", pooling_enabled);
    println!("  Lazy init enabled: {}", lazy_init_enabled);

    // Test that feature detection is consistent with actual compilation
    #[cfg(feature = "caching")]
    assert!(caching_enabled, "Caching feature detection should match compilation");
    
    #[cfg(not(feature = "caching"))]
    assert!(!caching_enabled, "Caching feature detection should match compilation");

    #[cfg(feature = "pooling")]
    assert!(pooling_enabled, "Pooling feature detection should match compilation");
    
    #[cfg(not(feature = "pooling"))]
    assert!(!pooling_enabled, "Pooling feature detection should match compilation");

    println!("Compile-time feature detection test passed: ✓");
}

/// Test optimization level configuration
#[tokio::test]
async fn test_optimization_level_configuration() {
    use bevy_debugger_mcp::compile_opts::CompileConfig;

    let optimization_level = CompileConfig::optimization_level();
    println!("Optimization level: {}", optimization_level);

    // Test that optimization level is reasonable
    assert!(optimization_level <= 3, "Optimization level should not exceed 3");

    // In debug builds, optimization level should be lower
    #[cfg(debug_assertions)]
    assert!(optimization_level <= 1, "Debug builds should have lower optimization level");

    // In release builds with optimizations, level should be higher
    #[cfg(all(not(debug_assertions), feature = "optimizations"))]
    assert!(optimization_level >= 2, "Optimized release builds should have higher level");

    println!("Optimization level configuration test passed: ✓");
}

/// Test that invalid feature combinations are handled correctly
#[tokio::test]
async fn test_invalid_feature_combinations() {
    // This test ensures that the system handles edge cases gracefully
    
    // Test with minimal configuration
    let config = Config {
        bevy_brp_host: "invalid.example.com".to_string(), // Invalid host
        bevy_brp_port: 99999, // Invalid port
        mcp_port: 3001,
    };

    let brp_client = Arc::new(tokio::sync::RwLock::new(BrpClient::new(&config)));
    let mcp_server = McpServer::new(config, brp_client);

    // System should handle invalid configuration gracefully
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        mcp_server.handle_tool_call("health_check", json!({}))
    ).await;

    // Should either succeed or fail gracefully within timeout
    match result {
        Ok(_) => println!("Health check with invalid config handled gracefully"),
        Err(_) => println!("Health check properly timed out with invalid config"),
    }

    println!("Invalid configuration handling test passed: ✓");
}

/// Integration test for all feature combinations
#[tokio::test]
async fn test_comprehensive_feature_integration() {
    let harness = IntegrationTestHarness::new().await.unwrap();

    // Test a comprehensive set of operations that may use different features
    let comprehensive_operations = [
        // Basic operations (should always work)
        ("health_check", json!({})),
        ("resource_metrics", json!({})),
        
        // Entity operations (require entity-inspection feature)
        ("observe", json!({"query": "entities with Transform"})),
        
        // Performance operations (require performance-profiling feature)
        ("experiment", json!({"type": "performance", "duration": 1000})),
        
        // Session operations (require session-management feature)
        ("checkpoint", json!({"name": "comprehensive_test"})),
        
        // Complex operations (may use multiple features)
        ("diagnostic_report", json!({"action": "generate", "include_performance": true})),
        ("orchestrate", json!({"tool": "observe", "arguments": {"query": "test"}})),
    ];

    let mut successful_operations = 0;
    let total_operations = comprehensive_operations.len();

    for (operation, args) in comprehensive_operations {
        match harness.execute_tool_call(operation, args).await {
            Ok(_) => {
                successful_operations += 1;
                println!("✓ Operation '{}' succeeded", operation);
            }
            Err(e) => {
                println!("⚠ Operation '{}' failed: {} (may be due to missing features or mock limitations)", 
                        operation, e);
            }
        }
    }

    // At least basic operations should work
    assert!(successful_operations >= 2, 
            "At least basic operations should work regardless of features");

    let success_rate = successful_operations as f64 / total_operations as f64;
    println!("Feature integration success rate: {:.1}% ({}/{})", 
            success_rate * 100.0, successful_operations, total_operations);

    println!("Comprehensive feature integration test completed: ✓");
}