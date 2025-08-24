/*
 * Bevy Debugger MCP Server - BRP Validation Tests
 * Copyright (C) 2025 ladvien
 *
 * Comprehensive test suite for BRP command validation functionality
 */

use bevy_debugger_mcp::brp_messages::{BrpRequest, QueryFilter};
use bevy_debugger_mcp::brp_validation::{
    BrpValidator, ValidationConfig, PermissionLevel, ComponentRegistry, ComponentTypeMetadata,
    EntityTracker,
};
use bevy_debugger_mcp::error::Error;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_basic_request_validation() {
    let validator = BrpValidator::new();
    let request = BrpRequest::ListComponents;
    let session_id = "test_session";
    let request_size = 100;

    let result = validator.validate_request(&request, session_id, request_size).await;
    assert!(result.is_ok(), "Basic ListComponents request should pass validation");
}

#[tokio::test]
async fn test_request_size_limits() {
    let validator = BrpValidator::new();
    let request = BrpRequest::ListComponents;
    let session_id = "test_session";

    // Should pass with normal size
    let result = validator.validate_request(&request, session_id, 1000).await;
    assert!(result.is_ok());

    // Should fail with oversized request
    let result = validator.validate_request(&request, session_id, usize::MAX).await;
    assert!(result.is_err());
    
    let error_message = result.unwrap_err().to_string();
    assert!(error_message.contains("exceeds maximum allowed size"));
    assert!(error_message.contains("Consider splitting large requests"));
}

#[tokio::test]
async fn test_rate_limiting() {
    let mut config = ValidationConfig::default();
    config.rate_limit = 3; // Very low limit for testing
    let validator = BrpValidator::with_config(config);
    let request = BrpRequest::ListComponents;
    let session_id = "rate_test_session";
    let request_size = 100;

    // First three requests should pass
    for i in 1..=3 {
        let result = validator.validate_request(&request, session_id, request_size).await;
        assert!(result.is_ok(), "Request {} should pass", i);
    }

    // Fourth request should fail due to rate limiting
    let result = validator.validate_request(&request, session_id, request_size).await;
    assert!(result.is_err());
    
    let error_message = result.unwrap_err().to_string();
    assert!(error_message.contains("Rate limit exceeded"));
    assert!(error_message.contains("Try again in"));
    assert!(error_message.contains("Consider reducing request frequency"));
}

#[tokio::test]
async fn test_rate_limiting_window_reset() {
    let mut config = ValidationConfig::default();
    config.rate_limit = 2;
    let validator = BrpValidator::with_config(config);
    let request = BrpRequest::ListComponents;
    let session_id = "window_test_session";
    let request_size = 100;

    // Use up the rate limit
    assert!(validator.validate_request(&request, session_id, request_size).await.is_ok());
    assert!(validator.validate_request(&request, session_id, request_size).await.is_ok());
    
    // Third request should fail
    assert!(validator.validate_request(&request, session_id, request_size).await.is_err());

    // Wait for window reset (rate limit window is 1 second)
    sleep(Duration::from_millis(1100)).await;

    // Should be able to make requests again
    let result = validator.validate_request(&request, session_id, request_size).await;
    assert!(result.is_ok(), "Request should pass after window reset");
}

#[tokio::test]
async fn test_permission_levels() {
    let validator = BrpValidator::new();
    let session_id = "permission_test_session";
    let request_size = 100;

    // Set read-only permissions
    validator.update_session_permissions(session_id, PermissionLevel::Read).await.unwrap();

    // Read operations should pass
    let read_request = BrpRequest::ListComponents;
    assert!(validator.validate_request(&read_request, session_id, request_size).await.is_ok());

    let query_request = BrpRequest::Query {
        filter: {
            let mut filter = QueryFilter::default();
            filter.with = Some(vec!["Transform".to_string()]);
            Some(filter)
        },
        limit: Some(10),
        strict: Some(false),
    };
    assert!(validator.validate_request(&query_request, session_id, request_size).await.is_ok());

    // Write operations should fail
    let destroy_request = BrpRequest::Destroy { entity: 123 };
    let result = validator.validate_request(&destroy_request, session_id, request_size).await;
    assert!(result.is_err());
    
    let error_message = result.unwrap_err().to_string();
    assert!(error_message.contains("Insufficient permissions"));
    assert!(error_message.contains("Contact administrator"));
}

