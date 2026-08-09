/*
 * Bevy Debugger MCP Server - BRP Integration Helper
 * Copyright (C) 2025 ladvien
 *
 * Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
 *
 * Relicensed from GPL-3.0 when this crate was adopted into Ladvien/foundation_vs_slop: a GPL
 * crate in the Bevy ecosystem cannot be adopted, and being adoptable is why it is published.
 */

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::brp_client::BrpClient;
use crate::debug_brp_handler::DebugBrpHandler;
use crate::debug_command_processor::DebugCommandRouter;

/// Setup BRP client with debug command routing
pub async fn setup_brp_client_with_debug(
    brp_client: Arc<RwLock<BrpClient>>,
    debug_router: Arc<DebugCommandRouter>,
) {
    info!("Setting up BRP client with debug command routing");
    
    // Create debug handler
    let debug_handler = Arc::new(DebugBrpHandler::new(debug_router.clone()));
    
    // Register with BRP client
    {
        let client = brp_client.read().await;
        client.register_handler(debug_handler).await;
    }
    
    // Debug router is already accessible via the debug handler
    // No need to modify the client directly
    
    info!("BRP client configured with extensible command handlers");
}

/// Migrate existing command processing to use new handler system
pub async fn migrate_existing_commands(brp_client: Arc<RwLock<BrpClient>>) {
    info!("Migrating existing commands to new handler system");
    
    // The CoreBrpHandler is already registered by default in BrpClient::new()
    // Additional migration can be added here if needed
    
    info!("Command migration complete");
}