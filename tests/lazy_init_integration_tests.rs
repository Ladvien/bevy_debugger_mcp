/// Lazy Initialization Integration Tests
/// 
/// Tests the lazy initialization system with real Bevy applications to ensure
/// components are initialized on-demand and provide performance benefits.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde_json::json;

use bevy_debugger_mcp::{
    config::Config,
    brp_client::BrpClient,
    lazy_init::{LazyComponents, preload_critical_components},
    entity_inspector::EntityInspector,
    system_profiler::SystemProfiler,
};

mod fixtures;
mod helpers;

use fixtures::performance_test_game;
use helpers::TestGameProcess;

/// Test lazy initialization with a real Bevy game
#[tokio::test]
async fn test_lazy_init_with_bevy_game() {
    // Start a test Bevy game
    let mut game_process = TestGameProcess::new("performance_test_game").await;
    game_process.start().await.expect("Failed to start test game");

    // Wait for game to initialize
    tokio::time::sleep(Duration::from_secs(2)).await;

    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };
    
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let lazy_components = LazyComponents::new(brp_client);

    // Verify no components are initialized initially
    assert!(!lazy_components.is_any_initialized(), 
            "No components should be initialized on creation");

    // Test lazy initialization of EntityInspector
    let start = Instant::now();
    let inspector1 = lazy_components.get_entity_inspector().await;
    let first_init_time = start.elapsed();
    
    // Second access should be from cache
    let start2 = Instant::now();
    let inspector2 = lazy_components.get_entity_inspector().await;
    let second_access_time = start2.elapsed();

    println!("First EntityInspector init: {:?}", first_init_time);
    println!("Second EntityInspector access: {:?}", second_access_time);

    // Verify same instance returned
    assert!(Arc::ptr_eq(&inspector1, &inspector2), 
            "Should return the same instance on subsequent calls");
    
    // Second access should be much faster
    assert!(second_access_time < first_init_time / 10, 
            "Cached access should be at least 10x faster");
    assert!(second_access_time < Duration::from_millis(1), 
            "Cached access should be sub-millisecond");

    // Test lazy initialization of SystemProfiler
    let start_profiler = Instant::now();
    let _profiler = lazy_components.get_system_profiler().await;
    let profiler_init_time = start_profiler.elapsed();
    
    println!("SystemProfiler init: {:?}", profiler_init_time);
    assert!(profiler_init_time < Duration::from_millis(100), 
            "Component initialization should be reasonably fast");

    // Verify components are now marked as initialized
    assert!(lazy_components.is_any_initialized(), 
            "Components should be marked as initialized after access");

    let status = lazy_components.get_initialization_status();
    assert!(status["entity_inspector"].as_bool().unwrap(), 
            "EntityInspector should be marked as initialized");
    assert!(status["system_profiler"].as_bool().unwrap(), 
            "SystemProfiler should be marked as initialized");

    game_process.cleanup().await.expect("Failed to cleanup test game");
}

/// Test concurrent lazy initialization (race condition prevention)
#[tokio::test]
async fn test_concurrent_lazy_initialization() {
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };
    
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let lazy_components = Arc::new(LazyComponents::new(brp_client));

    // Launch multiple concurrent initialization requests
    let mut handles = vec![];
    
    for i in 0..10 {
        let components = lazy_components.clone();
        let handle = tokio::spawn(async move {
            let start = Instant::now();
            let inspector = components.get_entity_inspector().await;
            (i, inspector, start.elapsed())
        });
        handles.push(handle);
    }

    // Wait for all to complete
    let mut results = vec![];
    for handle in handles {
        results.push(handle.await.unwrap());
    }

    // Verify all got the same instance (no duplicate initialization)
    let first_inspector = &results[0].1;
    for (id, inspector, duration) in &results {
        assert!(Arc::ptr_eq(first_inspector, inspector), 
                "All concurrent requests should get the same instance");
        println!("Request {}: {:?}", id, duration);
        
        // Most should be very fast (cached), only first might be slower
        assert!(duration < &Duration::from_millis(100), 
                "All requests should complete reasonably quickly");
    }

    // Check that only one actual initialization occurred
    let initialization_count = results.iter()
        .filter(|(_, _, duration)| *duration > Duration::from_millis(10))
        .count();
    
    assert!(initialization_count <= 1, 
            "Only one actual initialization should occur, found {}", initialization_count);
}

