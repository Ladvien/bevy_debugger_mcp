/// Performance Metrics Overlay Implementation
/// 
/// Provides on-screen display of performance metrics like FPS, frame time,
/// memory usage, and system-specific performance data.

use super::{OverlayMetrics, VisualOverlay};
use crate::brp_messages::DebugOverlayType;
use bevy::prelude::*;

/// Performance Metrics Overlay implementation (stub for now)
#[derive(Debug)]
pub struct PerformanceMetricsOverlay {
    enabled: bool,
    metrics: OverlayMetrics,
}

impl PerformanceMetricsOverlay {
    pub fn new() -> Self {
        Self {
            enabled: false,
            metrics: OverlayMetrics::default(),
        }
    }
}

impl Default for PerformanceMetricsOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl VisualOverlay for PerformanceMetricsOverlay {
    fn initialize(&mut self, _app: &mut App) {
        info!("Performance metrics overlay initialized (placeholder)");
    }
    
    fn update_config(&mut self, _config: &serde_json::Value) -> Result<(), String> {
        Ok(())
    }
    
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    fn get_metrics(&self) -> OverlayMetrics {
        self.metrics.clone()
    }
    
    fn overlay_type(&self) -> DebugOverlayType {
        DebugOverlayType::PerformanceMetrics
    }
    
    fn cleanup(&mut self) {
        // Placeholder
    }
}