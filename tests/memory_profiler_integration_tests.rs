/// Integration tests for Memory Profiler and Leak Detector functionality
/// 
/// These tests verify the complete memory profiling system including:
/// - Memory allocation tracking
/// - Leak detection algorithms
/// - Memory usage trend analysis
/// - Performance overhead monitoring
/// - MCP integration

use bevy_debugger_mcp::brp_messages::{DebugCommand, DebugResponse};
use bevy_debugger_mcp::config::Config;
use bevy_debugger_mcp::brp_client::BrpClient;
use bevy_debugger_mcp::debug_command_processor::DebugCommandProcessor;
use bevy_debugger_mcp::memory_profiler::{MemoryProfiler, MemoryProfilerConfig};
use bevy_debugger_mcp::memory_profiler_processor::MemoryProfilerProcessor;
use bevy_debugger_mcp::error::{Error, Result};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::sleep;

/// Helper to create test configuration
fn create_test_config() -> Config {
    {
        let mut config = Config::default();
        config.bevy_brp_host = "localhost".to_string();
        config.bevy_brp_port = 15702;
        config.mcp_port = 3000;
        config
    }
}

/// Helper to create memory profiler processor for testing
async fn create_test_memory_profiler_processor() -> MemoryProfilerProcessor {
    let config = create_test_config();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    MemoryProfilerProcessor::new(brp_client)
}

/// Helper to create memory profiler with test configuration
fn create_test_memory_profiler() -> MemoryProfiler {
    let config = MemoryProfilerConfig {
        max_overhead_percent: 5.0,
        capture_backtraces: true,
        enable_leak_detection: true,
        snapshot_interval: Duration::from_millis(100), // Fast for testing
        monitor_entity_count: true,
        track_resource_footprint: true,
    };
    MemoryProfiler::new(config)
}

#[tokio::test]
async fn test_memory_profiler_basic_functionality() {
    let profiler = create_test_memory_profiler();
    
    // Test initial state
    assert!(profiler.is_overhead_acceptable());
    let stats = profiler.get_statistics().await;
    assert_eq!(stats["total_allocations_tracked"], 0);
    assert_eq!(stats["entity_count"], 0);
}

#[tokio::test]
async fn test_allocation_tracking() {
    let profiler = create_test_memory_profiler();
    
    // Record some allocations
    let allocation1 = profiler.record_allocation(
        "TestSystem1", 
        1024, 
        Some(vec!["test_function1".to_string()])
    ).await.unwrap();
    
    let allocation2 = profiler.record_allocation(
        "TestSystem2", 
        2048, 
        Some(vec!["test_function2".to_string()])
    ).await.unwrap();
    
    assert!(allocation1 > 0);
    assert!(allocation2 > allocation1);
    
    // Check memory profile
    let profile = profiler.get_memory_profile().await.unwrap();
    assert_eq!(profile.total_allocated, 3072);
    assert_eq!(profile.allocations_per_system.len(), 2);
    assert_eq!(profile.allocations_per_system["TestSystem1"], 1024);
    assert_eq!(profile.allocations_per_system["TestSystem2"], 2048);
    
    // Test deallocation
    profiler.record_deallocation(allocation1).await.unwrap();
    
    let profile_after_dealloc = profiler.get_memory_profile().await.unwrap();
    assert_eq!(profile_after_dealloc.total_allocated, 2048);
    assert_eq!(profile_after_dealloc.allocations_per_system["TestSystem1"], 0);
}

#[tokio::test]
async fn test_entity_count_tracking() {
    let profiler = create_test_memory_profiler();
    
    profiler.update_entity_count(100).await;
    
    let snapshot = profiler.take_snapshot().await.unwrap();
    assert_eq!(snapshot.entity_count, 100);
    
    profiler.update_entity_count(150).await;
    
    let snapshot2 = profiler.take_snapshot().await.unwrap();
    assert_eq!(snapshot2.entity_count, 150);
}

#[tokio::test]
async fn test_resource_memory_tracking() {
    let profiler = create_test_memory_profiler();
    
    profiler.update_resource_memory("Textures", 1024000).await;
    profiler.update_resource_memory("Meshes", 512000).await;
    
    let snapshot = profiler.take_snapshot().await.unwrap();
    assert_eq!(snapshot.resource_footprint["Textures"], 1024000);
    assert_eq!(snapshot.resource_footprint["Meshes"], 512000);
}

