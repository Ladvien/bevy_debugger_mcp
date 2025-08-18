use serde_json::{json, Value};
use std::sync::Arc;
use tokio::io::{stdin, stdout, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

mod anomaly_detector;
mod brp_client;
mod brp_messages;
mod checkpoint;
mod config;
mod dead_letter_queue;
mod diagnostics;
mod error;
mod experiment_system;
mod hypothesis_system;
mod mcp_server;
mod playback_system;
mod query_parser;
mod recording_system;
mod resource_manager;
mod semantic_analyzer;
mod state_diff;
mod stress_test_system;
mod timeline_branching;
mod tool_orchestration;
mod tools;

use brp_client::BrpClient;
use config::Config;
use error::Result;
use mcp_server::McpServer;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Config::from_env()?;

    // Check if we should run in stdio mode (for Claude Code) or TCP mode
    let use_stdio = std::env::args().any(|arg| arg == "--stdio")
        || atty::isnt(atty::Stream::Stdin)
        || std::env::var("MCP_TRANSPORT")
            .map(|t| t == "stdio")
            .unwrap_or(false);

    if use_stdio {
        info!("Starting Bevy Debugger MCP Server in stdio mode for Claude Code");
        run_stdio_mode(config).await
    } else {
        info!(
            "Starting Bevy Debugger MCP Server in TCP mode on port {}",
            config.mcp_port
        );
        run_tcp_mode(config).await
    }
}

async fn run_stdio_mode(config: Config) -> Result<()> {
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let mcp_server = McpServer::new(config.clone(), brp_client.clone());

    let stdin = stdin();
    let mut stdout = stdout();
    let mut lines = BufReader::new(stdin).lines();

    // Start BRP connection in background
    let brp_handle = tokio::spawn(async move {
        loop {
            {
                let mut client = brp_client.write().await;
                if let Err(e) = client.connect_with_retry().await {
                    error!("Failed to connect to BRP: {}", e);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
        }
    });

    // Handle MCP protocol over stdio
    while let Some(line) = lines.next_line().await? {
        if let Ok(request) = serde_json::from_str::<Value>(&line) {
            let response = handle_mcp_request(&mcp_server, request).await;
            let response_json = serde_json::to_string(&response)?;
            stdout.write_all(response_json.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }
    }

    brp_handle.abort();
    Ok(())
}

async fn run_tcp_mode(config: Config) -> Result<()> {
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let mcp_server = McpServer::new(config.clone(), brp_client.clone());

    let listener = TcpListener::bind(format!("127.0.0.1:{}", config.mcp_port)).await?;
    info!("MCP Server listening on port {}", config.mcp_port);

    let server_handle = tokio::spawn(async move {
        if let Err(e) = mcp_server.run(listener).await {
            error!("MCP Server error: {}", e);
        }
    });

    let brp_handle = tokio::spawn(async move {
        loop {
            {
                let mut client = brp_client.write().await;
                if let Err(e) = client.connect_with_retry().await {
                    error!("Failed to connect to BRP: {}", e);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
        }
    });

    tokio::select! {
        _ = server_handle => {
            warn!("MCP Server task completed");
        }
        _ = brp_handle => {
            warn!("BRP client task completed");
        }
        _ = signal::ctrl_c() => {
            info!("Received SIGINT, shutting down gracefully");
        }
    }

    Ok(())
}

async fn handle_mcp_request(server: &McpServer, request: Value) -> Value {
    match request.get("method").and_then(|m| m.as_str()) {
        Some("initialize") => {
            json!({
                "jsonrpc": "2.0",
                "id": request.get("id"),
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {
                            "listChanged": false
                        }
                    },
                    "serverInfo": {
                        "name": "bevy-debugger-mcp",
                        "version": "0.1.0"
                    }
                }
            })
        }
        Some("tools/list") => {
            json!({
                "jsonrpc": "2.0",
                "id": request.get("id"),
                "result": {
                    "tools": [
                        {
                            "name": "observe",
                            "description": "Observe Bevy game state and entities",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "query": {
                                        "type": "string",
                                        "description": "Query to execute"
                                    }
                                }
                            }
                        },
                        {
                            "name": "experiment",
                            "description": "Run experiments on the game",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "type": {
                                        "type": "string",
                                        "description": "Type of experiment"
                                    }
                                }
                            }
                        },
                        {
                            "name": "screenshot",
                            "description": "Capture a screenshot of the Bevy game window for visual debugging and documentation. This tool captures exactly what the game is rendering, not the entire screen, making it perfect for bug reports, feature documentation, and visual testing. Supports timing controls to handle game startup and ensure proper frame rendering.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "path": {
                                        "type": "string",
                                        "description": "File path where to save the screenshot. Supports .png, .jpg, .bmp, .tga formats. Defaults to './screenshot.png'. Example: 'debug/player-bug.png'"
                                    },
                                    "warmup_duration": {
                                        "type": "integer",
                                        "description": "Time in milliseconds to wait after game connection before taking screenshot. Allows game systems to initialize properly. Default: 1000ms. Range: 0-30000ms. Use 2000-5000ms for complex scenes."
                                    },
                                    "capture_delay": {
                                        "type": "integer", 
                                        "description": "Additional delay in milliseconds before screenshot capture. Useful for waiting for animations to reach specific states. Default: 500ms. Range: 0-10000ms."
                                    },
                                    "wait_for_render": {
                                        "type": "boolean",
                                        "description": "Whether to wait for at least one frame to render before capture. Ensures the screenshot contains actual game content. Default: true. Set false only for debugging render issues."
                                    },
                                    "description": {
                                        "type": "string",
                                        "description": "Optional description of what this screenshot captures (e.g., 'Player UI bug with overlapping healthbars'). Logged for debugging and documentation purposes."
                                    }
                                },
                                "required": []
                            }
                        }
                    ]
                }
            })
        }
        Some("tools/call") => {
            let tool_name = request
                .get("params")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("unknown");

            let arguments = request
                .get("params")
                .and_then(|p| p.get("arguments"))
                .cloned()
                .unwrap_or(json!({}));

            match server.handle_tool_call(tool_name, arguments).await {
                Ok(result) => {
                    json!({
                        "jsonrpc": "2.0",
                        "id": request.get("id"),
                        "result": {
                            "content": [
                                {
                                    "type": "text",
                                    "text": result.to_string()
                                }
                            ]
                        }
                    })
                }
                Err(e) => {
                    json!({
                        "jsonrpc": "2.0",
                        "id": request.get("id"),
                        "error": {
                            "code": -32000,
                            "message": e.to_string()
                        }
                    })
                }
            }
        }
        _ => {
            json!({
                "jsonrpc": "2.0",
                "id": request.get("id"),
                "error": {
                    "code": -32601,
                    "message": "Method not found"
                }
            })
        }
    }
}
