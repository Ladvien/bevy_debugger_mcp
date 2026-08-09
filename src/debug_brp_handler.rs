/*
 * Bevy Debugger MCP Server - Debug BRP Command Handler
 * Copyright (C) 2025 ladvien
 *
 * Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
 *
 * Relicensed from GPL-3.0 when this crate was adopted into Ladvien/foundation_vs_slop: a GPL
 * crate in the Bevy ecosystem cannot be adopted, and being adoptable is why it is published.
 */

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::brp::{BrpError, BrpRequest, BrpResponse, DebugCommand};
use crate::brp_command_handler::{BrpCommandHandler, CommandHandlerMetadata, CommandVersion};
use crate::debug_command_processor::{DebugCommandRequest, DebugCommandRouter};
use crate::error::Result;

/// Custom BRP method name that carries debugger `DebugCommand`s.
///
/// This is an extension point: a Bevy game may register a
/// `bevy_debugger/debug` method via `RemotePlugin::with_method`; the debugger
/// routes the command through its local debug command processors regardless.
pub const BRP_DEBUG_METHOD: &str = "bevy_debugger/debug";

/// Decode a `BrpRequest` into `(command, correlation_id, priority)` if it's a
/// debug method call. Returns `None` for any other method.
pub fn decode_debug_request(
    request: &BrpRequest,
) -> Option<(DebugCommand, String, Option<u8>)> {
    if request.method != BRP_DEBUG_METHOD {
        return None;
    }
    let params = request.params.as_ref()?;
    let command: DebugCommand =
        serde_json::from_value(params.get("command")?.clone()).ok()?;
    let correlation_id = params
        .get("correlation_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let priority = params
        .get("priority")
        .and_then(Value::as_u64)
        .map(|p| p as u8);
    Some((command, correlation_id, priority))
}

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
        request.method == BRP_DEBUG_METHOD
    }

    async fn handle(&self, request: BrpRequest) -> Result<BrpResponse> {
        let Some((command, correlation_id, priority)) = decode_debug_request(&request) else {
            return Err(crate::error::Error::Validation(
                "DebugBrpHandler received malformed debug params".to_string(),
            ));
        };

        debug!("Processing debug command: {:?}", command);

        // Create a debug command request
        let command_request = DebugCommandRequest::new(command.clone(), correlation_id, priority);

        // Route through the debug command processor
        match self.debug_router.route(command_request).await {
            Ok(response) => {
                info!("Debug command processed successfully");
                match serde_json::to_value(response) {
                    Ok(value) => Ok(BrpResponse::new(request.id.clone(), Ok(value))),
                    Err(e) => Err(crate::error::Error::Json(e)),
                }
            }
            Err(e) => {
                error!("Failed to process debug command: {}", e);
                Ok(BrpResponse::new(
                    request.id.clone(),
                    Err(BrpError {
                        code: bevy_remote::error_codes::INVALID_PARAMS,
                        message: e.to_string(),
                        data: None,
                    }),
                ))
            }
        }
    }

    async fn validate(&self, request: &BrpRequest) -> Result<()> {
        if let Some((command, _, _)) = decode_debug_request(request) {
            // Validate through the debug router
            self.debug_router.validate_command(&command).await
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

/// Build the `BrpRequest` envelope that carries a `DebugCommand` over BRP.
pub fn encode_debug_request(
    command: &DebugCommand,
    correlation_id: &str,
    priority: Option<u8>,
) -> Result<BrpRequest> {
    Ok(BrpRequest {
        method: BRP_DEBUG_METHOD.to_string(),
        id: Some(json!(correlation_id)),
        params: Some(json!({
            "command": serde_json::to_value(command).map_err(crate::error::Error::Json)?,
            "correlation_id": correlation_id,
            "priority": priority,
        })),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brp::DebugResponse;
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

        let request = encode_debug_request(
            &DebugCommand::InspectEntity {
                entity_id: 123,
                include_metadata: None,
                include_relationships: None,
            },
            "test-123",
            Some(5),
        )
        .unwrap();

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

    #[tokio::test]
    async fn test_debug_round_trip() {
        let command = DebugCommand::GetStatus;
        let request = encode_debug_request(&command, "corr-1", None).unwrap();
        let (decoded, corr, prio) = decode_debug_request(&request).unwrap();
        assert!(matches!(decoded, DebugCommand::GetStatus));
        assert_eq!(corr, "corr-1");
        assert_eq!(prio, None);
    }
}
