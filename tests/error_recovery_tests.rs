use bevy_debugger_mcp::checkpoint::{Checkpoint, CheckpointConfig, CheckpointManager};
use bevy_debugger_mcp::dead_letter_queue::{DeadLetterConfig, DeadLetterQueue, FailedOperation};
use bevy_debugger_mcp::diagnostics::DiagnosticCollector;
use bevy_debugger_mcp::error::{Error, ErrorContext, ErrorSeverity};
use serde_json::json;

/// Test ErrorContext creation and manipulation
#[tokio::test]
async fn test_error_context_creation() {
    let context = ErrorContext::new("test_operation", "test_component")
        .add_cause("Test error occurred")
        .add_context("test_param", "value1")
        .add_context("password", "secret123") // Should be redacted
        .add_recovery_suggestion("Retry the operation")
        .set_retryable(true)
        .set_severity(ErrorSeverity::Error);

    assert_eq!(context.operation, "test_operation");
    assert_eq!(context.component, "test_component");
    assert!(context.is_retryable);
    assert!(matches!(context.severity, ErrorSeverity::Error));
    assert_eq!(context.error_chain.len(), 1);
    assert_eq!(context.recovery_suggestions.len(), 1);

    // Check that password was redacted but regular key was not
    assert_eq!(context.context_data.get("password").unwrap(), "[REDACTED]");
    assert_eq!(context.context_data.get("test_param").unwrap(), "value1");
}

/// Test ErrorContext display formatting
#[tokio::test]
async fn test_error_context_display() {
    let context = ErrorContext::new("test_op", "test_comp");
    let display_str = format!("{}", context);
    assert!(display_str.contains("test_op"));
    assert!(display_str.contains("test_comp"));
    assert!(display_str.contains(&context.error_id));
}

/// Test DeadLetterQueue basic functionality
#[tokio::test]
async fn test_dead_letter_queue_basic() {
    let config = DeadLetterConfig {
        max_size: 5,
        retention_period_secs: 3600,
        persist_to_disk: false,
        persistence_path: None,
        cleanup_interval_secs: 300,
    };

    let mut dlq = DeadLetterQueue::new(config);
    dlq.start().await.unwrap();

    // Create a failed operation
    let error_context = ErrorContext::new("test_op", "test_comp").add_cause("Test failure");

    let failed_op = FailedOperation::new(
        "test_operation",
        "test_component",
        3,
        error_context,
        json!({"param": "value"}),
        "Maximum retries exceeded",
    );

    // Add to dead letter queue
    dlq.add_failed_operation(failed_op.clone()).await.unwrap();

    // Verify it was added
    let operations = dlq.get_failed_operations().await;
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].operation, "test_operation");
    assert_eq!(operations[0].retry_count, 3);

    // Test statistics
    let stats = dlq.get_statistics().await;
    assert_eq!(stats.total_count, 1);
    assert_eq!(stats.by_operation.get("test_operation").unwrap(), &1);
    assert_eq!(stats.by_component.get("test_component").unwrap(), &1);
}

/// Test DeadLetterQueue capacity management
#[tokio::test]
async fn test_dead_letter_queue_capacity() {
    let config = DeadLetterConfig {
        max_size: 2, // Small capacity for testing
        retention_period_secs: 3600,
        persist_to_disk: false,
        persistence_path: None,
        cleanup_interval_secs: 300,
    };

    let mut dlq = DeadLetterQueue::new(config);
    dlq.start().await.unwrap();

    // Add operations beyond capacity
    for i in 0..3 {
        let error_context = ErrorContext::new(&format!("op_{}", i), "test_comp");
        let failed_op = FailedOperation::new(
            &format!("operation_{}", i),
            "test_component",
            1,
            error_context,
            json!({"index": i}),
            "Test failure",
        );
        dlq.add_failed_operation(failed_op).await.unwrap();
    }

    // Should only keep the most recent 2
    let operations = dlq.get_failed_operations().await;
    assert_eq!(operations.len(), 2);

    // Should be operations 1 and 2 (oldest removed)
    let operation_names: Vec<&str> = operations.iter().map(|op| op.operation.as_str()).collect();
    assert!(operation_names.contains(&"operation_1"));
    assert!(operation_names.contains(&"operation_2"));
    assert!(!operation_names.contains(&"operation_0"));
}

