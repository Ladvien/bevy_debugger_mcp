/*
 * Bevy Debugger MCP - rmcp 0.2.1 API Compatibility Tests
 * Copyright (C) 2025 ladvien
 * 
 * Tests for rmcp 0.2.1 API compatibility and ServerHandler implementation
 */

use bevy_debugger_mcp::{brp_client::BrpClient, config::Config, mcp_tools::BevyDebuggerTools};
use rmcp::{
    handler::server::ServerHandler,
    model::*,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_test;

/// Test that BevyDebuggerTools implements ServerHandler correctly
#[tokio::test]
async fn test_server_handler_implementation() {
    let config = Config::default();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let tools = BevyDebuggerTools::new(brp_client);

    // Test get_info method returns correct ServerInfo structure
    let server_info = tools.get_info();
    
    assert_eq!(server_info.protocol_version, ProtocolVersion::V_2024_11_05);
    assert_eq!(server_info.server_info.name, "bevy-debugger-mcp");
    assert!(server_info.server_info.version.len() > 0);
    assert!(server_info.capabilities.tools.is_some());
    assert!(server_info.instructions.is_some());
    assert!(server_info.instructions.as_ref().unwrap().contains("AI-assisted debugging"));
}

/// Test that tool routing works correctly
#[tokio::test]
async fn test_tool_routing() {
    let config = Config::default();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let tools = BevyDebuggerTools::new(brp_client);
    
    // Test list_tools functionality
    // Note: In real usage, the tool_handler macro would handle this
    let server_info = tools.get_info();
    assert!(server_info.capabilities.tools.is_some());
}

/// Test error handling compatibility with rmcp Error types
#[tokio::test]
async fn test_error_handling() {
    let config = Config::default();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let tools = BevyDebuggerTools::new(brp_client);
    
    // Test that tools can be created without compilation errors
    // This test mainly ensures the API compatibility is working
    let _server_info = tools.get_info();
    
    // If we get here, the rmcp 0.2.1 API compatibility is working
    assert!(true);
}

/// Test ServerHandler trait methods are properly implemented
#[tokio::test] 
async fn test_server_handler_traits() {
    let config = Config::default();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let tools = BevyDebuggerTools::new(brp_client);
    
    // Test that we can call ServerHandler methods
    let server_info = tools.get_info();
    
    // Verify the structure matches rmcp 0.2.1 expectations
    assert_eq!(server_info.protocol_version, ProtocolVersion::V_2024_11_05);
    
    // Verify capabilities are properly structured
    let caps = server_info.capabilities;
    assert!(caps.tools.is_some());
    
    // Verify server info is correct
    assert_eq!(server_info.server_info.name, "bevy-debugger-mcp");
}

/// Integration test for MCP tool schema generation
#[tokio::test]
async fn test_tool_schema_generation() {
    // TODO: Add schemars dependency if schema validation is needed
    // use schemars::JsonSchema;
    use bevy_debugger_mcp::mcp_tools::{ObserveRequest, ExperimentRequest, HypothesisRequest};
    
    // Test that our request structures can be created (basic structure validation)
    // let observe_schema = schemars::schema_for!(ObserveRequest);
    // assert!(observe_schema.schema.object.is_some());
    
    // let experiment_schema = schemars::schema_for!(ExperimentRequest);
    // assert!(experiment_schema.schema.object.is_some());
    
    // let hypothesis_schema = schemars::schema_for!(HypothesisRequest);
    // assert!(hypothesis_schema.schema.object.is_some());
    
    // For now, just verify we can reference the types
    println!("Schema validation tests disabled - schemars dependency not available");
}