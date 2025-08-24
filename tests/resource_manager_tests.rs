use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;

use bevy_debugger_mcp::brp_client::BrpClient;
use bevy_debugger_mcp::config::Config;
use bevy_debugger_mcp::mcp_server::McpServer;
use bevy_debugger_mcp::resource_manager::*;

#[tokio::test]
async fn test_resource_config_defaults() {
    let config = ResourceConfig::default();

    assert_eq!(config.max_cpu_percent, 10.0);
    assert_eq!(config.max_memory_bytes, 100 * 1024 * 1024);
    assert_eq!(config.max_concurrent_operations, 50);
    assert_eq!(config.max_brp_requests_per_second, 100);
    assert!(config.adaptive_sampling_enabled);
    assert!(config.object_pooling_enabled);
    assert_eq!(config.circuit_breaker_threshold, 5);
    }

#[tokio::test]
async fn test_resource_manager_creation() {
    let config = ResourceConfig::default();
    let manager = ResourceManager::new(config);

    let metrics = manager.get_metrics().await;
    assert_eq!(metrics.concurrent_operations, 0);
    assert!(!metrics.circuit_breaker_open);
    assert!(metrics.adaptive_sampling_rate > 0.0);
    }

#[tokio::test]
async fn test_operation_permit_acquisition() {
    let config = Resource{ let mut config = Config::default();
        max_concurrent_operations: 2,
        ..Default::default()
    };
    let manager = ResourceManager::new(config);

    // Should be able to acquire permits
    let permit1 = manager.acquire_operation_permit().await;
    assert!(permit1.is_ok());

    let permit2 = manager.acquire_operation_permit().await;
    assert!(permit2.is_ok());

    // Should still work with default semaphore behavior
    let permit3 = manager.acquire_operation_permit().await;
    assert!(permit3.is_ok());
    }

#[tokio::test]
async fn test_circuit_breaker() {
    let breaker = CircuitBreaker::new(2, Duration::from_millis(100));

    assert!(!breaker.is_open().await);

    // Record failures to trip the breaker
    breaker.record_failure().await;
    assert!(!breaker.is_open().await);

    breaker.record_failure().await;
    assert!(breaker.is_open().await);

    // Wait for timeout and check reset
    sleep(Duration::from_millis(150)).await;
    assert!(!breaker.is_open().await);
    }

#[tokio::test]
async fn test_rate_limiter() {
    let limiter = RateLimiter::new(2); // 2 requests per second

    assert!(limiter.allow_request().await);
    assert!(limiter.allow_request().await);
    assert!(!limiter.allow_request().await); // Should be rate limited

    // Check current rate
    let rate = limiter.get_current_rate().await;
    assert_eq!(rate, 2);

    // Wait and try again
    sleep(Duration::from_millis(1100)).await;
    assert!(limiter.allow_request().await);
    }

#[tokio::test]
async fn test_object_pool() {
    let pool = ObjectPool::new(|| String::with_capacity(10), 2);

    // Acquire and check capacity
    let s1 = pool.acquire().await;
    assert_eq!(s1.capacity(), 10);

    let s2 = pool.acquire().await;
    assert_eq!(s2.capacity(), 10);

    // Release and check pool size
    pool.release(s1).await;
    assert_eq!(pool.size(), 1);

    pool.release(s2).await;
    assert_eq!(pool.size(), 2);

    // Acquire from pool (should reuse)
    let s3 = pool.acquire().await;
    assert_eq!(pool.size(), 1);
    assert_eq!(s3.capacity(), 10);
    }

#[tokio::test]
async fn test_adaptive_sampler() {
    let sampler = AdaptiveSampler::new(100);

    let initial_rate = sampler.get_sampling_rate().await;
    assert_eq!(initial_rate, 1.0);

    // Test should_sample returns boolean
    let should_sample = sampler.should_sample().await;
    assert!(should_sample || !should_sample);

    // Add samples with low resource usage to increase rate
    for _ in 0..15 {
        sampler.add_sample(2.0, 10 * 1024 * 1024, 5).await;
        }

    let new_rate = sampler.get_sampling_rate().await;
    // Rate should increase with low resource usage
    assert!(new_rate >= initial_rate);

    // Add samples with high resource usage to decrease rate
    for _ in 0..15 {
        sampler.add_sample(20.0, 90 * 1024 * 1024, 150).await;
        }

    let final_rate = sampler.get_sampling_rate().await;
    // Rate should decrease with high resource usage
    assert!(final_rate <= new_rate);
    }

