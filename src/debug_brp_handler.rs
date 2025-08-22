/*
 * Bevy Debugger MCP Server - Debug BRP Command Handler
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

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::brp_command_handler::{BrpCommandHandler, CommandHandlerMetadata, CommandVersion};
use crate::brp_messages::{BrpRequest, BrpResponse, DebugCommand, DebugResponse};
use crate::debug_command_processor::{DebugCommandRouter, DebugCommandRequest};
use crate::error::Result;

/// Handler for debug commands that routes through the debug command processor
pub struct DebugBrpHandler {
    debug_router: Arc<DebugCommandRouter>,
}

impl DebugBrpHandler {
    pub fn new(debug_router: Arc<DebugCommandRouter>) -> Self {
        Self { debug_router }
    }
}

#[async_trait]
impl BrpCommandHandler for DebugBrpHandler {
    fn metadata(&self) -> CommandHandlerMetadata {
        CommandHandlerMetadata {
            name: "debug".to_string(),
            version: CommandVersion::new(1, 0, 0),
            description: "Handler for debug commands through the debug command processor".to_string(),
            supported_commands: vec![
                "InspectEntity".to_string(),
                "GetHierarchy".to_string(),
                "GetSystemInfo".to_string(),
                "ProfileSystem".to_string(),
                "SetVisualDebug".to_string(),
                "ValidateQuery".to_string(),
                "ProfileMemory".to_string(),
                "CreateSession".to_string(),
                "StartIssueDetection".to_string(),
                "SetPerformanceBudget".to_string(),
            ],
        }
    }

    fn can_handle(&self, request: &BrpRequest) -> bool {
        matches!(request, BrpRequest::Debug { .. })
    }

    async fn handle(&self, request: BrpRequest) -> Result<BrpResponse> {
        if let BrpRequest::Debug { command, correlation_id, priority } = request {
            debug!("Processing debug command: {:?}", command);
            
            // Create a debug command request
            let command_request = DebugCommandRequest::new(
                command.clone(),
                correlation_id.clone(),
                priority,
            );
            
            // Route through the debug command processor
            match self.debug_router.route(command_request).await {
                Ok(response) => {
                    info!("Debug command processed successfully");
                    
                    // Convert DebugResponse to BrpResponse
                    Ok(BrpResponse::Success(Box::new(crate::brp_messages::BrpResult::Debug(Box::new(response)))))
                }
                Err(e) => {
                    error!("Failed to process debug command: {}", e);
                    Ok(BrpResponse::Error(crate::brp_messages::BrpError {
                        code: crate::brp_messages::BrpErrorCode::DebugValidationError,
                        message: e.to_string(),
                        details: None,
                    }))
                }
            }
        } else {
            Err(crate::error::Error::Validation(
                "DebugBrpHandler received non-debug request".to_string(),
            ))
        }
    }

    async fn validate(&self, request: &BrpRequest) -> Result<()> {
        if let BrpRequest::Debug { command, .. } = request {
            // Validate through the debug router
            self.debug_router.validate_command(command).await
        } else {
            Err(crate::error::Error::Validation(
                "Invalid request type for debug handler".to_string(),
            ))
        }
    }

    fn priority(&self) -> i32 {
        50 // Medium-high priority for debug commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debug_command_processor::DebugCommandProcessor;

    struct MockDebugProcessor;

    #[async_trait]
    impl DebugCommandProcessor for MockDebugProcessor {
        async fn process(&self, _command: DebugCommand) -> Result<DebugResponse> {
            Ok(DebugResponse::Success { 
                message: "test success".to_string(), 
                data: Some(json!({ "test": "success" })) 
            })
        }

        async fn validate(&self, _command: &DebugCommand) -> Result<()> {
            Ok(())
        }

        fn estimate_processing_time(&self, _command: &DebugCommand) -> std::time::Duration {
            std::time::Duration::from_millis(100)
        }

        fn supports_command(&self, _command: &DebugCommand) -> bool {
            true
        }
    }

    #[tokio::test]
    async fn test_debug_handler() {
        let router = Arc::new(DebugCommandRouter::new());
        router
            .register_processor("mock".to_string(), Arc::new(MockDebugProcessor))
            .await;

        let handler = DebugBrpHandler::new(router);

        let request = BrpRequest::Debug { 
            command: DebugCommand::InspectEntity { 
                entity_id: 123, 
                include_metadata: None, 
                include_relationships: None 
            },
            correlation_id: "test-123".to_string(),
            priority: Some(5),
        };

        assert!(handler.can_handle(&request));

        let result = handler.handle(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_debug_handler_priority() {
        let router = Arc::new(DebugCommandRouter::new());
        let handler = DebugBrpHandler::new(router);

        assert_eq!(handler.priority(), 50);
    }

    #[tokio::test]
    async fn test_debug_handler_metadata() {
        let router = Arc::new(DebugCommandRouter::new());
        let handler = DebugBrpHandler::new(router);

        let metadata = handler.metadata();
        assert_eq!(metadata.name, "debug");
        assert_eq!(metadata.version.major, 1);
        assert!(metadata.supported_commands.contains(&"InspectEntity".to_string()));
    }
}