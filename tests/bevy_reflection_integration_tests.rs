/*
 * Bevy Debugger MCP Server - Bevy Reflection Integration Tests
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

//! Integration tests for Bevy reflection system

use bevy_debugger_mcp::bevy_reflection::*;
use bevy_debugger_mcp::bevy_reflection::custom_inspectors::*;
use bevy_debugger_mcp::bevy_reflection::type_registry_tools::*;
use bevy_debugger_mcp::bevy_reflection::reflection_queries::*;
use serde_json::{json, Value};
use std::collections::HashMap;

#[tokio::test]
async fn test_reflection_inspector_basic_functionality() {
    let inspector = BevyReflectionInspector::new();
    
    // Test basic component inspection
    let component_value = json!({
        "translation": {"x": 1.0, "y": 2.0, "z": 3.0},
        "rotation": {"x": 0.0, "y": 0.0, "z": 0.0, "w": 1.0},
        "scale": {"x": 1.0, "y": 1.0, "z": 1.0}
    });
    
    let result = inspector.inspect_component("bevy_transform::components::transform::Transform", &component_value).await;
    assert!(result.is_ok());
    
    let inspection = result.unwrap();
    assert_eq!(inspection.component_type, "bevy_transform::components::transform::Transform");
    assert!(!inspection.field_values.is_empty());
}

#[tokio::test]
async fn test_custom_inspectors() {
    // Test Option inspector
    let option_inspector = OptionInspector;
    
    // Test Some variant
    let some_value = json!({"Some": "test_value"});
    let result = option_inspector.inspect(&some_value, "Option<String>").unwrap();
    assert!(result.display_value.contains("Some"));
    assert!(result.inspectable);
    
    // Test None variant
    let none_value = json!(null);
    let result = option_inspector.inspect(&none_value, "Option<String>").unwrap();
    assert_eq!(result.display_value, "None");
    assert!(!result.inspectable);
    
    // Test Vec inspector
    let vec_inspector = VecInspector;
    let vec_value = json!([1, 2, 3, 4, 5]);
    let result = vec_inspector.inspect(&vec_value, "Vec<i32>").unwrap();
    assert_eq!(result.display_value, "Vec<T>[5]");
    assert!(result.inspectable);
    assert_eq!(result.children.as_ref().unwrap().len(), 5);
    
    // Test HashMap inspector
    let map_inspector = HashMapInspector;
    let map_value = json!({"key1": "value1", "key2": "value2"});
    let result = map_inspector.inspect(&map_value, "HashMap<String, String>").unwrap();
    assert_eq!(result.display_value, "HashMap<K,V>[2]");
    assert!(result.inspectable);
    
    // Test Entity inspector
    let entity_inspector = EntityInspector;
    let entity_value = json!({"id": 123, "generation": 1});
    let result = entity_inspector.inspect(&entity_value, "Entity").unwrap();
    assert!(result.display_value.contains("Entity"));
    assert!(result.display_value.contains("123"));
    
    // Test Color inspector
    let color_inspector = ColorInspector;
    let color_value = json!({"r": 1.0, "g": 0.5, "b": 0.0, "a": 1.0});
    let result = color_inspector.inspect(&color_value, "Color").unwrap();
    assert!(result.display_value.contains("RGBA"));
    assert!(result.display_value.contains("#"));
}

#[tokio::test]
async fn test_reflection_diffing() {
    let inspector = BevyReflectionInspector::new();
    
    let old_value = json!({
        "x": 10.0,
        "y": 20.0,
        "z": 30.0
    });
    
    let new_value = json!({
        "x": 15.0,   // Changed
        "y": 20.0,   // Same
        "z": 30.0,   // Same
        "w": 40.0    // Added
    });
    
    let diff_result = inspector.diff_components("Vec3", &old_value, &new_value).await.unwrap();
    
    assert_eq!(diff_result.type_name, "Vec3");
    assert!(diff_result.summary.changed_fields > 0);
    assert!(diff_result.summary.added_fields > 0);
    assert!(!diff_result.change_descriptions.is_empty());
    
    // Check for specific changes
    let has_added_field = diff_result.field_diffs.values()
        .any(|diff| matches!(diff.change_type, ChangeType::Added));
    assert!(has_added_field, "Should detect added field 'w'");
    
    let has_modified_field = diff_result.field_diffs.values()
        .any(|diff| matches!(diff.change_type, ChangeType::Modified));
    assert!(has_modified_field, "Should detect modified field 'x'");
}

#[tokio::test]
async fn test_complex_type_inspection() {
    let inspector = BevyReflectionInspector::new();
    
    // Test nested structure with various complex types
    let complex_component = json!({
        "optional_field": {"Some": "present"},
        "empty_optional": null,
        "vector_field": [1, 2, 3, 4, 5],
        "map_field": {
            "key1": "value1",
            "key2": "value2",
            "key3": "value3"
        },
        "nested_structure": {
            "inner_optional": {"Some": [1, 2, 3]},
            "inner_map": {
                "nested_key": {
                    "deep_value": 42
                }
            }
        }
    });
    
    let result = inspector.inspect_component("ComplexComponent", &complex_component).await.unwrap();
    
    assert_eq!(result.component_type, "ComplexComponent");
    assert!(!result.field_values.is_empty());
    assert!(result.errors.is_empty());
    
    // Check that the inspection handled the complex nested structure
    let root_value = result.field_values.get("root").unwrap();
    assert!(root_value.inspectable);
    assert!(root_value.children.is_some());
}

#[tokio::test]
async fn test_type_registry_manager() {
    let manager = TypeRegistryManager::new();
    
    // Test basic functionality
    let stats = manager.get_stats().await;
    assert_eq!(stats.total_types, 0);
    
    // Test type queries
    let query = TypeQuery {
        name_pattern: Some("Transform".to_string()),
        category: Some(TypeCategory::Struct),
        min_score: 0.5,
        limit: 10,
        requires_reflection: false,
        requires_constructible: false,
        required_fields: None,
    };
    
    let query_result = manager.query_types(&query).await.unwrap();
    assert_eq!(query_result.types.len(), 0); // No types registered yet
    assert!(query_result.execution_time_ms > 0);
}

#[tokio::test]
async fn test_reflection_query_engine() {
    let engine = ReflectionQueryEngine::new();
    
    // Test basic query structure
    let query = ReflectionQuery {
        base_query: "list all entities".to_string(),
        reflection_params: ReflectionQueryParams::default(),
        limits: QueryLimits::default(),
    };
    
    // Test query complexity calculation
    let complexity = engine.calculate_query_complexity(&query).await;
    assert!(complexity > 0.0);
    assert!(complexity <= 1.0);
    
    // Test statistics
    let stats = engine.get_stats().await;
    assert_eq!(stats.total_queries, 0);
}

#[tokio::test]
async fn test_type_path_utils() {
    use bevy_debugger_mcp::bevy_reflection::type_registry_tools::type_path_utils::*;
    
    // Test short name extraction
    assert_eq!(
        extract_short_name("bevy_transform::components::transform::Transform"),
        "Transform"
    );
    assert_eq!(extract_short_name("SimpleType"), "SimpleType");
    
    // Test Bevy type detection
    assert!(is_bevy_core_type("bevy_core::name::Name"));
    assert!(is_bevy_core_type("some::module::bevy_transform::Transform"));
    assert!(!is_bevy_core_type("my_game::MyComponent"));
    
    // Test generic type detection
    assert!(is_generic_type("Option<String>"));
    assert!(is_generic_type("Vec<MyComponent>"));
    assert!(is_generic_type("HashMap<String, i32>"));
    assert!(!is_generic_type("Transform"));
    
    // Test generic parameter extraction
    let params = extract_generic_params("HashMap<String, i32>");
    assert_eq!(params, vec!["String", "i32"]);
    
    let params = extract_generic_params("Option<Vec<String>>");
    assert_eq!(params, vec!["Vec<String>"]);
    
    let params = extract_generic_params("Transform");
    assert!(params.is_empty());
}

#[tokio::test]
async fn test_change_severity_assessment() {
    let inspector = BevyReflectionInspector::new();
    
    // Test numerical changes
    let old_num = json!(10.0);
    let trivial_change = json!(10.001);
    let minor_change = json!(10.5);
    let major_change = json!(15.0);
    
    assert!(matches!(
        inspector.assess_change_severity(&old_num, &trivial_change),
        ChangeSeverity::Trivial
    ));
    
    assert!(matches!(
        inspector.assess_change_severity(&old_num, &minor_change),
        ChangeSeverity::Minor
    ));
    
    assert!(matches!(
        inspector.assess_change_severity(&old_num, &major_change),
        ChangeSeverity::Major
    ));
    
    // Test type changes
    let old_string = json!("hello");
    let new_number = json!(42);
    assert!(matches!(
        inspector.assess_change_severity(&old_string, &new_number),
        ChangeSeverity::Major
    ));
}

#[tokio::test]
async fn test_reflection_cache_functionality() {
    let inspector = BevyReflectionInspector::new();
    
    // Test cache statistics
    let stats = inspector.get_reflection_stats().await;
    assert!(stats.get("cached_types").is_some());
    assert!(stats.get("custom_inspectors").is_some());
    
    // Test cache clearing
    inspector.clear_cache().await;
    let stats_after_clear = inspector.get_reflection_stats().await;
    assert_eq!(stats_after_clear["cached_types"], 0);
}

#[tokio::test]
async fn test_default_inspector_creation() {
    let inspectors = create_default_inspectors();
    
    // Should include all default inspector types
    assert!(!inspectors.is_empty());
    
    // Verify we have the expected inspector types
    let inspector_names: Vec<&str> = inspectors.iter()
        .map(|i| i.name())
        .collect();
    
    assert!(inspector_names.contains(&"OptionInspector"));
    assert!(inspector_names.contains(&"VecInspector"));
    assert!(inspector_names.contains(&"HashMapInspector"));
    assert!(inspector_names.contains(&"EntityInspector"));
    assert!(inspector_names.contains(&"ColorInspector"));
}

#[tokio::test]
async fn test_performance_limits_handling() {
    let inspector = BevyReflectionInspector::new();
    
    // Test with large array (should handle gracefully)
    let large_array: Vec<i32> = (0..1000).collect();
    let large_value = json!(large_array);
    
    let result = inspector.inspect_value_generic(&large_value, "LargeArray").await.unwrap();
    assert_eq!(result.type_info, "Vec<T> (length: 1000)");
    assert!(result.inspectable);
    
    // Should handle deeply nested structures
    let mut deeply_nested = json!({});
    let mut current = &mut deeply_nested;
    
    // Create a reasonably nested structure (not too deep to cause issues)
    for i in 0..10 {
        *current = json!({"level": i, "next": {}});
        current = current.get_mut("next").unwrap();
    }
    
    let result = inspector.inspect_value_generic(&deeply_nested, "DeepStruct").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_reflection_integration_with_bevy_types() {
    let inspector = BevyReflectionInspector::new();
    
    // Test Transform component
    let transform_component = json!({
        "translation": {"x": 1.0, "y": 2.0, "z": 3.0},
        "rotation": {"x": 0.0, "y": 0.0, "z": 0.0, "w": 1.0},
        "scale": {"x": 1.0, "y": 1.0, "z": 1.0}
    });
    
    let result = inspector.inspect_component("bevy_transform::components::transform::Transform", &transform_component).await.unwrap();
    assert_eq!(result.component_type, "bevy_transform::components::transform::Transform");
    
    // Test Name component
    let name_component = json!("Player Entity");
    let result = inspector.inspect_component("bevy_core::name::Name", &name_component).await.unwrap();
    assert_eq!(result.component_type, "bevy_core::name::Name");
    
    // Test Visibility component
    let visibility_component = json!("Visible");
    let result = inspector.inspect_component("bevy_render::view::visibility::Visibility", &visibility_component).await.unwrap();
    assert_eq!(result.component_type, "bevy_render::view::visibility::Visibility");
}

#[tokio::test] 
async fn test_error_handling_in_reflection() {
    let inspector = BevyReflectionInspector::new();
    
    // Test with invalid JSON structure
    let invalid_component = json!("not an object for a complex component");
    let result = inspector.inspect_component("ComplexComponent", &invalid_component).await.unwrap();
    
    // Should handle gracefully without panicking
    assert_eq!(result.component_type, "ComplexComponent");
    // May have errors but should still return a result
    
    // Test with extremely large component data
    let mut large_object = serde_json::Map::new();
    for i in 0..1000 {
        large_object.insert(format!("field_{}", i), json!(format!("value_{}", i)));
    }
    let large_component = Value::Object(large_object);
    
    let result = inspector.inspect_component("LargeComponent", &large_component).await;
    assert!(result.is_ok()); // Should handle large data without errors
}