#[tokio::test]
async fn test_memory_snapshots() {
    let profiler = create_test_memory_profiler();
    
    // Create some allocations over time
    for i in 0..5 {
        profiler.record_allocation("TestSystem", 1000 * (i + 1), None).await.unwrap();
        profiler.update_entity_count(10 * (i + 1)).await;
        profiler.take_snapshot().await.unwrap();
        sleep(Duration::from_millis(10)).await;
    }
    
    let stats = profiler.get_statistics().await;
    assert_eq!(stats["snapshots_stored"], 5);
    
    // Verify snapshots are ordered by time
    let final_profile = profiler.get_memory_profile().await.unwrap();
    assert_eq!(final_profile.total_allocated, 15000); // 1000 + 2000 + 3000 + 4000 + 5000
}

#[tokio::test]
async fn test_leak_detection() {
    let profiler = create_test_memory_profiler();
    
    // Create many allocations that look like leaks
    let mut leak_allocations = Vec::new();
    for i in 0..25 {
        let allocation_id = profiler.record_allocation(
            "LeakySystem", 
            1024, 
            Some(vec![format!("leaked_function_{}", i)])
        ).await.unwrap();
        leak_allocations.push(allocation_id);
    }
    
    // Make some legitimate allocations and deallocate them
    for i in 0..10 {
        let allocation_id = profiler.record_allocation("GoodSystem", 512, None).await.unwrap();
        profiler.record_deallocation(allocation_id).await.unwrap();
    }
    
    // Manually adjust timestamps to make allocations appear old
    // In real usage, this would happen naturally over time
    
    // Wait a bit to ensure different timestamps
    sleep(Duration::from_millis(50)).await;
    
    let leaks = profiler.detect_leaks().await.unwrap();
    
    // Should detect leaks in LeakySystem (25 allocations > 10 threshold)
    assert!(!leaks.is_empty());
    
    let leaky_system_leak = leaks.iter().find(|leak| leak.system_name == "LeakySystem");
    assert!(leaky_system_leak.is_some());
    
    let leak = leaky_system_leak.unwrap();
    assert!(leak.leak_count >= 10); // Should find significant number of leaks
    assert!(leak.leaked_memory > 10000); // Should account for leaked memory
}

#[tokio::test]
async fn test_trend_analysis() {
    let profiler = create_test_memory_profiler();
    
    // Create increasing memory usage pattern
    for i in 1..=8 {
        profiler.record_allocation("GrowingSystem", i * 1000, None).await.unwrap();
        profiler.take_snapshot().await.unwrap();
        sleep(Duration::from_millis(20)).await; // Give time for different timestamps
    }
    
    // Create stable pattern
    for _i in 1..=5 {
        let allocation_id = profiler.record_allocation("StableSystem", 5000, None).await.unwrap();
        profiler.record_deallocation(allocation_id).await.unwrap();
        profiler.record_allocation("StableSystem", 5000, None).await.unwrap();
        profiler.take_snapshot().await.unwrap();
        sleep(Duration::from_millis(20)).await;
    }
    
    let trends = profiler.analyze_trends().await.unwrap();
    assert!(!trends.is_empty());
    
    // Find growing system trend
    let growing_trend = trends.iter().find(|t| t.system_name == "GrowingSystem");
    assert!(growing_trend.is_some());
    
    let trend = growing_trend.unwrap();
    assert!(trend.growth_rate_bytes_per_min > 0.0);
    assert!(trend.predicted_usage_1h > 0);
    assert!(trend.prediction_confidence > 0.0);
    
    // Check stable system
    let stable_trend = trends.iter().find(|t| t.system_name == "StableSystem");
    if let Some(stable) = stable_trend {
        // Growth rate should be relatively low for stable system
        assert!(stable.growth_rate_bytes_per_min.abs() < 10000.0);
    }
}

#[tokio::test]
async fn test_overhead_monitoring() {
    let profiler = create_test_memory_profiler();
    
    // Perform many operations to generate measurable overhead
    let start_time = Instant::now();
    for i in 0..1000 {
        let allocation_id = profiler.record_allocation("BusySystem", i * 10, None).await.unwrap();
        if i % 2 == 0 {
            profiler.record_deallocation(allocation_id).await.unwrap();
        }
        if i % 100 == 0 {
            profiler.take_snapshot().await.unwrap();
        }
    }
    let total_test_time = start_time.elapsed();
    
    let overhead_percent = profiler.get_overhead_percentage();
    assert!(overhead_percent >= 0.0);
    assert!(profiler.is_overhead_acceptable()); // Should be under 5%
    
    println!("Memory profiler overhead: {:.3}% over {:.2}ms", 
             overhead_percent, total_test_time.as_millis());
    
    let stats = profiler.get_statistics().await;
    assert!(stats["total_allocations_tracked"].as_u64().unwrap() >= 500); // Many still active
}

