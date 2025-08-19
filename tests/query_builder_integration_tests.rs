/// Comprehensive integration tests for BEVDBG-006 - ECS Query Builder and Validator
/// 
/// Tests cover:
/// - QueryBuilder fluent interface functionality
/// - Query validation with helpful error messages
/// - Query optimization suggestions
/// - Query result pagination for large datasets
/// - Query execution time estimation
/// - Common query patterns caching and reuse
/// - Type mismatch detection
/// - Integration with existing query API compatibility
/// - QueryBuilderProcessor debug command handling
/// - End-to-end query building, validation, and execution workflow

use bevy_debugger_mcp::{
    brp_client::BrpClient,
    brp_messages::{DebugCommand, DebugResponse, QueryFilter, ComponentFilter, FilterOp},
    config::Config,
    debug_command_processor::{DebugCommandProcessor, DebugCommandRouter, DebugCommandRequest},
    query_builder::{QueryBuilder, QueryValidator, QueryCostEstimator, QueryOptimizer, MAX_COMPONENTS_PER_QUERY, QUERY_PERFORMANCE_BUDGET_US},
    query_builder_processor::QueryBuilderProcessor,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Helper to create test configuration
fn create_test_config() -> Config {
    Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3000,
    }
}

/// Helper to create test BRP client
fn create_test_brp_client() -> Arc<RwLock<BrpClient>> {
    let config = create_test_config();
    Arc::new(RwLock::new(BrpClient::new(&config)))
}

/// Test basic QueryBuilder fluent interface
#[tokio::test]
async fn test_query_builder_fluent_interface() {
    let query = QueryBuilder::new()
        .with_component("Transform")
        .with_component("Velocity")
        .without_component("Camera")
        .limit(50)
        .offset(10);

    assert_eq!(query.get_with_components(), &["Transform", "Velocity"]);
    assert_eq!(query.get_without_components(), &["Camera"]);
    assert_eq!(query.get_limit(), Some(50));
    assert_eq!(query.get_offset(), Some(10));
}

/// Test QueryBuilder with multiple components at once
#[tokio::test]
async fn test_query_builder_batch_components() {
    let with_components = vec!["Transform", "Velocity", "Name"];
    let without_components = vec!["Camera", "DirectionalLight"];

    let query = QueryBuilder::new()
        .with_components(with_components.clone())
        .without_components(without_components.clone());

    assert_eq!(query.get_with_components().len(), 3);
    assert_eq!(query.get_without_components().len(), 2);
    assert!(query.get_with_components().contains(&"Transform".to_string()));
    assert!(query.get_without_components().contains(&"Camera".to_string()));
}

/// Test query validation with valid components
#[tokio::test]
async fn test_query_validation_success() {
    let query = QueryBuilder::new()
        .with_component("Transform")
        .with_component("Velocity")
        .limit(10);

    let result = query.validate();
    assert!(result.is_ok());

    let validated = result.unwrap();
    assert!(!validated.id.is_empty());
    assert!(validated.filter.with.is_some());
    assert_eq!(validated.filter.with.unwrap().len(), 2);
}

/// Test query validation with unknown components
#[tokio::test]
async fn test_query_validation_unknown_component() {
    let query = QueryBuilder::new()
        .with_component("UnknownComponent123");

    let result = query.validate();
    assert!(result.is_err());
    
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Unknown component type"));
    assert!(error_msg.contains("UnknownComponent123"));
    assert!(error_msg.contains("Available components")); // Should provide suggestions
}

/// Test query validation with contradictory filters
#[tokio::test]
async fn test_query_validation_contradictory() {
    let query = QueryBuilder::new()
        .with_component("Transform")
        .without_component("Transform"); // Same component required and excluded

    let result = query.validate();
    assert!(result.is_err());
    
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("both required"));
    assert!(error_msg.contains("excluded"));
    assert!(error_msg.contains("Transform"));
}

/// Test query validation with empty query
#[tokio::test]
async fn test_query_validation_empty_query() {
    let query = QueryBuilder::new(); // No components specified

    let result = query.validate();
    assert!(result.is_err());
    
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("empty"));
    assert!(error_msg.contains("must specify at least one"));
}