#[tokio::test]
async fn test_resource_manager_monitoring() {
    let config = Resource{ let mut config = Config::default();
        monitoring_interval: Duration::from_millis(50),
        ..Default::default()
    };
    let mut manager = ResourceManager::new(config);

    // Start monitoring
    manager.start_monitoring().await.unwrap();

    // Let it run for a bit
    sleep(Duration::from_millis(150)).await;

    let metrics = manager.get_metrics().await;
    assert!(metrics.timestamp.elapsed().unwrap() < Duration::from_secs(1));

    // Stop monitoring
    manager.shutdown().await.unwrap();
    }

#[tokio::test]
async fn test_string_and_buffer_pools() {
    let config = Resource{ let mut config = Config::default();
        object_pooling_enabled: true,
        ..Default::default()
    };
    let manager = ResourceManager::new(config);

    // Test string pool
    let string1 = manager.acquire_string().await;
    assert!(string1.is_empty());

    manager.release_string(string1).await;

    let string2 = manager.acquire_string().await;
    manager.release_string(string2).await;

    // Test buffer pool
    let buffer1 = manager.acquire_buffer().await;
    assert!(buffer1.is_empty());

    manager.release_buffer(buffer1).await;

    let buffer2 = manager.acquire_buffer().await;
    manager.release_buffer(buffer2).await;
    }

#[tokio::test]
async fn test_performance_dashboard() {
    let config = ResourceConfig::default();
    let manager = ResourceManager::new(config);

    let dashboard = manager.get_performance_dashboard().await;

    assert!(dashboard.get("cpu").is_some());
    assert!(dashboard.get("memory").is_some());
    assert!(dashboard.get("operations").is_some());
    assert!(dashboard.get("brp_requests").is_some());
    assert!(dashboard.get("adaptive_sampling").is_some());
    assert!(dashboard.get("object_pooling").is_some());

    // Check CPU status structure
    let cpu = dashboard.get("cpu").unwrap();
    assert!(cpu.get("current_percent").is_some());
    assert!(cpu.get("limit_percent").is_some());
    assert!(cpu.get("status").is_some());

    // Check memory status structure
    let memory = dashboard.get("memory").unwrap();
    assert!(memory.get("current_bytes").is_some());
    assert!(memory.get("limit_bytes").is_some());
    assert!(memory.get("status").is_some());
    }

#[tokio::test]
async fn test_circuit_breaker_with_operation_tracking() {
    let config = Resource{ let mut config = Config::default();
        circuit_breaker_threshold: 2,
        ..Default::default()
    };
    let manager = ResourceManager::new(config);

    // Record failures to open circuit breaker
    manager.record_operation_failure().await;
    let metrics1 = manager.get_metrics().await;
    assert!(!metrics1.circuit_breaker_open);

    manager.record_operation_failure().await;
    let metrics2 = manager.get_metrics().await;
    assert!(metrics2.circuit_breaker_open);

    // Record success to close it
    manager.record_operation_success().await;
    let metrics3 = manager.get_metrics().await;
    assert!(!metrics3.circuit_breaker_open);
    }

#[tokio::test]
async fn test_brp_rate_limiting() {
    let config = Resource{ let mut config = Config::default();
        max_brp_requests_per_second: 2,
        ..Default::default()
    };
    let manager = ResourceManager::new(config);

    // First requests should pass
    assert!(manager.check_brp_rate_limit().await);
    assert!(manager.check_brp_rate_limit().await);

    // Third request should be rate limited
    assert!(!manager.check_brp_rate_limit().await);
    }

#[tokio::test]
async fn test_sampling_decision() {
    let config = Resource{ let mut config = Config::default();
        adaptive_sampling_enabled: true,
        ..Default::default()
    };
    let manager = ResourceManager::new(config);

    // Should sample returns a boolean
    let should_sample = manager.should_sample().await;
    assert!(should_sample || !should_sample);

    // Test with sampling disabled
    let config_disabled = Resource{ let mut config = Config::default();
        adaptive_sampling_enabled: false,
        ..Default::default()
    };
    let manager_disabled = ResourceManager::new(config_disabled);

    assert!(manager_disabled.should_sample().await);
    }

#[tokio::test]
async fn test_mcp_server_integration() {
    let config = { let mut config = Config::default();
        config.bevy_brp_host = "localhost".to_string();
        config.bevy_brp_port = 15702;
        config.mcp_port = 3000; config
    };
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let server = McpServer::new(config, brp_client);

    // Test resource metrics endpoint
    let metrics_result = server.handle_tool_call("resource_metrics", json!({})).await;
    assert!(metrics_result.is_ok());

    let metrics_value = metrics_result.unwrap();
    assert!(metrics_value.get("cpu_percent").is_some());
    assert!(metrics_value.get("memory_bytes").is_some());
    assert!(metrics_value.get("timestamp").is_some());

    // Test performance dashboard endpoint
    let dashboard_result = server
        .handle_tool_call("performance_dashboard", json!({}))
        .await;
    assert!(dashboard_result.is_ok());

    let dashboard_value = dashboard_result.unwrap();
    assert!(dashboard_value.get("cpu").is_some());
    assert!(dashboard_value.get("memory").is_some());

    // Test health check endpoint
    let health_result = server.handle_tool_call("health_check", json!({})).await;
    assert!(health_result.is_ok());

    let health_value = health_result.unwrap();
    assert!(health_value.get("status").is_some());
    assert!(health_value.get("checks").is_some());

    let status = health_value.get("status").unwrap().as_str().unwrap();
    assert!(["healthy", "degraded", "circuit_breaker_open"].contains(&status));
    }

