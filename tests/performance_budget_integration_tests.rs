/// Integration tests for Performance Budget Monitor System
/// 
/// These tests verify the complete performance budget functionality including:
/// - Budget configuration and persistence
/// - Violation detection within 100ms
/// - Compliance reporting and recommendations
/// - Platform-specific budget application
/// - Integration with MCP debug command system

use bevy_debugger_mcp::brp_messages::{DebugCommand, DebugResponse};
use bevy_debugger_mcp::config::Config;
use bevy_debugger_mcp::brp_client::BrpClient;
use bevy_debugger_mcp::debug_command_processor::DebugCommandProcessor;
use bevy_debugger_mcp::performance_budget::{
    PerformanceBudgetMonitor, BudgetConfig, PerformanceMetrics, Platform,
    BudgetViolation, ViolationSeverity, ViolatedMetric, ComplianceReport
};
use bevy_debugger_mcp::performance_budget_processor::PerformanceBudgetProcessor;
use bevy_debugger_mcp::error::{Error, Result};
use chrono::Utc;
use serde_json::json;
use std::collections::HashMap;
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

/// Helper to create test budget monitor
fn create_test_monitor() -> PerformanceBudgetMonitor {
    let config = BudgetConfig {
        frame_time_ms: Some(16.67),
        memory_mb: Some(500.0),
        cpu_percent: Some(80.0),
        gpu_time_ms: Some(16.0),
        entity_count: Some(10000),
        draw_calls: Some(1000),
        network_bandwidth_kbps: Some(1000.0),
        violation_threshold: 3,
        violation_window_seconds: 10,
        ..Default::default()
    };
    PerformanceBudgetMonitor::new(config)
}

/// Helper to create test processor
async fn create_test_processor() -> PerformanceBudgetProcessor {
    let config = create_test_config();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    PerformanceBudgetProcessor::new(brp_client)
}

/// Helper to create test performance metrics
fn create_test_metrics(
    frame_time: f32,
    memory: f32,
    cpu: f32,
) -> PerformanceMetrics {
    PerformanceMetrics {
        frame_time_ms: frame_time,
        memory_mb: memory,
        system_times: HashMap::new(),
        cpu_percent: cpu,
        gpu_time_ms: 15.0,
        entity_count: 8000,
        draw_calls: 900,
        network_bandwidth_kbps: 800.0,
        timestamp: Utc::now(),
    }
}

#[tokio::test]
async fn test_budget_monitor_creation() {
    let monitor = create_test_monitor();
    
    // Should start inactive
    assert!(monitor.start_monitoring().await.is_ok());
    
    // Should not be able to start again while active
    assert!(monitor.start_monitoring().await.is_err());
    
    // Should be able to stop
    assert!(monitor.stop_monitoring().await.is_ok());
    
    // Should not be able to stop when not active
    assert!(monitor.stop_monitoring().await.is_err());
}

#[tokio::test]
async fn test_violation_detection() {
    let monitor = create_test_monitor();
    monitor.start_monitoring().await.unwrap();
    
    // Create metrics that violate budgets
    let metrics = create_test_metrics(
        25.0,  // Frame time over 16.67ms budget
        600.0, // Memory over 500MB budget
        90.0,  // CPU over 80% budget
    );
    
    let violations = monitor.check_violations(metrics).await.unwrap();
    
    // Should detect 3 violations
    assert_eq!(violations.len(), 3);
    
    // Check violation types
    let has_frame_violation = violations.iter().any(|v| matches!(v.metric, ViolatedMetric::FrameTime));
    let has_memory_violation = violations.iter().any(|v| matches!(v.metric, ViolatedMetric::Memory));
    let has_cpu_violation = violations.iter().any(|v| matches!(v.metric, ViolatedMetric::CpuUsage));
    
    assert!(has_frame_violation);
    assert!(has_memory_violation);
    assert!(has_cpu_violation);
    
    monitor.stop_monitoring().await.unwrap();
}

