/*
 * Bevy Debugger MCP Server
 * Copyright (C) 2025 ladvien
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use std::sync::Arc;
use tokio::signal;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use is_terminal::IsTerminal;

// Modules are defined in lib.rs, no need to redeclare them here

use bevy_debugger_mcp::brp_client::BrpClient;
use bevy_debugger_mcp::config::Config;
use bevy_debugger_mcp::error::Result;
use bevy_debugger_mcp::mcp_server;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    // Check for help flag
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("Bevy Debugger MCP Server v{}", env!("CARGO_PKG_VERSION"));
        println!("\nUsage: {} [OPTIONS]", args[0]);
        println!("\nOptions:");
        println!("  --stdio              Run in stdio mode (default for Claude Code)");
        println!("  --tcp, --server      Run as TCP server on port {}", Config::from_env().unwrap_or_default().mcp_port);
        println!("  --help, -h           Show this help message");
        println!("\nEnvironment variables:");
        println!("  BEVY_BRP_HOST        Bevy Remote Protocol host (default: localhost)");
        println!("  BEVY_BRP_PORT        Bevy Remote Protocol port (default: 15702)");
        println!("  MCP_PORT             MCP server port for TCP mode (default: 3001)");
        println!("  RUST_LOG             Logging level (default: info)");
        return Ok(());
    }
    
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Config::from_env()?;

    // Check if we should run in stdio mode (for Claude Code) or TCP mode
    let use_tcp = args.iter().any(|arg| arg == "--tcp" || arg == "--server");
    let use_stdio = !use_tcp && (
        args.iter().any(|arg| arg == "--stdio")
        || !is_terminal::IsTerminal::is_terminal(&std::io::stdin())
        || std::env::var("MCP_TRANSPORT")
            .map(|t| t == "stdio")
            .unwrap_or(false)
    );

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
    // For now, stdio mode is not fully implemented
    // The rmcp library integration needs more work
    error!("Stdio mode is not yet fully implemented. Please use TCP mode with --tcp flag.");
    error!("To use with Claude Code, you may need to run in TCP mode and configure accordingly.");
    Err(crate::error::Error::DebugError("Stdio mode not implemented".to_string()))
}

async fn run_tcp_mode(config: Config) -> Result<()> {
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    {
        let client = brp_client.read().await;
        client.init().await?;
    }
    let mcp_server = mcp_server::McpServer::new(config.clone(), brp_client);
    
    // Start TCP server
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", config.mcp_port))
        .await
        .map_err(|e| bevy_debugger_mcp::error::Error::Connection(format!("Failed to bind TCP: {}", e)))?;
    
    info!("MCP server listening on 127.0.0.1:{}", config.mcp_port);
    
    let server_handle = tokio::spawn(async move {
        if let Err(e) = mcp_server.run(listener).await {
            error!("MCP Server error: {}", e);
        }
    });

    tokio::select! {
        _ = server_handle => {
            warn!("MCP Server task completed");
        }
        _ = signal::ctrl_c() => {
            info!("Received SIGINT, shutting down gracefully");
        }
    }

    Ok(())
}