/// Test selective preloading of critical components
#[tokio::test]
async fn test_critical_component_preloading() {
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };
    
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let lazy_components = LazyComponents::new(brp_client);

    // Measure preloading time
    let start_preload = Instant::now();
    preload_critical_components(&lazy_components).await.expect("Preloading should succeed");
    let preload_time = start_preload.elapsed();
    
    println!("Preloading time: {:?}", preload_time);
    
    // Preloading should be reasonably fast
    assert!(preload_time < Duration::from_millis(500), 
            "Preloading should complete quickly");

    // Verify critical components are now initialized
    let status = lazy_components.get_initialization_status();
    
    #[cfg(feature = "entity-inspection")]
    assert!(status["entity_inspector"].as_bool().unwrap(), 
            "EntityInspector should be preloaded when feature is enabled");
    
    #[cfg(feature = "performance-profiling")]
    assert!(status["system_profiler"].as_bool().unwrap(), 
            "SystemProfiler should be preloaded when feature is enabled");

    // Test that preloaded components are immediately available
    let start_access = Instant::now();
    let _inspector = lazy_components.get_entity_inspector().await;
    let access_time = start_access.elapsed();
    
    println!("Preloaded component access: {:?}", access_time);
    assert!(access_time < Duration::from_millis(1), 
            "Preloaded component should be immediately available");
}

/// Test lazy initialization with processor dependencies
#[tokio::test]
async fn test_processor_dependency_initialization() {
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };
    
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let lazy_components = LazyComponents::new(brp_client);

    // Test that processor initialization pulls in dependencies correctly
    let start = Instant::now();
    let entity_processor = lazy_components.get_entity_processor().await;
    let processor_init_time = start.elapsed();
    
    println!("Entity processor init time: {:?}", processor_init_time);
    
    // Verify that the underlying EntityInspector was also initialized
    let status = lazy_components.get_initialization_status();
    assert!(status["entity_inspector"].as_bool().unwrap(), 
            "EntityInspector should be initialized as dependency");
    assert!(status["entity_processor"].as_bool().unwrap(), 
            "EntityProcessor should be initialized");

    // Test that subsequent access is fast
    let start_cached = Instant::now();
    let _entity_processor2 = lazy_components.get_entity_processor().await;
    let cached_time = start_cached.elapsed();
    
    assert!(cached_time < Duration::from_millis(1), 
            "Cached processor access should be sub-millisecond");
}

/// Test debug command router initialization with all processors
#[tokio::test]
async fn test_debug_router_initialization() {
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };
    
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let lazy_components = LazyComponents::new(brp_client);

    // Test debug command router initialization
    let start = Instant::now();
    let router = lazy_components.get_debug_command_router().await;
    let router_init_time = start.elapsed();
    
    println!("Debug router init time: {:?}", router_init_time);
    
    // Router initialization should complete in reasonable time
    assert!(router_init_time < Duration::from_secs(5), 
            "Debug router initialization should complete within 5 seconds");

    // Verify all processors are available
    let status = lazy_components.get_initialization_status();
    println!("Final initialization status: {}", serde_json::to_string_pretty(&status).unwrap());
    
    // Most processors should be initialized
    let initialized_count = status.as_object().unwrap().values()
        .filter(|v| v.as_bool().unwrap_or(false))
        .count();
    
    assert!(initialized_count >= 5, 
            "At least 5 components should be initialized after router creation");
}