/// Test query validation with too many components
#[tokio::test]
async fn test_query_validation_too_many_components() {
    let mut query = QueryBuilder::new();
    
    // Add more than the maximum allowed components
    for i in 0..=MAX_COMPONENTS_PER_QUERY {
        query = query.with_component(format!("Component{}", i));
    }

    let result = query.validate();
    assert!(result.is_err());
    
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("too many components"));
    assert!(error_msg.contains(&MAX_COMPONENTS_PER_QUERY.to_string()));
}

/// Test query cost estimation functionality
#[tokio::test]
async fn test_query_cost_estimation() {
    let mut query = QueryBuilder::new()
        .with_component("Transform")
        .with_component("Velocity")
        .limit(100);

    let cost = query.estimate_cost();
    
    assert!(cost.estimated_entities > 0);
    assert!(cost.estimated_time_us > 0);
    assert!(cost.estimated_memory > 0);
    
    // Cost should be affected by limit
    assert!(cost.estimated_entities <= 100);
}

/// Test query cost estimation with different selectivity
#[tokio::test]
async fn test_query_cost_estimation_selectivity() {
    // Broad query (high selectivity)
    let mut broad_query = QueryBuilder::new()
        .with_component("Transform");

    // Specific query (low selectivity) 
    let mut specific_query = QueryBuilder::new()
        .with_component("Camera");

    let broad_cost = broad_query.estimate_cost();
    let specific_cost = specific_query.estimate_cost();

    // Camera should be much more selective than Transform
    assert!(specific_cost.estimated_entities <= broad_cost.estimated_entities);
}

/// Test optimization hints for broad queries
#[tokio::test]
async fn test_optimization_hints_broad_query() {
    let query = QueryBuilder::new()
        .with_component("Transform"); // Very broad component

    let hints = query.get_optimization_hints();
    assert!(!hints.is_empty());
    
    // Should suggest making the query more specific
    let hints_text = hints.join(" ");
    assert!(hints_text.contains("broad") || hints_text.contains("specific") || hints_text.contains("limit"));
}

/// Test optimization hints for large result sets
#[tokio::test]
async fn test_optimization_hints_large_results() {
    let query = QueryBuilder::new()
        .with_component("Transform")
        .with_component("GlobalTransform"); // Still quite broad

    let hints = query.get_optimization_hints();
    assert!(!hints.is_empty());
    
    let hints_text = hints.join(" ");
    assert!(hints_text.contains("limit") || hints_text.contains("pagination"));
}

/// Test optimization hints for expensive queries
#[tokio::test]
async fn test_optimization_hints_expensive_query() {
    // Create a query that should exceed performance budget
    let query = QueryBuilder::new()
        .with_component("Transform")
        .with_component("GlobalTransform")
        .with_component("Velocity");

    let hints = query.get_optimization_hints();
    let hints_text = hints.join(" ");
    
    // Should warn about performance
    let should_have_performance_hint = hints_text.contains("performance") || 
                                      hints_text.contains("budget") || 
                                      hints_text.contains("selective");
    assert!(should_have_performance_hint);
}

/// Test query cache key determinism
#[tokio::test]
async fn test_query_cache_key_deterministic() {
    let mut query1 = QueryBuilder::new()
        .with_component("Transform")
        .with_component("Velocity")
        .limit(10);

    let mut query2 = QueryBuilder::new()
        .with_component("Velocity")
        .with_component("Transform") // Different order
        .limit(10);

    let key1 = query1.cache_key();
    let key2 = query2.cache_key();
    
    // Keys should be identical despite different construction order
    assert_eq!(key1, key2);
}

/// Test query cache key uniqueness
#[tokio::test]
async fn test_query_cache_key_uniqueness() {
    let mut query1 = QueryBuilder::new()
        .with_component("Transform")
        .limit(10);

    let mut query2 = QueryBuilder::new()
        .with_component("Transform")
        .limit(20); // Different limit

    let key1 = query1.cache_key();
    let key2 = query2.cache_key();
    
    // Keys should be different for different queries
    assert_ne!(key1, key2);
}

/// Test building DebugCommand from QueryBuilder
#[tokio::test]
async fn test_build_debug_command() {
    let query = QueryBuilder::new()
        .with_component("Transform")
        .with_component("Velocity")
        .limit(50);

    let result = query.build();
    assert!(result.is_ok());

    match result.unwrap() {
        DebugCommand::ExecuteQuery { query, limit, offset } => {
            assert!(!query.id.is_empty());
            assert_eq!(limit, Some(50));
            assert_eq!(offset, None);
            assert!(query.filter.with.is_some());
            assert_eq!(query.filter.with.unwrap().len(), 2);
        }
        _ => panic!("Expected ExecuteQuery command"),
    }
}

