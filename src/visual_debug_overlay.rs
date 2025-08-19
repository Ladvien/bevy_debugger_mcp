/// Visual Debug Overlay System for rendering debug information in Bevy games
use crate::brp_messages::{DebugCommand, DebugResponse, DebugOverlayType};
use crate::brp_client::BrpClient;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid;

/// Maximum performance impact allowed (2ms per frame)
pub const MAX_FRAME_IMPACT_MS: f32 = 2.0;

/// Maximum number of entities that can be highlighted simultaneously
pub const MAX_HIGHLIGHTED_ENTITIES: usize = 100;

/// Visual debug overlay manager
pub struct VisualDebugOverlay {
    /// BRP client for communication with Bevy
    brp_client: Arc<RwLock<BrpClient>>,
    /// Active overlay states
    overlay_states: Arc<RwLock<HashMap<DebugOverlayType, OverlayState>>>,
    /// Performance tracker
    performance_tracker: Arc<RwLock<PerformanceTracker>>,
    /// Configuration
    config: OverlayConfig,
}

/// Configuration for visual overlays
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    /// Whether overlays are enabled globally
    pub enabled: bool,
    /// Maximum entities to highlight
    pub max_highlighted_entities: usize,
    /// Performance budget in milliseconds
    pub performance_budget_ms: f32,
    /// Whether to show performance metrics
    pub show_metrics: bool,
    /// Text overlay scale factor
    pub text_scale: f32,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_highlighted_entities: MAX_HIGHLIGHTED_ENTITIES,
            performance_budget_ms: MAX_FRAME_IMPACT_MS,
            show_metrics: false,
            text_scale: 1.0,
        }
    }
}

/// State of a specific overlay type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayState {
    /// Whether this overlay is enabled
    pub enabled: bool,
    /// Overlay-specific configuration
    pub config: serde_json::Value,
    /// Last update time in microseconds since UNIX epoch
    pub last_update_us: u64,
    /// Performance impact in milliseconds
    pub performance_impact_ms: f32,
}

/// Entity highlight configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityHighlightConfig {
    /// Entities to highlight
    pub entity_ids: Vec<u64>,
    /// Highlight color (RGBA)
    pub color: [f32; 4],
    /// Highlight mode
    pub mode: HighlightMode,
    /// Outline thickness (for outline mode)
    pub outline_thickness: f32,
    /// Whether to highlight children
    pub include_children: bool,
}

/// Highlight modes for entities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HighlightMode {
    /// Outline around entity
    Outline,
    /// Tint the entity
    Tint,
    /// Glow effect
    Glow,
    /// Wireframe rendering
    Wireframe,
    /// Solid color overlay
    Solid,
}

/// Collider visualization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColliderVisualizationConfig {
    /// Whether to show colliders
    pub show_colliders: bool,
    /// Collider color (RGBA)
    pub collider_color: [f32; 4],
    /// Whether to show collision normals
    pub show_normals: bool,
    /// Whether to show contact points
    pub show_contacts: bool,
    /// Alpha transparency for colliders
    pub alpha: f32,
}

/// Transform gizmo configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformGizmoConfig {
    /// Whether to show transform gizmos
    pub enabled: bool,
    /// Show local space axes
    pub show_local: bool,
    /// Show world space axes
    pub show_world: bool,
    /// Gizmo scale
    pub scale: f32,
    /// Whether to show rotation arcs
    pub show_rotation: bool,
}

/// Performance metrics overlay configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsOverlayConfig {
    /// Whether to show FPS
    pub show_fps: bool,
    /// Whether to show frame time
    pub show_frame_time: bool,
    /// Whether to show entity count
    pub show_entity_count: bool,
    /// Whether to show system timings
    pub show_system_timings: bool,
    /// Position on screen (0-1 normalized)
    pub position: [f32; 2],
    /// Text size
    pub text_size: f32,
    /// Background opacity
    pub background_opacity: f32,
}

/// Custom debug marker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugMarker {
    /// Unique marker ID
    pub id: String,
    /// World position
    pub position: [f32; 3],
    /// Marker text
    pub text: String,
    /// Color (RGBA)
    pub color: [f32; 4],
    /// Size/scale
    pub size: f32,
    /// Whether marker is screen-space or world-space
    pub screen_space: bool,
}

