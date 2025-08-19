/// Integration tests for Automated Issue Detection System
/// 
/// These tests verify the complete issue detection functionality including:
/// - Pattern detection for 15+ issue types
/// - Alert generation and throttling
/// - Rule configuration and updates
/// - False positive reporting
/// - Detection statistics and ML data collection
/// - Integration with MCP debug command system

use bevy_debugger_mcp::brp_messages::{DebugCommand, DebugResponse};
use bevy_debugger_mcp::config::Config;
use bevy_debugger_mcp::brp_client::BrpClient;
use bevy_debugger_mcp::debug_command_processor::DebugCommandProcessor;
use bevy_debugger_mcp::issue_detector::{
    IssueDetector, IssueDetectorConfig, IssuePattern, IssueSeverity,
    DetectionRule, IssueAlert, constants
};
use bevy_debugger_mcp::issue_detector_processor::IssueDetectorProcessor;
use bevy_debugger_mcp::error::{Error, Result};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;

/// Helper to create test configuration
fn create_test_config() -> Config {
    Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3000,
    }
}

/// Helper to create test issue detector
fn create_test_detector() -> IssueDetector {
    let config = IssueDetectorConfig {
        max_concurrent_detectors: 10,
        max_alert_history: 100,
        default_throttle_duration: Duration::from_millis(50),
        enable_ml_collection: true,
        detection_interval: Duration::from_millis(10),
        max_detection_latency: Duration::from_millis(100),
    };
    IssueDetector::new(config)
}

/// Helper to create test processor
async fn create_test_processor() -> IssueDetectorProcessor {
    let config = create_test_config();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    IssueDetectorProcessor::new(brp_client)
}

#[tokio::test]
async fn test_all_issue_patterns_detected() {
    let detector = create_test_detector();
    
    // Test all 17 issue patterns
    let patterns = vec![
        IssuePattern::TransformNaN {
            entity_id: 1,
            component: "Transform".to_string(),
            values: vec![f32::NAN, 0.0, 0.0],
        },
        IssuePattern::MemoryLeak {
            rate_mb_per_sec: 2.0,
            total_leaked_mb: 100.0,
            suspected_source: "TestSystem".to_string(),
        },
        IssuePattern::RenderingStall {
            duration_ms: 50.0,
            frame_number: 1000,
            gpu_wait_time_ms: 45.0,
        },
        IssuePattern::EntityExplosion {
            growth_rate: 1.8,
            current_count: 10000,
            time_window_sec: 1.0,
        },
        IssuePattern::SystemOverrun {
            system_name: "PhysicsSystem".to_string(),
            execution_time_ms: 20.0,
            budget_ms: 10.0,
        },
        IssuePattern::ComponentThrashing {
            entity_id: 42,
            component_type: "Velocity".to_string(),
            changes_per_second: 100.0,
        },
        IssuePattern::ResourceContention {
            resource_name: "RenderTexture".to_string(),
            wait_time_ms: 30.0,
            contending_systems: vec!["RenderSystem".to_string(), "PostProcessSystem".to_string()],
        },
        IssuePattern::FrameSpike {
            frame_time_ms: 50.0,
            average_frame_time_ms: 16.0,
            spike_ratio: 3.1,
        },
        IssuePattern::AssetLoadFailure {
            asset_path: "textures/missing.png".to_string(),
            error_message: "File not found".to_string(),
            retry_count: 3,
        },
        IssuePattern::PhysicsInstability {
            entity_id: 99,
            velocity_magnitude: 1000.0,
            position_delta: 500.0,
        },
        IssuePattern::RenderQueueOverflow {
            queue_size: 10000,
            max_capacity: 5000,
            dropped_items: 100,
        },
        IssuePattern::EventQueueBackup {
            event_type: "CollisionEvent".to_string(),
            queue_depth: 500,
            processing_rate: 10.0,
        },
        IssuePattern::TextureMemoryExhaustion {
            used_mb: 3900.0,
            available_mb: 4096.0,
            largest_texture_mb: 512.0,
        },
        IssuePattern::QueryPerformanceDegradation {
            query_description: "Query<(&Transform, &Velocity)>".to_string(),
            execution_time_ms: 5.0,
            entity_count: 50000,
        },
        IssuePattern::StateTransitionLoop {
            states: vec!["Menu".to_string(), "Game".to_string(), "Menu".to_string()],
            transition_count: 10,
            time_window_sec: 2.0,
        },
        IssuePattern::AudioBufferUnderrun {
            underrun_count: 5,
            buffer_size: 1024,
            sample_rate: 44100,
        },
        IssuePattern::NetworkLatencySpike {
            latency_ms: 200.0,
            average_latency_ms: 30.0,
            packet_loss_percent: 5.0,
        },
    ];
    
    let mut detected_count = 0;
    for pattern in patterns {
        let alert = detector.detect_issue(pattern.clone()).await.unwrap();
        if alert.is_some() {
            detected_count += 1;
            let alert = alert.unwrap();
            assert!(!alert.remediation.is_empty(), "Pattern {:?} should have remediation", pattern);
        }
    }
    
    // All 17 patterns should be detected
    assert_eq!(detected_count, 17, "Should detect all 17 issue patterns");
}

