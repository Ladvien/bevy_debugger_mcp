/*
 * Bevy Debugger MCP Server - Proper SDK Implementation
 * Copyright (C) 2025 ladvien
 *
 * Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
 *
 * Relicensed from GPL-3.0 when this crate was adopted into Ladvien/foundation_vs_slop: a GPL
 * crate in the Bevy ecosystem cannot be adopted, and being adoptable is why it is published.
 */

use rmcp::serve_server;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

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
    pub fn new(config: Config, brp_client: Arc<RwLock<BrpClient>>) -> Result<Self> {
        let tools = Arc::new(BevyDebuggerTools::new(brp_client.clone()));

        Ok(Self {
            config,
            brp_client,
            tools,
        })
    }
    
    /// Run the server in stdio mode for Claude Code
    pub async fn run_stdio(self) -> Result<()> {
        info!("Starting MCP server in stdio mode for Claude Code integration");
        
        // Register the core BRP handler. A missing game is not fatal: the heartbeat below reports it.
        {
            let client = self.brp_client.read().await;
            if let Err(e) = client.init().await {
                warn!("bevy-debugger: BRP client init failed: {e}; tools will retry");
            }
        }
        
        // Keep the connection state current in the background. Each probe is one HTTP request made under
        // the read lock, so tool calls never wait behind a retry sleep. Log only when the state changes.
        let brp_client = self.brp_client.clone();
        tokio::spawn(async move {
            let mut reachable: Option<bool> = None;
            loop {
                let (url, probe) = {
                    let client = brp_client.read().await;
                    (client.url().to_string(), client.probe().await)
                };
                match probe {
                    Ok(()) if reachable != Some(true) => {
                        info!("bevy-debugger: game answering at {url}");
                        reachable = Some(true);
                    }
                    Ok(()) => {}
                    Err(e) if reachable != Some(false) => {
                        warn!("bevy-debugger: no game answering at {url} yet; tools will retry ({e})");
                        reachable = Some(false);
                    }
                    Err(_) => {}
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
                
                let mut sigterm = match signal(SignalKind::terminate()) {
                    Ok(sig) => sig,
                    Err(e) => {
                        error!("Failed to setup SIGTERM handler: {e}");
                        // Park, do not return: returning drops shutdown_tx and the main loop would exit at once.
                        std::future::pending::<()>().await;
                        return;
                    }
                };
                let mut sigint = match signal(SignalKind::interrupt()) {
                    Ok(sig) => sig,
                    Err(e) => {
                        error!("Failed to setup SIGINT handler: {e}");
                        std::future::pending::<()>().await;
                        return;
                    }
                };
                
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
                if let Err(e) = tokio::signal::ctrl_c().await {
                    error!("Failed to setup Ctrl-C handler: {e}");
                    std::future::pending::<()>().await;
                }
                info!("Received Ctrl-C, shutting down gracefully");
            }
            
            let _ = shutdown_tx.send(()).await;
        });
        
        info!("MCP stdio transport starting - ready for Claude Code connection");
        
        // Create stdio transport
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        
        // Serve the tool router over stdio. Stdio ends when the client disconnects.
        tokio::select! {
            result = serve_server((*self.tools).clone(), (stdin, stdout)) => {
                match result {
                    // **`serve_server` resolves once the service is RUNNING, not once it is done.** It
                    // hands back a `RunningService` handle owning the task that reads the transport.
                    // Returning here dropped that handle, which cancelled the task the moment the
                    // client finished initializing — so `initialize` was answered and the very next
                    // request was not. Claude Code showed that as
                    // `Connected · tools fetch failed — MCP error -32000: Connection closed`,
                    // which reads like a transport fault and is really a dropped handle.
                    //
                    // `waiting()` awaits the service's own quit reason, which is what "the server ran"
                    // actually means.
                    Ok(service) => match service.waiting().await {
                        Ok(reason) => {
                            info!("MCP stdio server finished: {reason:?}");
                            Ok(())
                        }
                        Err(e) => {
                            error!("MCP stdio server task failed to join: {}", e);
                            Err(crate::error::Error::DebugError(format!(
                                "MCP stdio server task failed to join: {e}"
                            )))
                        }
                    },
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