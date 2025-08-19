/// Visual Debug Overlay Processor for handling SetVisualDebug commands
/// 
/// This processor handles the MCP side of visual debug overlays. The actual
/// rendering implementation is handled by the game-side Bevy systems.
use crate::brp_messages::{
    DebugCommand, DebugResponse, DebugOverlayType, BrpRequest, BrpResponse, BrpResult
};
use crate::brp_client::BrpClient;
use crate::debug_command_processor::DebugCommandProcessor;
use crate::error::{Error, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Configuration for individual overlay types
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    /// Whether the overlay is enabled
    pub enabled: bool,
    /// Overlay-specific configuration
    pub config: Value,
    /// Last update timestamp
    pub last_updated: Instant,
    /// Performance metrics for this overlay
    pub metrics: OverlayMetrics,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            config: json!({}),
            last_updated: Instant::now(),
            metrics: OverlayMetrics::default(),
        }
    }
}

/// Performance metrics for overlay rendering
#[derive(Debug, Clone, Default)]
pub struct OverlayMetrics {
    /// Render time in microseconds
    pub render_time_us: u64,
    /// Number of elements rendered
    pub element_count: usize,
    /// Memory usage in bytes
    pub memory_usage_bytes: usize,
    /// Number of updates this frame
    pub update_count: usize,
}

/// State manager for visual debug overlays
#[derive(Debug)]
pub struct VisualDebugOverlayState {
    /// Overlay configurations by type
    overlays: HashMap<String, OverlayConfig>,
    /// Total performance metrics
    total_metrics: OverlayMetrics,
    /// BRP client for communication with Bevy
    brp_client: Arc<RwLock<BrpClient>>,
    /// Performance budget (2ms = 2000us per frame)
    performance_budget_us: u64,
}

impl VisualDebugOverlayState {
    /// Create new overlay state
    pub fn new(brp_client: Arc<RwLock<BrpClient>>) -> Self {
        Self {
            overlays: HashMap::new(),
            total_metrics: OverlayMetrics::default(),
            brp_client,
            performance_budget_us: 2000, // 2ms budget as per requirements
        }
    }

    /// Enable or disable an overlay
    pub async fn set_overlay_enabled(
        &mut self,
        overlay_type: &DebugOverlayType,
        enabled: bool,
        config: Option<Value>,
    ) -> Result<()> {
        let overlay_key = self.overlay_type_to_key(overlay_type);
        
        // Create the overlay config first
        let mut new_config = OverlayConfig::default();
        new_config.enabled = enabled;
        new_config.last_updated = Instant::now();
        
        if let Some(cfg) = config {
            new_config.config = cfg;
        }

        info!(
            "Visual debug overlay '{}' {} with config: {}",
            overlay_key,
            if enabled { "enabled" } else { "disabled" },
            new_config.config
        );

        // Send command to Bevy game via BRP
        self.sync_overlay_to_bevy(overlay_type, &new_config).await?;

        // Store the config after successful sync
        self.overlays.insert(overlay_key, new_config);

        Ok(())
    }

    /// Get current overlay status
    pub fn get_overlay_status(&self, overlay_type: &DebugOverlayType) -> Option<&OverlayConfig> {
        let overlay_key = self.overlay_type_to_key(overlay_type);
        self.overlays.get(&overlay_key)
    }

    /// Get all overlay statuses
    pub fn get_all_overlay_statuses(&self) -> &HashMap<String, OverlayConfig> {
        &self.overlays
    }

    /// Check if performance budget is exceeded
    pub fn is_performance_budget_exceeded(&self) -> bool {
        self.total_metrics.render_time_us > self.performance_budget_us
    }

    /// Update performance metrics
    pub fn update_metrics(&mut self, overlay_type: &DebugOverlayType, metrics: OverlayMetrics) {
        let overlay_key = self.overlay_type_to_key(overlay_type);
        
        if let Some(overlay_config) = self.overlays.get_mut(&overlay_key) {
            overlay_config.metrics = metrics.clone();
        }

        // Update total metrics
        self.recalculate_total_metrics();
    }

    /// Convert overlay type to string key
    pub fn overlay_type_to_key(&self, overlay_type: &DebugOverlayType) -> String {
        match overlay_type {
            DebugOverlayType::EntityHighlight => "entity_highlight".to_string(),
            DebugOverlayType::Colliders => "colliders".to_string(),
            DebugOverlayType::ColliderVisualization => "collider_visualization".to_string(),
            DebugOverlayType::Transforms => "transforms".to_string(),
            DebugOverlayType::TransformGizmos => "transform_gizmos".to_string(),
            DebugOverlayType::SystemFlow => "system_flow".to_string(),
            DebugOverlayType::PerformanceMetrics => "performance_metrics".to_string(),
            DebugOverlayType::DebugMarkers => "debug_markers".to_string(),
            DebugOverlayType::Custom(name) => format!("custom_{}", name),
        }
    }

