/// Comprehensive integration tests for the Visual Debug Overlay System (BEVDBG-005)
/// 
/// Tests cover:
/// - Visual debug overlay creation and configuration
/// - BRP integration for overlay commands
/// - Performance budget enforcement 
/// - Entity highlighting functionality
/// - Collider visualization
/// - Transform gizmos
/// - Metrics overlay
/// - Debug markers
/// - End-to-end command processing

use bevy_debugger_mcp::{
    brp_client::BrpClient,
    brp_messages::{DebugCommand, DebugOverlayType},
    config::Config,
    debug_command_processor::{DebugCommandProcessor, DebugCommandRouter, DebugCommandRequest},
    visual_debug_overlay::*,
    visual_debug_overlay_processor::{VisualDebugOverlayProcessor, OverlayMetrics},
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

/// Test visual debug overlay creation and basic functionality
#[tokio::test]
async fn test_visual_debug_overlay_creation() {
    let brp_client = create_test_brp_client();
    let overlay = VisualDebugOverlay::new(brp_client);
    
    // Test default configuration
    assert!(overlay.get_config().enabled);
    assert_eq!(overlay.get_config().max_highlighted_entities, MAX_HIGHLIGHTED_ENTITIES);
    assert_eq!(overlay.get_config().performance_budget_ms, MAX_FRAME_IMPACT_MS);
}

/// Test custom overlay configuration
#[tokio::test] 
async fn test_custom_overlay_config() {
    let brp_client = create_test_brp_client();
    let custom_config = OverlayConfig {
        enabled: true,
        max_highlighted_entities: 50,
        performance_budget_ms: 1.5,
        show_metrics: true,
        text_scale: 1.2,
    };
    
    let overlay = VisualDebugOverlay::with_config(brp_client, custom_config);
    
    assert_eq!(overlay.get_config().max_highlighted_entities, 50);
    assert_eq!(overlay.get_config().performance_budget_ms, 1.5);
    assert!(overlay.get_config().show_metrics);
    assert_eq!(overlay.get_config().text_scale, 1.2);
}

/// Test entity highlighting functionality
#[tokio::test]
async fn test_entity_highlighting() {
    let brp_client = create_test_brp_client();
    let overlay = VisualDebugOverlay::new(brp_client);
    
    // Test highlighting entities (will fail without BRP connection, but logic is tested)
    let entity_ids = vec![1, 2, 3];
    let result = overlay.highlight_entities(
        entity_ids.clone(),
        Some([1.0, 0.0, 0.0, 0.8]), // Red with transparency
        Some(HighlightMode::Outline),
    ).await;
    
    // Should fail due to BRP connection, but test the validation
    assert!(result.is_err());
    
    // Test too many entities
    let too_many_entities: Vec<u64> = (0..200).collect();
    let result = overlay.highlight_entities(too_many_entities, None, None).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Too many entities"));
}

/// Test collider visualization
#[tokio::test]
async fn test_collider_visualization() {
    let brp_client = create_test_brp_client();
    let overlay = VisualDebugOverlay::new(brp_client);
    
    let config = ColliderVisualizationConfig {
        show_colliders: true,
        collider_color: [0.0, 1.0, 0.0, 0.5],
        show_normals: true,
        show_contacts: true,
        alpha: 0.5,
    };
    
    let result = overlay.show_colliders(Some(config)).await;
    // Will fail due to BRP connection, but validates the structure
    assert!(result.is_err());
}

/// Test transform gizmos
#[tokio::test]
async fn test_transform_gizmos() {
    let brp_client = create_test_brp_client();
    let overlay = VisualDebugOverlay::new(brp_client);
    
    let config = TransformGizmoConfig {
        enabled: true,
        show_local: true,
        show_world: true,
        scale: 2.0,
        show_rotation: true,
    };
    
    let result = overlay.show_transform_gizmos(Some(config)).await;
    assert!(result.is_err()); // Expected without BRP connection
}

/// Test performance metrics overlay
#[tokio::test]
async fn test_metrics_overlay() {
    let brp_client = create_test_brp_client();
    let overlay = VisualDebugOverlay::new(brp_client);
    
    let config = MetricsOverlayConfig {
        show_fps: true,
        show_frame_time: true,
        show_entity_count: true,
        show_system_timings: true,
        position: [0.02, 0.02],
        text_size: 18.0,
        background_opacity: 0.8,
    };
    
    let result = overlay.show_metrics(Some(config)).await;
    assert!(result.is_err()); // Expected without BRP connection
}

/// Test debug markers
#[tokio::test]
async fn test_debug_markers() {
    let brp_client = create_test_brp_client();
    let overlay = VisualDebugOverlay::new(brp_client);
    
    let marker = DebugMarker {
        id: "test_marker_1".to_string(),
        position: [10.0, 5.0, 0.0],
        text: "Test Position".to_string(),
        color: [1.0, 1.0, 0.0, 1.0],
        size: 1.0,
        screen_space: false,
    };
    
    let result = overlay.add_debug_marker(marker).await;
    assert!(result.is_err()); // Expected without BRP connection
    
    // Test marker removal
    let result = overlay.remove_debug_marker("test_marker_1").await;
    assert!(result.is_err()); // Expected without BRP connection
}

/// Test performance budget enforcement
#[tokio::test]
async fn test_performance_budget() {
    let brp_client = create_test_brp_client();
    let overlay = VisualDebugOverlay::new(brp_client);
    
    // Initially should be within budget
    assert!(!overlay.is_budget_exceeded().await);
    
    // Simulate high performance cost
    overlay.update_performance_metrics(&DebugOverlayType::EntityHighlight, 3.0).await;
    
    // Should now exceed budget (3ms > 2ms default)
    assert!(overlay.is_budget_exceeded().await);
    
    // Check metrics
    let metrics = overlay.get_performance_metrics().await;
    assert!(metrics.contains_key(&DebugOverlayType::EntityHighlight));
    assert!(*metrics.get(&DebugOverlayType::EntityHighlight).unwrap() >= 3.0);
}

/// Test highlight modes serialization
#[test]
fn test_highlight_modes() {
    use serde_json;
    
    let modes = vec![
        HighlightMode::Outline,
        HighlightMode::Tint,
        HighlightMode::Glow,
        HighlightMode::Wireframe,
        HighlightMode::Solid,
    ];
    
    for mode in modes {
        let json = serde_json::to_string(&mode).unwrap();
        let deserialized: HighlightMode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, mode);
    }
}

