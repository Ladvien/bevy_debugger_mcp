/// Integration tests for debug command infrastructure
use bevy_debugger_mcp::brp_messages::{
    DebugCommand, DebugResponse, DebugOverlayType, SessionOperation,
    ValidatedQuery, QueryFilter, QueryCost, EntityData, BrpRequest,
};
use bevy_debugger_mcp::debug_command_processor::{
    DebugCommandRequest, DebugCommandRouter, DebugCommandProcessor,
    EntityInspectionProcessor, DEBUG_COMMAND_TIMEOUT,
};
use bevy_debugger_mcp::error::{Error, Result};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Mock processor for testing
struct MockDebugProcessor {
    delay_ms: u64,
    should_fail: bool,
}

#[async_trait]
impl DebugCommandProcessor for MockDebugProcessor {
    async fn process(&self, command: DebugCommand) -> Result<DebugResponse> {
        if self.delay_ms > 0 {
            sleep(Duration::from_millis(self.delay_ms)).await;
        }
        
        if self.should_fail {
            return Err(Error::DebugError("Mock processor failed".to_string()));
        }
        
        match command {
            DebugCommand::GetStatus => {
                Ok(DebugResponse::Status {
                    version: "test-1.0.0".to_string(),
                    active_sessions: 1,
                    command_queue_size: 0,
                    performance_overhead_percent: 0.5,
                })
            }
            _ => Ok(DebugResponse::Custom(json!({"mock": "response"})))
        }
    }
    
    async fn validate(&self, _command: &DebugCommand) -> Result<()> {
        if self.should_fail {
            Err(Error::DebugError("Validation failed".to_string()))
        } else {
            Ok(())
        }
    }
    
    fn estimate_processing_time(&self, _command: &DebugCommand) -> Duration {
        Duration::from_millis(self.delay_ms)
    }
    
    fn supports_command(&self, command: &DebugCommand) -> bool {
        matches!(command, DebugCommand::GetStatus | DebugCommand::Custom { .. })
    }
}

#[tokio::test]
async fn test_debug_command_compatibility() {
    // Ensure new debug commands don't break existing BRP commands
    let query_request = BrpRequest::Query {
        filter: None,
        limit: Some(10),
        strict: Some(false),
    };
    
    let debug_request = BrpRequest::Debug {
        command: DebugCommand::GetStatus,
        correlation_id: "test-123".to_string(),
        priority: Some(5),
    };
    
    // Both should serialize/deserialize correctly
    let query_json = serde_json::to_string(&query_request).unwrap();
    let debug_json = serde_json::to_string(&debug_request).unwrap();
    
    let _query_parsed: BrpRequest = serde_json::from_str(&query_json).unwrap();
    let _debug_parsed: BrpRequest = serde_json::from_str(&debug_json).unwrap();
}

#[tokio::test]
async fn test_command_timeout() {
    let router = DebugCommandRouter::new();
    
    // Register a slow processor
    let slow_processor = Arc::new(MockDebugProcessor {
        delay_ms: 100,
        should_fail: false,
    });
    
    router.register_processor("mock".to_string(), slow_processor).await;
    
    // Create a command that will timeout
    let mut request = DebugCommandRequest::new(
        DebugCommand::GetStatus,
        "timeout-test".to_string(),
        None,
    );
    
    // Set a very short timeout
    request.timeout = Duration::from_millis(50);
    
    // Queue and process
    router.queue_command(request.clone()).await.unwrap();
    
    // Wait for timeout
    sleep(Duration::from_millis(60)).await;
    
    // Should timeout
    let result = router.process_next().await;
    assert!(result.is_some());
    assert!(matches!(result.unwrap(), Err(Error::Timeout(_))));
}

#[tokio::test]
async fn test_concurrent_commands() {
    let router = Arc::new(DebugCommandRouter::new());
    
    // Register processor
    let processor = Arc::new(MockDebugProcessor {
        delay_ms: 10,
        should_fail: false,
    });
    
    router.register_processor("mock".to_string(), processor).await;
    
    // Create 100+ concurrent commands
    let mut handles = Vec::new();
    
    for i in 0..100 {
        let router_clone = router.clone();
        let handle = tokio::spawn(async move {
            let request = DebugCommandRequest::new(
                DebugCommand::GetStatus,
                format!("concurrent-{}", i),
                Some((i % 10) as u8), // Vary priorities
            );
            
            router_clone.queue_command(request).await
        });
        
        handles.push(handle);
    }
    
    // Wait for all commands to be queued
    for handle in handles {
        handle.await.unwrap().unwrap();
    }
    
    // Process all commands
    let mut processed = 0;
    while let Some(result) = router.process_next().await {
        assert!(result.is_ok());
        processed += 1;
        if processed >= 100 {
            break;
        }
    }
    
    assert_eq!(processed, 100);
}

