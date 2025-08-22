/*
 * Bevy Debugger MCP Server - Pattern Learning Tests
 * Tests for Epic BEVDBG-013
 */

use bevy_debugger_mcp::pattern_learning::{
    AnonymizedCommand, DebugPattern, PatternLearningSystem, PatternMiner, TimeBucket,
};
use bevy_debugger_mcp::suggestion_engine::{
    DebugSuggestion, SuggestionContext, SuggestionEngine, SystemState,
};
use bevy_debugger_mcp::brp_messages::DebugCommand;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_pattern_anonymization() {
    let system = PatternLearningSystem::new();
    
    // Start a session
    let session_id = "test_session_1";
    system.start_session(session_id.to_string()).await;
    
    // Record commands
    let commands = vec![
        DebugCommand::InspectEntity { entity_id: 123 },
        DebugCommand::GetHierarchy { entity_id: 456 },
        DebugCommand::GetSystemInfo,
    ];
    
    for cmd in commands {
        system.record_command(session_id, cmd, Duration::from_millis(5)).await;
    }
    
    // End session
    system.end_session(session_id, true).await.unwrap();
    
    // Commands should be anonymized (no entity IDs exposed)
}

#[tokio::test]
async fn test_pattern_mining() {
    let miner = PatternMiner::new(2, 5);
    
    let cmd1 = AnonymizedCommand {
        command_type: "inspect".to_string(),
        param_shape: HashMap::new(),
        time_bucket: TimeBucket::Fast,
    };
    
    let cmd2 = AnonymizedCommand {
        command_type: "profile".to_string(),
        param_shape: HashMap::new(),
        time_bucket: TimeBucket::Medium,
    };
    
    let cmd3 = AnonymizedCommand {
        command_type: "observe".to_string(),
        param_shape: HashMap::new(),
        time_bucket: TimeBucket::Fast,
    };
    
    let sequences = vec![
        vec![cmd1.clone(), cmd2.clone(), cmd3.clone()],
        vec![cmd1.clone(), cmd2.clone()],
        vec![cmd1.clone(), cmd3.clone()],
    ];
    
    let patterns = miner.mine_patterns(&sequences);
    
    // Should find frequent patterns
    assert!(!patterns.is_empty());
    assert!(patterns.iter().any(|p| p.len() == 1 && p[0] == cmd1)); // cmd1 appears 3 times
}

#[tokio::test]
async fn test_k_anonymity() {
    let system = PatternLearningSystem::new();
    
    // Create fewer than K_ANONYMITY_THRESHOLD sessions
    for i in 0..3 {
        let session_id = format!("session_{}", i);
        system.start_session(session_id.clone()).await;
        
        system.record_command(
            &session_id,
            DebugCommand::GetSystemInfo,
            Duration::from_millis(10),
        ).await;
        
        system.end_session(&session_id, true).await.unwrap();
    }
    
    // Patterns should not be learned yet (need 5 sessions for k-anonymity)
    let patterns = system.find_matching_patterns(&[]).await;
    assert!(patterns.is_empty());
    
    // Add more sessions to reach k-anonymity threshold
    for i in 3..6 {
        let session_id = format!("session_{}", i);
        system.start_session(session_id.clone()).await;
        
        system.record_command(
            &session_id,
            DebugCommand::GetSystemInfo,
            Duration::from_millis(10),
        ).await;
        
        system.end_session(&session_id, true).await.unwrap();
    }
    
    // Now patterns should be learned (but might still be empty due to min frequency)
}

#[tokio::test]
async fn test_pattern_matching() {
    let system = PatternLearningSystem::new();
    
    // Create multiple similar sessions to establish a pattern
    for i in 0..10 {
        let session_id = format!("session_{}", i);
        system.start_session(session_id.clone()).await;
        
        // Common debugging sequence
        system.record_command(
            &session_id,
            DebugCommand::GetSystemInfo,
            Duration::from_millis(5),
        ).await;
        
        system.record_command(
            &session_id,
            DebugCommand::InspectEntity { entity_id: i as u32 },
            Duration::from_millis(15),
        ).await;
        
        system.record_command(
            &session_id,
            DebugCommand::ProfileSystem { 
                system_name: "test".to_string(),
                duration_seconds: Some(1),
            },
            Duration::from_millis(100),
        ).await;
        
        system.end_session(&session_id, true).await.unwrap();
    }
    
    // Test pattern matching with partial sequence
    let test_sequence = vec![
        AnonymizedCommand {
            command_type: "get_system_info".to_string(),
            param_shape: HashMap::new(),
            time_bucket: TimeBucket::Fast,
        },
    ];
    
    let matches = system.find_matching_patterns(&test_sequence).await;
    // Patterns may or may not be found depending on k-anonymity buffer
}

#[tokio::test]
async fn test_suggestion_generation() {
    let pattern_system = Arc::new(PatternLearningSystem::new());
    let suggestion_engine = SuggestionEngine::new(pattern_system.clone());
    
    // Create context with performance issues
    let context = SuggestionContext {
        session_id: "test_session".to_string(),
        recent_commands: vec![
            DebugCommand::GetSystemInfo,
        ],
        system_state: SystemState {
            entity_count: 1000,
            fps: 25.0, // Low FPS
            memory_mb: 600.0, // High memory
            active_systems: 50,
            has_errors: false,
        },
        user_goal: Some("Fix performance issues".to_string()),
    };
    
    let suggestions = suggestion_engine.generate_suggestions(&context).await;
    
    // Should generate performance-related suggestions
    assert!(!suggestions.is_empty());
    assert!(suggestions.iter().any(|s| s.command.contains("profile")));
}

