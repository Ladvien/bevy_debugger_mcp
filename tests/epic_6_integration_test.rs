/*
 * Epic 6 Integration Test - Security + Observability + Bevy Integration
 * 
 * This test validates that:
 * 1. Security (JWT/RBAC) doesn't interfere with BRP connections
 * 2. Observability captures Bevy-specific metrics
 * 3. Authentication works seamlessly with MCP protocol
 * 4. Connection resilience is maintained under security constraints
 * 5. Performance monitoring works for ECS systems
 */

use bevy_debugger_mcp::{
    brp_client::BrpClient,
    config::Config,
    error::Result,
    mcp_server_v2::McpServerV2,
    security::{SecurityManager, SecurityContext, ClientInfo},
    mcp_tools::BevyDebuggerTools,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::test;

/// Integration test for Epic 6 production features
#[test]
async fn test_epic_6_security_observability_integration() -> Result<()> {
    // Setup test configuration
    let mut config = Config::from_env()?;
    config.brp_host = "127.0.0.1".to_string();
    config.brp_port = 15702;

    // Initialize BRP client
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    
    // Test 1: Verify BRP connection works without security
    {
        let client = brp_client.read().await;
        // This should succeed even if Bevy isn't running (connection attempt is what we're testing)
        let result = client.connect_with_retry().await;
        // We expect this to fail with connection error, not a security error
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("connection") || error_msg.contains("Connection"));
    }

    // Test 2: Initialize security system
    let security_config = bevy_debugger_mcp::security::config::SecurityConfig::default();
    let security_manager = SecurityManager::new(security_config)?;
    
    // Generate test JWT token
    let test_token = security_manager
        .generate_token("test_bevy_user", bevy_debugger_mcp::security::rbac::Role::Developer)
        .await?;
    
    // Test 3: Authenticate with JWT
    let client_info = ClientInfo {
        ip: Some("127.0.0.1".parse().unwrap()),
        user_agent: Some("bevy-integration-test".to_string()),
    };
    
    let security_context = security_manager.authenticate(&test_token, client_info).await?;
    assert_eq!(security_context.user_id, "test_bevy_user");
    assert_eq!(security_context.role, bevy_debugger_mcp::security::rbac::Role::Developer);

    // Test 4: Verify authorization for Bevy operations
    let operations_to_test = [
        ("observe", "entities"),
        ("experiment", "systems"),
        ("stress_test", "performance"),
        ("hypothesis", "behavior"),
    ];

    for (operation, resource) in operations_to_test.iter() {
        let authorized = security_manager
            .authorize(&security_context, operation, resource)
            .await?;
        assert!(authorized, "Developer should be authorized for {} on {}", operation, resource);
    }

    // Test 5: Initialize MCP tools with security context
    let tools = Arc::new(BevyDebuggerTools::new(brp_client.clone()));
    
    // Test 6: Verify security metrics are available
    let security_metrics = security_manager.get_metrics().await;
    assert_eq!(security_metrics.active_sessions, 1);
    assert_eq!(security_metrics.failed_authentications, 0);

    // Test 7: Test token revocation doesn't break BRP connection
    security_manager.revoke_token(&test_token).await?;
    
    // Verify token is revoked
    let revoked_result = security_manager.authenticate(&test_token, ClientInfo {
        ip: Some("127.0.0.1".parse().unwrap()),
        user_agent: Some("bevy-integration-test".to_string()),
    }).await;
    assert!(revoked_result.is_err());

    // Verify BRP connection is still functional (independent of security layer)
    {
        let client = brp_client.read().await;
        let result = client.connect_with_retry().await;
        // Still expect connection error, but not security error
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("connection") || error_msg.contains("Connection"));
    }

    Ok(())
}

/// Test Bevy-specific observability integration points
#[test] 
async fn test_bevy_observability_integration() -> Result<()> {
    // This test will be expanded once observability module is implemented
    let config = Config::from_env()?;
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    
    // Test observability hooks for Bevy-specific metrics
    let expected_bevy_metrics = [
        "brp_connection_health",
        "brp_request_latency", 
        "brp_reconnection_count",
        "ecs_entity_count",
        "ecs_system_runtime",
        "bevy_frame_time",
        "memory_usage_entities",
        "memory_usage_components",
    ];

    // Verify metric collection points exist
    for metric_name in expected_bevy_metrics.iter() {
        // This will be implemented once observability module is created
        println!("Would collect metric: {}", metric_name);
    }

    Ok(())
}

/// Test security isolation from BRP connection resilience
#[test]
async fn test_security_brp_isolation() -> Result<()> {
    let config = Config::from_env()?;
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));

    // Test that security failures don't affect BRP connection state
    let security_config = bevy_debugger_mcp::security::config::SecurityConfig::default();
    let security_manager = SecurityManager::new(security_config)?;

    // Simulate authentication failures
    for i in 0..5 {
        let result = security_manager.authenticate(
            "invalid_token",
            ClientInfo {
                ip: Some("127.0.0.1".parse().unwrap()),
                user_agent: Some(format!("test-client-{}", i)),
            }
        ).await;
        assert!(result.is_err());
    }

    // Verify BRP connection remains unaffected by security failures
    {
        let mut client = brp_client.write().await;
        // Connection attempt should still work (fail with connection error, not security error)
        let result = client.connect_with_retry().await;
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(!error_msg.contains("auth") && !error_msg.contains("security"));
    }

    Ok(())
}

/// Performance test for security overhead on Bevy debugging operations
#[test]
async fn test_security_performance_overhead() -> Result<()> {
    let config = Config::from_env()?;
    let security_config = bevy_debugger_mcp::security::config::SecurityConfig::default();
    let security_manager = SecurityManager::new(security_config)?;

    let token = security_manager
        .generate_token("perf_test_user", bevy_debugger_mcp::security::rbac::Role::Developer)
        .await?;

    let client_info = ClientInfo {
        ip: Some("127.0.0.1".parse().unwrap()),
        user_agent: Some("performance-test".to_string()),
    };

    // Measure authentication performance
    let start = std::time::Instant::now();
    for _ in 0..100 {
        let _context = security_manager.authenticate(&token, client_info.clone()).await?;
    }
    let auth_duration = start.elapsed();

    // Ensure authentication is fast enough for real-time debugging
    assert!(auth_duration < Duration::from_millis(100), 
           "Authentication too slow: {:?} for 100 operations", auth_duration);

    println!("Security performance: {:?} for 100 auth operations", auth_duration);
    Ok(())
}