/// Test QueryBuilderProcessor creation and basic functionality
#[tokio::test]
async fn test_query_builder_processor_creation() {
    let brp_client = create_test_brp_client();
    let processor = QueryBuilderProcessor::new(brp_client);

    // Test command support
    let validate_cmd = DebugCommand::ValidateQuery { 
        params: json!({"with": ["Transform"]})
    };
    let cost_cmd = DebugCommand::EstimateCost { 
        params: json!({"with": ["Transform"]})
    };
    let suggestions_cmd = DebugCommand::GetQuerySuggestions { 
        params: json!({"with": ["Transform"]})
    };
    let build_exec_cmd = DebugCommand::BuildAndExecuteQuery { 
        params: json!({"with": ["Transform"], "limit": 10})
    };

    assert!(processor.supports_command(&validate_cmd));
    assert!(processor.supports_command(&cost_cmd));
    assert!(processor.supports_command(&suggestions_cmd));
    assert!(processor.supports_command(&build_exec_cmd));
}

/// Test QueryBuilderProcessor validation handling
#[tokio::test]
async fn test_processor_validate_query() {
    let brp_client = create_test_brp_client();
    let processor = QueryBuilderProcessor::new(brp_client);

    let params = json!({
        "with": ["Transform", "Velocity"],
        "without": ["Camera"],
        "limit": 10
    });

    let cmd = DebugCommand::ValidateQuery { params };
    let result = processor.process(cmd).await;
    assert!(result.is_ok());

    match result.unwrap() {
        DebugResponse::QueryValidation { valid, query, errors, suggestions } => {
            assert!(valid);
            assert!(query.is_some());
            assert!(errors.is_empty());
            // Optimization suggestions are optional but often present
        }
        _ => panic!("Expected QueryValidation response"),
    }
}

/// Test QueryBuilderProcessor validation with invalid query
#[tokio::test]
async fn test_processor_validate_invalid_query() {
    let brp_client = create_test_brp_client();
    let processor = QueryBuilderProcessor::new(brp_client);

    let params = json!({
        "with": ["UnknownComponent123"]
    });

    let cmd = DebugCommand::ValidateQuery { params };
    let result = processor.process(cmd).await;
    assert!(result.is_ok());

    match result.unwrap() {
        DebugResponse::QueryValidation { valid, query, errors, .. } => {
            assert!(!valid);
            assert!(query.is_none());
            assert!(!errors.is_empty());
            assert!(errors[0].contains("Unknown component"));
        }
        _ => panic!("Expected QueryValidation response"),
    }
}

/// Test QueryBuilderProcessor cost estimation
#[tokio::test]
async fn test_processor_estimate_cost() {
    let brp_client = create_test_brp_client();
    let processor = QueryBuilderProcessor::new(brp_client);

    let params = json!({
        "with": ["Transform"],
        "limit": 100
    });

    let cmd = DebugCommand::EstimateCost { params };
    let result = processor.process(cmd).await;
    assert!(result.is_ok());

    match result.unwrap() {
        DebugResponse::QueryCost { cost, performance_budget_exceeded, suggestions } => {
            assert!(cost.estimated_entities > 0);
            assert!(cost.estimated_time_us > 0);
            assert!(cost.estimated_memory > 0);
            // With a limit of 100, likely within budget
            // Budget exceeded check depends on estimated complexity
        }
        _ => panic!("Expected QueryCost response"),
    }
}

/// Test QueryBuilderProcessor suggestions
#[tokio::test]
async fn test_processor_get_suggestions() {
    let brp_client = create_test_brp_client();
    let processor = QueryBuilderProcessor::new(brp_client);

    let params = json!({
        "with": ["Transform"] // Broad query likely to generate suggestions
    });

    let cmd = DebugCommand::GetQuerySuggestions { params };
    let result = processor.process(cmd).await;
    assert!(result.is_ok());

    match result.unwrap() {
        DebugResponse::QuerySuggestions { suggestions, query_complexity } => {
            // Should have optimization suggestions for broad queries
            assert!(!suggestions.is_empty());
            assert!(query_complexity > 0);
        }
        _ => panic!("Expected QuerySuggestions response"),
    }
}

