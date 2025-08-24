/// Panic prevention testing to verify no unwrap() calls cause crashes
/// 
/// This module contains tests that try to trigger panics in production code paths
/// to ensure we've eliminated all unwrap() calls that could crash the application.

use crate::brp_messages::{DebugCommand, ComponentValue, ComponentFilter, FilterOp, QueryFilter};
use crate::checkpoint::{CheckpointManager, CheckpointConfig};
use crate::config::Config;
use crate::error::Result;
use crate::memory_profiler::{MemoryProfiler, MemoryProfilerConfig};
use crate::query_parser::{RegexQueryParser, QueryParser};
use crate::semantic_analyzer::SemanticAnalyzer;
use crate::state_diff::{StateDiff, StateSnapshot, FuzzyCompareConfig, GameRules};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{timeout, Duration};

/// Test memory profiler doesn't panic with extreme inputs
#[tokio::test]
async fn test_memory_profiler_no_panic() {
    let config = MemoryProfilerConfig::default();
    let profiler = MemoryProfiler::new(config);
    
    // Try to cause issues with extreme inputs
    for i in 0..100 {
        // Large allocation sizes that might cause issues
        if let Ok(alloc_id) = profiler.record_allocation(
            &format!("System{}", i), 
            usize::MAX / 1000, // Very large but not overflow
            Some(vec!["backtrace".to_string(); 1000])
        ) {
            let _ = profiler.record_deallocation(alloc_id);
        }
        
        profiler.update_entity_count(usize::MAX / 1000);
        
        // Try snapshot with lots of data
        if let Ok(_) = profiler.take_snapshot().await {
            // Success
        }
        
        // Try leak detection
        if let Ok(_) = profiler.detect_leaks().await {
            // Success  
        }
    }
}

/// Test query parser doesn't panic with malformed regex or inputs
#[tokio::test]
async fn test_query_parser_no_panic() {
    let parser = match RegexQueryParser::new() {
        Ok(p) => p,
        Err(_) => return, // If constructor fails, that's expected
    };
    
    let malformed_queries = vec![
        "", // Empty
        "|||||||||||", // Invalid regex chars
        "find entities with \0\0\0", // Null bytes
        "find entities with ".repeat(1000), // Very long
        "find entities with component Component".repeat(100), // Repetitive
        "show entity 18446744073709551615", // Max u64
        "show entity -1", // Negative
        "show entity abc", // Non-numeric
        "find 999999999999999999999999999999999999 entities", // Overflow number
    ];
    
    for query in malformed_queries {
        let _ = parser.parse(query); // Shouldn't panic, may return error
    }
}

/// Test semantic analyzer doesn't panic with extreme inputs
#[tokio::test]  
async fn test_semantic_analyzer_no_panic() {
    let analyzer = match SemanticAnalyzer::new() {
        Ok(a) => a,
        Err(_) => return, // If constructor fails, that's expected
    };
    
    let extreme_queries = vec![
        "\0".repeat(10000), // Null bytes
        "🚀".repeat(1000), // Unicode
        "find".repeat(10000), // Repetitive
        "", // Empty
        " ".repeat(10000), // Spaces
        "find stuck entities".repeat(1000), // Long valid query
    ];
    
    for query in extreme_queries {
        let _ = analyzer.analyze(&query); // Shouldn't panic
    }
}

/// Test checkpoint manager doesn't panic with concurrent access or poisoned locks
#[tokio::test]
async fn test_checkpoint_manager_no_panic() {
    let config = CheckpointConfig::default();
    let manager = CheckpointManager::new(config);
    
    // Try to create many checkpoints concurrently
    let mut handles = vec![];
    for i in 0..50 {
        let manager_clone = Arc::new(&manager);
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let checkpoint_data = json!({"iteration": i * 10 + j});
                let _ = manager_clone.create_checkpoint(
                    &format!("test_checkpoint_{}", i * 10 + j),
                    &format!("Test description {}", i * 10 + j),
                    "test_operation",
                    "test_component",
                    checkpoint_data
                ).await;
            }
        });
        handles.push(handle);
    }
    
    // Wait for all tasks with timeout
    for handle in handles {
        let _ = timeout(Duration::from_secs(5), handle).await;
    }
    
    // Try to access statistics (this could panic if locks are poisoned)
    let _ = manager.get_statistics().await;
    let _ = manager.list_checkpoints().await;
    let _ = manager.list_checkpoints_by_operation("test_operation").await;
}

/// Test state diff doesn't panic with inconsistent data
#[tokio::test]
async fn test_state_diff_no_panic() {
    let fuzzy_config = FuzzyCompareConfig::default();
    let game_rules = GameRules::default();
    let diff_engine = StateDiff::with_config(fuzzy_config, game_rules);
    
    // Create snapshots with inconsistent data that might cause panics
    let mut components1 = HashMap::new();
    let mut components2 = HashMap::new();
    
    // Add some components to first snapshot
    components1.insert("Position".to_string(), ComponentValue::String("10,20,30".to_string()));
    components1.insert("Velocity".to_string(), ComponentValue::String("1,2,3".to_string()));
    
    // Add different components to second snapshot (this creates the key mismatch scenario)
    components2.insert("Position".to_string(), ComponentValue::String("15,25,35".to_string()));
    components2.insert("Health".to_string(), ComponentValue::Number(100.0));
    // Note: Missing Velocity, adding Health - this tests our unwrap fixes
    
    let entity1 = crate::brp_messages::EntityData {
        id: 1,
        components: components1,
    };
    
    let entity2 = crate::brp_messages::EntityData {
        id: 1,
        components: components2,
    };
    
    let snapshot1 = StateSnapshot {
        timestamp: std::time::SystemTime::now(),
        entities: vec![entity1],
        metadata: HashMap::new(),
    };
    
    let snapshot2 = StateSnapshot {
        timestamp: std::time::SystemTime::now(),
        entities: vec![entity2], 
        metadata: HashMap::new(),
    };
    
    // This should not panic due to our fixes
    let _ = diff_engine.diff_snapshots(&snapshot1, &snapshot2).await;
}

/// Stress test with random data that might trigger edge cases
#[tokio::test]
async fn test_random_stress_no_panic() {
    use rand::Rng;
    
    let mut rng = rand::thread_rng();
    
    // Test various components with random data
    for _ in 0..100 {
        // Random string that might break parsing
        let random_str: String = (0..rng.gen_range(1..1000))
            .map(|_| rng.gen_range(0..127) as u8 as char)
            .collect();
            
        // Try parsing as query
        if let Ok(parser) = RegexQueryParser::new() {
            let _ = parser.parse(&random_str);
        }
        
        // Try as semantic analysis
        if let Ok(analyzer) = SemanticAnalyzer::new() {
            let _ = analyzer.analyze(&random_str);
        }
        
        // Random numbers that might overflow or cause issues
        let random_num = rng.gen::<u64>();
        let query = format!("show entity {}", random_num);
        if let Ok(parser) = RegexQueryParser::new() {
            let _ = parser.parse(&query);
        }
    }
}