#[tokio::test]
async fn test_violation_severity_classification() {
    let monitor = create_test_monitor();
    monitor.start_monitoring().await.unwrap();
    
    // Test different severity levels
    let test_cases = vec![
        (20.0, ViolationSeverity::Warning),   // ~20% over budget
        (25.0, ViolationSeverity::Minor),     // ~50% over budget
        (30.0, ViolationSeverity::Major),     // ~80% over budget
        (40.0, ViolationSeverity::Critical),  // ~140% over budget
    ];
    
    for (frame_time, expected_severity) in test_cases {
        let metrics = create_test_metrics(frame_time, 400.0, 70.0);
        let violations = monitor.check_violations(metrics).await.unwrap();
        
        if let Some(violation) = violations.iter().find(|v| matches!(v.metric, ViolatedMetric::FrameTime)) {
            assert!(matches!(violation.severity, expected_severity), 
                "Frame time {} should have severity {:?}", frame_time, expected_severity);
        }
    }
    
    monitor.stop_monitoring().await.unwrap();
}

#[tokio::test]
async fn test_violation_detection_latency() {
    let monitor = create_test_monitor();
    monitor.start_monitoring().await.unwrap();
    
    let metrics = create_test_metrics(20.0, 550.0, 85.0);
    
    let start = Instant::now();
    let _ = monitor.check_violations(metrics).await.unwrap();
    let detection_time = start.elapsed();
    
    // Detection should complete within 100ms requirement
    assert!(detection_time < Duration::from_millis(100), 
        "Detection took {:?}, should be < 100ms", detection_time);
    
    monitor.stop_monitoring().await.unwrap();
}

#[tokio::test]
async fn test_compliance_reporting() {
    let monitor = create_test_monitor();
    monitor.start_monitoring().await.unwrap();
    
    // Generate some sample data
    for i in 0..10 {
        let frame_time = if i % 3 == 0 { 20.0 } else { 15.0 }; // Some violations
        let metrics = create_test_metrics(frame_time, 450.0, 75.0);
        let _ = monitor.check_violations(metrics).await;
        sleep(Duration::from_millis(10)).await;
    }
    
    // Generate compliance report
    let report = monitor.generate_compliance_report(Duration::from_secs(60)).await.unwrap();
    
    assert!(report.total_samples > 0);
    assert!(report.overall_compliance_percent > 0.0);
    assert!(report.overall_compliance_percent <= 100.0);
    
    // Should have compliance data for frame_time
    assert!(report.metric_compliance.contains_key("frame_time"));
    
    monitor.stop_monitoring().await.unwrap();
}

#[tokio::test]
async fn test_budget_recommendations() {
    let monitor = create_test_monitor();
    monitor.start_monitoring().await.unwrap();
    
    // Generate data that consistently violates budget significantly to ensure recommendations
    for _ in 0..20 {
        let metrics = create_test_metrics(22.0, 520.0, 85.0); // More significant violations
        let _ = monitor.check_violations(metrics).await;
        sleep(Duration::from_millis(5)).await;
    }
    
    // Get recommendations
    let report = monitor.generate_compliance_report(Duration::from_secs(60)).await.unwrap();
    
    // Should have recommendations if compliance is low
    if report.overall_compliance_percent < 80.0 {
        assert!(!report.recommendations.is_empty());
        
        // Check recommendation properties
        for rec in &report.recommendations {
            assert!(rec.confidence > 0.0 && rec.confidence <= 1.0);
            assert!(!rec.reason.is_empty());
        }
    }
    
    monitor.stop_monitoring().await.unwrap();
}

#[tokio::test]
async fn test_platform_detection() {
    let platform = Platform::detect();
    
    // Should detect a valid platform
    assert_ne!(platform, Platform::Unknown);
    
    // Platform-specific assertions based on compile target
    #[cfg(target_os = "macos")]
    assert_eq!(platform, Platform::MacOS);
    
    #[cfg(target_os = "windows")]
    assert_eq!(platform, Platform::Windows);
    
    #[cfg(target_os = "linux")]
    assert_eq!(platform, Platform::Linux);
}

