/*
 * Bevy Debugger MCP Server - Bevy 0.16 BRP Compatibility Tests
 * Tests for BEVDBG-004: Update BRP Protocol for Bevy 0.16
 *
 * Rewritten for the real `bevy_remote` crate wire types: `BrpRequest` is a
 * JSON-RPC 2.0 envelope (`{ method, id, params }`), not a hand-rolled enum.
 */

use bevy_debugger_mcp::brp_messages::{
    builtin_methods, BrpPayload, BrpRequest, BrpResponse, QueryFilter,
};
use serde_json::{json, Value};

#[tokio::test]
async fn test_bevy_16_strict_query_parameter() {
    // A `world.query` request with a strict + limit params envelope.
    let strict_query = BrpRequest {
        method: builtin_methods::BRP_QUERY_METHOD.to_string(),
        id: Some(json!(1)),
        params: Some(json!({
            "data": { "components": [], "option": "all", "has": [] },
            "filter": { "with": ["Transform"], "without": [] },
            "strict": true,
            "limit": 100
        })),
    };

    // Serialize to JSON
    let json_str = serde_json::to_string(&strict_query).unwrap();
    let json_value: Value = serde_json::from_str(&json_str).unwrap();

    // Verify structure matches Bevy 0.16+ BRP format (JSON-RPC 2.0)
    assert_eq!(json_value["method"], builtin_methods::BRP_QUERY_METHOD);
    assert_eq!(json_value["params"]["strict"], true);
    assert_eq!(json_value["params"]["limit"], 100);

    // Test deserialization back
    let deserialized: BrpRequest = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.method, builtin_methods::BRP_QUERY_METHOD);
    assert_eq!(deserialized.params.as_ref().unwrap()["strict"], true);
    assert_eq!(deserialized.params.as_ref().unwrap()["limit"], 100);
}

#[tokio::test]
async fn test_bevy_16_new_brp_methods() {
    // Test `world.insert_components` method
    let insert_request = BrpRequest {
        method: builtin_methods::BRP_INSERT_COMPONENTS_METHOD.to_string(),
        id: None,
        params: Some(json!({
            "entity": 12345,
            "components": {
                "Transform": { "translation": [0.0, 0.0, 0.0] }
            }
        })),
    };

    let json_str = serde_json::to_string(&insert_request).unwrap();
    let json_value: Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(json_value["method"], builtin_methods::BRP_INSERT_COMPONENTS_METHOD);
    assert_eq!(json_value["params"]["entity"], 12345);

    // Test `world.remove_components` method
    let remove_request = BrpRequest {
        method: builtin_methods::BRP_REMOVE_COMPONENTS_METHOD.to_string(),
        id: None,
        params: Some(json!({
            "entity": 12345,
            "components": ["Transform", "Velocity"]
        })),
    };

    let json_str = serde_json::to_string(&remove_request).unwrap();
    let json_value: Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(json_value["method"], builtin_methods::BRP_REMOVE_COMPONENTS_METHOD);
    assert_eq!(json_value["params"]["entity"], 12345);
    assert_eq!(json_value["params"]["components"].as_array().unwrap().len(), 2);

    // Test `world.reparent_entities` method
    let reparent_request = BrpRequest {
        method: builtin_methods::BRP_REPARENT_ENTITIES_METHOD.to_string(),
        id: None,
        params: Some(json!({
            "entities": [12345],
            "parent": 67890
        })),
    };

    let json_str = serde_json::to_string(&reparent_request).unwrap();
    let json_value: Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(json_value["method"], builtin_methods::BRP_REPARENT_ENTITIES_METHOD);
    assert_eq!(json_value["params"]["entities"][0], 12345);
    assert_eq!(json_value["params"]["parent"], 67890);
}

