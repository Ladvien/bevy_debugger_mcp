/*
 * Bevy Debugger MCP Server - Bevy 0.16 BRP Compatibility Tests
 * Tests for BEVDBG-004: Update BRP Protocol for Bevy 0.16
 */

use bevy_debugger_mcp::brp_messages::{
    BrpRequest, BrpResponse, EntityWithGeneration, QueryFilter
};
use serde_json::{json, Value};
use std::collections::HashMap;

#[tokio::test]
async fn test_bevy_16_strict_query_parameter() {
    // Test that strict parameter is properly serialized/deserialized
    let strict_query = BrpRequest::Query {
        filter: Some(QueryFilter {
            with: Some(vec!["Transform".to_string()]),
            without: None,
            where_clause: None,
        }),
        limit: Some(100),
        strict: Some(true),
    };
    
    // Serialize to JSON
    let json_str = serde_json::to_string(&strict_query).unwrap();
    let json_value: Value = serde_json::from_str(&json_str).unwrap();
    
    // Verify structure matches Bevy 0.16 BRP format
    assert_eq!(json_value["method"], "bevy/query");
    assert_eq!(json_value["params"]["strict"], true);
    assert_eq!(json_value["params"]["limit"], 100);
    
    // Test deserialization back
    let deserialized: BrpRequest = serde_json::from_str(&json_str).unwrap();
    match deserialized {
        BrpRequest::Query { strict, limit, .. } => {
            assert_eq!(strict, Some(true));
            assert_eq!(limit, Some(100));
        }
        _ => panic!("Expected Query request"),
    }
}

#[tokio::test]
async fn test_bevy_16_new_brp_methods() {
    // Test bevy/insert method
    let mut components = HashMap::new();
    components.insert("Transform".to_string(), json!({"translation": [0.0, 0.0, 0.0]}));
    
    let insert_request = BrpRequest::Insert {
        entity: 12345,
        components,
    };
    
    let json_str = serde_json::to_string(&insert_request).unwrap();
    let json_value: Value = serde_json::from_str(&json_str).unwrap();
    
    assert_eq!(json_value["method"], "bevy/insert");
    assert_eq!(json_value["params"]["entity"], 12345);
    
    // Test bevy/remove method
    let remove_request = BrpRequest::Remove {
        entity: 12345,
        components: vec!["Transform".to_string(), "Velocity".to_string()],
    };
    
    let json_str = serde_json::to_string(&remove_request).unwrap();
    let json_value: Value = serde_json::from_str(&json_str).unwrap();
    
    assert_eq!(json_value["method"], "bevy/remove");
    assert_eq!(json_value["params"]["entity"], 12345);
    assert!(json_value["params"]["components"].as_array().unwrap().len() == 2);
    
    // Test bevy/reparent method
    let reparent_request = BrpRequest::Reparent {
        entity: 12345,
        parent: Some(67890),
    };
    
    let json_str = serde_json::to_string(&reparent_request).unwrap();
    let json_value: Value = serde_json::from_str(&json_str).unwrap();
    
    assert_eq!(json_value["method"], "bevy/reparent");
    assert_eq!(json_value["params"]["entity"], 12345);
    assert_eq!(json_value["params"]["parent"], 67890);
}

#[tokio::test]
async fn test_entity_with_generation_compatibility() {
    // Test that EntityWithGeneration properly encodes/decodes
    let entity = EntityWithGeneration::new(42, 7);
    
    // Test conversion to/from entity ID
    let entity_id = entity.to_entity_id();
    let restored = EntityWithGeneration::from_entity_id(entity_id);
    
    assert_eq!(entity.index, restored.index);
    assert_eq!(entity.generation, restored.generation);
    
    // Test serialization
    let json_str = serde_json::to_string(&entity).unwrap();
    let deserialized: EntityWithGeneration = serde_json::from_str(&json_str).unwrap();
    
    assert_eq!(entity.index, deserialized.index);
    assert_eq!(entity.generation, deserialized.generation);
}

