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

use rmcp::{model::*, service::server::McpService};
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
        info!("Starting MCP server in stdio mode with proper SDK");
        
        // Create server info
        let server_info = Implementation {
            name: "bevy-debugger-mcp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        
        // Create server capabilities
        let capabilities = ServerCapabilities {
            experimental: None,
            logging: None,
            prompts: None,
            resources: None,
            tools: Some(ToolCapability {
                list_changed: None,
            }),
        };
        
        // Start BRP connection in background
        let brp_client = self.brp_client.clone();
        tokio::spawn(async move {
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
        
        // Create MCP service with our tools
        let service = McpService::builder()
            .server_info(server_info)
            .capabilities(capabilities)
            .tool_handler(self.tools.router())
            .build();
        
        // Run the service on stdio
        service.stdio_transport().await.map_err(|e| {
            error!("MCP server error: {}", e);
            crate::error::Error::DebugError(format!("MCP server failed: {}", e))
        })?;
        
        Ok(())
    }
    
    /// Run the server in TCP mode for background operation
    pub async fn run_tcp(self) -> Result<()> {
        info!("Starting MCP server in TCP mode on port {}", self.config.mcp_port);
        
        // Create server info
        let server_info = Implementation {
            name: "bevy-debugger-mcp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        
        // Create server capabilities
        let capabilities = ServerCapabilities {
            experimental: None,
            logging: None,
            prompts: None,
            resources: None,
            tools: Some(ToolCapability {
                list_changed: None,
            }),
        };
        
        // Start BRP connection in background
        let brp_client = self.brp_client.clone();
        tokio::spawn(async move {
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
        
        // Create MCP service with our tools
        let service = McpService::builder()
            .server_info(server_info)
            .capabilities(capabilities)
            .tool_handler(self.tools.router())
            .build();
        
        // Run the service on TCP
        let addr = format!("127.0.0.1:{}", self.config.mcp_port);
        service.tcp_transport(&addr).await.map_err(|e| {
            error!("MCP TCP server error: {}", e);
            crate::error::Error::DebugError(format!("MCP TCP server failed: {}", e))
        })?;
        
        Ok(())
    }
}