#[tokio::test] 
async fn test_memory_profiler_processor_commands() {
    let processor = create_test_memory_profiler_processor().await;
    
    // Test ProfileMemory command
    let profile_cmd = DebugCommand::ProfileMemory {
        target_systems: Some(vec!["TestSystem".to_string()]),
        capture_backtraces: Some(true),
        duration_seconds: Some(300),
    };
    
    assert!(processor.supports_command(&profile_cmd));
    assert!(processor.validate(&profile_cmd).await.is_ok());
    
    let result = processor.process(profile_cmd).await;
    assert!(result.is_ok());
    
    match result.unwrap() {
        DebugResponse::Success { message, .. } => {
            assert!(message.contains("Memory profiling started"));
        }
        _ => panic!("Expected Success response"),
    }
}

#[tokio::test]
async fn test_memory_profile_command() {
    let processor = create_test_memory_profiler_processor().await;
    
    // Start profiling first
    let start_cmd = DebugCommand::ProfileMemory {
        target_systems: None,
        capture_backtraces: Some(false),
        duration_seconds: None,
    };
    processor.process(start_cmd).await.unwrap();
    
    // Record some allocations
    processor.record_allocation("TestSystem1", 1024, None).await.unwrap();
    processor.record_allocation("TestSystem2", 2048, None).await.unwrap();
    
    // Get memory profile
    let profile_cmd = DebugCommand::GetMemoryProfile;
    assert!(processor.supports_command(&profile_cmd));
    
    let result = processor.process(profile_cmd).await;
    assert!(result.is_ok());
    
    match result.unwrap() {
        DebugResponse::MemoryProfile { total_allocated, allocations_per_system, .. } => {
            assert_eq!(total_allocated, 3072);
            assert_eq!(allocations_per_system.len(), 2);
        }
        _ => panic!("Expected MemoryProfile response"),
    }
}

#[tokio::test]
async fn test_leak_detection_command() {
    let processor = create_test_memory_profiler_processor().await;
    
    // Start profiling
    processor.process(DebugCommand::ProfileMemory {
        target_systems: None,
        capture_backtraces: Some(true),
        duration_seconds: None,
    }).await.unwrap();
    
    // Simulate some allocations that could be leaks
    for i in 0..15 {
        processor.record_allocation("PotentiallyLeakySystem", 1024, 
                                   Some(vec![format!("function_{}", i)])).await.unwrap();
    }
    
    let detect_cmd = DebugCommand::DetectMemoryLeaks {
        target_systems: Some(vec!["PotentiallyLeakySystem".to_string()]),
    };
    
    assert!(processor.supports_command(&detect_cmd));
    let result = processor.process(detect_cmd).await;
    assert!(result.is_ok());
    
    match result.unwrap() {
        DebugResponse::Success { data: Some(data), .. } => {
            assert!(data.get("leak_count").is_some());
            assert!(data.get("leaks").is_some());
        }
        _ => panic!("Expected Success response with leak data"),
    }
}

#[tokio::test]
async fn test_trend_analysis_command() {
    let processor = create_test_memory_profiler_processor().await;
    
    // Start profiling
    processor.process(DebugCommand::ProfileMemory {
        target_systems: None,
        capture_backtraces: Some(false),
        duration_seconds: None,
    }).await.unwrap();
    
    // Create multiple snapshots with some memory growth
    for i in 1..=6 {
        processor.record_allocation("GrowingSystem", i * 1000, None).await.unwrap();
        processor.process(DebugCommand::TakeMemorySnapshot).await.unwrap();
        sleep(Duration::from_millis(10)).await;
    }
    
    let trends_cmd = DebugCommand::AnalyzeMemoryTrends {
        target_systems: Some(vec!["GrowingSystem".to_string()]),
    };
    
    assert!(processor.supports_command(&trends_cmd));
    let result = processor.process(trends_cmd).await;
    assert!(result.is_ok());
    
    match result.unwrap() {
        DebugResponse::Success { data: Some(data), .. } => {
            assert!(data.get("trend_count").is_some());
            assert!(data.get("trends").is_some());
            
            let trend_count = data.get("trend_count").unwrap().as_u64().unwrap();
            assert!(trend_count > 0);
        }
        _ => panic!("Expected Success response with trend data"),
    }
}