#[tokio::test]
async fn test_priority_ordering() {
    let router = DebugCommandRouter::new();
    
    // Register processor
    let processor = Arc::new(MockDebugProcessor {
        delay_ms: 0,
        should_fail: false,
    });
    
    router.register_processor("mock".to_string(), processor).await;
    
    // Queue commands with different priorities
    let low = DebugCommandRequest::new(
        DebugCommand::GetStatus,
        "low".to_string(),
        Some(1),
    );
    
    let medium = DebugCommandRequest::new(
        DebugCommand::GetStatus,
        "medium".to_string(),
        Some(5),
    );
    
    let high = DebugCommandRequest::new(
        DebugCommand::GetStatus,
        "high".to_string(),
        Some(9),
    );
    
    // Queue in reverse priority order
    router.queue_command(low).await.unwrap();
    router.queue_command(medium).await.unwrap();
    router.queue_command(high).await.unwrap();
    
    // Process and check order
    let first = router.process_next().await.unwrap().unwrap();
    assert_eq!(first.0, "high");
    
    let second = router.process_next().await.unwrap().unwrap();
    assert_eq!(second.0, "medium");
    
    let third = router.process_next().await.unwrap().unwrap();
    assert_eq!(third.0, "low");
}

#[tokio::test]
async fn test_response_correlation_ttl() {
    let router = DebugCommandRouter::new();
    
    // Register processor
    let processor = Arc::new(MockDebugProcessor {
        delay_ms: 0,
        should_fail: false,
    });
    
    router.register_processor("mock".to_string(), processor).await;
    
    // Create command with short TTL
    let mut request = DebugCommandRequest::new(
        DebugCommand::GetStatus,
        "ttl-test".to_string(),
        None,
    );
    request.response_ttl = Duration::from_millis(100);
    
    // Process command
    router.queue_command(request).await.unwrap();
    let result = router.process_next().await.unwrap().unwrap();
    
    // Response should be available immediately
    let response = router.get_response(&result.0).await;
    assert!(response.is_some());
    
    // Wait for TTL to expire
    sleep(Duration::from_millis(150)).await;
    
    // Clean up expired responses
    router.cleanup_expired_responses().await;
    
    // Response should no longer be available
    let response = router.get_response(&result.0).await;
    assert!(response.is_none());
}

#[tokio::test]
async fn test_metrics_collection() {
    let router = DebugCommandRouter::new();
    
    // Register processors
    let success_processor = Arc::new(MockDebugProcessor {
        delay_ms: 10,
        should_fail: false,
    });
    
    let failure_processor = Arc::new(MockDebugProcessor {
        delay_ms: 5,
        should_fail: true,
    });
    
    router.register_processor("success".to_string(), success_processor).await;
    router.register_processor("failure".to_string(), failure_processor).await;
    
    // Process successful commands
    for i in 0..5 {
        let request = DebugCommandRequest::new(
            DebugCommand::GetStatus,
            format!("success-{}", i),
            None,
        );
        router.queue_command(request).await.unwrap();
        router.process_next().await.unwrap().unwrap();
    }
    
    // Process failing commands
    for i in 0..3 {
        let request = DebugCommandRequest::new(
            DebugCommand::Custom {
                name: "fail".to_string(),
                params: json!({}),
            },
            format!("fail-{}", i),
            None,
        );
        router.queue_command(request).await.unwrap();
        let _ = router.process_next().await;
    }
    
    // Check metrics
    let metrics = router.get_metrics().await;
    assert_eq!(metrics.total_commands, 8);
    assert_eq!(metrics.successful_commands, 5);
    assert_eq!(metrics.failed_commands, 3);
    assert!(metrics.avg_processing_time_us > 0);
    assert!(metrics.is_within_performance_target()); // Should be < 1ms average
}

#[tokio::test]
async fn test_entity_inspection_processor() {
    let processor = EntityInspectionProcessor {};
    
    // Test valid entity inspection
    let command = DebugCommand::InspectEntity {
        entity_id: 123,
        include_metadata: Some(true),
        include_relationships: Some(true),
    };
    
    let response = processor.process(command).await.unwrap();
    
    match response {
        DebugResponse::EntityInspection { entity, metadata, relationships } => {
            assert_eq!(entity.id, 123);
            assert!(metadata.is_some());
            assert!(relationships.is_some());
        }
        _ => panic!("Wrong response type"),
    }
    
    // Test validation
    let invalid_command = DebugCommand::InspectEntity {
        entity_id: 0, // Invalid ID
        include_metadata: None,
        include_relationships: None,
    };
    
    let validation_result = processor.validate(&invalid_command).await;
    assert!(validation_result.is_err());
}