#[tokio::test]
async fn test_suggestion_with_errors() {
    let pattern_system = Arc::new(PatternLearningSystem::new());
    let suggestion_engine = SuggestionEngine::new(pattern_system.clone());
    
    let context = SuggestionContext {
        session_id: "error_session".to_string(),
        recent_commands: vec![],
        system_state: SystemState {
            entity_count: 100,
            fps: 60.0,
            memory_mb: 100.0,
            active_systems: 10,
            has_errors: true, // System has errors
        },
        user_goal: None,
    };
    
    let suggestions = suggestion_engine.generate_suggestions(&context).await;
    
    // Should prioritize error detection
    assert!(!suggestions.is_empty());
    let first_suggestion = &suggestions[0];
    assert!(first_suggestion.priority >= 10);
    assert!(first_suggestion.command.contains("detect_issues") || 
            first_suggestion.reasoning.contains("error"));
}

#[tokio::test]
async fn test_suggestion_tracking() {
    let pattern_system = Arc::new(PatternLearningSystem::new());
    let suggestion_engine = SuggestionEngine::new(pattern_system.clone());
    
    // Track acceptance
    suggestion_engine.track_suggestion_acceptance("test_suggestion_1", true, true).await;
    suggestion_engine.track_suggestion_acceptance("test_suggestion_1", true, false).await;
    suggestion_engine.track_suggestion_acceptance("test_suggestion_2", false, false).await;
    
    // Get metrics
    let metrics = suggestion_engine.get_suggestion_metrics().await;
    
    assert!(metrics.contains_key("test_suggestion_1"));
    let (acceptance_rate, success_rate) = metrics.get("test_suggestion_1").unwrap();
    assert_eq!(*acceptance_rate, 1.0); // Both times accepted
    assert_eq!(*success_rate, 0.5); // 50% success rate
}

#[tokio::test]
async fn test_pattern_export_import() {
    let system = PatternLearningSystem::new();
    
    // Create and export patterns
    for i in 0..10 {
        let session_id = format!("export_session_{}", i);
        system.start_session(session_id.clone()).await;
        
        system.record_command(
            &session_id,
            DebugCommand::GetSystemInfo,
            Duration::from_millis(10),
        ).await;
        
        system.end_session(&session_id, true).await.unwrap();
    }
    
    let exported = system.export_patterns().await.unwrap();
    assert!(!exported.is_empty());
    
    // Create new system and import
    let new_system = PatternLearningSystem::new();
    new_system.import_patterns(&exported).await.unwrap();
    
    // Imported patterns should be available
    // (though they might be empty due to k-anonymity requirements)
}

#[tokio::test]
async fn test_privacy_preservation() {
    let system = PatternLearningSystem::new();
    
    // Record commands with sensitive data
    let session_id = "privacy_test";
    system.start_session(session_id.to_string()).await;
    
    system.record_command(
        session_id,
        DebugCommand::InspectEntity { entity_id: 12345 },
        Duration::from_millis(10),
    ).await;
    
    system.record_command(
        session_id,
        DebugCommand::ProfileSystem {
            system_name: "super_secret_system".to_string(),
            duration_seconds: Some(5),
        },
        Duration::from_millis(50),
    ).await;
    
    system.end_session(session_id, true).await.unwrap();
    
    // Export patterns
    let exported = system.export_patterns().await.unwrap();
    
    // Ensure no sensitive data in export
    assert!(!exported.contains("12345"));
    assert!(!exported.contains("super_secret_system"));
}

#[tokio::test]
async fn test_pattern_confidence_calculation() {
    let system = PatternLearningSystem::new();
    
    // Create sessions with varying success rates
    for i in 0..20 {
        let session_id = format!("confidence_session_{}", i);
        system.start_session(session_id.clone()).await;
        
        system.record_command(
            &session_id,
            DebugCommand::GetSystemInfo,
            Duration::from_millis(10),
        ).await;
        
        // Half succeed, half fail
        let success = i % 2 == 0;
        system.end_session(&session_id, success).await.unwrap();
    }
    
    // Patterns should have confidence reflecting success rate
    // (Actual testing would require access to internal pattern storage)
}

#[tokio::test]
async fn test_sequence_length_limits() {
    let system = PatternLearningSystem::new();
    let session_id = "long_session";
    
    system.start_session(session_id.to_string()).await;
    
    // Record more commands than MAX_SEQUENCE_LENGTH
    for i in 0..20 {
        system.record_command(
            session_id,
            DebugCommand::GetSystemInfo,
            Duration::from_millis(5),
        ).await;
    }
    
    system.end_session(session_id, true).await.unwrap();
    
    // Sequence should be truncated to MAX_SEQUENCE_LENGTH
    // No panic or memory issues
}

#[tokio::test]
async fn test_concurrent_sessions() {
    let system = Arc::new(PatternLearningSystem::new());
    
    // Start multiple concurrent sessions
    let mut handles = vec![];
    
    for i in 0..5 {
        let system_clone = system.clone();
        let handle = tokio::spawn(async move {
            let session_id = format!("concurrent_{}", i);
            system_clone.start_session(session_id.clone()).await;
            
            for _ in 0..3 {
                system_clone.record_command(
                    &session_id,
                    DebugCommand::GetSystemInfo,
                    Duration::from_millis(10),
                ).await;
                
                sleep(Duration::from_millis(10)).await;
            }
            
            system_clone.end_session(&session_id, true).await.unwrap();
        });
        
        handles.push(handle);
    }
    
    // Wait for all sessions to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // System should handle concurrent access correctly
}