#[tokio::test]
async fn test_memory_snapshot_command() {
    let processor = create_test_memory_profiler_processor().await;
    
    // Start profiling
    processor.process(DebugCommand::ProfileMemory {
        target_systems: None,
        capture_backtraces: Some(false),
        duration_seconds: None,
    }).await.unwrap();
    
    // Create some memory state
    processor.record_allocation("SnapshotTestSystem", 4096, None).await.unwrap();
    processor.update_entity_count(42).await;
    processor.update_resource_memory("TestResource", 8192).await;
    
    let snapshot_cmd = DebugCommand::TakeMemorySnapshot;
    assert!(processor.supports_command(&snapshot_cmd));
    
    let result = processor.process(snapshot_cmd).await;
    assert!(result.is_ok());
    
    match result.unwrap() {
        DebugResponse::Success { data: Some(data), .. } => {
            let snapshot = data.get("snapshot").unwrap();
            assert!(snapshot.get("total_allocated").unwrap().as_u64().unwrap() >= 4096);
            assert_eq!(snapshot.get("entity_count").unwrap().as_u64().unwrap(), 42);
            assert!(snapshot.get("resource_footprint").is_some());
        }
        _ => panic!("Expected Success response with snapshot data"),
    }
}

#[tokio::test]
async fn test_memory_statistics_command() {
    let processor = create_test_memory_profiler_processor().await;
    
    let stats_cmd = DebugCommand::GetMemoryStatistics;
    assert!(processor.supports_command(&stats_cmd));
    
    let result = processor.process(stats_cmd).await;
    assert!(result.is_ok());
    
    match result.unwrap() {
        DebugResponse::Success { data: Some(data), .. } => {
            assert!(data.get("active_sessions").is_some());
            assert!(data.get("total_allocations_tracked").is_some());
            assert!(data.get("overhead_percentage").is_some());
            assert!(data.get("sessions").is_some());
        }
        _ => panic!("Expected Success response with statistics"),
    }
}

#[tokio::test]
async fn test_profiling_session_management() {
    let processor = create_test_memory_profiler_processor().await;
    
    // Start multiple sessions
    let session1_cmd = DebugCommand::ProfileMemory {
        target_systems: Some(vec!["System1".to_string()]),
        capture_backtraces: Some(true),
        duration_seconds: Some(600),
    };
    
    processor.process(session1_cmd).await.unwrap();
    
    // Check that session is active
    let stats_result = processor.process(DebugCommand::GetMemoryStatistics).await.unwrap();
    match stats_result {
        DebugResponse::Success { data: Some(data), .. } => {
            assert_eq!(data.get("active_sessions").unwrap().as_u64().unwrap(), 1);
        }
        _ => panic!("Expected Success response"),
    }
    
    // Stop the session
    let stop_cmd = DebugCommand::StopMemoryProfiling {
        session_id: None, // Stop default session
    };
    
    assert!(processor.supports_command(&stop_cmd));
    let result = processor.process(stop_cmd).await;
    assert!(result.is_ok());
    
    // Check that session is stopped
    let stats_after_stop = processor.process(DebugCommand::GetMemoryStatistics).await.unwrap();
    match stats_after_stop {
        DebugResponse::Success { data: Some(data), .. } => {
            assert_eq!(data.get("active_sessions").unwrap().as_u64().unwrap(), 0);
        }
        _ => panic!("Expected Success response"),
    }
}

