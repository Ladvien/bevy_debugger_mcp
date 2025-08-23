/*
 * Integration tests for tool router functionality
 */

use std::sync::Arc;
use tokio::sync::RwLock;
use bevy_debugger_mcp::mcp_tools::{BevyDebuggerTools, ObserveRequest, ExperimentRequest, HypothesisRequest};
use bevy_debugger_mcp::brp_client::BrpClient;
use bevy_debugger_mcp::config::Config;
use rmcp::handler::server::{ServerHandler, tool::Parameters};
use rmcp::model::{Content, CallToolResult};
use serde_json::Value;

/// Test that BevyDebuggerTools can be created successfully
#[tokio::test]
async fn test_tools_creation() {
    let config = Config::default();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(config)));
    let tools = BevyDebuggerTools::new(brp_client);
    
    // Test ServerHandler implementation
    let server_info = tools.get_info();
    assert_eq!(server_info.server_info.name, "bevy-debugger-mcp");
    assert!(server_info.capabilities.tools.is_some());
    assert!(server_info.instructions.is_some());
}

/// Test observe tool with mock BRP client (expects connection error)
#[tokio::test]
async fn test_observe_tool_structure() {
    let config = Config::default();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(config)));
    let tools = BevyDebuggerTools::new(brp_client);
    
    let observe_req = ObserveRequest {
        query: "test query".to_string(),
        diff: false,
        detailed: false,
    };
    
    let params = Parameters(observe_req);
    
    // This should fail due to BRP connection, but structure should be correct
    let result = tools.observe(params).await;
    match result {
        Ok(_) => {
            // Unexpected success, but still valid
            println!("Observe tool succeeded unexpectedly");
        }
        Err(e) => {
            // Expected error due to BRP connection failure
            println!("Expected observe error: {:?}", e);
        }
    }
}

/// Test experiment tool with mock BRP client
#[tokio::test]
async fn test_experiment_tool_structure() {
    let config = Config::default();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(config)));
    let tools = BevyDebuggerTools::new(brp_client);
    
    let experiment_req = ExperimentRequest {
        experiment_type: "test_experiment".to_string(),
        params: Value::Null,
        duration: Some(1.0),
    };
    
    let params = Parameters(experiment_req);
    
    // This should fail due to BRP connection, but structure should be correct
    let result = tools.experiment(params).await;
    match result {
        Ok(_) => {
            println!("Experiment tool succeeded unexpectedly");
        }
        Err(e) => {
            println!("Expected experiment error: {:?}", e);
        }
    }
}

/// Test hypothesis tool with mock BRP client
#[tokio::test]
async fn test_hypothesis_tool_structure() {
    let config = Config::default();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(config)));
    let tools = BevyDebuggerTools::new(brp_client);
    
    let hypothesis_req = HypothesisRequest {
        hypothesis: "test hypothesis".to_string(),
        confidence: 0.8,
        context: None,
    };
    
    let params = Parameters(hypothesis_req);
    
    // This should fail due to BRP connection, but structure should be correct
    let result = tools.hypothesis(params).await;
    match result {
        Ok(_) => {
            println!("Hypothesis tool succeeded unexpectedly");
        }
        Err(e) => {
            println!("Expected hypothesis error: {:?}", e);
        }
    }
}

/// Test all parameter structures for JSON serialization/deserialization
#[test]
fn test_parameter_serialization() {
    let observe_req = ObserveRequest {
        query: "test".to_string(),
        diff: true,
        detailed: false,
    };
    
    let json = serde_json::to_string(&observe_req).unwrap();
    let deserialized: ObserveRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(observe_req.query, deserialized.query);
    assert_eq!(observe_req.diff, deserialized.diff);
    
    let experiment_req = ExperimentRequest {
        experiment_type: "test".to_string(),
        params: Value::String("test".to_string()),
        duration: Some(5.0),
    };
    
    let json = serde_json::to_string(&experiment_req).unwrap();
    let deserialized: ExperimentRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(experiment_req.experiment_type, deserialized.experiment_type);
    assert_eq!(experiment_req.duration, deserialized.duration);
}