#[tokio::test]
async fn test_permission_upgrade() {
    let validator = BrpValidator::new();
    let session_id = "upgrade_test_session";
    let request_size = 100;

    // Start with read permissions
    validator.update_session_permissions(session_id, PermissionLevel::Read).await.unwrap();
    
    let write_request = BrpRequest::Destroy { entity: 123 };
    assert!(validator.validate_request(&write_request, session_id, request_size).await.is_err());

    // Upgrade to write permissions
    validator.update_session_permissions(session_id, PermissionLevel::Write).await.unwrap();
    
    // Now write operations should pass (though they might fail for other reasons like entity existence)
    // We're just testing permission validation here
    let result = validator.validate_request(&write_request, session_id, request_size).await;
    // The validation might still fail due to entity existence checks, but not due to permissions
    if let Err(e) = result {
        assert!(!e.to_string().contains("Insufficient permissions"));
    }
}

#[tokio::test]
async fn test_entity_existence_validation() {
    let mut config = ValidationConfig::default();
    config.enforce_entity_existence = true;
    let validator = BrpValidator::with_config(config);
    let session_id = "entity_test_session";
    let request_size = 100;

    // Set write permissions to bypass permission checks
    validator.update_session_permissions(session_id, PermissionLevel::Write).await.unwrap();

    // Update entity tracker with some entities
    let entity_tracker = validator.get_entity_tracker();
    let mut tracker = entity_tracker.write().await;
    tracker.update_entities(vec![123, 456, 789]);
    drop(tracker);

    // Request for existing entity should pass entity existence check
    let valid_request = BrpRequest::Get {
        entity: 123,
        components: None,
    };
    let result = validator.validate_request(&valid_request, session_id, request_size).await;
    // Might still fail due to component registry, but not entity existence
    if let Err(e) = result {
        assert!(!e.to_string().contains("does not exist"));
    }

    // Request for non-existing entity should fail
    let invalid_request = BrpRequest::Get {
        entity: 999,
        components: None,
    };
    let result = validator.validate_request(&invalid_request, session_id, request_size).await;
    assert!(result.is_err());
    
    let error_message = result.unwrap_err().to_string();
    assert!(error_message.contains("does not exist"));
    assert!(error_message.contains("Refresh entity list"));
}

#[tokio::test]
async fn test_component_registry_validation() {
    let mut config = ValidationConfig::default();
    config.enforce_component_registry = true;
    let validator = BrpValidator::with_config(config);
    let session_id = "component_test_session";
    let request_size = 100;

    // Set write permissions and disable entity existence checks for this test
    validator.update_session_permissions(session_id, PermissionLevel::Write).await.unwrap();
    
    // Test with registered component type (Transform is registered by default)
    let valid_request = BrpRequest::Set {
        entity: 123,
        components: {
            let mut map = HashMap::new();
            map.insert("Transform".to_string(), serde_json::json!({
                "translation": [1.0, 2.0, 3.0],
                "rotation": [0.0, 0.0, 0.0, 1.0],
                "scale": [1.0, 1.0, 1.0]
            }));
            map
        },
    };

    // This might still fail due to other validations, but not component registry
    let result = validator.validate_request(&valid_request, session_id, request_size).await;
    if let Err(e) = result {
        assert!(!e.to_string().contains("is not registered"));
    }

    // Test with unregistered component type
    let invalid_request = BrpRequest::Set {
        entity: 123,
        components: {
            let mut map = HashMap::new();
            map.insert("NonexistentComponent".to_string(), serde_json::json!({}));
            map
        },
    };

    let result = validator.validate_request(&invalid_request, session_id, request_size).await;
    assert!(result.is_err());
    
    let error_message = result.unwrap_err().to_string();
    assert!(error_message.contains("is not registered"));
    assert!(error_message.contains("Available types can be retrieved"));
}

