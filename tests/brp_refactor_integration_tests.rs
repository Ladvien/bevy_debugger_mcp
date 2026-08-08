/*
 * Bevy Debugger MCP Server - BRP Refactor Integration Tests
 * Tests for Story BEVDBG-014
 *
 * Rewritten for the real `bevy_remote` crate wire types: `BrpRequest` is a
 * JSON-RPC 2.0 envelope (`{ method, id, params }`), not a hand-rolled enum,
 * and `BrpResponse` is `{ id, payload: BrpPayload }`.
 */

use bevy_debugger_mcp::brp_client::BrpClient;
use bevy_debugger_mcp::brp_command_handler::{
    BrpCommandHandler, CommandHandlerMetadata, CommandHandlerRegistry, CommandVersion,
};
use bevy_debugger_mcp::brp_messages::{
    builtin_methods, BrpError, BrpPayload, BrpRequest, BrpResponse,
};
use bevy_debugger_mcp::config::Config;
use bevy_debugger_mcp::debug_brp_handler::DebugBrpHandler;
use bevy_debugger_mcp::debug_command_processor::DebugCommandRouter;
use bevy_debugger_mcp::error::Result;

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Helper: build a `BrpRequest` envelope for a method.
fn request(method: &str, params: Option<Value>) -> BrpRequest {
    BrpRequest {
        method: method.to_string(),
        id: None,
        params,
    }
}

/// Helper: build a successful `BrpResponse` with no id.
fn success_response(value: Value) -> BrpResponse {
    BrpResponse {
        id: None,
        payload: BrpPayload::Result(value),
    }
}

/// Mock command handler for testing
struct MockCommandHandler {
    name: String,
    priority: i32,
}

#[async_trait]
impl BrpCommandHandler for MockCommandHandler {
    fn metadata(&self) -> CommandHandlerMetadata {
        CommandHandlerMetadata {
            name: self.name.clone(),
            version: CommandVersion::new(1, 0, 0),
            description: "Mock handler for testing".to_string(),
            supported_commands: vec!["mock_command".to_string()],
        }
    }

    fn can_handle(&self, request: &BrpRequest) -> bool {
        request.method == builtin_methods::BRP_QUERY_METHOD
    }

    async fn handle(&self, _request: BrpRequest) -> Result<BrpResponse> {
        Ok(success_response(json!({ "mock": true })))
    }

    fn priority(&self) -> i32 {
        self.priority
    }
}

#[tokio::test]
async fn test_command_handler_registry() {
    let registry = CommandHandlerRegistry::new();

    // Register multiple handlers
    let handler1 = Arc::new(MockCommandHandler {
        name: "handler1".to_string(),
        priority: 10,
    });
    let handler2 = Arc::new(MockCommandHandler {
        name: "handler2".to_string(),
        priority: 20,
    });

    registry.register(handler1).await;
    registry.register(handler2).await;

    // Test finding handler
    let request = request(builtin_methods::BRP_QUERY_METHOD, None);
    let handler = registry.find_handler(&request).await;

    assert!(handler.is_some());
    assert_eq!(handler.unwrap().priority(), 20); // Higher priority handler selected
}

#[tokio::test]
async fn test_version_compatibility() {
    let registry = CommandHandlerRegistry::new();

    let handler = Arc::new(MockCommandHandler {
        name: "versioned".to_string(),
        priority: 0,
    });

    registry.register(handler).await;

    // Check version compatibility
    let compatible_version = CommandVersion::new(1, 1, 0);
    let incompatible_version = CommandVersion::new(2, 0, 0);

    assert!(registry.is_version_supported("versioned", &compatible_version).await);
    assert!(!registry.is_version_supported("versioned", &incompatible_version).await);
}

#[tokio::test]
async fn test_brp_client_with_handlers() {
    let config = Config::default();
    let client = Arc::new(RwLock::new(BrpClient::new(&config)));

    // Register custom handler
    let custom_handler = Arc::new(MockCommandHandler {
        name: "custom".to_string(),
        priority: 100,
    });

    {
        let client = client.read().await;
        client.register_handler(custom_handler).await;
    }

    // Verify registry is accessible
    let registry = {
        let client = client.read().await;
        client.command_registry()
    };

    let versions = registry.get_versions().await;
    assert!(versions.contains_key("custom"));
}

#[tokio::test]
async fn test_debug_handler_integration() {
    use bevy_debugger_mcp::brp_messages::DebugCommand;
    use bevy_debugger_mcp::debug_brp_handler::encode_debug_request;

    // Create debug router
    let debug_router = Arc::new(DebugCommandRouter::new());

    // Create debug handler
    let debug_handler = DebugBrpHandler::new(debug_router);

    // Test metadata
    let metadata = debug_handler.metadata();
    assert_eq!(metadata.name, "debug");
    assert!(metadata.supported_commands.contains(&"InspectEntity".to_string()));

    // Test can_handle on a real debug-method request envelope
    let debug_request = encode_debug_request(
        &DebugCommand::InspectEntity {
            entity_id: 123,
            include_metadata: None,
            include_relationships: None,
        },
        "test-123",
        None,
    )
    .unwrap();
    assert!(debug_handler.can_handle(&debug_request));

    // A non-debug request should not be handled.
    let non_debug_request = request(builtin_methods::BRP_QUERY_METHOD, None);
    assert!(!debug_handler.can_handle(&non_debug_request));
}

