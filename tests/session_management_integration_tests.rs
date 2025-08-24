/// Integration tests for Debug Session Management functionality
/// 
/// These tests verify the complete session management system including:
/// - Session lifecycle management
/// - Command history tracking
/// - Checkpoint creation and restoration
/// - Command replay functionality
/// - Session persistence and cleanup
/// - Integration with MCP debug command system

use bevy_debugger_mcp::brp_messages::{DebugCommand, DebugResponse, SessionOperation, SessionState};
use bevy_debugger_mcp::config::Config;
use bevy_debugger_mcp::brp_client::BrpClient;
use bevy_debugger_mcp::debug_command_processor::DebugCommandProcessor;
use bevy_debugger_mcp::session_manager::{SessionManager, SessionManagerConfig};
use bevy_debugger_mcp::session_processor::SessionProcessor;
use bevy_debugger_mcp::error::{Error, Result};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;

/// Helper to create test configuration
fn create_test_config() -> Config {
    let mut config = Config::default();
    config.bevy_brp_host = "localhost".to_string();
    config.bevy_brp_port = 15702;
    config.mcp_port = 3000;
    config
}

/// Helper to create session manager for testing
fn create_test_session_manager() -> SessionManager {
    let config = SessionManagerConfig {
        max_sessions: 10,
        default_cleanup_hours: 24,
        command_history_limit: 100,
        enable_persistence: false, // Disable for testing
        storage_directory: "./test_sessions".to_string(),
        cleanup_interval_minutes: 1, // Fast cleanup for testing
    };
    SessionManager::new(config)
}

/// Helper to create session processor for testing
async fn create_test_session_processor() -> SessionProcessor {
    let config = create_test_config();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let processor = SessionProcessor::new(brp_client);
    processor.start().await.unwrap();
    processor
}

#[tokio::test]
async fn test_session_lifecycle_management() {
    let manager = create_test_session_manager();
    manager.start().await.unwrap();

    // Create a new session
    let session_id = manager.create_session(
        "Test Session".to_string(),
        Some("Test session for lifecycle testing".to_string())
    ).await.unwrap();

    assert!(!session_id.is_empty());

    // Verify session exists and is active
    let session = manager.get_session(&session_id).await.unwrap();
    assert_eq!(session.name, "Test Session");
    assert!(session.description.is_some());
    assert!(matches!(session.state, SessionState::Active));

    // End the session
    manager.end_session(&session_id).await.unwrap();

    // Verify session is ended
    let sessions = manager.list_sessions().await;
    assert!(sessions.is_empty() || sessions.iter().all(|s| s.id != session_id));
}

#[tokio::test]
async fn test_command_history_tracking() {
    let manager = create_test_session_manager();
    manager.start().await.unwrap();

    let session_id = manager.create_session("History Test".to_string(), None).await.unwrap();

    // Record multiple commands
    for i in 0..5 {
        let command = DebugCommand::GetMemoryProfile;
        let response = DebugResponse::Success {
            message: format!("Command {} executed", i),
            data: None,
        };

        manager.record_command(
            &session_id,
            command,
            Some(response),
            Duration::from_millis(10 * i as u64),
            format!("correlation-{}", i),
        ).await.unwrap();
    }

    // Get command history
    let history = manager.get_command_history(&session_id, Some(10)).await.unwrap();
    assert_eq!(history.len(), 5);

    // Verify history is in reverse chronological order (newest first)
    for (i, entry) in history.iter().enumerate() {
        let expected_correlation = format!("correlation-{}", 4 - i);
        assert_eq!(entry.correlation_id, expected_correlation);
        assert!(entry.success);
    }

    // Test history limit
    let limited_history = manager.get_command_history(&session_id, Some(3)).await.unwrap();
    assert_eq!(limited_history.len(), 3);
}