/// Test QueryBuilderProcessor parameter validation
#[tokio::test]
async fn test_processor_parameter_validation() {
    let brp_client = create_test_brp_client();
    let processor = QueryBuilderProcessor::new(brp_client);

    // Invalid parameters - not an object
    let invalid_cmd = DebugCommand::ValidateQuery { 
        params: json!("not an object")
    };
    let result = processor.validate(&invalid_cmd).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must be an object"));

    // Invalid 'with' parameter - not an array
    let invalid_with_cmd = DebugCommand::ValidateQuery { 
        params: json!({"with": "not an array"})
    };
    let result = processor.validate(&invalid_with_cmd).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must be an array"));
}

/// Test QueryBuilderProcessor processing time estimates
#[tokio::test]
async fn test_processor_timing_estimates() {
    let brp_client = create_test_brp_client();
    let processor = QueryBuilderProcessor::new(brp_client);

    let validate_cmd = DebugCommand::ValidateQuery { 
        params: json!({"with": ["Transform"]})
    };
    let cost_cmd = DebugCommand::EstimateCost { 
        params: json!({"with": ["Transform"]})
    };
    let suggestions_cmd = DebugCommand::GetQuerySuggestions { 
        params: json!({"with": ["Transform"]})
    };
    let build_exec_cmd = DebugCommand::BuildAndExecuteQuery { 
        params: json!({"with": ["Transform"]})
    };

    let validate_time = processor.estimate_processing_time(&validate_cmd);
    let cost_time = processor.estimate_processing_time(&cost_cmd);
    let suggestions_time = processor.estimate_processing_time(&suggestions_cmd);
    let build_exec_time = processor.estimate_processing_time(&build_exec_cmd);

    // BuildAndExecute should take longest (involves actual BRP execution)
    assert!(build_exec_time > validate_time);
    assert!(build_exec_time > cost_time);
    
    // All should be reasonable durations
    assert!(validate_time <= Duration::from_millis(50));
    assert!(cost_time <= Duration::from_millis(50));
    assert!(suggestions_time <= Duration::from_millis(50));
    assert!(build_exec_time <= Duration::from_millis(500));
}

/// Test QueryBuilderProcessor cache functionality
#[tokio::test]
async fn test_processor_cache_functionality() {
    let brp_client = create_test_brp_client();
    let processor = QueryBuilderProcessor::new(brp_client);

    let params = json!({
        "with": ["Transform", "Velocity"]
    });

    // First validation request
    let cmd1 = DebugCommand::ValidateQuery { params: params.clone() };
    let result1 = processor.process(cmd1).await;
    assert!(result1.is_ok());

    // Second identical validation request (should hit cache)
    let cmd2 = DebugCommand::ValidateQuery { params };
    let result2 = processor.process(cmd2).await;
    assert!(result2.is_ok());

    // Check cache stats
    let stats = processor.get_cache_stats().await;
    assert!(stats["validated_queries_cached"].as_u64().unwrap() > 0);
    // Total hit count might be > 0 if cache was hit
}

/// Test integration with DebugCommandRouter
#[tokio::test]
async fn test_debug_command_router_integration() {
    let brp_client = create_test_brp_client();
    let router = DebugCommandRouter::new();
    let processor = Arc::new(QueryBuilderProcessor::new(brp_client));

    router.register_processor("query_builder".to_string(), processor).await;

    let command = DebugCommand::ValidateQuery {
        params: json!({
            "with": ["Transform", "Velocity"],
            "limit": 10
        })
    };

    let request = DebugCommandRequest::new(
        command,
        Uuid::new_v4().to_string(),
        Some(5),
    );

    let result = router.queue_command(request).await;
    assert!(result.is_ok());

    // Processing should work
    let process_result = router.process_next().await;
    assert!(process_result.is_some());
    
    match process_result.unwrap() {
        Ok((correlation_id, response)) => {
            assert!(!correlation_id.is_empty());
            match response {
                DebugResponse::QueryValidation { valid, .. } => {
                    assert!(valid); // Should be valid query
                }
                _ => panic!("Expected QueryValidation response"),
            }
        }
        Err(e) => panic!("Processing failed: {}", e),
    }
}

