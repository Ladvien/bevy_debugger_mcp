/// Visual Debug Overlay Module
/// 
/// This module contains all the visual overlay implementations and the manager
/// that coordinates them. Each overlay type has its own module with specific
/// rendering logic and configuration options.
///
/// This module is only available when the "visual_overlays" feature is enabled.

pub mod entity_highlight;
pub mod colliders;
pub mod transforms;
pub mod system_flow;
pub mod performance_metrics;
pub mod custom_markers;

use crate::brp_messages::DebugOverlayType;
#[cfg(feature = "visual_overlays")]
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Common trait for all visual debug overlays
pub trait VisualOverlay: Send + Sync + std::fmt::Debug {
    /// Initialize the overlay system
    fn initialize(&mut self, app: &mut App);
    
    /// Update the overlay configuration
    fn update_config(&mut self, config: &serde_json::Value) -> Result<(), String>;
    
    /// Enable or disable the overlay
    fn set_enabled(&mut self, enabled: bool);
    
    /// Check if the overlay is enabled
    fn is_enabled(&self) -> bool;
    
    /// Get performance metrics for this overlay
    fn get_metrics(&self) -> OverlayMetrics;
    
    /// Get the overlay type
    fn overlay_type(&self) -> DebugOverlayType;
    
    /// Cleanup when the overlay is disabled
    fn cleanup(&mut self);
}

/// Performance metrics for individual overlays
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OverlayMetrics {
    /// Render time in microseconds
    pub render_time_us: u64,
    /// Number of visual elements rendered
    pub element_count: usize,
    /// Memory usage in bytes
    pub memory_usage_bytes: usize,
    /// Frame update count
    pub frame_updates: usize,
    /// Whether the overlay is active this frame
    pub active_this_frame: bool,
}

/// Manager for all visual debug overlays
#[cfg_attr(feature = "visual_overlays", derive(Resource))]
#[derive(Debug)]
pub struct VisualOverlayManager {
    /// All registered overlays
    overlays: HashMap<String, Box<dyn VisualOverlay>>,
    /// Global enable/disable state
    global_enabled: bool,
    /// Performance budget in microseconds (2ms = 2000us)
    performance_budget_us: u64,
    /// Total metrics across all overlays
    total_metrics: OverlayMetrics,
}

impl VisualOverlayManager {
    /// Create new visual overlay manager
    pub fn new() -> Self {
        Self {
            overlays: HashMap::new(),
            global_enabled: true,
            performance_budget_us: 2000, // 2ms as per requirements
            total_metrics: OverlayMetrics::default(),
        }
    }
    
    /// Initialize the manager with all overlay types
    pub fn initialize(&mut self, app: &mut App) {
        // Register all overlay implementations
        self.register_overlay("entity_highlight", Box::new(entity_highlight::EntityHighlightOverlay::new()));
        self.register_overlay("colliders", Box::new(colliders::CollidersOverlay::new()));
        self.register_overlay("transforms", Box::new(transforms::TransformsOverlay::new()));
        self.register_overlay("system_flow", Box::new(system_flow::SystemFlowOverlay::new()));
        self.register_overlay("performance_metrics", Box::new(performance_metrics::PerformanceMetricsOverlay::new()));
        
        // Initialize all overlays
        for overlay in self.overlays.values_mut() {
            overlay.initialize(app);
        }
        
        // Add global systems
        app.add_systems(PostUpdate, (
            Self::update_performance_metrics,
            Self::check_performance_budget.after(Self::update_performance_metrics),
        ));
    }
    
    /// Register a new overlay
    fn register_overlay(&mut self, key: &str, overlay: Box<dyn VisualOverlay>) {
        self.overlays.insert(key.to_string(), overlay);
    }
    