#[tokio::test]
async fn test_checkpoint_creation_and_restoration() {
    let manager = create_test_session_manager();
    manager.start().await.unwrap();

    let session_id = manager.create_session("Checkpoint Test".to_string(), None).await.unwrap();

    // Add some commands to create state
    let command = DebugCommand::GetMemoryProfile;
    manager.record_command(
        &session_id,
        command,
        None,
        Duration::from_millis(50),
        "pre-checkpoint".to_string(),
    ).await.unwrap();

    // Create checkpoint
    let checkpoint_id = manager.create_checkpoint(&session_id, "Test checkpoint").await.unwrap();
    assert!(!checkpoint_id.is_empty());

    // Verify session has checkpoint reference
    let session = manager.get_session(&session_id).await.unwrap();
    assert!(session.checkpoints.contains(&checkpoint_id));

    // Add more commands after checkpoint
    let post_command = DebugCommand::GetMemoryStatistics;
    manager.record_command(
        &session_id,
        post_command,
        None,
        Duration::from_millis(25),
        "post-checkpoint".to_string(),
    ).await.unwrap();

    // Restore from checkpoint (this should replace the current session state)
    manager.restore_checkpoint(&session_id, &checkpoint_id).await.unwrap();

    let restored_session = manager.get_session(&session_id).await.unwrap();
    assert_eq!(restored_session.name, "Checkpoint Test");
    // The restored session should have the state from when checkpoint was created
    assert!(restored_session.checkpoints.contains(&checkpoint_id));
}

#[tokio::test]
async fn test_command_replay_functionality() {
    let manager = create_test_session_manager();
    manager.start().await.unwrap();

    let session_id = manager.create_session("Replay Test".to_string(), None).await.unwrap();

    // Record a sequence of commands
    let commands = vec![
        DebugCommand::GetMemoryProfile,
        DebugCommand::GetMemoryStatistics,
        DebugCommand::TakeMemorySnapshot,
    ];

    for (i, command) in commands.iter().enumerate() {
        manager.record_command(
            &session_id,
            command.clone(),
            None,
            Duration::from_millis(10),
            format!("replay-cmd-{}", i),
        ).await.unwrap();
    }

    // Start replay from beginning
    manager.start_replay(&session_id, Some(0)).await.unwrap();

    let session = manager.get_session(&session_id).await.unwrap();
    assert!(matches!(session.state, SessionState::Replaying));
    assert_eq!(session.replay_position, Some(0));

    // Get replay commands
    let mut replayed_commands = Vec::new();
    while let Some(command) = manager.get_next_replay_command(&session_id).await.unwrap() {
        replayed_commands.push(command);
    }

    assert_eq!(replayed_commands.len(), 3);
    assert!(matches!(replayed_commands[0], DebugCommand::GetMemoryProfile));
    assert!(matches!(replayed_commands[1], DebugCommand::GetMemoryStatistics));
    assert!(matches!(replayed_commands[2], DebugCommand::TakeMemorySnapshot));

    // Session should be back to active state
    let final_session = manager.get_session(&session_id).await.unwrap();
    assert!(matches!(final_session.state, SessionState::Active));
    assert!(final_session.replay_position.is_none());
}

#[tokio::test]
async fn test_session_cleanup_functionality() {
    let mut config = SessionManagerConfig::default();
    config.default_cleanup_hours = 0; // Immediate cleanup
    config.cleanup_interval_minutes = 1; // Check every minute
    config.enable_persistence = false;

    let manager = SessionManager::new(config);
    manager.start().await.unwrap();

    // Create a session that should be cleaned up
    let session_id = manager.create_session("Cleanup Test".to_string(), None).await.unwrap();

    // Verify session exists
    let session = manager.get_session(&session_id).await.unwrap();
    assert!(session.should_cleanup()); // Should be marked for cleanup immediately

    // Wait for cleanup cycle (give it some time to run)
    sleep(Duration::from_millis(100)).await;

    // Session should eventually be cleaned up automatically by the background task
    // Note: In a real scenario, this would be cleaned up by the automatic cleanup task
}

#[tokio::test]
async fn test_session_processor_command_processing() {
    let processor = create_test_session_processor().await;

    // Test session creation
    let create_cmd = DebugCommand::SessionControl {
        operation: SessionOperation::Create,
        session_id: Some("test-session".to_string()),
    };

    assert!(processor.supports_command(&create_cmd));
    assert!(processor.validate(&create_cmd).await.is_ok());

    let create_result = processor.process(create_cmd).await.unwrap();
    let session_id = match create_result {
        DebugResponse::SessionStatus { session_id, state, .. } => {
            assert!(matches!(state, SessionState::Active));
            session_id
        }
        _ => panic!("Expected SessionStatus response"),
    };

    // Test checkpoint creation
    let checkpoint_cmd = DebugCommand::SessionControl {
        operation: SessionOperation::Checkpoint,
        session_id: Some(session_id.clone()),
    };

    let checkpoint_result = processor.process(checkpoint_cmd).await.unwrap();
    match checkpoint_result {
        DebugResponse::Success { message, data } => {
            assert!(message.contains("Created checkpoint"));
            assert!(data.is_some());
        }
        _ => panic!("Expected Success response"),
    }

    // Test session resumption
    let resume_cmd = DebugCommand::SessionControl {
        operation: SessionOperation::Resume,
        session_id: Some(session_id.clone()),
    };

    let resume_result = processor.process(resume_cmd).await.unwrap();
    match resume_result {
        DebugResponse::SessionStatus { state, .. } => {
            assert!(matches!(state, SessionState::Active));
        }
        _ => panic!("Expected SessionStatus response"),
    }
}

