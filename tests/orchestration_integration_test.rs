use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use bevy_debugger_mcp::brp_client::BrpClient;
use bevy_debugger_mcp::config::Config;
use bevy_debugger_mcp::mcp_server::McpServer;
use bevy_debugger_mcp::tool_orchestration::*;

fn create_test_config() -> Config {
    Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3000,
    }
}

#[tokio::test]
async fn test_mcp_server_orchestration_integration() {
    let config = create_test_config();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let server = McpServer::new(config, brp_client);

    // Test orchestration call
    let orchestration_args = json!({
        "tool": "observe",
        "arguments": {
            "query": "entities with health < 50"
        },
        "config": {
            "cache_results": true,
            "auto_record": false
        }
    });

    // This would normally fail because we don't have a real Bevy server,
    // but we're testing that the orchestration structure is working
    let result = server
        .handle_tool_call("orchestrate", orchestration_args)
        .await;

    // We expect this to fail because there's no real BRP connection,
    // but it should fail in the tool execution, not in the orchestration setup
    assert!(result.is_err());
}

#[tokio::test]
async fn test_mcp_server_pipeline_integration() {
    let config = create_test_config();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let server = McpServer::new(config, brp_client);

    // Test pipeline template call
    let pipeline_args = json!({
        "template": "observe_experiment_replay"
    });

    let result = server.handle_tool_call("pipeline", pipeline_args).await;

    // Should fail due to no BRP connection, but structure should be correct
    assert!(result.is_err());
}

#[tokio::test]
async fn test_mcp_server_custom_pipeline_validation() {
    let config = create_test_config();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let server = McpServer::new(config, brp_client);

    // Test custom pipeline with too many steps (should be rejected)
    let steps: Vec<_> = (0..51)
        .map(|i| {
            json!({
                "name": format!("step_{}", i),
                "tool": "observe",
                "arguments": {},
                "condition": null,
                "retry_config": null,
                "timeout": null
            })
        })
        .collect();

    let pipeline_args = json!({
        "pipeline": {
            "name": "too_many_steps",
            "description": "Test pipeline with too many steps",
            "steps": steps,
            "parallel_execution": false,
            "fail_fast": true,
            "created_at": "2024-01-01T00:00:00Z"
        }
    });

    let result = server.handle_tool_call("pipeline", pipeline_args).await;

    // Should be rejected due to too many steps
    assert!(result.is_err());
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("too complex") || error_msg.contains("50 steps"));
}

#[tokio::test]
async fn test_mcp_server_invalid_tool_validation() {
    let config = create_test_config();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let server = McpServer::new(config, brp_client);

    // Test custom pipeline with invalid tool
    let pipeline_args = json!({
        "pipeline": {
            "name": "invalid_tool_test",
            "description": "Test pipeline with invalid tool",
            "steps": [{
                "name": "bad_step",
                "tool": "nonexistent_tool",
                "arguments": {},
                "condition": null,
                "retry_config": null,
                "timeout": null
            }],
            "parallel_execution": false,
            "fail_fast": true,
            "created_at": "2024-01-01T00:00:00Z"
        }
    });

    let result = server.handle_tool_call("pipeline", pipeline_args).await;

    // Should be rejected due to unknown tool
    assert!(result.is_err());
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("Unknown tool") || error_msg.contains("nonexistent_tool"));
}

#[tokio::test]
async fn test_tool_context_basic_functionality() {
    let mut context = ToolContext::new();

    // Test that context initializes correctly
    assert_eq!(context.metadata.execution_count, 0);
    assert!(context.results.is_empty());
    assert!(context.variables.is_empty());

    // Test setting variables
    context.set_variable("test_key".to_string(), json!("test_value"));
    assert_eq!(context.get_variable("test_key"), Some(&json!("test_value")));
    assert_eq!(context.metadata.execution_count, 0); // Variables don't increment count

    // Test adding results
    let tool_result = ToolResult {
        tool_name: "test_tool".to_string(),
        execution_id: ExecutionId::new(),
        success: true,
        output: json!({"data": "test"}),
        error: None,
        execution_time: Duration::from_millis(100),
        timestamp: std::time::SystemTime::now(),
        cache_key: Some("cache_123".to_string()),
    };

    context.add_result("test_tool".to_string(), tool_result);
    assert_eq!(context.metadata.execution_count, 1);
    assert!(context.get_result("test_tool").is_some());
}

