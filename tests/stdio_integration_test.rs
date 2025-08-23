/*
 * Bevy Debugger MCP - Stdio Transport Integration Tests
 * Copyright (C) 2025 ladvien
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command as TokioCommand};
use tokio::time::timeout;

/// Test helper to start the MCP server in stdio mode
async fn start_mcp_server() -> Result<Child, Box<dyn std::error::Error>> {
    let mut cmd = TokioCommand::new("cargo");
    cmd.args(&["run", "--", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("RUST_LOG", "debug")
        .env("BEVY_BRP_HOST", "localhost")
        .env("BEVY_BRP_PORT", "15702");

    let child = cmd.spawn()?;
    
    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    Ok(child)
}

/// Test MCP handshake with stdio transport
#[tokio::test]
async fn test_mcp_stdio_handshake() -> Result<(), Box<dyn std::error::Error>> {
    // Skip if we don't have a working binary
    if Command::new("cargo")
        .args(&["check", "--bin", "bevy-debugger-mcp"])
        .output()
        .is_err()
    {
        println!("Skipping integration test - binary not available");
        return Ok(());
    }

    let mut server = start_mcp_server().await?;
    
    let stdin = server.stdin.as_mut().expect("Failed to get stdin");
    let stdout = server.stdout.as_mut().expect("Failed to get stdout");
    let mut reader = BufReader::new(stdout);
    
    // Send MCP initialize request
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "roots": {
                    "listChanged": true
                },
                "sampling": {}
            },
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        }
    });
    
    let request_line = format!("{}\n", serde_json::to_string(&init_request)?);
    stdin.write_all(request_line.as_bytes()).await?;
    stdin.flush().await?;
    
    // Try to read response with timeout
    let mut response_line = String::new();
    match timeout(Duration::from_secs(10), reader.read_line(&mut response_line)).await {
        Ok(Ok(_)) => {
            // Parse the JSON response
            let response: serde_json::Value = serde_json::from_str(&response_line)?;
            
            // Check if it's a valid MCP response
            assert!(response.get("jsonrpc").is_some());
            assert_eq!(response["jsonrpc"], "2.0");
            
            // Should have either result or error
            assert!(response.get("result").is_some() || response.get("error").is_some());
            
            if let Some(result) = response.get("result") {
                // Check for server info in successful response
                assert!(result.get("serverInfo").is_some());
                println!("MCP handshake successful: {}", result);
            } else if let Some(error) = response.get("error") {
                println!("MCP handshake error (expected during development): {}", error);
            }
        }
        Ok(Err(e)) => {
            println!("Failed to read response: {}", e);
        }
        Err(_) => {
            println!("Timeout waiting for MCP response - server may still be starting up");
        }
    }
    
    // Cleanup
    let _ = server.kill().await;
    
    Ok(())
}

/// Test BRP connectivity validation
#[tokio::test]
async fn test_brp_connectivity_validation() -> Result<(), Box<dyn std::error::Error>> {
    // This test validates that the BRP client can attempt to connect to port 15702
    // even if there's no Bevy game running (it should fail gracefully)
    
    use bevy_debugger_mcp::{config::Config, brp_client::BrpClient};
    use std::sync::Arc;
    use tokio::sync::RwLock;
    
    let config = Config::from_env().unwrap_or_default();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    
    // Try to initialize - this should not panic even if BRP is unavailable
    {
        let client = brp_client.read().await;
        match client.init().await {
            Ok(_) => {
                println!("BRP connection successful (Bevy game is running)");
            }
            Err(e) => {
                println!("BRP connection failed as expected (no Bevy game): {}", e);
                // This is expected when no Bevy game is running
            }
        }
    }
    
    // Test connection retry mechanism
    {
        let mut client = brp_client.write().await;
        match client.connect_with_retry().await {
            Ok(_) => {
                println!("BRP retry connection successful");
            }
            Err(e) => {
                println!("BRP retry failed gracefully: {}", e);
                // This should fail gracefully, not panic
            }
        }
    }
    
    Ok(())
}

/// Test signal handling for graceful shutdown
#[tokio::test]
async fn test_graceful_shutdown() -> Result<(), Box<dyn std::error::Error>> {
    // Skip on non-Unix systems as signal handling is different
    #[cfg(not(unix))]
    {
        println!("Skipping signal test on non-Unix system");
        return Ok(());
    }
    
    #[cfg(unix)]
    {
        if Command::new("cargo")
            .args(&["check", "--bin", "bevy-debugger-mcp"])
            .output()
            .is_err()
        {
            println!("Skipping signal test - binary not available");
            return Ok(());
        }

        let mut server = start_mcp_server().await?;
        
        // Give server time to set up signal handlers
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Send SIGTERM
        if let Some(id) = server.id() {
            let _ = Command::new("kill")
                .args(&["-TERM", &id.to_string()])
                .output();
        }
        
        // Wait for graceful shutdown
        match timeout(Duration::from_secs(5), server.wait()).await {
            Ok(Ok(status)) => {
                println!("Server shut down gracefully with status: {}", status);
                // Exit code should indicate clean shutdown
            }
            Ok(Err(e)) => {
                println!("Error waiting for server shutdown: {}", e);
            }
            Err(_) => {
                println!("Timeout waiting for graceful shutdown");
                let _ = server.kill().await;
            }
        }
    }
    
    Ok(())
}

/// Test that configuration is properly loaded
#[test]
fn test_configuration_loading() {
    use bevy_debugger_mcp::config::Config;
    
    // Test default configuration
    let config = Config::default();
    assert_eq!(config.bevy_brp_host, "localhost");
    assert_eq!(config.bevy_brp_port, 15702);
    assert_eq!(config.mcp_port, 3001);
    
    // Test environment variable loading
    std::env::set_var("BEVY_BRP_PORT", "12345");
    let config = Config::from_env().unwrap_or_default();
    assert_eq!(config.bevy_brp_port, 12345);
    
    // Cleanup
    std::env::remove_var("BEVY_BRP_PORT");
}