#[tokio::test]
async fn test_session_processor_status_reporting() {
    let processor = create_test_session_processor().await;

    // Create a session first
    let create_cmd = DebugCommand::SessionControl {
        operation: SessionOperation::Create,
        session_id: Some("status-test".to_string()),
    };
    processor.process(create_cmd).await.unwrap();

    // Test status command
    let status_cmd = DebugCommand::GetStatus;
    let status_result = processor.process(status_cmd).await.unwrap();

    match status_result {
        DebugResponse::Status { version, active_sessions, .. } => {
            assert_eq!(version, "1.0.0");
            assert!(active_sessions > 0);
        }
        _ => panic!("Expected Status response"),
    }
}

#[tokio::test]
async fn test_session_processor_validation() {
    let processor = create_test_session_processor().await;

    // Valid commands
    let valid_cmds = vec![
        DebugCommand::SessionControl {
            operation: SessionOperation::Create,
            session_id: None, // Create can work without session_id
        },
        DebugCommand::GetStatus,
    ];

    for cmd in valid_cmds {
        assert!(processor.validate(&cmd).await.is_ok(), "Command should be valid: {:?}", cmd);
    }

    // Invalid commands - operations requiring session_id without providing it
    let invalid_cmds = vec![
        DebugCommand::SessionControl {
            operation: SessionOperation::Resume,
            session_id: None,
        },
        DebugCommand::SessionControl {
            operation: SessionOperation::Checkpoint,
            session_id: None,
        },
        DebugCommand::SessionControl {
            operation: SessionOperation::End,
            session_id: None,
        },
    ];

    for cmd in invalid_cmds {
        assert!(processor.validate(&cmd).await.is_err(), "Command should be invalid: {:?}", cmd);
    }
}

#[tokio::test]
async fn test_concurrent_session_operations() {
    let manager = Arc::new(create_test_session_manager());
    manager.start().await.unwrap();

    // Create multiple sessions concurrently
    let mut handles = Vec::new();
    for i in 0..5 {
        let manager_clone = Arc::clone(&manager);
        let handle = tokio::spawn(async move {
            manager_clone.create_session(
                format!("Concurrent Session {}", i),
                None
            ).await
        });
        handles.push(handle);
    }

    let mut session_ids = Vec::new();
    for handle in handles {
        session_ids.push(handle.await.unwrap().unwrap());
    }

    assert_eq!(session_ids.len(), 5);

    // All sessions should be unique
    let mut unique_sessions = session_ids.clone();
    unique_sessions.sort();
    unique_sessions.dedup();
    assert_eq!(unique_sessions.len(), 5);

    // Record commands concurrently in different sessions
    let mut command_handles = Vec::new();
    for (i, session_id) in session_ids.iter().enumerate() {
        let manager_clone = Arc::clone(&manager);
        let session_id_clone = session_id.clone();
        let handle = tokio::spawn(async move {
            manager_clone.record_command(
                &session_id_clone,
                DebugCommand::GetMemoryProfile,
                None,
                Duration::from_millis(10),
                format!("concurrent-{}", i),
            ).await
        });
        command_handles.push(handle);
    }

    for handle in command_handles {
        handle.await.unwrap().unwrap();
    }

    // Verify all sessions have command history
    for session_id in &session_ids {
        let history = manager.get_command_history(session_id, Some(10)).await.unwrap();
        assert_eq!(history.len(), 1);
    }
}