    /// Recalculate total performance metrics
    fn recalculate_total_metrics(&mut self) {
        self.total_metrics = OverlayMetrics::default();
        
        for overlay_config in self.overlays.values() {
            if overlay_config.enabled {
                self.total_metrics.render_time_us += overlay_config.metrics.render_time_us;
                self.total_metrics.element_count += overlay_config.metrics.element_count;
                self.total_metrics.memory_usage_bytes += overlay_config.metrics.memory_usage_bytes;
                self.total_metrics.update_count += overlay_config.metrics.update_count;
            }
        }
    }

    /// Send overlay state to Bevy game via BRP
    async fn sync_overlay_to_bevy(
        &self,
        overlay_type: &DebugOverlayType,
        overlay_config: &OverlayConfig,
    ) -> Result<()> {
        // Create a custom BRP command for visual debug overlay
        // Since SetVisualDebug is already defined in brp_messages.rs, we'll use the Debug wrapper
        let debug_command = DebugCommand::SetVisualDebug {
            overlay_type: overlay_type.clone(),
            enabled: overlay_config.enabled,
            config: Some(overlay_config.config.clone()),
        };

        let correlation_id = Uuid::new_v4().to_string();
        let brp_request = BrpRequest::Debug {
            command: debug_command,
            correlation_id: correlation_id.clone(),
            priority: Some(5), // Medium priority
        };

        let mut client = self.brp_client.write().await;
        if !client.is_connected() {
            return Err(Error::Connection("BRP client not connected".to_string()));
        }

        match client.send_request(&brp_request).await {
            Ok(BrpResponse::Success(boxed_result)) => {
                if let BrpResult::Debug(response) = boxed_result.as_ref() {
                    debug!(
                        "Visual debug overlay sync successful: correlation_id={}, response={:?}",
                        correlation_id, response
                    );
                    Ok(())
                } else {
                    Err(Error::Brp("Expected debug response".to_string()))
                }
            }
            Ok(BrpResponse::Error(error)) => {
                warn!(
                    "Visual debug overlay sync failed: correlation_id={}, error={:?}",
                    correlation_id, error
                );
                Err(Error::Brp(format!(
                    "Overlay sync failed: {}",
                    error.message
                )))
            }
            Ok(response) => {
                warn!(
                    "Unexpected BRP response for overlay sync: {:?}",
                    response
                );
                Err(Error::Brp("Unexpected response type".to_string()))
            }
            Err(e) => {
                error!(
                    "BRP request failed for overlay sync: correlation_id={}, error={}",
                    correlation_id, e
                );
                Err(e)
            }
        }
    }
}

/// Visual Debug Overlay Processor
pub struct VisualDebugOverlayProcessor {
    /// Overlay state manager
    state: Arc<RwLock<VisualDebugOverlayState>>,
}

impl VisualDebugOverlayProcessor {
    /// Create new visual debug overlay processor
    pub fn new(brp_client: Arc<RwLock<BrpClient>>) -> Self {
        Self {
            state: Arc::new(RwLock::new(VisualDebugOverlayState::new(brp_client))),
        }
    }

    /// Get reference to overlay state for external access
    pub fn get_state(&self) -> Arc<RwLock<VisualDebugOverlayState>> {
        self.state.clone()
    }
}

#[async_trait]
impl DebugCommandProcessor for VisualDebugOverlayProcessor {
    async fn process(&self, command: DebugCommand) -> Result<DebugResponse> {
        match command {
            DebugCommand::SetVisualDebug {
                overlay_type,
                enabled,
                config,
            } => {
                let mut state = self.state.write().await;
                
                // Set overlay state
                state
                    .set_overlay_enabled(&overlay_type, enabled, config)
                    .await?;

                // Check performance budget
                if state.is_performance_budget_exceeded() {
                    warn!(
                        "Visual debug overlay performance budget exceeded: {}us > {}us",
                        state.total_metrics.render_time_us,
                        state.performance_budget_us
                    );
                }

                // Create response
                let status = state.get_overlay_status(&overlay_type)
                    .ok_or_else(|| Error::DebugError("Failed to get overlay status".to_string()))?;

                Ok(DebugResponse::VisualDebugStatus {
                    overlay_type: overlay_type.clone(),
                    enabled: status.enabled,
                    config: Some(status.config.clone()),
                })
            }
            DebugCommand::GetStatus => {
                let state = self.state.read().await;
                
                // Create comprehensive status report
                let overlay_statuses: HashMap<String, Value> = state
                    .get_all_overlay_statuses()
                    .iter()
                    .map(|(key, config)| {
                        (
                            key.clone(),
                            json!({
                                "enabled": config.enabled,
                                "config": config.config,
                                "last_updated": config.last_updated.elapsed().as_secs(),
                                "metrics": {
                                    "render_time_us": config.metrics.render_time_us,
                                    "element_count": config.metrics.element_count,
                                    "memory_usage_bytes": config.metrics.memory_usage_bytes,
                                    "update_count": config.metrics.update_count,
                                }
                            }),
                        )
                    })
                    .collect();

                Ok(DebugResponse::Status {
                    version: "0.1.2".to_string(),
                    active_sessions: overlay_statuses.len(),
                    command_queue_size: 0,
                    performance_overhead_percent: (state.total_metrics.render_time_us as f32 / state.performance_budget_us as f32) * 100.0,
                })
            }
            _ => Err(Error::DebugError("Unsupported command for visual debug overlay processor".to_string())),
        }
    }