#[tokio::test]
async fn test_command_validation() {
    let processor = create_test_memory_profiler_processor().await;
    
    // Valid commands
    let valid_cmds = vec![
        DebugCommand::ProfileMemory {
            target_systems: Some(vec!["System1".to_string()]),
            capture_backtraces: Some(true),
            duration_seconds: Some(300),
        },
        DebugCommand::DetectMemoryLeaks {
            target_systems: Some(vec!["System1".to_string(), "System2".to_string()]),
        },
        DebugCommand::StopMemoryProfiling {
            session_id: Some("test_session".to_string()),
        },
    ];
    
    for cmd in valid_cmds {
        assert!(processor.validate(&cmd).await.is_ok(), "Command should be valid: {:?}", cmd);
    }
    
    // Invalid commands
    let invalid_cmds = vec![
        DebugCommand::ProfileMemory {
            target_systems: Some((0..150).map(|i| format!("System{}", i)).collect()), // Too many systems
            capture_backtraces: Some(true),
            duration_seconds: Some(300),
        },
        DebugCommand::ProfileMemory {
            target_systems: None,
            capture_backtraces: Some(true),
            duration_seconds: Some(100000), // Duration too long
        },
        DebugCommand::StopMemoryProfiling {
            session_id: Some("x".repeat(150)), // Session ID too long
        },
    ];
    
    for cmd in invalid_cmds {
        assert!(processor.validate(&cmd).await.is_err(), "Command should be invalid: {:?}", cmd);
    }
}

#[tokio::test]
async fn test_processing_time_estimates() {
    let processor = create_test_memory_profiler_processor().await;
    
    let commands = vec![
        (DebugCommand::ProfileMemory {
            target_systems: None,
            capture_backtraces: Some(true),
            duration_seconds: Some(300),
        }, Duration::from_millis(50)),
        (DebugCommand::GetMemoryProfile, Duration::from_millis(100)),
        (DebugCommand::DetectMemoryLeaks { target_systems: None }, Duration::from_millis(500)),
        (DebugCommand::AnalyzeMemoryTrends { target_systems: None }, Duration::from_millis(300)),
        (DebugCommand::TakeMemorySnapshot, Duration::from_millis(150)),
        (DebugCommand::GetMemoryStatistics, Duration::from_millis(30)),
    ];
    
    for (cmd, expected_duration) in commands {
        let estimated = processor.estimate_processing_time(&cmd);
        assert_eq!(estimated, expected_duration, 
                   "Processing time estimate should match for command: {:?}", cmd);
    }
}

#[tokio::test]
async fn test_concurrent_memory_operations() {
    let processor = Arc::new(create_test_memory_profiler_processor().await);
    
    // Start profiling
    processor.process(DebugCommand::ProfileMemory {
        target_systems: None,
        capture_backtraces: Some(false),
        duration_seconds: None,
    }).await.unwrap();
    
    // Simulate concurrent operations
    let mut handles = Vec::new();
    
    // Concurrent allocation recording
    for i in 0..10 {
        let processor_clone = Arc::clone(&processor);
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let _ = processor_clone.record_allocation(
                    &format!("ConcurrentSystem{}", i), 
                    (i * 100 + j) as usize, 
                    None
                ).await;
            }
        });
        handles.push(handle);
    }
    
    // Concurrent command processing
    for _i in 0..5 {
        let processor_clone = Arc::clone(&processor);
        let handle = tokio::spawn(async move {
            let _ = processor_clone.process(DebugCommand::TakeMemorySnapshot).await;
            let _ = processor_clone.process(DebugCommand::GetMemoryProfile).await;
        });
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Verify system is still functional
    let stats_result = processor.process(DebugCommand::GetMemoryStatistics).await;
    assert!(stats_result.is_ok());
    
    let profile_result = processor.process(DebugCommand::GetMemoryProfile).await;
    assert!(profile_result.is_ok());
}

#[tokio::test]
async fn test_memory_profiler_cleanup() {
    let processor = create_test_memory_profiler_processor().await;
    
    // Start profiling with short duration
    processor.process(DebugCommand::ProfileMemory {
        target_systems: None,
        capture_backtraces: Some(true),
        duration_seconds: Some(1), // 1 second for quick test
    }).await.unwrap();
    
    // Create many allocations and deallocations
    let mut allocation_ids = Vec::new();
    for i in 0..100 {
        let allocation_id = processor.record_allocation(
            "CleanupTestSystem", 
            i * 10, 
            Some(vec![format!("function_{}", i)])
        ).await.unwrap();
        allocation_ids.push(allocation_id);
    }
    
    // Deallocate half of them
    for &allocation_id in allocation_ids.iter().take(50) {
        processor.record_deallocation(allocation_id).await.unwrap();
    }
    
    // Wait for session to potentially expire
    sleep(Duration::from_millis(1100)).await;
    
    // System should still be responsive
    let result = processor.process(DebugCommand::GetMemoryStatistics).await;
    assert!(result.is_ok());
    
    // Overhead should still be acceptable
    assert!(processor.is_overhead_acceptable());
}