    /// Set overlay enabled/disabled state
    pub fn set_overlay_enabled(
        &mut self,
        overlay_type: &DebugOverlayType,
        enabled: bool,
        config: Option<&serde_json::Value>,
    ) -> Result<(), String> {
        let key = self.overlay_type_to_key(overlay_type);
        
        if let Some(overlay) = self.overlays.get_mut(&key) {
            overlay.set_enabled(enabled);
            
            if let Some(config) = config {
                overlay.update_config(config)?;
            }
            
            info!(
                "Visual overlay '{}' {} with config: {:?}",
                key,
                if enabled { "enabled" } else { "disabled" },
                config
            );
            
            Ok(())
        } else {
            Err(format!("Unknown overlay type: {:?}", overlay_type))
        }
    }
    
    /// Get overlay status
    pub fn get_overlay_status(&self, overlay_type: &DebugOverlayType) -> Option<(bool, OverlayMetrics)> {
        let key = self.overlay_type_to_key(overlay_type);
        self.overlays.get(&key).map(|overlay| {
            (overlay.is_enabled(), overlay.get_metrics())
        })
    }
    
    /// Get all overlay statuses
    pub fn get_all_statuses(&self) -> HashMap<String, (bool, OverlayMetrics)> {
        self.overlays
            .iter()
            .map(|(key, overlay)| {
                (key.clone(), (overlay.is_enabled(), overlay.get_metrics()))
            })
            .collect()
    }
    
    /// Get total performance metrics
    pub fn get_total_metrics(&self) -> &OverlayMetrics {
        &self.total_metrics
    }
    
    /// Check if performance budget is exceeded
    pub fn is_performance_budget_exceeded(&self) -> bool {
        self.total_metrics.render_time_us > self.performance_budget_us
    }
    
    /// Convert overlay type to string key
    fn overlay_type_to_key(&self, overlay_type: &DebugOverlayType) -> String {
        match overlay_type {
            DebugOverlayType::EntityHighlight => "entity_highlight".to_string(),
            DebugOverlayType::Colliders => "colliders".to_string(),
            DebugOverlayType::Transforms => "transforms".to_string(),
            DebugOverlayType::SystemFlow => "system_flow".to_string(),
            DebugOverlayType::PerformanceMetrics => "performance_metrics".to_string(),
            DebugOverlayType::Custom(name) => format!("custom_{}", name),
        }
    }
    
    /// System to update performance metrics
    fn update_performance_metrics(
        // This would be implemented as a proper Bevy system
        // For now, it's a placeholder
    ) {
        // Implementation would gather metrics from all overlays
    }
    
    /// System to check performance budget and warn if exceeded
    fn check_performance_budget(
        // This would be implemented as a proper Bevy system
        // For now, it's a placeholder
    ) {
        // Implementation would check if budget is exceeded and warn
    }
}

impl Default for VisualOverlayManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Plugin to add visual debug overlay support to a Bevy app
pub struct VisualDebugOverlayPlugin {
    /// Whether to initialize overlays immediately
    pub auto_initialize: bool,
}

impl Default for VisualDebugOverlayPlugin {
    fn default() -> Self {
        Self {
            auto_initialize: true,
        }
    }
}

impl Plugin for VisualDebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        if self.auto_initialize {
            let mut manager = VisualOverlayManager::new();
            manager.initialize(app);
            app.insert_resource(manager);
        } else {
            app.insert_resource(VisualOverlayManager::new());
        }
    }
}

// Re-export overlay implementations
pub use entity_highlight::{EntityHighlightOverlay, HighlightedEntity, HighlightMode, HighlightConfig};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_type_to_key() {
        let manager = VisualOverlayManager::new();
        
        assert_eq!(
            manager.overlay_type_to_key(&DebugOverlayType::EntityHighlight),
            "entity_highlight"
        );
        assert_eq!(
            manager.overlay_type_to_key(&DebugOverlayType::Colliders),
            "colliders"
        );
        assert_eq!(
            manager.overlay_type_to_key(&DebugOverlayType::Custom("test".to_string())),
            "custom_test"
        );
    }

    #[test]
    fn test_performance_budget_tracking() {
        let manager = VisualOverlayManager::new();
        
        // Default budget is 2000us (2ms)
        assert_eq!(manager.performance_budget_us, 2000);
        assert!(!manager.is_performance_budget_exceeded()); // Should be false initially
    }
}