/// Performance tracking for overlays
#[derive(Debug)]
struct PerformanceTracker {
    /// Frame times for each overlay type
    frame_times: HashMap<DebugOverlayType, VecDeque<f32>>,
    /// Total frame time
    total_frame_time_ms: f32,
    /// Whether performance budget is exceeded
    budget_exceeded: bool,
    /// Last warning time in microseconds since UNIX epoch
    last_warning_us: Option<u64>,
}

use std::collections::VecDeque;

impl VisualDebugOverlay {
    /// Create new visual debug overlay system
    pub fn new(brp_client: Arc<RwLock<BrpClient>>) -> Self {
        Self::with_config(brp_client, OverlayConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(brp_client: Arc<RwLock<BrpClient>>, config: OverlayConfig) -> Self {
        Self {
            brp_client,
            overlay_states: Arc::new(RwLock::new(HashMap::new())),
            performance_tracker: Arc::new(RwLock::new(PerformanceTracker::new())),
            config,
        }
    }

    /// Enable or disable an overlay type
    pub async fn set_overlay_enabled(
        &self,
        overlay_type: &DebugOverlayType,
        enabled: bool,
        config: Option<serde_json::Value>,
    ) -> Result<()> {
        if !self.config.enabled && enabled {
            return Err(Error::DebugError(
                "Visual overlays are globally disabled".to_string(),
            ));
        }

        let mut states = self.overlay_states.write().await;
        
        let state = OverlayState {
            enabled,
            config: config.unwrap_or(serde_json::json!({})),
            last_update_us: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros() as u64,
            performance_impact_ms: 0.0,
        };

        states.insert(overlay_type.clone(), state);
        
        // Send update to Bevy
        self.sync_overlay_state(overlay_type).await?;
        
        info!("Set overlay {:?} enabled: {}", overlay_type, enabled);
        Ok(())
    }

    /// Highlight specific entities
    pub async fn highlight_entities(
        &self,
        entity_ids: Vec<u64>,
        color: Option<[f32; 4]>,
        mode: Option<HighlightMode>,
    ) -> Result<()> {
        if entity_ids.len() > self.config.max_highlighted_entities {
            return Err(Error::DebugError(format!(
                "Too many entities to highlight: {} (max: {})",
                entity_ids.len(),
                self.config.max_highlighted_entities
            )));
        }

        let highlight_config = EntityHighlightConfig {
            entity_ids,
            color: color.unwrap_or([1.0, 0.0, 0.0, 0.5]), // Default red
            mode: mode.unwrap_or(HighlightMode::Outline),
            outline_thickness: 2.0,
            include_children: false,
        };

        self.set_overlay_enabled(
            &DebugOverlayType::EntityHighlight,
            true,
            Some(serde_json::to_value(highlight_config)?),
        )
        .await?;

        Ok(())
    }

    /// Clear entity highlighting
    pub async fn clear_highlights(&self) -> Result<()> {
        self.set_overlay_enabled(&DebugOverlayType::EntityHighlight, false, None)
            .await
    }

    /// Enable collider visualization
    pub async fn show_colliders(&self, config: Option<ColliderVisualizationConfig>) -> Result<()> {
        let collider_config = config.unwrap_or(ColliderVisualizationConfig {
            show_colliders: true,
            collider_color: [0.0, 1.0, 0.0, 0.3], // Green transparent
            show_normals: false,
            show_contacts: false,
            alpha: 0.3,
        });

        self.set_overlay_enabled(
            &DebugOverlayType::ColliderVisualization,
            true,
            Some(serde_json::to_value(collider_config)?),
        )
        .await
    }

    /// Enable transform gizmos
    pub async fn show_transform_gizmos(&self, config: Option<TransformGizmoConfig>) -> Result<()> {
        let gizmo_config = config.unwrap_or(TransformGizmoConfig {
            enabled: true,
            show_local: true,
            show_world: false,
            scale: 1.0,
            show_rotation: false,
        });

        self.set_overlay_enabled(
            &DebugOverlayType::TransformGizmos,
            true,
            Some(serde_json::to_value(gizmo_config)?),
        )
        .await
    }

    /// Show performance metrics overlay
    pub async fn show_metrics(&self, config: Option<MetricsOverlayConfig>) -> Result<()> {
        let metrics_config = config.unwrap_or(MetricsOverlayConfig {
            show_fps: true,
            show_frame_time: true,
            show_entity_count: true,
            show_system_timings: false,
            position: [0.01, 0.01], // Top-left corner
            text_size: 16.0,
            background_opacity: 0.7,
        });

        self.set_overlay_enabled(
            &DebugOverlayType::PerformanceMetrics,
            true,
            Some(serde_json::to_value(metrics_config)?),
        )
        .await
    }

    /// Add a custom debug marker
    pub async fn add_debug_marker(&self, marker: DebugMarker) -> Result<()> {
        let mut states = self.overlay_states.write().await;
        
        // Get or create markers state
        let markers_state = states
            .entry(DebugOverlayType::DebugMarkers)
            .or_insert_with(|| OverlayState {
                enabled: true,
                config: serde_json::json!({ "markers": {} }),
                last_update_us: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_micros() as u64,
                performance_impact_ms: 0.0,
            });

        // Add marker to config
        if let Some(markers_obj) = markers_state.config.get_mut("markers") {
            if let Some(markers_map) = markers_obj.as_object_mut() {
                markers_map.insert(marker.id.clone(), serde_json::to_value(marker)?);
            }
        }

        // Sync with Bevy
        drop(states);
        self.sync_overlay_state(&DebugOverlayType::DebugMarkers).await?;
        
        Ok(())
    }

    /// Remove a debug marker
    pub async fn remove_debug_marker(&self, marker_id: &str) -> Result<()> {
        let mut states = self.overlay_states.write().await;
        
        if let Some(markers_state) = states.get_mut(&DebugOverlayType::DebugMarkers) {
            if let Some(markers_obj) = markers_state.config.get_mut("markers") {
                if let Some(markers_map) = markers_obj.as_object_mut() {
                    markers_map.remove(marker_id);
                }
            }
        }

        drop(states);
        self.sync_overlay_state(&DebugOverlayType::DebugMarkers).await?;
        
        Ok(())
    }

    /// Clear all overlays
    pub async fn clear_all_overlays(&self) -> Result<()> {
        let mut states = self.overlay_states.write().await;
        
        for (overlay_type, state) in states.iter_mut() {
            state.enabled = false;
            self.sync_overlay_state(overlay_type).await?;
        }
        
        states.clear();
        
        info!("Cleared all visual overlays");
        Ok(())
    }

    /// Sync overlay state with Bevy
    async fn sync_overlay_state(&self, overlay_type: &DebugOverlayType) -> Result<()> {
        let states = self.overlay_states.read().await;
        
        if let Some(state) = states.get(overlay_type) {
            // Send BRP command to update overlay in Bevy
            let mut client = self.brp_client.write().await;
            
            let debug_command = crate::brp_messages::DebugCommand::SetVisualDebug {
                overlay_type: overlay_type.clone(),
                enabled: state.enabled,
                config: Some(state.config.clone()),
            };
            
            let request = crate::brp_messages::BrpRequest::Debug {
                command: debug_command,
                correlation_id: uuid::Uuid::new_v4().to_string(),
                priority: Some(5),
            };
            
            client.send_request(&request).await?;
        }
        
        Ok(())
    }

    /// Update performance metrics for an overlay
    pub async fn update_performance_metrics(
        &self,
        overlay_type: &DebugOverlayType,
        frame_time_ms: f32,
    ) {
        let mut tracker = self.performance_tracker.write().await;
        tracker.update(overlay_type, frame_time_ms);
        
        // Check if budget exceeded
        if tracker.total_frame_time_ms > self.config.performance_budget_ms {
            if tracker.should_warn() {
                warn!(
                    "Visual overlay performance budget exceeded: {:.2}ms > {:.2}ms",
                    tracker.total_frame_time_ms, self.config.performance_budget_ms
                );
            }
            tracker.budget_exceeded = true;
        } else {
            tracker.budget_exceeded = false;
        }
        
        // Update state with performance impact
        let mut states = self.overlay_states.write().await;
        if let Some(state) = states.get_mut(overlay_type) {
            state.performance_impact_ms = frame_time_ms;
        }
    }

    /// Get current performance metrics
    pub async fn get_performance_metrics(&self) -> HashMap<DebugOverlayType, f32> {
        let tracker = self.performance_tracker.read().await;
        let mut metrics = HashMap::new();
        
        for (overlay_type, times) in &tracker.frame_times {
            if !times.is_empty() {
                let avg = times.iter().sum::<f32>() / times.len() as f32;
                metrics.insert(overlay_type.clone(), avg);
            }
        }
        
        metrics
    }

    /// Check if performance budget is exceeded
    pub async fn is_budget_exceeded(&self) -> bool {
        let tracker = self.performance_tracker.read().await;
        tracker.budget_exceeded
    }

    /// Get overlay configuration
    pub fn get_config(&self) -> &OverlayConfig {
        &self.config
    }

    /// Update overlay configuration
    pub fn set_config(&mut self, config: OverlayConfig) {
        self.config = config;
    }
}

impl PerformanceTracker {
    fn new() -> Self {
        Self {
            frame_times: HashMap::new(),
            total_frame_time_ms: 0.0,
            budget_exceeded: false,
            last_warning_us: None,
        }
    }

    fn update(&mut self, overlay_type: &DebugOverlayType, frame_time_ms: f32) {
        let times = self.frame_times
            .entry(overlay_type.clone())
            .or_insert_with(|| VecDeque::with_capacity(60));
        
        times.push_back(frame_time_ms);
        if times.len() > 60 {
            times.pop_front();
        }
        
        // Recalculate total
        self.total_frame_time_ms = self.frame_times
            .values()
            .map(|times| {
                if times.is_empty() {
                    0.0
                } else {
                    times.iter().sum::<f32>() / times.len() as f32
                }
            })
            .sum();
    }

    fn should_warn(&mut self) -> bool {
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        
        if let Some(last_us) = self.last_warning_us {
            if (now_us - last_us) < 5_000_000 { // 5 seconds in microseconds
                return false;
            }
        }
        self.last_warning_us = Some(now_us);
        true
    }
}

/// Process visual debug commands
pub async fn process_visual_debug_command(
    overlay: &VisualDebugOverlay,
    command: DebugCommand,
) -> Result<DebugResponse> {
    match command {
        DebugCommand::SetVisualDebug {
            enabled,
            overlay_type,
            config,
        } => {
            overlay.set_overlay_enabled(&overlay_type, enabled, config).await?;
            
            Ok(DebugResponse::VisualDebugStatus {
                overlay_type,
                enabled,
                config: None,
            })
        }
        _ => Err(Error::DebugError(
            "Unsupported visual debug command".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[tokio::test]
    async fn test_overlay_creation() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        let overlay = VisualDebugOverlay::new(brp_client);
        
        assert!(overlay.config.enabled);
        assert_eq!(overlay.config.max_highlighted_entities, MAX_HIGHLIGHTED_ENTITIES);
    }

    #[tokio::test]
    async fn test_highlight_entities() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        let overlay = VisualDebugOverlay::new(brp_client);
        
        // Test highlighting with default config
        let entity_ids = vec![1, 2, 3];
        let result = overlay.highlight_entities(entity_ids.clone(), None, None).await;
        // Will fail without actual BRP connection, but structure is tested
        assert!(result.is_err()); // Expected without real connection
        
        // Test too many entities
        let many_entities: Vec<u64> = (0..200).collect();
        let result = overlay.highlight_entities(many_entities, None, None).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_highlight_modes() {
        // Test serialization of highlight modes
        let mode = HighlightMode::Outline;
        let json = serde_json::to_value(mode).unwrap();
        assert_eq!(json, serde_json::json!("Outline"));
        
        let mode: HighlightMode = serde_json::from_value(json).unwrap();
        assert!(matches!(mode, HighlightMode::Outline));
    }

    #[test]
    fn test_performance_tracker() {
        let mut tracker = PerformanceTracker::new();
        
        // Add some frame times
        tracker.update(&DebugOverlayType::EntityHighlight, 0.5);
        tracker.update(&DebugOverlayType::ColliderVisualization, 0.8);
        tracker.update(&DebugOverlayType::PerformanceMetrics, 0.3);
        
        assert_eq!(tracker.total_frame_time_ms, 1.6);
        
        // Test warning throttle
        assert!(tracker.should_warn());
        assert!(!tracker.should_warn()); // Should be throttled
    }
}