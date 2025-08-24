/// Integration tests for visual debug overlay functionality
/// 
/// These tests verify that the visual debug overlay system works correctly
/// with the BRP communication layer and can handle various overlay types
/// and configurations.

use bevy_debugger_mcp::{
    brp_client::BrpClient,
    brp_messages::{DebugCommand, DebugOverlayType},
    config::Config,
    visual_debug_overlay_processor::VisualDebugOverlayProcessor,
    debug_command_processor::DebugCommandProcessor,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(feature = "visual_overlays")]
use bevy_debugger_mcp::visual_overlays::{
    OverlayMetrics, VisualOverlayManager, VisualDebugOverlayPlugin,
};

#[cfg(feature = "visual_overlays")]
use bevy_debugger_mcp::visual_overlays::entity_highlight::{HighlightMode, HighlightConfig};

/// Create a test configuration for BRP client
fn create_test_config() -> Config {
    let mut config = Config::default();
    config.bevy_brp_host = "localhost".to_string();
    config.bevy_brp_port = 15702;
    config.mcp_port = 3000;
    config
}

/// Create a test visual debug overlay processor
async fn create_test_processor() -> VisualDebugOverlayProcessor {
    let config = create_test_config();
    let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
    VisualDebugOverlayProcessor::new(brp_client)
}

#[tokio::test]
async fn test_visual_overlay_processor_creation() {
    let processor = create_test_processor().await;
    
    // Verify that the processor was created successfully
    assert!(processor.get_state().read().await.get_all_overlay_statuses().is_empty());
}

#[tokio::test]
async fn test_supports_visual_debug_commands() {
    let processor = create_test_processor().await;
    
    let set_visual_debug_cmd = DebugCommand::SetVisualDebug {
        overlay_type: DebugOverlayType::EntityHighlight,
        enabled: true,
        config: Some(json!({
            "default_color": [1.0, 0.0, 0.0, 1.0],
            "default_mode": "outline"
        })),
    };
    
    let get_status_cmd = DebugCommand::GetStatus;
    
    assert!(processor.supports_command(&set_visual_debug_cmd));
    assert!(processor.supports_command(&get_status_cmd));
    
    // This processor only supports SetVisualDebug and GetStatus
    // All other commands should return false
}

#[tokio::test]
async fn test_validate_overlay_config_size() {
    let processor = create_test_processor().await;
    
    // Test valid config
    let valid_cmd = DebugCommand::SetVisualDebug {
        overlay_type: DebugOverlayType::EntityHighlight,
        enabled: true,
        config: Some(json!({
            "default_color": [1.0, 0.0, 0.0, 1.0],
            "default_mode": "outline",
            "max_highlighted": 50
        })),
    };
    
    assert!(processor.validate(&valid_cmd).await.is_ok());
    
    // Test oversized config (over 10KB limit)
    let large_config = json!({
        "data": "x".repeat(11_000) // Over 10KB
    });
    
    let invalid_cmd = DebugCommand::SetVisualDebug {
        overlay_type: DebugOverlayType::EntityHighlight,
        enabled: true,
        config: Some(large_config),
    };
    
    assert!(processor.validate(&invalid_cmd).await.is_err());
}

#[tokio::test]
async fn test_processing_time_estimation() {
    let processor = create_test_processor().await;
    
    let set_visual_cmd = DebugCommand::SetVisualDebug {
        overlay_type: DebugOverlayType::EntityHighlight,
        enabled: true,
        config: None,
    };
    
    let get_status_cmd = DebugCommand::GetStatus;
    
    // SetVisualDebug should take longer than GetStatus
    let set_time = processor.estimate_processing_time(&set_visual_cmd);
    let get_time = processor.estimate_processing_time(&get_status_cmd);
    
    assert!(set_time > get_time);
    assert!(set_time.as_millis() <= 100); // Should be reasonable
}