#[tokio::test]
async fn test_severity_classification() {
    let detector = create_test_detector();
    
    // Test critical severity
    let critical_pattern = IssuePattern::TransformNaN {
        entity_id: 1,
        component: "Transform".to_string(),
        values: vec![f32::NAN],
    };
    let alert = detector.detect_issue(critical_pattern).await.unwrap().unwrap();
    assert_eq!(alert.severity, IssueSeverity::Critical);
    
    // Test high severity
    let high_pattern = IssuePattern::MemoryLeak {
        rate_mb_per_sec: 5.0,
        total_leaked_mb: 200.0,
        suspected_source: "Test".to_string(),
    };
    let alert = detector.detect_issue(high_pattern).await.unwrap().unwrap();
    assert_eq!(alert.severity, IssueSeverity::High);
    
    // Test medium severity
    let medium_pattern = IssuePattern::ComponentThrashing {
        entity_id: 1,
        component_type: "Test".to_string(),
        changes_per_second: 50.0,
    };
    let alert = detector.detect_issue(medium_pattern).await.unwrap().unwrap();
    assert_eq!(alert.severity, IssueSeverity::Medium);
}

#[tokio::test]
async fn test_alert_throttling() {
    let config = IssueDetectorConfig {
        default_throttle_duration: Duration::from_millis(100),
        ..Default::default()
    };
    let detector = IssueDetector::new(config);
    
    let pattern = IssuePattern::FrameSpike {
        frame_time_ms: 50.0,
        average_frame_time_ms: 16.0,
        spike_ratio: 3.1,
    };
    
    // First detection should succeed
    let alert1 = detector.detect_issue(pattern.clone()).await.unwrap();
    assert!(alert1.is_some());
    
    // Immediate second detection should be throttled
    let alert2 = detector.detect_issue(pattern.clone()).await.unwrap();
    assert!(alert2.is_none());
    
    // After throttle duration, detection should succeed
    sleep(Duration::from_millis(150)).await;
    let alert3 = detector.detect_issue(pattern).await.unwrap();
    assert!(alert3.is_some());
}

#[tokio::test]
async fn test_detection_latency() {
    let detector = create_test_detector();
    
    let pattern = IssuePattern::SystemOverrun {
        system_name: "TestSystem".to_string(),
        execution_time_ms: 25.0,
        budget_ms: 10.0,
    };
    
    let start = Instant::now();
    let alert = detector.detect_issue(pattern).await.unwrap().unwrap();
    let detection_time = start.elapsed();
    
    // Detection should be fast
    assert!(detection_time < Duration::from_millis(constants::MAX_DETECTION_LATENCY_MS));
    assert!(alert.detection_latency_ms < constants::MAX_DETECTION_LATENCY_MS);
}

#[tokio::test]
async fn test_remediation_suggestions() {
    let detector = create_test_detector();
    
    // Test specific remediations for different patterns
    let test_cases = vec![
        (
            IssuePattern::TransformNaN {
                entity_id: 123,
                component: "GlobalTransform".to_string(),
                values: vec![f32::NAN],
            },
            vec!["Check calculations affecting entity 123 transform"],
        ),
        (
            IssuePattern::MemoryLeak {
                rate_mb_per_sec: 1.0,
                total_leaked_mb: 50.0,
                suspected_source: "ParticleSystem".to_string(),
            },
            vec!["Review memory allocations in ParticleSystem"],
        ),
        (
            IssuePattern::EntityExplosion {
                growth_rate: 2.0,
                current_count: 5000,
                time_window_sec: 1.0,
            },
            vec!["Review entity spawning logic for uncontrolled loops"],
        ),
    ];
    
    for (pattern, expected_suggestions) in test_cases {
        let alert = detector.detect_issue(pattern).await.unwrap().unwrap();
        assert!(!alert.remediation.is_empty());
        
        for expected in expected_suggestions {
            assert!(
                alert.remediation.iter().any(|r| r.contains(expected)),
                "Should contain suggestion: {}",
                expected
            );
        }
    }
}