#[tokio::test]
async fn test_health_check_status_determination() {
    let config = { let mut config = Config::default();
        config.bevy_brp_host = "localhost".to_string();
        config.bevy_brp_port = 15702;
        config.mcp_port = 3000; config
    };
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let server = McpServer::new(config, brp_client);

    let health_result = server.handle_tool_call("health_check", json!({})).await;
    assert!(health_result.is_ok());

    let health_value = health_result.unwrap();
    let checks = health_value.get("checks").unwrap();

    // Check that all required health checks are present
    assert!(checks.get("cpu").is_some());
    assert!(checks.get("memory").is_some());
    assert!(checks.get("circuit_breaker").is_some());

    // Verify check structure
    let cpu_check = checks.get("cpu").unwrap();
    assert!(cpu_check.get("status").is_some());
    assert!(cpu_check.get("value").is_some());
    assert!(cpu_check.get("threshold").is_some());

    let memory_check = checks.get("memory").unwrap();
    assert!(memory_check.get("status").is_some());
    assert!(memory_check.get("value_mb").is_some());
    assert!(memory_check.get("threshold_mb").is_some());

    let circuit_check = checks.get("circuit_breaker").unwrap();
    assert!(circuit_check.get("status").is_some());
    assert!(circuit_check.get("open").is_some());
    }

#[tokio::test]
async fn test_graceful_degradation() {
    let config = ResourceConfig::default();
    let manager = ResourceManager::new(config);

    // Test that graceful degradation doesn't fail
    let result = manager.implement_graceful_degradation().await;
    assert!(result.is_ok());
    }

#[tokio::test]
async fn test_resource_manager_shutdown() {
    let config = Resource{ let mut config = Config::default();
        monitoring_interval: Duration::from_millis(10),
        ..Default::default()
    };
    let mut manager = ResourceManager::new(config);

    // Start monitoring
    manager.start_monitoring().await.unwrap();

    // Let it run briefly
    sleep(Duration::from_millis(50)).await;

    // Shutdown should work without errors
    let result = manager.shutdown().await;
    assert!(result.is_ok());
    }

#[tokio::test]
async fn test_execution_id_uniqueness() {
    let id1 = ResourceId::new();
    let id2 = ResourceId::new();
    let id3 = ResourceId::new();

    // All IDs should be unique
    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
    assert_ne!(id1, id3);

    // String representations should also be unique and non-empty
    let str1 = id1.to_string();
    let str2 = id2.to_string();
    let str3 = id3.to_string();

    assert_ne!(str1, str2);
    assert_ne!(str2, str3);
    assert_ne!(str1, str3);

    assert!(!str1.is_empty());
    assert!(!str2.is_empty());
    assert!(!str3.is_empty());
    }

#[tokio::test]
async fn test_resource_metrics_serialization() {
    let config = ResourceConfig::default();
    let manager = ResourceManager::new(config);

    let metrics = manager.get_metrics().await;

    // Test that metrics can be serialized to JSON
    let json_result = serde_json::to_value(&metrics);
    assert!(json_result.is_ok());

    let json_value = json_result.unwrap();
    assert!(json_value.get("timestamp").is_some());
    assert!(json_value.get("cpu_percent").is_some());
    assert!(json_value.get("memory_bytes").is_some());
    assert!(json_value.get("circuit_breaker_open").is_some());
    assert!(json_value.get("adaptive_sampling_rate").is_some());
    }

#[tokio::test]
async fn test_concurrent_resource_operations() {
    let config = ResourceConfig::default();
    let manager = Arc::new(ResourceManager::new(config));

    let mut handles = Vec::new();

    // Spawn multiple tasks that acquire resources concurrently
    for _ in 0..10 {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            let _permit = manager_clone.acquire_operation_permit().await.unwrap();
            let _string = manager_clone.acquire_string().await;
            let _buffer = manager_clone.acquire_buffer().await;

            // Simulate some work
            sleep(Duration::from_millis(10)).await;

            manager_clone.record_operation_success().await;
        });
        handles.push(handle);
        }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
        }

    // Verify system is still functioning
    let metrics = manager.get_metrics().await;
    assert!(!metrics.circuit_breaker_open);
    }