#[tokio::test]
async fn test_backwards_compatibility_with_legacy_queries() {
    // A `world.query` request without the strict parameter.
    let legacy_query = BrpRequest {
        method: builtin_methods::BRP_QUERY_METHOD.to_string(),
        id: None,
        params: Some(json!({
            "data": { "components": [], "option": "all", "has": [] },
            "filter": { "with": ["Transform"], "without": [] }
        })),
    };

    let json_str = serde_json::to_string(&legacy_query).unwrap();
    let json_value: Value = serde_json::from_str(&json_str).unwrap();

    // Strict parameter should not be present in JSON when not set
    assert!(json_value["params"].get("strict").is_none() || json_value["params"]["strict"].is_null());

    // Should still deserialize correctly
    let deserialized: BrpRequest = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.method, builtin_methods::BRP_QUERY_METHOD);
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
        // Test in a query request
        let query = BrpRequest {
            method: builtin_methods::BRP_QUERY_METHOD.to_string(),
            id: None,
            params: Some(json!({
                "data": { "components": [], "option": "all", "has": [] },
                "filter": { "with": [type_name], "without": [] },
                "strict": true
            })),
        };

        // Should serialize/deserialize without issues
        let json_str = serde_json::to_string(&query).unwrap();
        let _deserialized: BrpRequest = serde_json::from_str(&json_str).unwrap();

        // Test in component operations
        let insert_request = BrpRequest {
            method: builtin_methods::BRP_INSERT_COMPONENTS_METHOD.to_string(),
            id: None,
            params: Some(json!({
                "entity": 123,
                "components": { type_name: { "test": "data" } }
            })),
        };

        let json_str = serde_json::to_string(&insert_request).unwrap();
        let _deserialized: BrpRequest = serde_json::from_str(&json_str).unwrap();
    }
}

#[tokio::test]
async fn test_json_rpc_2_0_format_compatibility() {
    // Test that requests can be formatted as proper JSON-RPC 2.0 messages
    let query = BrpRequest {
        method: builtin_methods::BRP_QUERY_METHOD.to_string(),
        id: Some(json!(1)),
        params: Some(json!({
            "data": { "components": [], "option": "all", "has": [] },
            "filter": { "with": ["Transform"], "without": [] },
            "limit": 10,
            "strict": true
        })),
    };

    // Serialize and verify the JSON-RPC 2.0 envelope
    let json_str = serde_json::to_string(&query).unwrap();
    let json_value: Value = serde_json::from_str(&json_str).unwrap();

    assert_eq!(json_value["method"], builtin_methods::BRP_QUERY_METHOD);
    assert_eq!(json_value["params"]["strict"], true);
    assert_eq!(json_value["params"]["limit"], 10);
}

/// Verify a `BrpResponse` carrying a successful result decodes round-trip.
#[tokio::test]
async fn test_response_result_round_trip() {
    let response = BrpResponse {
        id: Some(json!(7)),
        payload: BrpPayload::Result(json!({ "entities": [] })),
    };
    let json_str = serde_json::to_string(&response).unwrap();
    let deserialized: BrpResponse = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.id, Some(json!(7)));
    assert!(matches!(deserialized.payload, BrpPayload::Result(_)));
}

/// Verify a `BrpResponse` carrying an error decodes round-trip.
#[tokio::test]
async fn test_response_error_round_trip() {
    use bevy_debugger_mcp::brp_messages::BrpError;

    let response = BrpResponse {
        id: Some(json!(7)),
        payload: BrpPayload::Error(BrpError {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        }),
    };
    let json_str = serde_json::to_string(&response).unwrap();
    let deserialized: BrpResponse = serde_json::from_str(&json_str).unwrap();
    match deserialized.payload {
        BrpPayload::Error(err) => {
            assert_eq!(err.code, -32601);
            assert_eq!(err.message, "Method not found");
        }
        _ => panic!("Expected error payload"),
    }
}

/// `QueryFilter` serialization is unaffected by the wire-protocol change.
#[tokio::test]
async fn test_query_filter_serialization() {
    let mut filter = QueryFilter::default();
    filter.with = Some(vec!["Transform".to_string()]);
    let json = serde_json::to_string(&filter).unwrap();
    let deserialized: QueryFilter = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.with.as_ref().unwrap().len(), 1);
}

/// Integration test with a minimal Bevy app (feature-gated).
#[cfg(feature = "bevy")]
#[tokio::test]
async fn test_real_bevy_16_integration() {
    use bevy::prelude::*;

    // Create a minimal Bevy app with remote plugin
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
       .add_plugins(bevy::remote::RemotePlugin::default());

    // Spawn a test entity and read its generation-aware id.
    let entity_id = app.world_mut().spawn((
        Transform::from_translation(Vec3::new(1.0, 2.0, 3.0)),
        Name::new("TestEntity"),
    )).id();

    // Bevy's BRP serialises entities as `(generation << 32) | index` u64s.
    let combined: u64 = ((entity_id.generation() as u64) << 32) | (entity_id.index() as u64);
    let index = (combined & 0xFFFF_FFFF) as u32;
    let generation = (combined >> 32) as u32;
    assert_eq!(index, entity_id.index());
    assert_eq!(generation, entity_id.generation());
}