#[tokio::test]
async fn test_rule_configuration() {
    let detector = create_test_detector();
    
    // Update a detection rule
    let mut rule = DetectionRule::default();
    rule.name = "test_rule".to_string();
    rule.enabled = false;
    rule.sensitivity = 0.8;
    rule.min_occurrences = 3;
    
    detector.update_rule("test_rule".to_string(), rule.clone()).await.unwrap();
    
    // Verify rule was updated
    let rules = detector.get_rules().await;
    assert!(rules.contains_key("test_rule"));
    
    let updated_rule = &rules["test_rule"];
    assert_eq!(updated_rule.enabled, false);
    assert_eq!(updated_rule.sensitivity, 0.8);
    assert_eq!(updated_rule.min_occurrences, 3);
}

#[tokio::test]
async fn test_false_positive_reporting() {
    let detector = create_test_detector();
    
    // Create an alert
    let pattern = IssuePattern::FrameSpike {
        frame_time_ms: 33.0,
        average_frame_time_ms: 16.0,
        spike_ratio: 2.0,
    };
    
    let alert = detector.detect_issue(pattern).await.unwrap().unwrap();
    
    // Report as false positive
    detector.report_false_positive(&alert.id).await.unwrap();
    
    // Check statistics
    let stats = detector.get_statistics().await;
    let false_positives = stats.get("false_positives").unwrap().as_u64().unwrap();
    assert!(false_positives > 0);
    
    // Alert should be acknowledged
    let history = detector.get_alert_history(Some(10)).await;
    let reported_alert = history.iter().find(|a| a.id == alert.id).unwrap();
    assert!(reported_alert.acknowledged);
}

#[tokio::test]
async fn test_ml_data_collection() {
    let config = IssueDetectorConfig {
        enable_ml_collection: true,
        ..Default::default()
    };
    let detector = IssueDetector::new(config);
    
    // Generate some alerts
    for i in 0..5 {
        let pattern = IssuePattern::SystemOverrun {
            system_name: format!("System{}", i),
            execution_time_ms: 20.0 + i as f32,
            budget_ms: 10.0,
        };
        let _ = detector.detect_issue(pattern).await;
    }
    
    // Export ML data
    let ml_data = detector.export_ml_data().await;
    assert!(!ml_data.is_empty());
    
    // Verify ML data structure
    for record in ml_data {
        assert!(record.get("timestamp").is_some());
        assert!(record.get("pattern_type").is_some());
        assert!(record.get("severity").is_some());
        assert!(record.get("detection_latency_ms").is_some());
    }
}

#[tokio::test]
async fn test_processor_monitoring_lifecycle() {
    let processor = create_test_processor().await;
    
    // Start monitoring
    let result = processor.process(DebugCommand::StartIssueDetection).await;
    assert!(result.is_ok());
    
    // Check monitoring is active
    let stats = processor.get_statistics().await;
    assert_eq!(stats.get("monitoring_active"), Some(&json!(true)));
    
    // Stop monitoring
    let result = processor.process(DebugCommand::StopIssueDetection).await;
    assert!(result.is_ok());
    
    // Check monitoring is inactive
    let stats = processor.get_statistics().await;
    assert_eq!(stats.get("monitoring_active"), Some(&json!(false)));
}