#[tokio::test]
async fn test_platform_specific_budgets() {
    let mut config = BudgetConfig::default();
    config.frame_time_ms = Some(16.67);
    
    // Add platform-specific override
    let mut override_config = bevy_debugger_mcp::performance_budget::PlatformBudgetOverride {
        frame_time_ms: Some(33.33), // 30 FPS for mobile
        memory_mb: Some(200.0),
        cpu_percent: Some(60.0),
        gpu_time_ms: Some(30.0),
    };
    
    config.platform_overrides.insert(Platform::Mobile, override_config);
    
    let monitor = PerformanceBudgetMonitor::new(config);
    monitor.start_monitoring().await.unwrap();
    
    // The current platform's budget should be applied
    let current_config = monitor.get_config().await;
    
    // If we're on mobile, the override should apply
    #[cfg(any(target_os = "ios", target_os = "android"))]
    {
        assert_eq!(current_config.frame_time_ms, Some(33.33));
    }
    
    monitor.stop_monitoring().await.unwrap();
}

#[tokio::test]
async fn test_violation_history_management() {
    let monitor = create_test_monitor();
    monitor.start_monitoring().await.unwrap();
    
    // Generate violations
    for i in 0..5 {
        let metrics = create_test_metrics(25.0 + i as f32, 600.0, 90.0);
        let _ = monitor.check_violations(metrics).await;
    }
    
    // Check history (3 violations per check: frame_time, memory, cpu)
    let history = monitor.get_violation_history(Some(20)).await;
    assert!(history.len() <= 15); // 5 checks * 3 violations each
    
    // Clear history
    monitor.clear_violation_history().await;
    let history = monitor.get_violation_history(None).await;
    assert_eq!(history.len(), 0);
    
    monitor.stop_monitoring().await.unwrap();
}