#[tokio::test]
async fn test_component_value_size_limits() {
    let mut config = ValidationConfig::default();
    config.limits.max_component_value_size = 100; // Very small limit for testing
    let validator = BrpValidator::with_config(config);
    let session_id = "size_test_session";
    let request_size = 1000;

    validator.update_session_permissions(session_id, PermissionLevel::Write).await.unwrap();

    // Create a large component value
    let large_value = serde_json::json!({
        "data": "x".repeat(200) // Larger than 100 bytes limit
    });

    let request = BrpRequest::Set {
        entity: 123,
        components: {
            let mut map = HashMap::new();
            map.insert("Transform".to_string(), large_value);
            map
        },
    };

    let result = validator.validate_request(&request, session_id, request_size).await;
    assert!(result.is_err());
    
    let error_message = result.unwrap_err().to_string();
    assert!(error_message.contains("value size"));
    assert!(error_message.contains("exceeds maximum"));
}

#[tokio::test]
async fn test_query_limits() {
    let mut config = ValidationConfig::default();
    config.max_entities_per_query = 5; // Very low limit for testing
    let validator = BrpValidator::with_config(config);
    let session_id = "query_limit_test_session";
    let request_size = 100;

    // Query within limit should pass
    let valid_query = BrpRequest::Query {
        filter: {
            let mut filter = QueryFilter::default();
            filter.with = Some(vec!["Transform".to_string()]);
            Some(filter)
        },
        limit: Some(3),
        strict: Some(false),
    };

    let result = validator.validate_request(&valid_query, session_id, request_size).await;
    assert!(result.is_ok());

    // Query exceeding limit should fail
    let invalid_query = BrpRequest::Query {
        filter: {
            let mut filter = QueryFilter::default();
            filter.with = Some(vec!["Transform".to_string()]);
            Some(filter)
        },
        limit: Some(10),
        strict: Some(false),
    };

    let result = validator.validate_request(&invalid_query, session_id, request_size).await;
    assert!(result.is_err());
    
    let error_message = result.unwrap_err().to_string();
    assert!(error_message.contains("exceeds maximum"));
    assert!(error_message.contains("Use pagination"));
}

#[tokio::test]
async fn test_component_registry_operations() {
    let registry = ComponentRegistry::new();
    
    // Test default registered types
    assert!(registry.is_registered("Transform"));
    assert!(registry.is_registered("Name"));
    assert!(registry.is_registered("Visibility"));
    assert!(!registry.is_registered("CustomComponent"));

    // Test metadata retrieval
    let transform_metadata = registry.get_metadata("Transform");
    assert!(transform_metadata.is_some());
    
    let metadata = transform_metadata.unwrap();
    assert_eq!(metadata.size_bytes, 48); // 3x Vec3 + Quat
    assert!(metadata.is_mutable);
}

#[tokio::test]
async fn test_entity_tracker_cache() {
    let mut tracker = EntityTracker::new();
    
    // Initially no entities
    assert!(!tracker.entity_exists(123));
    
    // Update with entities
    tracker.update_entities(vec![123, 456, 789]);
    assert!(tracker.entity_exists(123));
    assert!(tracker.entity_exists(456));
    assert!(!tracker.entity_exists(999));
    
    // Test cache staleness
    assert!(!tracker.is_cache_stale()); // Should be fresh initially
}

