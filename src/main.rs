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
use bevy_debugger_mcp::{mcp_server, mcp_server_v2};

#[cfg(feature = "observability")]
use bevy_debugger_mcp::observability::ObservabilityService;

#[tokio::main]
async fn main() -> Result<()> {
    // Set up global panic handler that logs before exit
    std::panic::set_hook(Box::new(|panic_info| {
        let payload = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload".to_string()
        };
        
        let location = if let Some(location) = panic_info.location() {
            format!("{}:{}:{}", location.file(), location.line(), location.column())
        } else {
            "Unknown location".to_string()
        };
        
        eprintln!("FATAL PANIC in bevy-debugger-mcp: {} at {}", payload, location);
        eprintln!("This indicates a critical bug that should be reported.");
        eprintln!("Stack trace should appear above this message.");
        
        // Ensure logs are flushed
        std::io::Write::flush(&mut std::io::stderr()).unwrap_or(());
    }));
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
    
    // Determine if we're in stdio mode (for MCP protocol)
    let is_stdio_mode = args.iter().any(|arg| arg == "--stdio") || 
                        (!args.iter().any(|arg| arg == "--tcp" || arg == "--server") && !std::io::stdout().is_terminal());
    
    // Initialize tracing to stderr when in stdio mode (stdout is reserved for MCP protocol)
    // This prevents log output from contaminating the JSON-RPC stream
    if is_stdio_mode {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_writer(std::io::stderr)
            .with_ansi(false)  // Disable ANSI color codes in stdio mode
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();
    }

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
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    {
        let client = brp_client.read().await;
        client.init().await?;
    }
    
    // Initialize observability if enabled
    #[cfg(feature = "observability")]
    let _observability = if config.observability.metrics_enabled || config.observability.tracing_enabled {
        let obs_service = ObservabilityService::new(config.clone(), brp_client.clone()).await?;
        obs_service.start().await?;
        Some(obs_service)
    } else {
        None
    };
    
    let server = mcp_server_v2::McpServerV2::new(config, brp_client)?;
    server.run_stdio().await
}

async fn run_tcp_mode(config: Config) -> Result<()> {
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    {
        let client = brp_client.read().await;
        client.init().await?;
    }
    
    // Initialize observability if enabled
    #[cfg(feature = "observability")]
    let observability = if config.observability.metrics_enabled || config.observability.tracing_enabled {
        let obs_service = ObservabilityService::new(config.clone(), brp_client.clone()).await?;
        obs_service.start().await?;
        
        // Start health endpoints if enabled
        if config.observability.health_check_enabled {
            let health_service = obs_service.health();
            let health_router = health_service.create_router();
            
            tokio::spawn(async move {
                let app = health_router.with_state(health_service);
                let health_listener = tokio::net::TcpListener::bind(
                    format!("127.0.0.1:{}", config.observability.health_check_port)
                ).await.expect("Failed to bind health check port");
                
                info!("Health endpoints available at http://127.0.0.1:{}", config.observability.health_check_port);
                axum::serve(health_listener, app).await.expect("Health server failed");
            });
        }
        
        Some(obs_service)
    } else {
        None
    };
    
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
            
            #[cfg(feature = "observability")]
            if let Some(obs) = observability {
                if let Err(e) = obs.shutdown().await {
                    warn!("Error shutting down observability: {}", e);
                }
            }
        }
    }

    Ok(())
}
