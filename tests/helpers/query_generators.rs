/// Query Generators for Testing
/// 
/// Utilities for generating realistic and stress-testing queries
/// to validate caching and optimization performance.

use serde_json::{json, Value};

/// Generate realistic debugging queries that would be used in practice
pub fn generate_realistic_queries() -> Vec<(String, Value)> {
    vec![
        // Entity queries
        ("observe".to_string(), json!({
            "query": "entities with Transform"
        })),
        ("observe".to_string(), json!({
            "query": "entities with (Transform and Mesh)"
        })),
        ("observe".to_string(), json!({
            "query": "entities with Name containing 'player'"
        })),
        ("observe".to_string(), json!({
            "query": "entities with Health < 50"
        })),
        ("observe".to_string(), json!({
            "query": "entities in range 10 of (0, 0, 0)"
        })),
        
        // System profiling queries
        ("experiment".to_string(), json!({
            "type": "performance",
            "systems": ["movement", "physics", "rendering"],
            "duration": 1000
        })),
        ("experiment".to_string(), json!({
            "type": "memory",
            "duration": 2000,
            "include_entities": true
        })),
        
        // Resource monitoring
        ("resource_metrics".to_string(), json!({})),
        ("health_check".to_string(), json!({})),
        
        // Complex queries
        ("observe".to_string(), json!({
            "query": "entities with (Combat and Health > 80) or (Merchant and Inventory.gold > 100)",
            "sort": "health",
            "limit": 50
        })),
        ("observe".to_string(), json!({
            "query": "all entities in faction 'warriors'",
            "include_relationships": true
        })),
        
        // Debugging specific queries
        ("diagnostic_report".to_string(), json!({
            "action": "generate",
            "include_performance": true,
            "include_memory": true
        })),
        ("anomaly".to_string(), json!({
            "metric": "frame_time",
            "threshold": 16.67,
            "window": 1000
        })),
        
        // Session management
        ("replay".to_string(), json!({
            "session_id": "debug_session_001",
            "time_range": {"start": 0, "end": 30000}
        })),
        ("checkpoint".to_string(), json!({
            "name": "before_optimization_test",
            "include_world_state": true
        })),
        
        // Tool orchestration
        ("orchestrate".to_string(), json!({
            "tool": "observe",
            "arguments": {"query": "entities with Position.y > 10"},
            "follow_up": "experiment"
        })),
        ("pipeline".to_string(), json!({
            "template": "performance_analysis",
            "parameters": {
                "duration": 5000,
                "entity_types": ["warrior", "merchant", "villager"]
            }
        })),
        
        // Stress testing
        ("stress".to_string(), json!({
            "type": "entity_spawn",
            "count": 100,
            "entity_type": "complex"
        })),
        ("stress".to_string(), json!({
            "type": "continuous_spawn",
            "intensity": 2,
            "duration": 30000
        })),
    ]
}

/// Generate stress-testing queries for performance validation
pub fn generate_stress_queries() -> Vec<(String, Value)> {
    vec![
        // High-frequency entity queries
        ("observe".to_string(), json!({
            "query": "all entities",
            "cache_ttl": 0 // Force no caching
        })),
        ("observe".to_string(), json!({
            "query": "entities with any component",
            "include_all_data": true
        })),
        
        // Complex relationship queries
        ("observe".to_string(), json!({
            "query": "entities with relationships to entities with Combat > 50",
            "recursive_depth": 3,
            "include_relationship_data": true
        })),
        
        // Large data queries
        ("observe".to_string(), json!({
            "query": "entities with Inventory",
            "include_inventory_contents": true,
            "expand_references": true
        })),
        
        // Expensive computation queries
        ("experiment".to_string(), json!({
            "type": "pathfinding_analysis",
            "start_points": (0..20).map(|i| json!([i * 2, 0, i * 3])).collect::<Vec<_>>(),
            "end_points": (0..20).map(|i| json!([i * -2, 0, i * -3])).collect::<Vec<_>>(),
            "algorithm": "a_star"
        })),
        
        // Memory-intensive queries
        ("observe".to_string(), json!({
            "query": "history of all entities over last 60 seconds",
            "include_state_changes": true,
            "resolution": "high"
        })),
        
        // Concurrent stress queries
        ("observe".to_string(), json!({
            "query": format!("entities with id in range {} to {}", 0, 10000),
            "batch_size": 1000
        })),
        
        // Deep analysis queries
        ("experiment".to_string(), json!({
            "type": "system_dependency_analysis",
            "include_performance_impact": true,
            "analyze_bottlenecks": true,
            "correlation_analysis": true
        })),
        
        // High-volume data export
        ("diagnostic_report".to_string(), json!({
            "action": "export_all_data",
            "format": "detailed_json",
            "include_history": true,
            "compression": false
        })),
    ]
}