/// Test query builder with component filters (advanced)
#[tokio::test]
async fn test_query_builder_with_component_filters() {
    let filter = ComponentFilter {
        component: "Transform".to_string(),
        field: Some("translation.x".to_string()),
        op: FilterOp::GreaterThan,
        value: json!(10.0),
    };

    let query = QueryBuilder::new()
        .with_component("Transform")
        .where_component(filter);

    assert_eq!(query.get_with_components(), &["Transform"]);
    assert_eq!(query.get_component_filters().len(), 1);
    assert_eq!(query.get_component_filters()[0].field, Some("translation.x".to_string()));
}

/// Test query performance with complex filters
#[tokio::test]
async fn test_complex_query_performance() {
    let mut query = QueryBuilder::new()
        .with_components(vec!["Transform", "Velocity", "Name"])
        .without_components(vec!["Camera", "DirectionalLight"]);

    // Add multiple component filters
    for i in 0..3 {
        let filter = ComponentFilter {
            component: "Transform".to_string(),
            field: Some(format!("field_{}", i)),
            op: FilterOp::Equal,
            value: json!(format!("value_{}", i)),
        };
        query = query.where_component(filter);
    }

    let cost = query.estimate_cost();
    let hints = query.get_optimization_hints();

    // Complex query should have higher cost
    assert!(cost.estimated_time_us > 1000); // At least 1ms

    // Should have optimization suggestions
    assert!(!hints.is_empty());
    let hints_text = hints.join(" ");
    assert!(hints_text.contains("simplifying") || hints_text.contains("complex"));
}

/// Performance test - large batch queries
#[tokio::test]
async fn test_batch_query_performance() {
    let brp_client = create_test_brp_client();
    let processor = QueryBuilderProcessor::new(brp_client);

    let start = std::time::Instant::now();

    // Process multiple validation requests quickly
    for i in 0..10 {
        let params = json!({
            "with": [format!("Component{}", i % 3)], // Rotate through components
            "limit": i * 10 + 10
        });

        let cmd = DebugCommand::ValidateQuery { params };
        let result = processor.process(cmd).await;
        assert!(result.is_ok());
    }

    let duration = start.elapsed();
    
    // Should complete batch reasonably quickly
    assert!(duration < Duration::from_millis(1000)); // Less than 1 second
}

/// Test pagination recommendations
#[tokio::test]
async fn test_pagination_recommendations() {
    let query = QueryBuilder::new()
        .with_component("Transform")
        .with_component("GlobalTransform"); // Broad query likely to return many results

    let mut cost_query = query.clone();
    let cost = cost_query.estimate_cost();
    let hints = query.get_optimization_hints();

    // Should suggest pagination for large result sets
    if cost.estimated_entities > 1000 {
        let hints_text = hints.join(" ");
        assert!(hints_text.contains("limit") || hints_text.contains("pagination"));
    }
}

/// Test query builder API compatibility
#[tokio::test]
async fn test_query_builder_api_compatibility() {
    // Test that QueryBuilder can be used to construct the same kinds of queries
    // as the existing ValidatedQuery structure

    let query = QueryBuilder::new()
        .with_component("Transform")
        .with_component("Velocity")
        .without_component("Camera")
        .limit(100)
        .offset(50);

    let validated = query.validate().unwrap();
    let filter = &validated.filter;

    // Check filter structure matches expected format
    assert!(filter.with.is_some());
    assert!(filter.without.is_some());
    assert_eq!(filter.with.as_ref().unwrap().len(), 2);
    assert_eq!(filter.without.as_ref().unwrap().len(), 1);

    // Check that all required fields are present
    assert!(!validated.id.is_empty());
    assert!(validated.estimated_cost.estimated_entities > 0);
    // Optimization hints are optional but often present
}

/// Test error message quality and helpfulness
#[tokio::test]
async fn test_error_message_quality() {
    // Test that error messages provide helpful suggestions
    let query = QueryBuilder::new()
        .with_component("Transformm"); // Typo in component name

    let result = query.validate();
    assert!(result.is_err());
    
    let error_msg = result.unwrap_err().to_string();
    
    // Error should be helpful
    assert!(error_msg.contains("Unknown component"));
    assert!(error_msg.contains("Transformm"));
    assert!(error_msg.contains("Available components") || error_msg.contains("Transform")); // Suggestions
}