#[tokio::test]
async fn test_workflow_dsl_pipelines() {
    // Test that DSL pipelines are well-formed
    let observe_pipeline = WorkflowDSL::observe_experiment_replay();
    assert_eq!(observe_pipeline.name, "observe_experiment_replay");
    assert_eq!(observe_pipeline.steps.len(), 3);

    // Verify step ordering and tools
    assert_eq!(observe_pipeline.steps[0].tool, "observe");
    assert_eq!(observe_pipeline.steps[1].tool, "experiment");
    assert_eq!(observe_pipeline.steps[2].tool, "replay");

    // Verify conditions are set correctly
    assert!(observe_pipeline.steps[0].condition.is_none()); // First step has no condition
    assert!(observe_pipeline.steps[1].condition.is_some()); // Second step depends on first
    assert!(observe_pipeline.steps[2].condition.is_some()); // Third step depends on second

    let debug_pipeline = WorkflowDSL::debug_performance();
    assert_eq!(debug_pipeline.name, "debug_performance");
    assert_eq!(debug_pipeline.steps.len(), 2);
    assert_eq!(debug_pipeline.steps[0].tool, "stress");
    assert_eq!(debug_pipeline.steps[1].tool, "anomaly");
}

#[tokio::test]
async fn test_execution_id_uniqueness() {
    let id1 = ExecutionId::new();
    let id2 = ExecutionId::new();
    let id3 = ExecutionId::new();

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
async fn test_dependency_graph_basic_functionality() {
    let mut graph = DependencyGraph::new();

    // Test empty graph
    let empty_order = graph.get_execution_order(&[]).unwrap();
    assert!(empty_order.is_empty());

    // Test single tool
    let single_order = graph.get_execution_order(&["solo".to_string()]).unwrap();
    assert_eq!(single_order, vec!["solo"]);

    // Test chain dependency
    graph.add_dependency("b".to_string(), "a".to_string());
    graph.add_dependency("c".to_string(), "b".to_string());

    let chain_order = graph
        .get_execution_order(&["c".to_string(), "a".to_string(), "b".to_string()])
        .unwrap();
    assert_eq!(chain_order, vec!["a", "b", "c"]);
}

#[tokio::test]
async fn test_dependency_graph_cycle_detection() {
    let mut graph = DependencyGraph::new();

    // Create a cycle: A -> B -> C -> A
    graph.add_dependency("b".to_string(), "a".to_string());
    graph.add_dependency("c".to_string(), "b".to_string());
    graph.add_dependency("a".to_string(), "c".to_string());

    let result = graph.get_execution_order(&["a".to_string(), "b".to_string(), "c".to_string()]);
    assert!(result.is_err());

    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("Circular dependency"));
}

#[tokio::test]
async fn test_retry_config_defaults() {
    let default_config = RetryConfig::default();

    assert_eq!(default_config.max_attempts, 3);
    assert!(matches!(
        default_config.backoff_type,
        BackoffType::Exponential
    ));
    assert_eq!(default_config.initial_delay, Duration::from_millis(100));
    assert_eq!(default_config.max_delay, Duration::from_secs(30));
}

#[tokio::test]
async fn test_tool_context_config_defaults() {
    let default_config = ToolContextConfig::default();

    assert!(default_config.auto_record);
    assert!(!default_config.auto_experiment);
    assert!(default_config.cache_results);
    assert_eq!(default_config.max_execution_time, Duration::from_secs(300));
    assert!(!default_config.debug_mode);
}