/// Generate queries that test specific optimization features
pub fn generate_optimization_test_queries() -> Vec<(String, Value)> {
    vec![
        // Cache effectiveness tests
        ("observe".to_string(), json!({
            "query": "entities with Transform",
            "test_id": "cache_test_1"
        })),
        ("observe".to_string(), json!({
            "query": "entities with Transform", // Identical to above for cache hit
            "test_id": "cache_test_1_repeat"
        })),
        
        // Pool utilization tests
        ("observe".to_string(), json!({
            "query": "small dataset",
            "expected_response_size": "small"
        })),
        ("observe".to_string(), json!({
            "query": "entities with detailed data",
            "include_all_components": true,
            "expected_response_size": "large"
        })),
        
        // Lazy initialization tests
        ("health_check".to_string(), json!({
            "initialize_components": false
        })),
        ("observe".to_string(), json!({
            "query": "first entity access",
            "force_initialization": true
        })),
        
        // Feature flag tests
        ("observe".to_string(), json!({
            "query": "test basic functionality"
        })),
        ("experiment".to_string(), json!({
            "type": "feature_flag_test",
            "check_enabled_features": true
        })),
    ]
}

/// Generate queries with varying complexity for performance testing
pub fn generate_complexity_queries() -> Vec<(String, Value)> {
    let mut queries = Vec::new();
    
    // Simple queries (O(n) complexity)
    for i in 0..10 {
        queries.push(("observe".to_string(), json!({
            "query": format!("entities with component_type_{}", i),
            "complexity": "simple"
        })));
    }
    
    // Medium complexity queries (O(n log n))
    for i in 0..5 {
        queries.push(("observe".to_string(), json!({
            "query": format!("entities with component_type_{} sorted by value", i),
            "sort": "value",
            "complexity": "medium"
        })));
    }
    
    // Complex queries (O(n²) or higher)
    queries.push(("observe", json!({
        "query": "entities with relationships to entities with relationships",
        "recursive": true,
        "max_depth": 3,
        "complexity": "high"
    })));
    
    queries.push(("experiment".to_string(), json!({
        "type": "cross_reference_analysis",
        "analyze_all_relationships": true,
        "include_indirect_dependencies": true,
        "complexity": "very_high"
    })));
    
    queries
}

/// Generate queries that test edge cases and error conditions
pub fn generate_edge_case_queries() -> Vec<(String, Value)> {
    vec![
        // Empty results
        ("observe".to_string(), json!({
            "query": "entities with NonExistentComponent"
        })),
        
        // Invalid queries
        ("observe".to_string(), json!({
            "query": "invalid query syntax ((("
        })),
        
        // Very large limits
        ("observe".to_string(), json!({
            "query": "all entities",
            "limit": 1000000
        })),
        
        // Invalid parameters
        ("experiment".to_string(), json!({
            "type": "nonexistent_experiment_type"
        })),
        
        // Null/undefined values
        ("observe".to_string(), json!({
            "query": null
        })),
        
        // Very long strings
        ("observe".to_string(), json!({
            "query": "a".repeat(10000)
        })),
        
        // Nested complexity
        ("observe".to_string(), json!({
            "query": {
                "nested": {
                    "very": {
                        "deeply": {
                            "nested": "query"
                        }
                    }
                }
            }
        })),
        
        // Circular references (in parameters)
        ("experiment".to_string(), json!({
            "type": "circular_test",
            "parameters": {
                "reference": "self"
            }
        })),
    ]
}

/// Generate sequential queries to test caching patterns
pub fn generate_sequential_queries() -> Vec<Vec<(String, Value)>> {
    vec![
        // Cache warm-up sequence
        vec![
            ("health_check".to_string(), json!({})), // Initialize systems
            ("resource_metrics".to_string(), json!({})), // Get baseline
            ("observe".to_string(), json!({"query": "entities with Transform"})), // First entity query
            ("observe".to_string(), json!({"query": "entities with Transform"})), // Should hit cache
        ],
        
        // Progressive complexity sequence
        vec![
            ("observe".to_string(), json!({"query": "entity count"})),
            ("observe".to_string(), json!({"query": "entities with Transform"})),
            ("observe".to_string(), json!({"query": "entities with (Transform and Mesh)"})),
            ("observe".to_string(), json!({"query": "entities with (Transform and Mesh and Health)"})),
        ],
        
        // Cache invalidation sequence
        vec![
            ("observe".to_string(), json!({"query": "all entities"})), // Cache initial state
            ("stress", json!({"type": "entity_spawn", "count": 10})), // Modify world
            ("observe".to_string(), json!({"query": "all entities"})), // Should invalidate cache
        ],
        
        // Performance regression sequence
        vec![
            ("checkpoint", json!({"name": "performance_baseline"})),
            ("experiment".to_string(), json!({"type": "performance", "duration": 1000})),
            ("stress", json!({"type": "continuous_spawn", "intensity": 1})),
            ("experiment".to_string(), json!({"type": "performance", "duration": 1000})),
            ("diagnostic_report", json!({"action": "compare_checkpoints"})),
        ],
    ]
}

