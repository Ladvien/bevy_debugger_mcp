/*
 * Bevy Debugger MCP Server - Proper SDK Implementation
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

use rmcp::{
    model::*,
    handler::server::ServerHandler,
    serve_server,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::brp_client::BrpClient;
use crate::config::Config;
use crate::error::Result;
use crate::mcp_tools::BevyDebuggerTools;

/// Proper MCP server implementation using the official SDK
pub struct McpServerV2 {
    config: Config,
    brp_client: Arc<RwLock<BrpClient>>,
    tools: Arc<BevyDebuggerTools>,
}

impl McpServerV2 {
    pub fn new(config: Config, brp_client: Arc<RwLock<BrpClient>>) -> Self {
        let tools = Arc::new(BevyDebuggerTools::new(brp_client.clone()));
        
        Self {
            config,
            brp_client,
            tools,
        }
    }
    
    /// Run the server in stdio mode for Claude Code
    pub async fn run_stdio(self) -> Result<()> {
        info!("Starting MCP server in stdio mode for Claude Code integration");
        
        // Initialize BRP connection
        {
            let client = self.brp_client.read().await;
            if let Err(e) = client.init().await {
                error!("Failed to initialize BRP client: {}", e);
                return Err(crate::error::Error::Connection(format!("BRP initialization failed: {}", e)));
            }
        }
        
        // Start BRP connection heartbeat in background
        let brp_client = self.brp_client.clone();
        tokio::spawn(async move {
            loop {
                {
                    let mut client = brp_client.write().await;
                    if let Err(e) = client.connect_with_retry().await {
                        error!("BRP heartbeat failed: {}", e);
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            }
        });
        
        // Setup signal handlers for graceful shutdown
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
        
        // Handle SIGTERM and SIGINT
        tokio::spawn(async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};
                
                let mut sigterm = signal(SignalKind::terminate()).expect("Failed to setup SIGTERM handler");
                let mut sigint = signal(SignalKind::interrupt()).expect("Failed to setup SIGINT handler");
                
                tokio::select! {
                    _ = sigterm.recv() => {
                        info!("Received SIGTERM, shutting down gracefully");
                    }
                    _ = sigint.recv() => {
                        info!("Received SIGINT, shutting down gracefully");  
                    }
                }
            }
            #[cfg(not(unix))]
            {
                tokio::signal::ctrl_c().await.expect("Failed to setup Ctrl-C handler");
                info!("Received Ctrl-C, shutting down gracefully");
            }
            
            let _ = shutdown_tx.send(()).await;
        });
        
        info!("MCP stdio transport starting - ready for Claude Code connection");
        
        // Create stdio transport
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        
        // Run the server using the tools handler with proper error handling
        tokio::select! {
            result = serve_server(self.tools.as_ref().clone(), (stdin, stdout)) => {
                match result {
                    Ok(_) => {
                        info!("MCP stdio server completed successfully");
                        Ok(())
                    }
                    Err(e) => {
                        error!("MCP stdio server error: {}", e);
                        Err(crate::error::Error::DebugError(format!("MCP stdio server failed: {}", e)))
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                info!("Graceful shutdown requested");
                Ok(())
            }
        }
    }
    
    /// Run the server in TCP mode for background operation
    pub async fn run_tcp(self) -> Result<()> {
        info!("Starting MCP server in TCP mode on port {}", self.config.mcp_port);
        
        // For now, TCP mode is not implemented
        // You can use stdio mode instead
        error!("TCP mode not implemented, use stdio mode");
        Err(crate::error::Error::DebugError("TCP mode not implemented".to_string()))
    }
}

// McpServerV2 acts as a coordinator - the actual MCP handling is done by BevyDebuggerTools
// No ServerHandler implementation needed here since tools handle the MCP protocol directly