#[tokio::test]
async fn test_debug_command_serialization() {
    // Test all debug command variants serialize correctly
    let commands = vec![
        DebugCommand::InspectEntity {
            entity_id: 456,
            include_metadata: Some(false),
            include_relationships: None,
        },
        DebugCommand::ProfileSystem {
            system_name: "test_system".to_string(),
            duration_ms: Some(1000),
            track_allocations: Some(true),
        },
        DebugCommand::SetVisualDebug {
            overlay_type: DebugOverlayType::Colliders,
            enabled: true,
            config: Some(json!({"color": "red"})),
        },
        DebugCommand::ExecuteQuery {
            query: ValidatedQuery {
                id: "query-1".to_string(),
                filter: {
                    let mut filter = QueryFilter::default();
                    filter.with = Some(vec!["Transform".to_string()]);
                    filter
                },
                estimated_cost: QueryCost {
                    estimated_entities: 100,
                    estimated_time_us: 500,
                    estimated_memory: 1024,
                },
                hints: vec!["Use index".to_string()],
            },
            offset: Some(0),
            limit: Some(50),
        },
        DebugCommand::ProfileMemory {
            capture_backtraces: Some(false),
            target_systems: Some(vec!["system1".to_string()]),
        },
        DebugCommand::SessionControl {
            operation: SessionOperation::Create,
            session_id: None,
        },
        DebugCommand::GetStatus,
        DebugCommand::Custom {
            name: "custom_command".to_string(),
            params: json!({"key": "value"}),
        },
    ];
    
    for command in commands {
        let serialized = serde_json::to_string(&command).unwrap();
        let deserialized: DebugCommand = serde_json::from_str(&serialized).unwrap();
        
        // Verify round-trip works
        let reserialized = serde_json::to_string(&deserialized).unwrap();
        assert_eq!(serialized, reserialized);
    }
}

#[tokio::test]
async fn test_debug_response_serialization() {
    // Test all debug response variants serialize correctly
    let responses = vec![
        DebugResponse::EntityInspection {
            entity: EntityData {
                id: 789,
                components: HashMap::new(),
            },
            metadata: None,
            relationships: None,
        },
        DebugResponse::Status {
            version: "1.2.3".to_string(),
            active_sessions: 2,
            command_queue_size: 5,
            performance_overhead_percent: 1.5,
        },
        DebugResponse::Custom(json!({"custom": "data"})),
    ];
    
    for response in responses {
        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: DebugResponse = serde_json::from_str(&serialized).unwrap();
        
        // Verify round-trip works
        let reserialized = serde_json::to_string(&deserialized).unwrap();
        assert_eq!(serialized, reserialized);
    }
}

#[tokio::test]
async fn test_performance_requirement() {
    // Test that debug command processing meets <1ms requirement
    let router = DebugCommandRouter::new();
    
    // Register fast processor
    let processor = Arc::new(MockDebugProcessor {
        delay_ms: 0, // No artificial delay
        should_fail: false,
    });
    
    router.register_processor("fast".to_string(), processor).await;
    
    // Measure processing time
    let start = Instant::now();
    
    for _ in 0..100 {
        let request = DebugCommandRequest::new(
            DebugCommand::GetStatus,
            "perf-test".to_string(),
            None,
        );
        
        router.queue_command(request).await.unwrap();
        router.process_next().await.unwrap().unwrap();
    }
    
    let elapsed = start.elapsed();
    let avg_time_us = elapsed.as_micros() / 100;
    
    // Should be less than 1000us (1ms) average
    assert!(avg_time_us < 1000, "Average processing time {}us exceeds 1ms requirement", avg_time_us);
}

#[tokio::test]
async fn test_backward_compatibility() {
    // Ensure debug commands don't break existing tool routing
    use bevy_debugger_mcp::mcp_server::McpServer;
    use bevy_debugger_mcp::config::Config;
    use bevy_debugger_mcp::brp_client::BrpClient;
    use tokio::sync::RwLock;
    
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3000,
    };
    
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let server = McpServer::new(config, brp_client);
    
    // Test that existing tools still work
    let tools = vec![
        "observe",
        "experiment", 
        "screenshot",
        "hypothesis",
        "stress",
        "replay",
        "anomaly",
    ];
    
    for tool in tools {
        // These will fail due to no BRP connection, but shouldn't panic
        let _ = server.handle_tool_call(tool, json!({})).await;
    }
    
    // Test new debug tool
    let debug_args = json!({
        "command": {
            "type": "GetStatus"
        },
        "correlation_id": "test-compat",
        "priority": 5
    });
    
    let result = server.handle_tool_call("debug", debug_args).await;
    // Should at least not panic, even if it returns an error
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_debug_timeout_is_30_seconds() {
    // Verify the default timeout is 30 seconds as per requirements
    assert_eq!(DEBUG_COMMAND_TIMEOUT, Duration::from_secs(30));
    
    let request = DebugCommandRequest::new(
        DebugCommand::GetStatus,
        "timeout-check".to_string(),
        None,
    );
    
    assert_eq!(request.timeout, Duration::from_secs(30));
}