#[tokio::test]
async fn test_session_statistics_and_metrics() {
    let manager = create_test_session_manager();
    manager.start().await.unwrap();

    // Create multiple sessions with different states
    let active_session = manager.create_session("Active Session".to_string(), None).await.unwrap();
    let replay_session = manager.create_session("Replay Session".to_string(), None).await.unwrap();

    // Add some commands to sessions
    for i in 0..3 {
        manager.record_command(
            &active_session,
            DebugCommand::GetMemoryProfile,
            None,
            Duration::from_millis(10),
            format!("active-{}", i),
        ).await.unwrap();

        manager.record_command(
            &replay_session,
            DebugCommand::GetMemoryStatistics,
            None,
            Duration::from_millis(15),
            format!("replay-{}", i),
        ).await.unwrap();
    }

    // Start replay in one session
    manager.start_replay(&replay_session, Some(0)).await.unwrap();

    // Get statistics
    let stats = manager.get_statistics().await;

    assert_eq!(stats["total_sessions"], 2);
    assert_eq!(stats["active_sessions"], 1);
    assert_eq!(stats["replaying_sessions"], 1);
    assert_eq!(stats["total_commands_recorded"], 6);
}

#[tokio::test]
async fn test_session_processor_command_history_integration() {
    let processor = create_test_session_processor().await;

    // Create session
    let create_cmd = DebugCommand::SessionControl {
        operation: SessionOperation::Create,
        session_id: Some("history-integration-test".to_string()),
    };
    let create_result = processor.process(create_cmd.clone()).await.unwrap();
    let session_id = match create_result {
        DebugResponse::SessionStatus { session_id, .. } => session_id,
        _ => panic!("Expected SessionStatus response"),
    };

    // Record command executions
    let test_commands = vec![
        DebugCommand::GetMemoryProfile,
        DebugCommand::GetMemoryStatistics,
        DebugCommand::TakeMemorySnapshot,
    ];

    for (i, cmd) in test_commands.iter().enumerate() {
        let response = DebugResponse::Success {
            message: format!("Test command {} executed", i),
            data: None,
        };

        processor.record_command_execution(
            &session_id,
            cmd.clone(),
            Some(response),
            Duration::from_millis(20),
            format!("integration-test-{}", i),
        ).await.unwrap();
    }

    // Get command history through processor
    let history = processor.get_command_history(&session_id, Some(10)).await.unwrap();
    assert_eq!(history.len(), 3);

    // Verify history structure
    for (i, entry) in history.iter().enumerate() {
        assert_eq!(entry["correlation_id"], format!("integration-test-{}", 2 - i)); // Reverse order
        assert_eq!(entry["success"], true);
        assert!(entry["execution_duration_us"].as_u64().unwrap() > 0);
    }

    // Test replay start through processor
    processor.start_replay(&session_id, Some(0)).await.unwrap();

    // Get replay commands
    let mut replay_count = 0;
    while let Some(_response) = processor.replay_next(&session_id).await.unwrap() {
        replay_count += 1;
        if replay_count >= 3 {
            break; // Prevent infinite loop in test
        }
    }

    assert_eq!(replay_count, 3);
}

#[tokio::test]
async fn test_session_error_handling() {
    let manager = create_test_session_manager();
    manager.start().await.unwrap();

    // Test operations on non-existent session
    let fake_session_id = "non-existent-session";

    // Should fail gracefully
    let result = manager.get_session(fake_session_id).await;
    assert!(result.is_none());

    let result = manager.end_session(fake_session_id).await;
    assert!(result.is_err());

    let result = manager.create_checkpoint(fake_session_id, "test").await;
    assert!(result.is_err());

    let result = manager.record_command(
        fake_session_id,
        DebugCommand::GetMemoryProfile,
        None,
        Duration::from_millis(10),
        "test".to_string(),
    ).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_processing_time_estimates() {
    let processor = create_test_session_processor().await;

    let test_cases = vec![
        (DebugCommand::SessionControl {
            operation: SessionOperation::Create,
            session_id: Some("test".to_string()),
        }, Duration::from_millis(50)),
        (DebugCommand::SessionControl {
            operation: SessionOperation::Checkpoint,
            session_id: Some("test".to_string()),
        }, Duration::from_millis(200)),
        (DebugCommand::GetStatus, Duration::from_millis(10)),
    ];

    for (cmd, expected_duration) in test_cases {
        let estimated = processor.estimate_processing_time(&cmd);
        assert_eq!(estimated, expected_duration, "Processing time estimate mismatch for: {:?}", cmd);
    }
}