/// Test DiagnosticCollector functionality
#[tokio::test]
async fn test_diagnostic_collector() {
    let collector = DiagnosticCollector::new(10);

    // Record some errors
    for i in 0..5 {
        let error_context = ErrorContext::new(&format!("operation_{}", i), "test_component")
            .add_cause("Test error")
            .set_severity(if i % 2 == 0 {
                ErrorSeverity::Error
            } else {
                ErrorSeverity::Warning
            });

        collector.record_error(error_context);
    }

    // Generate diagnostic report
    let report = collector.generate_report(None).await.unwrap();

    assert_eq!(report.error_summary.recent_errors.len(), 5);
    assert!(report.system_info.hostname.len() > 0);
    assert!(report.environment_info.working_directory.len() > 0);

    // Check error distribution
    let error_count = report
        .error_summary
        .recent_errors
        .iter()
        .filter(|e| matches!(e.severity, ErrorSeverity::Error))
        .count();
    let warning_count = report
        .error_summary
        .recent_errors
        .iter()
        .filter(|e| matches!(e.severity, ErrorSeverity::Warning))
        .count();

    assert_eq!(error_count, 3); // 0, 2, 4
    assert_eq!(warning_count, 2); // 1, 3
}

/// Test DiagnosticCollector capacity management
#[tokio::test]
async fn test_diagnostic_collector_capacity() {
    let collector = DiagnosticCollector::new(3); // Small capacity

    // Record more errors than capacity
    for i in 0..5 {
        let error_context = ErrorContext::new(&format!("operation_{}", i), "test_component");
        collector.record_error(error_context);
    }

    let report = collector.generate_report(None).await.unwrap();

    // Should only keep the most recent 3
    assert_eq!(report.error_summary.recent_errors.len(), 3);

    // Should be operations 2, 3, 4 (oldest removed)
    let operations: Vec<&str> = report
        .error_summary
        .recent_errors
        .iter()
        .map(|e| e.operation.as_str())
        .collect();

    assert!(operations.contains(&"operation_2"));
    assert!(operations.contains(&"operation_3"));
    assert!(operations.contains(&"operation_4"));
    assert!(!operations.contains(&"operation_0"));
    assert!(!operations.contains(&"operation_1"));
}

/// Test CheckpointManager basic functionality
#[tokio::test]
async fn test_checkpoint_manager_basic() {
    let config = CheckpointConfig {
        max_checkpoints: 10,
        max_age_seconds: 3600,
        persist_to_disk: false,
        storage_directory: "./test_checkpoints".to_string(),
        cleanup_interval_seconds: 300,
        max_state_size_bytes: 1024 * 1024,
    };

    let mut manager = CheckpointManager::new(config);
    manager.start().await.unwrap();

    // Create a checkpoint
    let checkpoint = Checkpoint::new(
        "test_checkpoint",
        "Test checkpoint for unit test",
        "manual",
        "test_component",
        json!({"state": "test_data", "counter": 42}),
    );

    let checkpoint_id = manager.create_checkpoint(checkpoint.clone()).await.unwrap();

    // Verify checkpoint was created
    let checkpoints = manager.list_checkpoints().await.unwrap();
    assert_eq!(checkpoints.len(), 1);
    assert_eq!(checkpoints[0].name, "test_checkpoint");
    assert_eq!(checkpoints[0].operation_type, "manual");

    // Test restoration
    let restored = manager.restore_checkpoint(&checkpoint_id).await.unwrap();
    assert_eq!(restored.name, checkpoint.name);
    assert_eq!(restored.state_data, checkpoint.state_data);

    // Test statistics
    let stats = manager.get_statistics().await.unwrap();
    assert_eq!(stats.total_count, 1);
    assert_eq!(stats.by_operation_type.get("manual").unwrap(), &1);
    assert_eq!(stats.by_component.get("test_component").unwrap(), &1);
    assert_eq!(stats.auto_restorable_count, 1);

    manager.shutdown().await.unwrap();
}