    async fn validate(&self, command: &DebugCommand) -> Result<()> {
        match command {
            DebugCommand::SetVisualDebug { config, .. } => {
                // Validate configuration size to prevent abuse
                if let Some(cfg) = config {
                    let config_size = cfg.to_string().len();
                    if config_size > 10_000 {
                        return Err(Error::DebugError(format!(
                            "Overlay configuration too large: {} bytes (max: 10,000)",
                            config_size
                        )));
                    }
                }
                Ok(())
            }
            DebugCommand::GetStatus => Ok(()),
            _ => Err(Error::DebugError("Command not supported by visual debug overlay processor".to_string())),
        }
    }

    fn estimate_processing_time(&self, command: &DebugCommand) -> Duration {
        match command {
            DebugCommand::SetVisualDebug { .. } => Duration::from_millis(50), // Overlay changes can be more expensive
            DebugCommand::GetStatus => Duration::from_millis(5), // Status is quick
            _ => Duration::from_millis(10),
        }
    }

    fn supports_command(&self, command: &DebugCommand) -> bool {
        matches!(
            command,
            DebugCommand::SetVisualDebug { .. } | DebugCommand::GetStatus
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    async fn create_test_processor() -> VisualDebugOverlayProcessor {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        VisualDebugOverlayProcessor::new(brp_client)
    }

    #[tokio::test]
    async fn test_supports_visual_debug_commands() {
        let processor = create_test_processor().await;
        
        let set_command = DebugCommand::SetVisualDebug {
            overlay_type: DebugOverlayType::EntityHighlight,
            enabled: true,
            config: None,
        };
        
        assert!(processor.supports_command(&set_command));
        assert!(processor.supports_command(&DebugCommand::GetStatus));
    }

    #[tokio::test]
    async fn test_validate_overlay_config_size() {
        let processor = create_test_processor().await;
        
        // Test valid config
        let valid_command = DebugCommand::SetVisualDebug {
            overlay_type: DebugOverlayType::EntityHighlight,
            enabled: true,
            config: Some(json!({"color": [1.0, 0.0, 0.0, 1.0]})),
        };
        
        assert!(processor.validate(&valid_command).await.is_ok());
        
        // Test oversized config
        let large_config = json!({
            "data": "x".repeat(11_000) // Over 10k limit
        });
        
        let invalid_command = DebugCommand::SetVisualDebug {
            overlay_type: DebugOverlayType::EntityHighlight,
            enabled: true,
            config: Some(large_config),
        };
        
        assert!(processor.validate(&invalid_command).await.is_err());
    }

    #[tokio::test]
    async fn test_overlay_state_management() {
        let processor = create_test_processor().await;
        let state = processor.get_state();
        
        {
            let mut state_guard = state.write().await;
            
            // Test enabling overlay
            state_guard
                .set_overlay_enabled(
                    &DebugOverlayType::EntityHighlight,
                    true,
                    Some(json!({"color": [1.0, 0.0, 0.0, 1.0]})),
                )
                .await
                .unwrap_err(); // Should fail because BRP client is not connected
        }

        {
            let state_guard = state.read().await;
            let overlay_status = state_guard.get_overlay_status(&DebugOverlayType::EntityHighlight);
            assert!(overlay_status.is_some());
            
            let status = overlay_status.unwrap();
            assert!(status.enabled);
            assert_eq!(status.config["color"], json!([1.0, 0.0, 0.0, 1.0]));
        }
    }

    #[tokio::test]
    async fn test_performance_budget_tracking() {
        let processor = create_test_processor().await;
        let state = processor.get_state();
        
        {
            let mut state_guard = state.write().await;
            
            // Simulate performance metrics that exceed budget
            let high_cost_metrics = OverlayMetrics {
                render_time_us: 3000, // 3ms, exceeds 2ms budget
                element_count: 1000,
                memory_usage_bytes: 1024 * 1024,
                update_count: 50,
            };
            
            state_guard.update_metrics(&DebugOverlayType::EntityHighlight, high_cost_metrics);
            assert!(state_guard.is_performance_budget_exceeded());
        }
    }

    #[tokio::test]
    async fn test_custom_overlay_types() {
        let processor = create_test_processor().await;
        let state = processor.get_state();
        
        {
            let state_guard = state.read().await;
            let custom_overlay = DebugOverlayType::Custom("test_overlay".to_string());
            let key = state_guard.overlay_type_to_key(&custom_overlay);
            assert_eq!(key, "custom_test_overlay");
        }
    }
}