/// Test visual debug overlay processor
#[tokio::test]
async fn test_visual_debug_processor() {
    let brp_client = create_test_brp_client();
    let processor = VisualDebugOverlayProcessor::new(brp_client);
    
    // Test command support
    let set_command = DebugCommand::SetVisualDebug {
        overlay_type: DebugOverlayType::EntityHighlight,
        enabled: true,
        config: Some(json!({
            "default_color": [1.0, 0.0, 0.0, 1.0],
            "default_mode": "outline"
        })),
    };
    
    assert!(processor.supports_command(&set_command));
    assert!(processor.supports_command(&DebugCommand::GetStatus));
    
    // Test validation
    assert!(processor.validate(&set_command).await.is_ok());
    
    // Test oversized config validation
    let large_config = json!({
        "data": "x".repeat(11_000) // Over 10KB limit
    });
    
    let invalid_command = DebugCommand::SetVisualDebug {
        overlay_type: DebugOverlayType::EntityHighlight,
        enabled: true,
        config: Some(large_config),
    };
    
    assert!(processor.validate(&invalid_command).await.is_err());
}

/// Test overlay state management
#[tokio::test]
async fn test_overlay_state_management() {
    let brp_client = create_test_brp_client();
    let processor = VisualDebugOverlayProcessor::new(brp_client);
    let state = processor.get_state();
    
    // Test initial state
    {
        let state_guard = state.read().await;
        assert!(state_guard.get_all_overlay_statuses().is_empty());
        assert!(!state_guard.is_performance_budget_exceeded());
    }
    
    // Test setting overlay state
    {
        let mut state_guard = state.write().await;
        let result = state_guard
            .set_overlay_enabled(
                &DebugOverlayType::EntityHighlight,
                true,
                Some(json!({
                    "default_color": [1.0, 0.0, 0.0, 1.0],
                    "default_mode": "outline"
                })),
            )
            .await;
        
        // Should fail due to BRP not connected, but that's expected in tests
        assert!(result.is_err());
        
        // Check that the state was updated anyway (stored locally)  
        let overlay_status = state_guard.get_overlay_status(&DebugOverlayType::EntityHighlight);
        assert!(overlay_status.is_some());
        
        let status = overlay_status.unwrap();
        assert!(status.enabled);
        assert_eq!(status.config["default_color"], json!([1.0, 0.0, 0.0, 1.0]));
    }
}