/// Test CheckpointManager capacity management
#[tokio::test]
async fn test_checkpoint_manager_capacity() {
    let config = CheckpointConfig {
        max_checkpoints: 2, // Small capacity for testing
        max_age_seconds: 3600,
        persist_to_disk: false,
        storage_directory: "./test_checkpoints".to_string(),
        cleanup_interval_seconds: 300,
        max_state_size_bytes: 1024 * 1024,
    };

    let mut manager = CheckpointManager::new(config);
    manager.start().await.unwrap();

    // Create checkpoints beyond capacity
    let mut checkpoint_ids = Vec::new();
    for i in 0..3 {
        // Add small delay to ensure different timestamps
        if i > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        let checkpoint = Checkpoint::new(
            &format!("checkpoint_{}", i),
            "Test checkpoint",
            "test",
            "test_component",
            json!({"index": i}),
        );

        let id = manager.create_checkpoint(checkpoint).await.unwrap();
        checkpoint_ids.push(id);
    }

    // Should only keep the most recent 2
    let checkpoints = manager.list_checkpoints().await.unwrap();
    assert_eq!(checkpoints.len(), 2);

    // Oldest should be removed (checkpoint_0)
    let names: Vec<&str> = checkpoints.iter().map(|cp| cp.name.as_str()).collect();
    assert!(names.contains(&"checkpoint_1") || names.contains(&"checkpoint_2"));
    assert!(!names.contains(&"checkpoint_0"));

    manager.shutdown().await.unwrap();
}

/// Test CheckpointManager state size validation
#[tokio::test]
async fn test_checkpoint_manager_size_validation() {
    let config = CheckpointConfig {
        max_checkpoints: 10,
        max_age_seconds: 3600,
        persist_to_disk: false,
        storage_directory: "./test_checkpoints".to_string(),
        cleanup_interval_seconds: 300,
        max_state_size_bytes: 100, // Very small size for testing
    };

    let mut manager = CheckpointManager::new(config);
    manager.start().await.unwrap();

    // Create a checkpoint with large state data
    let large_data = json!({
        "large_field": "x".repeat(200) // Larger than max_state_size_bytes
    });

    let checkpoint = Checkpoint::new(
        "large_checkpoint",
        "Test checkpoint with large data",
        "test",
        "test_component",
        large_data,
    );

    // Should fail due to size validation
    let result = manager.create_checkpoint(checkpoint).await;
    assert!(result.is_err());

    if let Err(Error::Validation(msg)) = result {
        assert!(msg.contains("too large"));
    } else {
        panic!("Expected validation error for large checkpoint");
    }

    manager.shutdown().await.unwrap();
}

/// Test Checkpoint expiration functionality
#[tokio::test]
async fn test_checkpoint_expiration() {
    let checkpoint = Checkpoint::new(
        "test_checkpoint",
        "Test checkpoint",
        "test",
        "test_component",
        json!({"data": "test"}),
    )
    .with_expiration(1); // Expires in 1 second

    // Should not be expired immediately
    assert!(!checkpoint.is_expired());

    // Wait for expiration
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Should now be expired
    assert!(checkpoint.is_expired());
}

/// Test environment variable filtering in diagnostics
#[tokio::test]
async fn test_environment_variable_filtering() {
    let collector = DiagnosticCollector::new(10);

    // Set some test environment variables
    std::env::set_var("RUST_TEST_VAR", "safe_value"); // Safe prefix
    std::env::set_var("TEST_PASSWORD", "secret123"); // Contains unsafe keyword
    std::env::set_var("TEST_API_KEY", "key123"); // Contains unsafe keyword

    let report = collector.generate_report(None).await.unwrap();

    // Safe variables with RUST_ prefix should be included
    assert!(report
        .environment_info
        .environment_variables
        .contains_key("RUST_TEST_VAR"));

    // Sensitive variables should not be included at all
    assert!(!report
        .environment_info
        .environment_variables
        .contains_key("TEST_PASSWORD"));
    assert!(!report
        .environment_info
        .environment_variables
        .contains_key("TEST_API_KEY"));

    // Standard system variables should be included if they exist
    if std::env::var("USER").is_ok() {
        assert!(report
            .environment_info
            .environment_variables
            .contains_key("USER"));
    }

    // Clean up test environment variables
    std::env::remove_var("RUST_TEST_VAR");
    std::env::remove_var("TEST_PASSWORD");
    std::env::remove_var("TEST_API_KEY");
}

