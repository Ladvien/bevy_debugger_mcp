/*
 * Bevy Debugger MCP Server - BRP Refactor Integration Tests
 * Tests for Story BEVDBG-014
 */

use bevy_debugger_mcp::brp_client::BrpClient;
use bevy_debugger_mcp::brp_command_handler::{
    BrpCommandHandler, CommandHandlerMetadata, CommandHandlerRegistry, CommandVersion,
};
use bevy_debugger_mcp::brp_messages::{BrpRequest, BrpResponse, DebugCommand};
use bevy_debugger_mcp::config::Config;
use bevy_debugger_mcp::debug_brp_handler::DebugBrpHandler;
use bevy_debugger_mcp::debug_command_processor::{DebugCommandProcessor, DebugCommandRouter};
use bevy_debugger_mcp::error::Result;

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;

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
        matches!(request, BrpRequest::Query { .. })
    }

    async fn handle(&self, _request: BrpRequest) -> Result<BrpResponse> {
        Ok(BrpResponse::Success(json!({ "mock": true })))
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
    let request = BrpRequest::Query { 
        filter: None, 
        limit: None, 
        strict: Some(false) 
    };
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
    // Create debug router
    let debug_router = Arc::new(DebugCommandRouter::new());
    
    // Create debug handler
    let debug_handler = DebugBrpHandler::new(debug_router);
    
    // Test metadata
    let metadata = debug_handler.metadata();
    assert_eq!(metadata.name, "debug");
    assert!(metadata.supported_commands.contains(&"InspectEntity".to_string()));
    
    // Test can_handle
    let debug_request = BrpRequest::Debug(DebugCommand::InspectEntity { entity_id: 123 });
    assert!(debug_handler.can_handle(&debug_request));
    
    let non_debug_request = BrpRequest::ListEntities { filter: None };
    assert!(!debug_handler.can_handle(&non_debug_request));
}

#[tokio::test]
async fn test_backward_compatibility() {
    let config = Config::default();
    let client = BrpClient::new(&config);
    
    // Verify core handler is registered by default
    let registry = client.command_registry();
    
    // Core requests should be handleable
    let core_requests = vec![
        BrpRequest::ListEntities { filter: None },
        BrpRequest::ListComponents,
        BrpRequest::Query(json!({})),
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
    let request = BrpRequest::Query(json!({}));
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
            matches!(request, BrpRequest::Get { .. })
        }
        
        async fn handle(&self, _request: BrpRequest) -> Result<BrpResponse> {
            Ok(BrpResponse::Success(Value::Null))
        }
        
        async fn validate(&self, request: &BrpRequest) -> Result<()> {
            if let BrpRequest::Get { entity, .. } = request {
                if *entity == 0 {
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
    let valid_request = BrpRequest::Get { entity: 123, components: None };
    assert!(registry.process(valid_request).await.is_ok());
    
    // Invalid request should fail validation
    let invalid_request = BrpRequest::Get { entity: 0, components: None };
    assert!(registry.process(invalid_request).await.is_err());
}

#[tokio::test]
async fn test_no_handler_error() {
    let registry = CommandHandlerRegistry::new();
    
    // Request with no registered handler
    let request = BrpRequest::Screenshot(json!({}));
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
    
    // List of all command types that should be supported
    let test_requests = vec![
        BrpRequest::Query(json!({})),
        BrpRequest::Get(123),
        BrpRequest::Set(json!({})),
        BrpRequest::ListEntities { filter: None },
        BrpRequest::ListComponents,
        BrpRequest::SpawnEntity(json!({})),
        BrpRequest::DestroyEntity(123),
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