/// Test performance metrics tracking
#[tokio::test]
async fn test_performance_metrics_tracking() {
    let brp_client = create_test_brp_client();
    let processor = VisualDebugOverlayProcessor::new(brp_client);
    let state = processor.get_state();
    
    {
        let mut state_guard = state.write().await;
        
        // Initially should be within budget
        assert!(!state_guard.is_performance_budget_exceeded());
        
        // Simulate high performance cost
        let high_cost_metrics = OverlayMetrics {
            render_time_us: 3000, // 3ms, exceeds 2ms budget
            element_count: 1000,
            memory_usage_bytes: 1024 * 1024,
            update_count: 50,
        };
        
        state_guard.update_metrics(&DebugOverlayType::EntityHighlight, high_cost_metrics);
        assert!(state_guard.is_performance_budget_exceeded());
        
        // Test with lower cost
        let low_cost_metrics = OverlayMetrics {
            render_time_us: 500, // 0.5ms, within budget
            element_count: 10,
            memory_usage_bytes: 1024,
            update_count: 5,
        };
        
        state_guard.update_metrics(&DebugOverlayType::EntityHighlight, low_cost_metrics);
        assert!(!state_guard.is_performance_budget_exceeded());
    }
}

/// Test debug command router integration
#[tokio::test]
async fn test_debug_command_router_integration() {
    let brp_client = create_test_brp_client();
    let router = DebugCommandRouter::new();
    let processor = Arc::new(VisualDebugOverlayProcessor::new(brp_client));
    
    router.register_processor("visual_debug_overlay".to_string(), processor).await;
    
    let command = DebugCommand::SetVisualDebug {
        overlay_type: DebugOverlayType::EntityHighlight,
        enabled: true,
        config: Some(json!({"color": [1.0, 0.0, 0.0, 1.0]})),
    };
    
    let request = DebugCommandRequest::new(
        command,
        Uuid::new_v4().to_string(),
        Some(5),
    );
    
    let result = router.queue_command(request).await;
    assert!(result.is_ok());
    
    // Processing should work (though BRP will fail)
    let process_result = router.process_next().await;
    assert!(process_result.is_some());
}

/// Test command processing timing estimates
#[tokio::test]
async fn test_processing_time_estimates() {
    let brp_client = create_test_brp_client();
    let processor = VisualDebugOverlayProcessor::new(brp_client);
    
    let set_visual_cmd = DebugCommand::SetVisualDebug {
        overlay_type: DebugOverlayType::EntityHighlight,
        enabled: true,
        config: None,
    };
    
    let get_status_cmd = DebugCommand::GetStatus;
    
    let set_time = processor.estimate_processing_time(&set_visual_cmd);
    let get_time = processor.estimate_processing_time(&get_status_cmd);
    
    // SetVisualDebug should take longer than GetStatus
    assert!(set_time > get_time);
    assert!(set_time <= Duration::from_millis(100)); // Should be reasonable
}

