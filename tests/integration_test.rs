use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_mcp_handshake() {
    let mut config = bevy_debugger_mcp::config::Config::default();
    config.bevy_brp_host = "localhost".to_string();
    config.bevy_brp_port = 15702;
    config.mcp_port = 3001; // Use different port for testing

    // This is a basic test to ensure the server can be created
    // In a real test, we would simulate MCP handshake protocol
    let brp_client = std::sync::Arc::new(tokio::sync::RwLock::new(
        bevy_debugger_mcp::brp_client::BrpClient::new(&config),
    ));

    let server = bevy_debugger_mcp::mcp_server::McpServer::new(config, brp_client);

    // Test tool call handling
    let result = timeout(
        Duration::from_secs(1),
        server.handle_tool_call("observe", serde_json::json!({"query": "test"})),
    )
    .await;

    assert!(result.is_ok());
    let response = result.unwrap().unwrap();
    // Should return error since "test" is not a valid query and BRP client is not connected
    assert!(response.get("error").is_some());
}