#[tokio::test]
async fn test_session_management() {
    let validator = BrpValidator::new();
    let session1 = "session1";
    let session2 = "session2";

    // Set different permissions for different sessions
    validator.update_session_permissions(session1, PermissionLevel::Read).await.unwrap();
    validator.update_session_permissions(session2, PermissionLevel::Admin).await.unwrap();

    let admin_request = BrpRequest::Debug {
        command: bevy_debugger_mcp::brp_messages::DebugCommand::GetStatus,
        correlation_id: "test".to_string(),
        priority: None,
    };

    // Session1 (Read) should fail
    let result = validator.validate_request(&admin_request, session1, 100).await;
    assert!(result.is_err());

    // Session2 (Admin) should pass permission check
    let result = validator.validate_request(&admin_request, session2, 100).await;
    if let Err(e) = result {
        assert!(!e.to_string().contains("Insufficient permissions"));
    }
}

#[tokio::test]
async fn test_error_message_quality() {
    let validator = BrpValidator::new();
    let session_id = "error_test_session";
    
    // Test rate limit error message
    let mut config = ValidationConfig::default();
    config.rate_limit = 1;
    let validator = BrpValidator::with_config(config);
    let request = BrpRequest::ListComponents;
    
    // Use up rate limit
    validator.validate_request(&request, session_id, 100).await.unwrap();
    
    // Next request should have helpful error
    let result = validator.validate_request(&request, session_id, 100).await;
    assert!(result.is_err());
    
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Rate limit exceeded"));
    assert!(error_msg.contains("operations per second"));
    assert!(error_msg.contains("Try again in"));
    assert!(error_msg.contains("reducing request frequency"));
    assert!(error_msg.contains("batch operations"));
}

#[tokio::test]
async fn test_configuration_flexibility() {
    let mut config = ValidationConfig::default();
    config.enforce_entity_existence = false;
    config.enforce_component_registry = false;
    config.enforce_permissions = false;
    
    let validator = BrpValidator::with_config(config);
    let session_id = "flexible_config_session";

    // With all enforcement disabled, most validations should pass
    // (except basic ones like size limits)
    let request = BrpRequest::Set {
        entity: 99999, // Non-existent entity
        components: {
            let mut map = HashMap::new();
            map.insert("NonexistentComponent".to_string(), serde_json::json!({}));
            map
        },
    };

    let result = validator.validate_request(&request, session_id, 100).await;
    // Should pass validation with enforcement disabled
    // (though it would fail in actual execution)
    assert!(result.is_ok());
}

/// Integration test that validates the entire validation pipeline
#[tokio::test]
async fn test_comprehensive_validation_pipeline() {
    let validator = BrpValidator::new();
    let session_id = "pipeline_test_session";
    
    // Set up proper permissions
    validator.update_session_permissions(session_id, PermissionLevel::Write).await.unwrap();
    
    // Set up entity tracker
    let entity_tracker = validator.get_entity_tracker();
    let mut tracker = entity_tracker.write().await;
    tracker.update_entities(vec![123, 456]);
    drop(tracker);
    
    // Valid request that should pass all validations
    let valid_request = BrpRequest::Set {
        entity: 123, // Exists in tracker
        components: {
            let mut map = HashMap::new();
            map.insert("Transform".to_string(), serde_json::json!({ // Registered component
                "translation": [1.0, 2.0, 3.0],
                "rotation": [0.0, 0.0, 0.0, 1.0],
                "scale": [1.0, 1.0, 1.0]
            }));
            map
        },
    };
    
    let result = validator.validate_request(&valid_request, session_id, 1000).await;
    assert!(result.is_ok(), "Comprehensive valid request should pass all validations");
    
    // Invalid request that should fail multiple validations
    let invalid_request = BrpRequest::Set {
        entity: 999, // Does not exist
        components: {
            let mut map = HashMap::new();
            map.insert("NonexistentComponent".to_string(), serde_json::json!({}));
            map
        },
    };
    
    let result = validator.validate_request(&invalid_request, session_id, 1000).await;
    assert!(result.is_err(), "Invalid request should fail validation");
}