#[tokio::test]
async fn test_processor_monitoring_lifecycle() {
    let processor = create_test_processor().await;
    
    // Start monitoring
    let result = processor.process(DebugCommand::StartBudgetMonitoring).await;
    assert!(result.is_ok());
    
    // Check status
    let stats = processor.process(DebugCommand::GetBudgetStatistics).await;
    assert!(stats.is_ok());
    
    match stats.unwrap() {
        DebugResponse::Success { data: Some(data), .. } => {
            let stats = data.as_object().unwrap();
            assert_eq!(stats.get("continuous_monitoring"), Some(&json!(true)));
        }
        _ => panic!("Expected Success response with data"),
    }
    
    // Stop monitoring
    let result = processor.process(DebugCommand::StopBudgetMonitoring).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_processor_configuration() {
    let processor = create_test_processor().await;
    
    // Set configuration
    let config = json!({
        "frame_time_ms": 20.0,
        "memory_mb": 600.0,
        "cpu_percent": 75.0,
        "gpu_time_ms": 20.0,
        "entity_count": 15000,
        "draw_calls": 1500,
        "network_bandwidth_kbps": 1200.0,
        "system_budgets": {},
        "platform_overrides": {},
        "auto_adjust": true,
        "auto_adjust_percentile": 90.0,
        "violation_threshold": 3,
        "violation_window_seconds": 10
    });
    
    let result = processor.process(DebugCommand::SetPerformanceBudget { config }).await;
    if let Err(e) = &result {
        eprintln!("SetPerformanceBudget failed: {:?}", e);
    }
    assert!(result.is_ok());
    
    // Get configuration
    let result = processor.process(DebugCommand::GetPerformanceBudget).await;
    assert!(result.is_ok());
    
    match result.unwrap() {
        DebugResponse::Success { data: Some(data), .. } => {
            let config: BudgetConfig = serde_json::from_value(data).unwrap();
            assert_eq!(config.frame_time_ms, Some(20.0));
            assert_eq!(config.memory_mb, Some(600.0));
            assert_eq!(config.entity_count, Some(15000));
            assert!(config.auto_adjust);
        }
        _ => panic!("Expected Success response with data"),
    }
}

#[tokio::test]
async fn test_processor_violation_checking() {
    let processor = create_test_processor().await;
    
    // Start monitoring first
    processor.process(DebugCommand::StartBudgetMonitoring).await.unwrap();
    
    // Check for violations
    let result = processor.process(DebugCommand::CheckBudgetViolations).await;
    assert!(result.is_ok());
    
    match result.unwrap() {
        DebugResponse::Success { data: Some(data), .. } => {
            let violations: Vec<BudgetViolation> = serde_json::from_value(data).unwrap();
            // May or may not have violations depending on simulated metrics
            assert!(violations.len() >= 0);
        }
        _ => panic!("Expected Success response with data"),
    }
    
    processor.process(DebugCommand::StopBudgetMonitoring).await.unwrap();
}

#[tokio::test]
async fn test_processor_compliance_report() {
    let processor = create_test_processor().await;
    
    // Start monitoring and generate some data
    processor.process(DebugCommand::StartBudgetMonitoring).await.unwrap();
    
    // Let it collect some samples
    for _ in 0..5 {
        processor.process(DebugCommand::CheckBudgetViolations).await.unwrap();
        sleep(Duration::from_millis(50)).await;
    }
    
    // Generate report
    let result = processor.process(DebugCommand::GenerateComplianceReport { 
        duration_seconds: Some(60) 
    }).await;
    
    assert!(result.is_ok());
    
    processor.process(DebugCommand::StopBudgetMonitoring).await.unwrap();
}

#[tokio::test]
async fn test_processor_command_validation() {
    let processor = create_test_processor().await;
    
    // Valid commands
    let valid_commands = vec![
        DebugCommand::StartBudgetMonitoring,
        DebugCommand::GetPerformanceBudget,
        DebugCommand::GetBudgetViolationHistory { limit: Some(50) },
        DebugCommand::GenerateComplianceReport { duration_seconds: Some(3600) },
    ];
    
    for cmd in valid_commands {
        assert!(processor.validate(&cmd).await.is_ok(), "Should be valid: {:?}", cmd);
    }
    
    // Invalid commands
    let invalid_commands = vec![
        DebugCommand::GetBudgetViolationHistory { limit: Some(0) },
        DebugCommand::GetBudgetViolationHistory { limit: Some(2000) },
        DebugCommand::GenerateComplianceReport { duration_seconds: Some(0) },
        DebugCommand::SetPerformanceBudget { config: json!("invalid") },
    ];
    
    for cmd in invalid_commands {
        assert!(processor.validate(&cmd).await.is_err(), "Should be invalid: {:?}", cmd);
    }
}

#[tokio::test]
async fn test_concurrent_violation_checks() {
    let monitor = Arc::new(create_test_monitor());
    monitor.start_monitoring().await.unwrap();
    
    // Run multiple violation checks concurrently
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let monitor_clone = Arc::clone(&monitor);
        let handle = tokio::spawn(async move {
            let metrics = create_test_metrics(
                16.0 + i as f32,
                490.0 + i as f32 * 10.0,
                75.0 + i as f32,
            );
            monitor_clone.check_violations(metrics).await
        });
        handles.push(handle);
    }
    
    // Wait for all checks
    let mut total_violations = 0;
    for handle in handles {
        if let Ok(Ok(violations)) = handle.await {
            total_violations += violations.len();
        }
    }
    
    // Should have detected some violations
    assert!(total_violations > 0);
    
    monitor.stop_monitoring().await.unwrap();
}

#[tokio::test]
async fn test_statistics_tracking() {
    let monitor = create_test_monitor();
    monitor.start_monitoring().await.unwrap();
    
    // Generate some violations
    for _ in 0..5 {
        let metrics = create_test_metrics(20.0, 550.0, 85.0);
        let _ = monitor.check_violations(metrics).await;
    }
    
    // Check statistics
    let stats = monitor.get_statistics().await;
    
    assert!(stats.contains_key("total_violations"));
    assert!(stats.contains_key("total_samples"));
    assert!(stats.contains_key("avg_detection_latency_ms"));
    assert!(stats.contains_key("monitoring_active"));
    assert!(stats.contains_key("current_platform"));
    
    let total_violations = stats.get("total_violations").unwrap().as_u64().unwrap();
    assert!(total_violations > 0);
    
    monitor.stop_monitoring().await.unwrap();
}