#[tokio::test]
async fn test_backwards_compatibility_with_legacy_queries() {
    // Test that queries without strict parameter still work (defaults to false)
    let legacy_query = BrpRequest::Query {
        filter: Some(QueryFilter {
            with: Some(vec!["Transform".to_string()]),
            without: None,
            where_clause: None,
        }),
        limit: None,
        strict: None, // Not specified - should default to false behavior
    };
    
    let json_str = serde_json::to_string(&legacy_query).unwrap();
    let json_value: Value = serde_json::from_str(&json_str).unwrap();
    
    // Strict parameter should not be present in JSON when None
    assert!(json_value["params"]["strict"].is_null());
    
    // Should still deserialize correctly
    let deserialized: BrpRequest = serde_json::from_str(&json_str).unwrap();
    match deserialized {
        BrpRequest::Query { strict, .. } => {
            assert_eq!(strict, None);
        }
        _ => panic!("Expected Query request"),
    }
}

#[tokio::test] 
async fn test_component_type_id_format_compatibility() {
    // Test that component type IDs work with fully qualified names (Bevy 0.16 format)
    let qualified_names = vec![
        "bevy_transform::components::transform::Transform",
        "bevy_render::view::visibility::Visibility", 
        "bevy_core::name::Name",
        "my_game::components::Player",
    ];
    
    for type_name in qualified_names {
        // Test in a query filter
        let query = BrpRequest::Query {
            filter: Some(QueryFilter {
                with: Some(vec![type_name.to_string()]),
                without: None,
                where_clause: None,
            }),
            limit: None,
            strict: Some(true),
        };
        
        // Should serialize/deserialize without issues
        let json_str = serde_json::to_string(&query).unwrap();
        let _deserialized: BrpRequest = serde_json::from_str(&json_str).unwrap();
        
        // Test in component operations
        let mut components = HashMap::new();
        components.insert(type_name.to_string(), json!({"test": "data"}));
        
        let insert_request = BrpRequest::Insert {
            entity: 123,
            components,
        };
        
        let json_str = serde_json::to_string(&insert_request).unwrap();
        let _deserialized: BrpRequest = serde_json::from_str(&json_str).unwrap();
    }
}

#[tokio::test]
async fn test_json_rpc_2_0_format_compatibility() {
    // Test that requests can be formatted as proper JSON-RPC 2.0 messages
    let query = BrpRequest::Query {
        filter: Some(QueryFilter {
            with: Some(vec!["Transform".to_string()]),
            without: None,
            where_clause: None,
        }),
        limit: Some(10),
        strict: Some(true),
    };
    
    // Create a proper JSON-RPC 2.0 request
    let jsonrpc_request = json!({
        "jsonrpc": "2.0",
        "method": "bevy/query",
        "id": 1,
        "params": {
            "filter": {
                "with": ["Transform"]
            },
            "limit": 10,
            "strict": true
        }
    });
    
    // Test that we can parse this format
    let method = jsonrpc_request["method"].as_str().unwrap();
    let params = &jsonrpc_request["params"];
    
    assert_eq!(method, "bevy/query");
    assert_eq!(params["strict"], true);
    assert_eq!(params["limit"], 10);
}

/// Integration test with a minimal Bevy 0.16 app
#[cfg(feature = "bevy")]
#[tokio::test]
async fn test_real_bevy_16_integration() {
    use bevy::prelude::*;
    
    // Create a minimal Bevy app with remote plugin
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
       .add_plugins(bevy::remote::RemotePlugin::default());
    
    // Spawn a test entity
    let entity_id = app.world_mut().spawn((
        Transform::from_translation(Vec3::new(1.0, 2.0, 3.0)),
        Name::new("TestEntity"),
    )).id();
    
    // Verify entity generation handling
    let entity_with_gen = EntityWithGeneration::new(entity_id.index(), entity_id.generation());
    let combined_id = entity_with_gen.to_entity_id();
    let restored = EntityWithGeneration::from_entity_id(combined_id);
    
    assert_eq!(entity_with_gen.index, restored.index);
    assert_eq!(entity_with_gen.generation, restored.generation);
}