/// Generate time-based query patterns for realistic usage simulation
pub fn generate_time_based_queries(duration_seconds: u64) -> Vec<(u64, String, Value)> {
    let mut queries = Vec::new();
    let mut time_offset = 0;

    // Regular monitoring queries (every 5 seconds)
    while time_offset < duration_seconds {
        queries.push((time_offset, "health_check".to_string(), json!({})));
        queries.push((time_offset + 2, "resource_metrics".to_string(), json!({})));
        time_offset += 5;
    }

    // Entity queries (every 3 seconds)
    time_offset = 1;
    let entity_queries = [
        "entities with Transform",
        "entities with Health < 50",
        "entities in combat",
        "entities with Inventory",
    ];
    
    let mut query_index = 0;
    while time_offset < duration_seconds {
        queries.push((
            time_offset,
            "observe".to_string(),
            json!({"query": entity_queries[query_index % entity_queries.len()]}),
        ));
        query_index += 1;
        time_offset += 3;
    }

    // Periodic experiments (every 15 seconds)
    time_offset = 10;
    while time_offset < duration_seconds {
        queries.push((
            time_offset,
            "experiment".to_string(),
            json!({"type": "performance", "duration": 2000}),
        ));
        time_offset += 15;
    }

    // Sort by time
    queries.sort_by_key(|(time, _, _)| *time);
    queries
}

/// Generate workload patterns for specific test scenarios
pub fn generate_workload_pattern(pattern_type: &str) -> Vec<(String, Value)> {
    match pattern_type {
        "debugging_session" => vec![
            ("health_check".to_string(), json!({})),
            ("observe".to_string(), json!({"query": "all entities", "limit": 10})),
            ("observe".to_string(), json!({"query": "entities with Health < 30"})),
            ("experiment".to_string(), json!({"type": "performance", "duration": 3000})),
            ("observe".to_string(), json!({"query": "entities in combat"})),
            ("diagnostic_report", json!({"action": "generate"})),
            ("checkpoint", json!({"name": "debug_checkpoint"})),
        ],
        
        "performance_analysis" => vec![
            ("resource_metrics".to_string(), json!({})),
            ("experiment".to_string(), json!({"type": "performance", "systems": ["movement", "physics"]})),
            ("observe".to_string(), json!({"query": "entities with high CPU impact"})),
            ("experiment".to_string(), json!({"type": "memory", "duration": 5000})),
            ("observe".to_string(), json!({"query": "entities with large memory footprint"})),
            ("diagnostic_report", json!({"action": "performance_summary"})),
        ],
        
        "stress_testing" => vec![
            ("health_check".to_string(), json!({})),
            ("stress", json!({"type": "entity_spawn", "count": 100})),
            ("observe".to_string(), json!({"query": "entity count"})),
            ("stress", json!({"type": "continuous_spawn", "intensity": 3})),
            ("experiment".to_string(), json!({"type": "performance", "duration": 10000})),
            ("stress", json!({"type": "entity_despawn", "count": 50})),
            ("diagnostic_report", json!({"action": "stress_test_report"})),
        ],
        
        "cache_warming" => {
            let mut queries = Vec::new();
            // Warm up common queries
            for query in ["entities with Transform", "entities with Health", "entities with Inventory"] {
                queries.push(("observe".to_string(), json!({"query": query})));
                queries.push(("observe".to_string(), json!({"query": query}))); // Immediate repeat for cache hit
            }
            queries
        },
        
        _ => generate_realistic_queries(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_realistic_queries() {
        let queries = generate_realistic_queries();
        assert!(!queries.is_empty(), "Should generate some queries");
        
        // Check that we have different types of queries
        let observe_count = queries.iter().filter(|(cmd, _)| cmd == "observe").count();
        let experiment_count = queries.iter().filter(|(cmd, _)| cmd == "experiment").count();
        
        assert!(observe_count > 0, "Should have observe queries");
        assert!(experiment_count > 0, "Should have experiment queries");
    }

    #[test]
    fn test_generate_complexity_queries() {
        let queries = generate_complexity_queries();
        assert!(!queries.is_empty(), "Should generate complexity queries");
        
        // Check that we have different complexity levels
        let simple_count = queries.iter()
            .filter(|(_, args)| args.get("complexity").and_then(|c| c.as_str()) == Some("simple"))
            .count();
        let complex_count = queries.iter()
            .filter(|(_, args)| args.get("complexity").and_then(|c| c.as_str()).map_or(false, |c| c.contains("high")))
            .count();
        
        assert!(simple_count > 0, "Should have simple queries");
        assert!(complex_count > 0, "Should have complex queries");
    }

    #[test]
    fn test_generate_time_based_queries() {
        let queries = generate_time_based_queries(30);
        assert!(!queries.is_empty(), "Should generate time-based queries");
        
        // Check that times are in ascending order
        for i in 1..queries.len() {
            assert!(queries[i].0 >= queries[i-1].0, "Times should be in ascending order");
        }
        
        // Check that all times are within the specified duration
        for (time, _, _) in &queries {
            assert!(*time < 30, "All times should be within duration");
        }
    }

    #[test]
    fn test_workload_patterns() {
        let patterns = ["debugging_session", "performance_analysis", "stress_testing", "cache_warming"];
        
        for pattern in patterns {
            let queries = generate_workload_pattern(pattern);
            assert!(!queries.is_empty(), "Pattern '{}' should generate queries", pattern);
        }
    }
}