#[tokio::test]
async fn test_backward_compatibility() {
    let config = Config::default();
    let client = BrpClient::new(&config);

    // Verify core handler is registered by default
    let registry = client.command_registry();

    // Core BRP methods should be handleable.
    let core_requests = vec![
        request(builtin_methods::BRP_QUERY_METHOD, None),
        request(builtin_methods::BRP_LIST_COMPONENTS_METHOD, None),
        request(builtin_methods::BRP_GET_COMPONENTS_METHOD, Some(json!({ "entity": 1 }))),
    ];

    for request in core_requests {
        let handler = registry.find_handler(&request).await;
        assert!(handler.is_some(), "Core request {:?} should have a handler", request);
    }
}

#[tokio::test]
async fn test_handler_priority_ordering() {
    let registry = CommandHandlerRegistry::new();

    // Register handlers with different priorities
    for priority in vec![5, 15, 10, 20, 1] {
        let handler = Arc::new(MockCommandHandler {
            name: format!("handler_{}", priority),
            priority,
        });
        registry.register(handler).await;
    }

    // The handler with highest priority (20) should be selected
    let request = request(builtin_methods::BRP_QUERY_METHOD, None);
    let handler = registry.find_handler(&request).await.unwrap();

    assert_eq!(handler.priority(), 20);
}

#[tokio::test]
async fn test_handler_validation() {
    struct ValidatingHandler;

    #[async_trait]
    impl BrpCommandHandler for ValidatingHandler {
        fn metadata(&self) -> CommandHandlerMetadata {
            CommandHandlerMetadata {
                name: "validator".to_string(),
                version: CommandVersion::default(),
                description: "Validating handler".to_string(),
                supported_commands: vec![],
            }
        }

        fn can_handle(&self, request: &BrpRequest) -> bool {
            request.method == builtin_methods::BRP_GET_COMPONENTS_METHOD
        }

        async fn handle(&self, _request: BrpRequest) -> Result<BrpResponse> {
            Ok(success_response(Value::Null))
        }

        async fn validate(&self, request: &BrpRequest) -> Result<()> {
            if request.method == builtin_methods::BRP_GET_COMPONENTS_METHOD {
                let entity = request
                    .params
                    .as_ref()
                    .and_then(|p| p.get("entity"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                if entity == 0 {
                    return Err(bevy_debugger_mcp::error::Error::Validation(
                        "Invalid entity ID: 0".to_string(),
                    ));
                }
            }
            Ok(())
        }
    }

    let registry = CommandHandlerRegistry::new();
    registry.register(Arc::new(ValidatingHandler)).await;

    // Valid request should pass
    let valid_request = request(
        builtin_methods::BRP_GET_COMPONENTS_METHOD,
        Some(json!({ "entity": 123 })),
    );
    assert!(registry.process(valid_request).await.is_ok());

    // Invalid request should fail validation
    let invalid_request = request(
        builtin_methods::BRP_GET_COMPONENTS_METHOD,
        Some(json!({ "entity": 0 })),
    );
    assert!(registry.process(invalid_request).await.is_err());
}

#[tokio::test]
async fn test_no_handler_error() {
    let registry = CommandHandlerRegistry::new();

    // Request with no registered handler — use a real but uncommon method
    // so nothing in the registry claims it.
    let request = request(builtin_methods::BRP_LIST_RESOURCES_METHOD, None);
    let result = registry.process(request).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No handler found"));
}

/// Test that all existing command types have handlers
#[tokio::test]
async fn test_all_commands_have_handlers() {
    let config = Config::default();
    let client = BrpClient::new(&config);
    let registry = client.command_registry();

    // List of all core BRP methods that should be supported by the core handler.
    let test_requests = vec![
        request(builtin_methods::BRP_QUERY_METHOD, None),
        request(builtin_methods::BRP_GET_COMPONENTS_METHOD, Some(json!({ "entity": 123 }))),
        request(builtin_methods::BRP_LIST_COMPONENTS_METHOD, None),
        request(builtin_methods::BRP_SPAWN_ENTITY_METHOD, Some(json!({}))),
        request(builtin_methods::BRP_DESPAWN_COMPONENTS_METHOD, Some(json!({ "entity": 123 }))),
    ];

    for request in test_requests {
        let handler = registry.find_handler(&request).await;
        assert!(
            handler.is_some(),
            "No handler found for request type: {:?}",
            request
        );
    }
}

/// Verify an error `BrpResponse` can be constructed from the new struct API.
#[tokio::test]
async fn test_error_response_construction() {
    let response = BrpResponse {
        id: None,
        payload: BrpPayload::Error(BrpError {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        }),
    };
    let json = serde_json::to_string(&response).unwrap();
    let decoded: BrpResponse = serde_json::from_str(&json).unwrap();
    assert!(matches!(decoded.payload, BrpPayload::Error(_)));
}