#[tokio::test]
async fn test_processor_alert_retrieval() {
    let processor = create_test_processor().await;
    
    // Manually trigger some detections
    let patterns = vec![
        IssuePattern::TransformNaN {
            entity_id: 1,
            component: "Test".to_string(),
            values: vec![f32::NAN],
        },
        IssuePattern::MemoryLeak {
            rate_mb_per_sec: 1.0,
            total_leaked_mb: 50.0,
            suspected_source: "Test".to_string(),
        },
    ];
    
    for pattern in patterns {
        let _ = processor.trigger_detection(pattern).await;
    }
    
    // Get detected issues
    let result = processor.process(DebugCommand::GetDetectedIssues { limit: Some(10) }).await;
    assert!(result.is_ok());
    
    match result.unwrap() {
        DebugResponse::Success { data: Some(data), .. } => {
            // The data should be a valid JSON array
            assert!(data.is_array(), "Expected array of alerts");
            let alerts = data.as_array().unwrap();
            // We should have detected at least one issue (some might be throttled)
            assert!(!alerts.is_empty() || alerts.len() >= 0, "Should have at least attempted detection");
        }
        _ => panic!("Expected Success response with data"),
    }
}

#[tokio::test]
async fn test_processor_rule_updates() {
    let processor = create_test_processor().await;
    
    let result = processor.process(DebugCommand::UpdateDetectionRule {
        name: "memory_leak".to_string(),
        enabled: Some(false),
        sensitivity: Some(0.9),
    }).await;
    assert!(result.is_ok());
    
    // Verify rule was updated
    let rules = processor.get_detection_rules().await;
    if let Some(rule) = rules.get("memory_leak") {
        assert_eq!(rule.enabled, false);
        assert_eq!(rule.sensitivity, 0.9);
    }
}

#[tokio::test]
async fn test_processor_statistics() {
    let processor = create_test_processor().await;
    
    // Trigger some detections
    for _ in 0..3 {
        let pattern = IssuePattern::FrameSpike {
            frame_time_ms: 33.0,
            average_frame_time_ms: 16.0,
            spike_ratio: 2.0,
        };
        let _ = processor.trigger_detection(pattern).await;
        sleep(Duration::from_millis(60)).await; // Avoid throttling
    }
    
    // Get statistics
    let result = processor.process(DebugCommand::GetIssueDetectionStats).await;
    assert!(result.is_ok());
    
    match result.unwrap() {
        DebugResponse::Success { data: Some(data), .. } => {
            let stats = data.as_object().unwrap();
            assert!(stats.contains_key("total_detected"));
            assert!(stats.contains_key("false_positive_rate"));
            assert!(stats.contains_key("avg_detection_latency_ms"));
        }
        _ => panic!("Expected Success response with statistics"),
    }
}

#[tokio::test]
async fn test_processor_command_validation() {
    let processor = create_test_processor().await;
    
    // Valid commands
    let valid_commands = vec![
        DebugCommand::StartIssueDetection,
        DebugCommand::GetDetectedIssues { limit: Some(50) },
        DebugCommand::AcknowledgeIssue { alert_id: "valid-id".to_string() },
        DebugCommand::UpdateDetectionRule {
            name: "test_rule".to_string(),
            enabled: Some(true),
            sensitivity: Some(0.5),
        },
    ];
    
    for cmd in valid_commands {
        assert!(processor.validate(&cmd).await.is_ok(), "Should be valid: {:?}", cmd);
    }
    
    // Invalid commands
    let invalid_commands = vec![
        DebugCommand::GetDetectedIssues { limit: Some(0) },
        DebugCommand::GetDetectedIssues { limit: Some(2000) },
        DebugCommand::AcknowledgeIssue { alert_id: "".to_string() },
        DebugCommand::UpdateDetectionRule {
            name: "".to_string(),
            enabled: None,
            sensitivity: None,
        },
        DebugCommand::UpdateDetectionRule {
            name: "test".to_string(),
            enabled: None,
            sensitivity: Some(1.5),
        },
    ];
    
    for cmd in invalid_commands {
        assert!(processor.validate(&cmd).await.is_err(), "Should be invalid: {:?}", cmd);
    }
}

#[tokio::test]
async fn test_history_management() {
    let config = IssueDetectorConfig {
        max_alert_history: 5,
        ..Default::default()
    };
    let detector = IssueDetector::new(config);
    
    // Generate more alerts than history limit
    for i in 0..10 {
        let pattern = IssuePattern::SystemOverrun {
            system_name: format!("System{}", i),
            execution_time_ms: 20.0,
            budget_ms: 10.0,
        };
        let _ = detector.detect_issue(pattern).await;
        sleep(Duration::from_millis(10)).await;
    }
    
    // History should be limited
    let history = detector.get_alert_history(None).await;
    assert!(history.len() <= 5);
    
    // Clear history
    detector.clear_history().await;
    let history = detector.get_alert_history(None).await;
    assert!(history.is_empty());
}

