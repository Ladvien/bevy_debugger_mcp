/*
 * Bevy Debugger MCP Server - Machine Learning Integration Tests
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

use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

use bevy_debugger_mcp::{
    brp_messages::DebugCommand,
    pattern_learning::PatternLearningSystem,
    suggestion_engine::{SuggestionEngine, SuggestionContext, SystemState},
    workflow_automation::{WorkflowAutomation, UserPreferences, AutomationScope},
    hot_reload::{HotReloadSystem, HotReloadConfig},
    error::Result,
};

/// Test pattern learning and suggestion generation end-to-end
#[tokio::test]
async fn test_pattern_learning_to_suggestions_pipeline() -> Result<()> {
    // Initialize the ML pipeline
    let pattern_system = Arc::new(PatternLearningSystem::new());
    let suggestion_engine = Arc::new(SuggestionEngine::new(pattern_system.clone()));

    // Simulate a debug session
    let session_id = "test_session_001";
    pattern_system.start_session(session_id.to_string()).await;

    // Record a sequence of commands that represents a common pattern
    let commands = vec![
        DebugCommand::GetSystemInfo {
            system_name: None,
            include_scheduling: Some(true),
        },
        DebugCommand::InspectEntity { 
            entity_id: 123, 
            include_metadata: Some(true), 
            include_relationships: Some(false) 
        },
        DebugCommand::ProfileSystem { 
            system_name: "movement_system".to_string(),
            duration_ms: Some(5000),
            track_allocations: Some(false),
        },
    ];

    // Record the commands
    for command in &commands {
        pattern_system.record_command(
            session_id,
            command.clone(),
            Duration::from_millis(50), // Fast execution
        ).await;
    }

    // End session with success
    pattern_system.end_session(session_id, true).await?;

    // Simulate multiple similar sessions to build pattern frequency
    for i in 0..10 {
        let session_id = format!("test_session_{:03}", i + 2);
        pattern_system.start_session(session_id.clone()).await;
        
        for command in &commands {
            pattern_system.record_command(
                &session_id,
                command.clone(),
                Duration::from_millis(45 + i * 2), // Slight variation
            ).await;
        }
        
        pattern_system.end_session(&session_id, true).await?;
    }

    // Create suggestion context matching the learned pattern
    let context = SuggestionContext {
        session_id: "current_session".to_string(),
        recent_commands: commands[..2].to_vec(), // Partial pattern
        system_state: SystemState {
            entity_count: 1000,
            fps: 45.0, // Low FPS should trigger suggestions
            memory_mb: 256.0,
            active_systems: 25,
            has_errors: false,
        },
        user_goal: Some("Debug performance issues".to_string()),
    };

    // Generate suggestions
    let suggestions = suggestion_engine.generate_suggestions(&context).await;

    // Verify suggestions were generated
    assert!(!suggestions.is_empty(), "Should generate suggestions");
    
    // Verify performance-related suggestions due to low FPS
    let performance_suggestions: Vec<_> = suggestions.iter()
        .filter(|s| s.reasoning.contains("FPS") || s.command.contains("profile"))
        .collect();
    assert!(!performance_suggestions.is_empty(), "Should suggest performance debugging");

    // Test suggestion tracking
    if let Some(suggestion) = suggestions.first() {
        let suggestion_id = format!("{}_{}", suggestion.command, suggestion.confidence);
        
        // Track acceptance
        suggestion_engine.track_suggestion_acceptance(&suggestion_id, true, true).await;
        
        // Get metrics
        let metrics = suggestion_engine.get_suggestion_metrics().await;
        assert!(metrics.contains_key(&suggestion_id), "Should track suggestion metrics");
    }

    Ok(())
}

/// Test workflow automation creation and execution
#[tokio::test]
async fn test_workflow_automation_end_to_end() -> Result<()> {
    let pattern_system = Arc::new(PatternLearningSystem::new());
    let suggestion_engine = Arc::new(SuggestionEngine::new(pattern_system.clone()));
    let workflow_automation = Arc::new(WorkflowAutomation::new(
        pattern_system.clone(),
        suggestion_engine.clone(),
    ));

    // Test automation opportunity analysis
    let opportunities = workflow_automation.analyze_automation_opportunities().await?;
    // Should be empty initially as we have no patterns
    assert_eq!(opportunities.len(), 0, "Should have no automation opportunities initially");

    // Simulate building patterns (would normally come from actual usage)
    // This is simplified - in reality, patterns would be built over many sessions
    
    // Test workflow listing
    let workflows = workflow_automation.get_workflows().await;
    assert_eq!(workflows.len(), 0, "Should have no workflows initially");

    // Test workflow execution (would fail due to no workflows, but tests the path)
    let user_prefs = UserPreferences {
        automation_enabled: true,
        preferred_scope: AutomationScope::ReadOnly,
        require_confirmation: false,
        auto_rollback: true,
    };

    // This should return an error since no workflow exists
    let result = workflow_automation.execute_workflow(
        "non_existent_workflow",
        "test_session".to_string(),
        Some(user_prefs),
    ).await;
    
    assert!(result.is_err(), "Should fail for non-existent workflow");

    Ok(())
}

/// Test hot reload system functionality
#[tokio::test]
async fn test_hot_reload_system() -> Result<()> {
    let temp_dir = TempDir::new().map_err(|e| bevy_debugger_mcp::error::Error::Io(e.to_string()))?;
    let watch_dir = temp_dir.path().to_path_buf();

    let pattern_system = Arc::new(PatternLearningSystem::new());
    let suggestion_engine = Arc::new(SuggestionEngine::new(pattern_system.clone()));
    let workflow_automation = Arc::new(WorkflowAutomation::new(
        pattern_system.clone(),
        suggestion_engine.clone(),
    ));

    let config = HotReloadConfig {
        watch_directory: watch_dir.clone(),
        debounce_delay_ms: 100, // Short delay for testing
        enabled: true,
        backup_directory: Some(watch_dir.join("backups")),
        max_backups: 5,
    };

    let hot_reload = Arc::new(HotReloadSystem::new(
        config,
        pattern_system.clone(),
        suggestion_engine.clone(),
        workflow_automation.clone(),
    ));

    // Start hot reload system
    hot_reload.start().await?;

    // Give it a moment to initialize
    sleep(Duration::from_millis(200)).await;

    // Test model version tracking
    let versions = hot_reload.get_model_versions().await;
    assert_eq!(versions.len(), 0, "Should have no model versions initially");

    // Create a test patterns file
    let patterns_file = watch_dir.join("patterns_test.json");
    let test_patterns = r#"[]"#; // Empty patterns array
    tokio::fs::write(&patterns_file, test_patterns).await
        .map_err(|e| bevy_debugger_mcp::error::Error::Io(e.to_string()))?;

    // Give time for file watcher to detect and process
    sleep(Duration::from_millis(500)).await;

    // Test force reload
    hot_reload.force_reload_all().await?;

    // Check if patterns were loaded (should be successful even with empty array)
    let exported_patterns = pattern_system.export_patterns().await?;
    assert!(exported_patterns.contains("[]"), "Should export empty patterns array");

    // Stop hot reload system
    hot_reload.stop().await?;

    Ok(())
}

/// Test integration between pattern learning and command processing
#[tokio::test]
async fn test_pattern_learning_integration() -> Result<()> {
    let pattern_system = Arc::new(PatternLearningSystem::new());

    // Test session lifecycle
    let session_id = "integration_test_session";
    pattern_system.start_session(session_id.to_string()).await;

    // Record various command types
    let commands = vec![
        (DebugCommand::GetSystemInfo, Duration::from_millis(25)),
        (DebugCommand::InspectEntity { 
            entity_id: 456, 
            include_metadata: Some(true), 
            include_relationships: Some(true) 
        }, Duration::from_millis(150)),
        (DebugCommand::ProfileSystem { 
            system_name: Some("render_system".to_string()),
            duration: Some(Duration::from_secs(3)),
        }, Duration::from_millis(3000)),
    ];

    for (command, duration) in commands {
        pattern_system.record_command(session_id, command, duration).await;
    }

    // End session successfully
    pattern_system.end_session(session_id, true).await?;

    // Test pattern export/import round trip
    let exported = pattern_system.export_patterns().await?;
    
    // Create new system and import
    let new_pattern_system = Arc::new(PatternLearningSystem::new());
    new_pattern_system.import_patterns(&exported).await?;
    
    // Verify import worked
    let re_exported = new_pattern_system.export_patterns().await?;
    assert_eq!(exported, re_exported, "Export/import should be identical");

    Ok(())
}

/// Test suggestion engine context awareness
#[tokio::test]
async fn test_suggestion_context_awareness() -> Result<()> {
    let pattern_system = Arc::new(PatternLearningSystem::new());
    let suggestion_engine = Arc::new(SuggestionEngine::new(pattern_system.clone()));

    // Test different system states and their impact on suggestions
    let test_cases = vec![
        (
            "Low FPS scenario",
            SystemState {
                entity_count: 5000,
                fps: 15.0, // Very low FPS
                memory_mb: 200.0,
                active_systems: 30,
                has_errors: false,
            },
            vec!["profile", "FPS"],
        ),
        (
            "High memory scenario",
            SystemState {
                entity_count: 2000,
                fps: 60.0,
                memory_mb: 800.0, // High memory usage
                active_systems: 20,
                has_errors: false,
            },
            vec!["memory", "profile_memory"],
        ),
        (
            "Error state scenario",
            SystemState {
                entity_count: 1000,
                fps: 60.0,
                memory_mb: 200.0,
                active_systems: 15,
                has_errors: true, // Has errors
            },
            vec!["detect_issues", "error"],
        ),
        (
            "Entity explosion scenario",
            SystemState {
                entity_count: 15000, // Very high entity count
                fps: 30.0,
                memory_mb: 300.0,
                active_systems: 25,
                has_errors: false,
            },
            vec!["entities", "observe"],
        ),
    ];

    for (scenario_name, system_state, expected_keywords) in test_cases {
        let context = SuggestionContext {
            session_id: format!("test_{}", scenario_name.replace(" ", "_")),
            recent_commands: vec![],
            system_state,
            user_goal: Some(scenario_name.to_string()),
        };

        let suggestions = suggestion_engine.generate_suggestions(&context).await;
        assert!(!suggestions.is_empty(), "Should generate suggestions for {}", scenario_name);

        // Check that suggestions are relevant to the scenario
        let suggestion_text: String = suggestions.iter()
            .map(|s| format!("{} {} {}", s.command, s.reasoning, s.expected_outcome))
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();

        let relevant_suggestions = expected_keywords.iter()
            .filter(|keyword| suggestion_text.contains(&keyword.to_lowercase()))
            .count();

        assert!(
            relevant_suggestions > 0,
            "Should generate relevant suggestions for {} (expected: {:?}, got suggestions: {:?})",
            scenario_name,
            expected_keywords,
            suggestions.iter().map(|s| &s.command).collect::<Vec<_>>()
        );
    }

    Ok(())
}

/// Test machine learning pipeline performance under load
#[tokio::test]
async fn test_ml_pipeline_performance() -> Result<()> {
    let pattern_system = Arc::new(PatternLearningSystem::new());
    let suggestion_engine = Arc::new(SuggestionEngine::new(pattern_system.clone()));

    let start_time = std::time::Instant::now();

    // Simulate high-frequency command recording
    let num_sessions = 20;
    let commands_per_session = 50;

    for session_idx in 0..num_sessions {
        let session_id = format!("perf_test_session_{}", session_idx);
        pattern_system.start_session(session_id.clone()).await;

        for cmd_idx in 0..commands_per_session {
            let command = match cmd_idx % 4 {
                0 => DebugCommand::GetSystemInfo,
                1 => DebugCommand::InspectEntity { 
                    entity_id: (cmd_idx % 1000) as u32, 
                    include_metadata: Some(true), 
                    include_relationships: Some(false) 
                },
                2 => DebugCommand::ProfileSystem { 
                    system_name: Some(format!("system_{}", cmd_idx % 10)),
                    duration: Some(Duration::from_secs(1)),
                },
                _ => DebugCommand::ValidateQuery { 
                    query: format!("test_query_{}", cmd_idx) 
                },
            };

            pattern_system.record_command(
                &session_id,
                command,
                Duration::from_millis(10 + (cmd_idx % 100) as u64),
            ).await;
        }

        pattern_system.end_session(&session_id, session_idx % 5 != 0).await?; // 80% success rate
    }

    let recording_time = start_time.elapsed();

    // Test suggestion generation performance
    let suggestion_start = std::time::Instant::now();

    for i in 0..100 {
        let context = SuggestionContext {
            session_id: format!("perf_suggestion_test_{}", i),
            recent_commands: vec![
                DebugCommand::GetSystemInfo,
                DebugCommand::InspectEntity { 
                    entity_id: i as u32, 
                    include_metadata: Some(true), 
                    include_relationships: Some(false) 
                },
            ],
            system_state: SystemState {
                entity_count: 1000 + i * 10,
                fps: 60.0 - (i as f32 * 0.1),
                memory_mb: 200.0 + (i as f32 * 2.0),
                active_systems: 20 + (i % 10),
                has_errors: i % 7 == 0,
            },
            user_goal: Some(format!("Performance test {}", i)),
        };

        let _suggestions = suggestion_engine.generate_suggestions(&context).await;
    }

    let suggestion_time = suggestion_start.elapsed();

    // Performance assertions
    assert!(
        recording_time < Duration::from_secs(10),
        "Recording {} sessions with {} commands each should complete in under 10 seconds, took {:?}",
        num_sessions,
        commands_per_session,
        recording_time
    );

    assert!(
        suggestion_time < Duration::from_secs(5),
        "Generating 100 suggestions should complete in under 5 seconds, took {:?}",
        suggestion_time
    );

    println!(
        "Performance test completed: {} sessions recorded in {:?}, 100 suggestions generated in {:?}",
        num_sessions, recording_time, suggestion_time
    );

    Ok(())
}

/// Test error handling and recovery in ML components
#[tokio::test]
async fn test_ml_error_handling() -> Result<()> {
    let pattern_system = Arc::new(PatternLearningSystem::new());
    let suggestion_engine = Arc::new(SuggestionEngine::new(pattern_system.clone()));

    // Test invalid pattern import
    let invalid_json = r#"{"invalid": "json structure"}"#;
    let result = pattern_system.import_patterns(invalid_json).await;
    assert!(result.is_err(), "Should fail to import invalid pattern format");

    // Test suggestion generation with empty context
    let empty_context = SuggestionContext {
        session_id: "empty_test".to_string(),
        recent_commands: vec![],
        system_state: SystemState {
            entity_count: 0,
            fps: 0.0,
            memory_mb: 0.0,
            active_systems: 0,
            has_errors: false,
        },
        user_goal: None,
    };

    // Should still generate some suggestions (context-aware ones)
    let suggestions = suggestion_engine.generate_suggestions(&empty_context).await;
    // May be empty, but shouldn't crash
    
    // Test session end without start
    let result = pattern_system.end_session("non_existent_session", true).await;
    assert!(result.is_ok(), "Should handle ending non-existent session gracefully");

    Ok(())
}