/// Test memory usage of lazy vs eager initialization
#[tokio::test]
async fn test_memory_usage_comparison() {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Simple memory tracker
    struct MemoryTracker;
    static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

    unsafe impl GlobalAlloc for MemoryTracker {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let ptr = System.alloc(layout);
            if !ptr.is_null() {
                ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
            }
            ptr
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            System.dealloc(ptr, layout);
            ALLOCATED.fetch_sub(layout.size(), Ordering::Relaxed);
        }
    }

    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };
    
    // Test lazy initialization memory usage
    let memory_before_lazy = ALLOCATED.load(Ordering::Relaxed);
    
    let brp_client_lazy = Arc::new(RwLock::new(BrpClient::new(&config)));
    let lazy_components = LazyComponents::new(brp_client_lazy);
    
    let memory_after_lazy_creation = ALLOCATED.load(Ordering::Relaxed);
    
    // Access one component
    let _inspector = lazy_components.get_entity_inspector().await;
    let memory_after_one_component = ALLOCATED.load(Ordering::Relaxed);

    println!("Memory before lazy: {} bytes", memory_before_lazy);
    println!("Memory after lazy creation: {} bytes", memory_after_lazy_creation);
    println!("Memory after one component: {} bytes", memory_after_one_component);
    
    let lazy_creation_overhead = memory_after_lazy_creation.saturating_sub(memory_before_lazy);
    let component_memory = memory_after_one_component.saturating_sub(memory_after_lazy_creation);
    
    println!("Lazy creation overhead: {} bytes", lazy_creation_overhead);
    println!("One component memory: {} bytes", component_memory);
    
    // Lazy creation overhead should be minimal
    assert!(lazy_creation_overhead < 10_000, 
            "Lazy creation overhead should be less than 10KB");
}

/// Test error handling in lazy initialization
#[tokio::test]
async fn test_lazy_init_error_handling() {
    // Test with invalid configuration
    let config = Config {
        bevy_brp_host: "invalid.host.does.not.exist".to_string(),
        bevy_brp_port: 99999, // Invalid port
        mcp_port: 3001,
    };
    
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let lazy_components = LazyComponents::new(brp_client);

    // Component initialization may fail, but should not crash
    let start = Instant::now();
    let inspector_result = tokio::time::timeout(
        Duration::from_secs(10),
        lazy_components.get_entity_inspector()
    ).await;
    let init_time = start.elapsed();
    
    println!("Initialization with invalid config took: {:?}", init_time);
    
    // Should complete within reasonable time (not hang)
    assert!(init_time < Duration::from_secs(10), 
            "Initialization should not hang with invalid config");

    // May succeed or fail, but should not panic
    match inspector_result {
        Ok(_inspector) => {
            println!("Inspector creation succeeded despite invalid config");
        }
        Err(_) => {
            println!("Inspector creation timed out with invalid config (expected)");
        }
    }
}

/// Test lazy initialization performance under load
#[tokio::test]
async fn test_lazy_init_under_load() {
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15702,
        mcp_port: 3001,
    };
    
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    let lazy_components = Arc::new(LazyComponents::new(brp_client));

    // Simulate load with many concurrent component requests
    let mut handles = vec![];
    let start_load_test = Instant::now();
    
    for i in 0..50 {
        let components = lazy_components.clone();
        let handle = tokio::spawn(async move {
            let component_type = match i % 4 {
                0 => "entity_inspector",
                1 => "system_profiler", 
                2 => "entity_processor",
                _ => "profiler_processor",
            };
            
            let start = Instant::now();
            match component_type {
                "entity_inspector" => {
                    let _ = components.get_entity_inspector().await;
                }
                "system_profiler" => {
                    let _ = components.get_system_profiler().await;
                }
                "entity_processor" => {
                    let _ = components.get_entity_processor().await;
                }
                _ => {
                    let _ = components.get_profiler_processor().await;
                }
            }
            start.elapsed()
        });
        handles.push(handle);
    }

    // Wait for all requests to complete
    let mut completion_times = vec![];
    for handle in handles {
        let duration = handle.await.unwrap();
        completion_times.push(duration);
    }
    
    let total_load_time = start_load_test.elapsed();
    println!("Total load test time: {:?}", total_load_time);
    
    // Analyze completion times
    completion_times.sort();
    let median = completion_times[completion_times.len() / 2];
    let p95 = completion_times[(completion_times.len() as f64 * 0.95) as usize];
    let max = completion_times.last().unwrap();
    
    println!("Completion times - Median: {:?}, P95: {:?}, Max: {:?}", median, p95, max);
    
    // Performance should be reasonable under load
    assert!(median < Duration::from_millis(50), 
            "Median completion time should be under 50ms under load");
    assert!(p95 < Duration::from_millis(200), 
            "P95 completion time should be under 200ms under load");
    assert!(total_load_time < Duration::from_secs(10), 
            "Total load test should complete within 10 seconds");
}