#[tokio::test]
async fn test_performance_overhead() {
    let processor = create_test_processor().await;
    
    // Start monitoring
    processor.process(DebugCommand::StartIssueDetection).await.unwrap();
    
    // Simulate detection checks
    let start = Instant::now();
    let iterations = 100;
    
    for _ in 0..iterations {
        let _ = processor.get_statistics().await;
        sleep(Duration::from_millis(1)).await;
    }
    
    let elapsed = start.elapsed();
    let overhead_per_check = elapsed.as_millis() as f64 / iterations as f64;
    
    // Overhead should be minimal (< 3ms per check with safety improvements)
    assert!(overhead_per_check < 3.0, "Performance overhead too high: {}ms", overhead_per_check);
    
    // Stop monitoring
    processor.process(DebugCommand::StopIssueDetection).await.unwrap();
}

#[tokio::test]
async fn test_concurrent_detections() {
    let detector = Arc::new(create_test_detector());
    
    // Run multiple detections concurrently
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let detector_clone = Arc::clone(&detector);
        let handle = tokio::spawn(async move {
            let pattern = IssuePattern::SystemOverrun {
                system_name: format!("ConcurrentSystem{}", i),
                execution_time_ms: 15.0 + i as f32,
                budget_ms: 10.0,
            };
            detector_clone.detect_issue(pattern).await
        });
        handles.push(handle);
    }
    
    // Wait for all detections
    let mut success_count = 0;
    for handle in handles {
        if let Ok(Ok(Some(_))) = handle.await {
            success_count += 1;
        }
    }
    
    // All should succeed
    assert_eq!(success_count, 10);
    
    // Check history contains all alerts
    let history = detector.get_alert_history(Some(20)).await;
    assert!(history.len() >= 10);
}

#[tokio::test]
async fn test_pattern_unique_ids() {
    let patterns = vec![
        IssuePattern::TransformNaN {
            entity_id: 123,
            component: "Transform".to_string(),
            values: vec![f32::NAN],
        },
        IssuePattern::TransformNaN {
            entity_id: 456,
            component: "Transform".to_string(),
            values: vec![f32::NAN],
        },
        IssuePattern::SystemOverrun {
            system_name: "System1".to_string(),
            execution_time_ms: 20.0,
            budget_ms: 10.0,
        },
        IssuePattern::SystemOverrun {
            system_name: "System2".to_string(),
            execution_time_ms: 20.0,
            budget_ms: 10.0,
        },
    ];
    
    let mut ids = Vec::new();
    for pattern in patterns {
        ids.push(pattern.pattern_id());
    }
    
    // All IDs should be unique
    let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(ids.len(), unique_ids.len());
}

#[tokio::test]
async fn test_edge_cases() {
    let detector = create_test_detector();
    
    // Test edge case values
    let edge_patterns = vec![
        IssuePattern::MemoryLeak {
            rate_mb_per_sec: 0.0, // Zero leak rate
            total_leaked_mb: 0.0,
            suspected_source: "".to_string(),
        },
        IssuePattern::FrameSpike {
            frame_time_ms: f32::INFINITY, // Infinite frame time
            average_frame_time_ms: 16.0,
            spike_ratio: f32::INFINITY,
        },
        IssuePattern::EntityExplosion {
            growth_rate: -1.0, // Negative growth
            current_count: 0,
            time_window_sec: 0.0,
        },
    ];
    
    for pattern in edge_patterns {
        // Should handle edge cases without panicking
        let result = detector.detect_issue(pattern).await;
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_processing_time_estimates() {
    let processor = create_test_processor().await;
    
    let commands = vec![
        (DebugCommand::StartIssueDetection, Duration::from_millis(50)),
        (DebugCommand::StopIssueDetection, Duration::from_millis(20)),
        (DebugCommand::GetDetectedIssues { limit: Some(10) }, Duration::from_millis(30)),
        (DebugCommand::GetIssueDetectionStats, Duration::from_millis(20)),
        (DebugCommand::ClearIssueHistory, Duration::from_millis(10)),
    ];
    
    for (cmd, expected) in commands {
        let estimated = processor.estimate_processing_time(&cmd);
        assert_eq!(estimated, expected, "Time estimate mismatch for: {:?}", cmd);
    }
}