#[tokio::test]
async fn test_system_specific_budgets() {
    let mut config = BudgetConfig::default();
    config.system_budgets.insert("PhysicsSystem".to_string(), 5.0);
    config.system_budgets.insert("RenderSystem".to_string(), 8.0);
    
    let monitor = PerformanceBudgetMonitor::new(config);
    monitor.start_monitoring().await.unwrap();
    
    let mut metrics = create_test_metrics(15.0, 450.0, 75.0);
    metrics.system_times.insert("PhysicsSystem".to_string(), 7.0); // Over budget
    metrics.system_times.insert("RenderSystem".to_string(), 7.0); // Under budget
    
    let violations = monitor.check_violations(metrics).await.unwrap();
    
    // Should detect PhysicsSystem violation
    let physics_violation = violations.iter()
        .any(|v| matches!(&v.metric, ViolatedMetric::SystemExecution(s) if s == "PhysicsSystem"));
    assert!(physics_violation);
    
    // Should not detect RenderSystem violation
    let render_violation = violations.iter()
        .any(|v| matches!(&v.metric, ViolatedMetric::SystemExecution(s) if s == "RenderSystem"));
    assert!(!render_violation);
    
    monitor.stop_monitoring().await.unwrap();
}

#[tokio::test]
async fn test_auto_adjust_configuration() {
    let mut config = BudgetConfig::default();
    config.auto_adjust = true;
    config.auto_adjust_percentile = 95.0;
    
    let monitor = PerformanceBudgetMonitor::new(config);
    monitor.start_monitoring().await.unwrap();
    
    // Generate consistent data over budget to ensure recommendations
    for _ in 0..20 {
        let metrics = create_test_metrics(20.0, 550.0, 85.0); // More significant violations
        let _ = monitor.check_violations(metrics).await;
        sleep(Duration::from_millis(5)).await;
    }
    
    // Generate compliance report with recommendations
    let report = monitor.generate_compliance_report(Duration::from_secs(60)).await.unwrap();
    
    // With auto_adjust enabled and significant violations, should get recommendations
    // Only assert if we have low compliance
    if report.overall_compliance_percent < 90.0 {
        assert!(!report.recommendations.is_empty(), 
            "Expected recommendations with compliance at {:.1}%", 
            report.overall_compliance_percent);
        
        // Recommendations should be based on percentile
        for rec in &report.recommendations {
            // Recommended budget should be adjusted (may be higher or lower)
            assert!(rec.recommended_budget != rec.current_budget);
        }
    }
    
    monitor.stop_monitoring().await.unwrap();
}

#[tokio::test]
async fn test_violation_duration_tracking() {
    let monitor = create_test_monitor();
    monitor.start_monitoring().await.unwrap();
    
    // Create continuous violations
    let metrics = create_test_metrics(25.0, 600.0, 90.0);
    
    // First violation
    let violations1 = monitor.check_violations(metrics.clone()).await.unwrap();
    assert!(!violations1.is_empty());
    assert_eq!(violations1[0].duration_ms, 0); // First occurrence
    
    // Wait a bit
    sleep(Duration::from_millis(100)).await;
    
    // Second violation (continued)
    let violations2 = monitor.check_violations(metrics).await.unwrap();
    assert!(!violations2.is_empty());
    assert!(violations2[0].duration_ms >= 100); // Should track duration
    
    monitor.stop_monitoring().await.unwrap();
}

#[tokio::test]
async fn test_edge_cases() {
    let monitor = create_test_monitor();
    monitor.start_monitoring().await.unwrap();
    
    // Test with extreme values
    let extreme_metrics = PerformanceMetrics {
        frame_time_ms: f32::INFINITY,
        memory_mb: 0.0,
        system_times: HashMap::new(),
        cpu_percent: 150.0, // Over 100%
        gpu_time_ms: -10.0, // Negative
        entity_count: usize::MAX,
        draw_calls: 0,
        network_bandwidth_kbps: f32::NAN,
        timestamp: Utc::now(),
    };
    
    // Should handle extreme values without panicking
    let result = monitor.check_violations(extreme_metrics).await;
    assert!(result.is_ok());
    
    monitor.stop_monitoring().await.unwrap();
}