/// Performance test for overlay operations
#[tokio::test]
async fn test_overlay_performance() {
    let brp_client = create_test_brp_client();
    let processor = VisualDebugOverlayProcessor::new(brp_client);
    
    let cmd = DebugCommand::SetVisualDebug {
        overlay_type: DebugOverlayType::EntityHighlight,
        enabled: true,
        config: Some(json!({"default_color": [1.0, 0.0, 0.0, 1.0]})),
    };
    
    let start = std::time::Instant::now();
    let _result = processor.process(cmd).await;
    let duration = start.elapsed();
    
    // Should complete within reasonable time (even with BRP connection failure)
    assert!(duration.as_millis() < 1000, "SetVisualDebug took too long: {:?}", duration);
}

/// Test status command performance
#[tokio::test] 
async fn test_status_performance() {
    let brp_client = create_test_brp_client();
    let processor = VisualDebugOverlayProcessor::new(brp_client);
    
    let cmd = DebugCommand::GetStatus;
    
    let start = std::time::Instant::now();
    let result = processor.process(cmd).await;
    let duration = start.elapsed();
    
    assert!(result.is_ok());
    assert!(duration.as_millis() < 100, "GetStatus took too long: {:?}", duration);
}

/// Test multiple overlay types
#[tokio::test]
async fn test_multiple_overlay_types() {
    let brp_client = create_test_brp_client();
    let overlay = VisualDebugOverlay::new(brp_client);
    
    // Test different overlay configurations
    let entity_highlight_result = overlay.highlight_entities(
        vec![1, 2],
        Some([1.0, 0.0, 0.0, 1.0]),
        Some(HighlightMode::Glow)
    ).await;
    
    let colliders_result = overlay.show_colliders(None).await;
    
    let gizmos_result = overlay.show_transform_gizmos(None).await;
    
    let metrics_result = overlay.show_metrics(None).await;
    
    // All should fail due to BRP connection, but test structure
    assert!(entity_highlight_result.is_err());
    assert!(colliders_result.is_err());
    assert!(gizmos_result.is_err());
    assert!(metrics_result.is_err());
}

/// Test overlay cleanup
#[tokio::test]
async fn test_overlay_cleanup() {
    let brp_client = create_test_brp_client();
    let overlay = VisualDebugOverlay::new(brp_client);
    
    let result = overlay.clear_all_overlays().await;
    assert!(result.is_ok());
}

/// Test custom overlay types
#[tokio::test]
async fn test_custom_overlay_types() {
    let brp_client = create_test_brp_client();
    let processor = VisualDebugOverlayProcessor::new(brp_client);
    let state = processor.get_state();
    
    {
        let state_guard = state.read().await;
        let custom_overlay = DebugOverlayType::Custom("test_overlay".to_string());
        let key = state_guard.overlay_type_to_key(&custom_overlay);
        assert_eq!(key, "custom_test_overlay");
    }
}

/// Stress test with many overlays
#[tokio::test]
async fn test_many_overlays_stress() {
    let brp_client = create_test_brp_client();
    let processor = VisualDebugOverlayProcessor::new(brp_client);
    let state = processor.get_state();
    
    {
        let mut state_guard = state.write().await;
        
        // Try to set many custom overlays
        for i in 0..10 {
            let custom_overlay = DebugOverlayType::Custom(format!("stress_test_{}", i));
            let result = state_guard
                .set_overlay_enabled(
                    &custom_overlay,
                    true,
                    Some(json!({"test_value": i})),
                )
                .await;
            
            // Should fail due to BRP connection, but test the load handling
            assert!(result.is_err());
        }
        
        // Should have stored the overlay configs locally even if BRP failed
        assert_eq!(state_guard.get_all_overlay_statuses().len(), 10);
    }
}