#[tokio::test]
async fn test_overlay_state_management() {
    let processor = create_test_processor().await;
    let state = processor.get_state();
    
    // Test initial state
    {
        let state_guard = state.read().await;
        assert!(state_guard.get_all_overlay_statuses().is_empty());
        assert!(!state_guard.is_performance_budget_exceeded());
    }
    
    // Test setting overlay state (will fail due to no BRP connection, but state should be updated)
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

// Overlay type to key conversion is tested internally via the integration

#[tokio::test]
async fn test_performance_budget_tracking() {
    let processor = create_test_processor().await;
    let state = processor.get_state();
    
    {
        let mut state_guard = state.write().await;
        
        // Initially should be within budget
        assert!(!state_guard.is_performance_budget_exceeded());
        
        // Simulate high performance cost
        use bevy_debugger_mcp::visual_debug_overlay_processor::OverlayMetrics;
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

/// Tests for the visual overlays feature when enabled
#[cfg(feature = "visual_overlays")]
mod visual_overlays_tests {
    use super::*;
    
    #[test]
    fn test_overlay_metrics_serialization() {
        let metrics = OverlayMetrics {
            render_time_us: 1500,
            element_count: 25,
            memory_usage_bytes: 4096,
            frame_updates: 3,
            active_this_frame: true,
        };
        
        // Test JSON serialization
        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: OverlayMetrics = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.render_time_us, 1500);
        assert_eq!(deserialized.element_count, 25);
        assert_eq!(deserialized.memory_usage_bytes, 4096);
        assert_eq!(deserialized.frame_updates, 3);
        assert!(deserialized.active_this_frame);
    }
    
    #[test]
    fn test_highlight_mode_serialization() {
        use bevy_debugger_mcp::visual_overlays::entity_highlight::HighlightMode;
        
        let modes = vec![
            HighlightMode::Outline,
            HighlightMode::Tint,
            HighlightMode::Glow,
            HighlightMode::Wireframe,
            HighlightMode::SolidColor,
        ];
        
        for mode in modes {
            let json = serde_json::to_string(&mode).unwrap();
            let deserialized: HighlightMode = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, mode);
        }
    }
    
    #[test]
    fn test_highlight_config_json_update() {
        
        let mut config = HighlightConfig::new();
        
        let json_config = json!({
            "default_color": [1.0, 0.0, 0.0, 1.0],
            "default_mode": "glow",
            "max_highlighted": 50,
            "outline_thickness": 0.05,
            "glow_intensity": 2.0,
            "animation_speed": 3.0,
            "show_info_ui": false
        });
        
        assert!(config.update_from_json(&json_config).is_ok());
        
        assert_eq!(config.default_mode, HighlightMode::Glow);
        assert_eq!(config.max_highlighted, 50);
        assert_eq!(config.outline_thickness, 0.05);
        assert_eq!(config.glow_intensity, 2.0);
        assert_eq!(config.animation_speed, 3.0);
        assert!(!config.show_info_ui);
    }
    
    #[test]
    fn test_visual_overlay_manager_creation() {
        let manager = VisualOverlayManager::new();
        
        // Test initial state
        assert!(!manager.is_performance_budget_exceeded());
        assert_eq!(manager.get_total_metrics().render_time_us, 0);
        
        // Test that the manager was created successfully
        assert!(manager.get_all_statuses().is_empty());
    }
}

/// Benchmark tests for performance validation
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;
    
    #[tokio::test]
    async fn test_set_visual_debug_performance() {
        let processor = create_test_processor().await;
        
        let cmd = DebugCommand::SetVisualDebug {
            overlay_type: DebugOverlayType::EntityHighlight,
            enabled: true,
            config: Some(json!({
                "default_color": [1.0, 0.0, 0.0, 1.0],
                "default_mode": "outline"
            })),
        };
        
        let start = Instant::now();
        let _result = processor.process(cmd).await;
        let duration = start.elapsed();
        
        // Should complete within reasonable time (even with BRP connection failure)
        assert!(duration.as_millis() < 1000, "SetVisualDebug took too long: {:?}", duration);
    }
    
    #[tokio::test]
    async fn test_get_status_performance() {
        let processor = create_test_processor().await;
        
        let cmd = DebugCommand::GetStatus;
        
        let start = Instant::now();
        let result = processor.process(cmd).await;
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        assert!(duration.as_millis() < 100, "GetStatus took too long: {:?}", duration);
    }
}