/// Test bug report generation
#[tokio::test]
async fn test_bug_report_generation() {
    let collector = DiagnosticCollector::new(10);

    // Record some errors
    let error_context = ErrorContext::new("test_operation", "test_component")
        .add_cause("Test error for bug report")
        .set_severity(ErrorSeverity::Error);

    collector.record_error(error_context);

    let diagnostic_report = collector.generate_report(None).await.unwrap();
    let bug_report = bevy_debugger_mcp::diagnostics::create_bug_report(
        &diagnostic_report,
        "Test bug description",
        "1. Do something\n2. See error",
    );

    assert!(bug_report.contains("Test bug description"));
    assert!(bug_report.contains("1. Do something"));
    assert!(bug_report.contains("## Environment Information"));
    assert!(bug_report.contains("## Error Summary"));
    assert!(bug_report.contains("Report ID"));
}

/// Integration test for error recovery workflow
#[tokio::test]
async fn test_error_recovery_integration() {
    // Set up all systems
    let dlq_config = DeadLetterConfig {
        max_size: 100,
        retention_period_secs: 3600,
        persist_to_disk: false,
        persistence_path: None,
        cleanup_interval_secs: 300,
    };

    let checkpoint_config = CheckpointConfig {
        max_checkpoints: 50,
        max_age_seconds: 3600,
        persist_to_disk: false,
        storage_directory: "./test_checkpoints".to_string(),
        cleanup_interval_seconds: 300,
        max_state_size_bytes: 1024 * 1024,
    };

    let mut dlq = DeadLetterQueue::new(dlq_config);
    let mut checkpoint_manager = CheckpointManager::new(checkpoint_config);
    let diagnostic_collector = DiagnosticCollector::new(50);

    dlq.start().await.unwrap();
    checkpoint_manager.start().await.unwrap();

    // Simulate a workflow with checkpointing and error recovery

    // 1. Create a checkpoint before starting operation
    let initial_state = json!({
        "operation": "complex_task",
        "progress": 0,
        "data": vec![1, 2, 3, 4, 5]
    });

    let checkpoint = Checkpoint::new(
        "complex_task_start",
        "Checkpoint before complex task",
        "complex_task",
        "integration_test",
        initial_state,
    );

    let checkpoint_id = checkpoint_manager
        .create_checkpoint(checkpoint)
        .await
        .unwrap();

    // 2. Simulate operation failure and error recording
    let error_context = ErrorContext::new("complex_task", "integration_test")
        .add_cause("Network timeout during operation")
        .add_context("operation_id", "task_123")
        .add_context("progress", "50%")
        .set_retryable(true)
        .set_severity(ErrorSeverity::Error);

    diagnostic_collector.record_error(error_context.clone());

    // 3. Add to dead letter queue after max retries
    let failed_op = FailedOperation::new(
        "complex_task",
        "integration_test",
        3,
        error_context,
        json!({"task_id": "task_123", "data": "sensitive_data"}),
        "Maximum retries exceeded",
    );

    dlq.add_failed_operation(failed_op).await.unwrap();

    // 4. Generate comprehensive diagnostic report
    let diagnostic_report = diagnostic_collector
        .generate_report(Some(&dlq))
        .await
        .unwrap();

    // Verify the complete error recovery state
    assert_eq!(diagnostic_report.error_summary.recent_errors.len(), 1);
    // Check if dead letter queue stats are included in error summary
    assert!(diagnostic_report.error_summary.dead_letter_stats.is_some());

    let stats = checkpoint_manager.get_statistics().await;
    assert_eq!(stats.total_count, 1);

    let dlq_stats = dlq.get_statistics().await;
    assert_eq!(dlq_stats.total_count, 1);

    // 5. Test recovery by restoring checkpoint
    let restored_checkpoint = checkpoint_manager
        .restore_checkpoint(&checkpoint_id)
        .await
        .unwrap();
    assert_eq!(restored_checkpoint.name, "complex_task_start");

    // Verify state can be used for recovery
    let restored_data = &restored_checkpoint.state_data;
    assert_eq!(restored_data["operation"], "complex_task");
    assert_eq!(restored_data["progress"], 0);

    // Clean up
    checkpoint_manager.shutdown().await.unwrap();
}
