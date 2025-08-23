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
    service::{RoleServer, Service, RequestContext, NotificationContext},
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
        info!("Starting MCP server in stdio mode with proper SDK");
        
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
        
        // Create stdio transport
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        
        // Run the server using serve_server
        let _running = serve_server(self, (stdin, stdout)).await.map_err(|e| {
            error!("MCP server initialization error: {}", e);
            crate::error::Error::DebugError(format!("MCP server initialization failed: {}", e))
        })?;
        
        // The server will run until the connection closes
        info!("MCP server running on stdio");
        
        Ok(())
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

// Implement Service for McpServerV2
impl Service<RoleServer> for McpServerV2 {
    async fn handle_request(
        &self,
        request: ClientRequest,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<ServerResult, rmcp::Error> {
        match request {
            ClientRequest::InitializeRequest(req) => {
                // Set peer info
                if context.peer.peer_info().is_none() {
                    context.peer.set_peer_info(req.params.clone());
                }
                
                Ok(ServerResult::InitializeResult(InitializeResult {
                    protocol_version: "2024-11-05".to_string(),
                    server_info: Implementation {
                        name: "bevy-debugger-mcp".to_string(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                    },
                    capabilities: ServerCapabilities {
                        experimental: None,
                        logging: None,
                        prompts: None,
                        resources: None,
                        tools: Some(ToolsCapability {
                            list_changed: None,
                        }),
                        completions: None,
                    },
                    instructions: None,
                }))
            }
            ClientRequest::ListToolsRequest(_req) => {
                // For now, return empty tool list
                // The actual tools are handled by the tool_router macro
                Ok(ServerResult::ListToolsResult(ListToolsResult { 
                    tools: vec![],
                    next_cursor: None,
                }))
            }
            ClientRequest::CallToolRequest(req) => {
                // For now, return an error
                // The actual tool calls should be handled by the tool_router
                Err(rmcp::Error::MethodNotFound {
                    method: req.params.name,
                })
            }
            _ => Err(rmcp::Error::MethodNotFound {
                method: "unknown".to_string(),
            })
        }
    }
    
    async fn handle_notification(
        &self,
        _notification: ClientNotification,
        _context: NotificationContext<RoleServer>,
    ) -> std::result::Result<(), rmcp::Error> {
        // Handle notifications if needed
        Ok(())
    }
    
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: "2024-11-05".to_string(),
            name: "bevy-debugger-mcp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string()
        }
    }
}