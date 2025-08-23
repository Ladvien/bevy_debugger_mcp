use serde_json::{json, Value};
/// Anomaly detection tool for automatic game state monitoring
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::anomaly_detector::{Anomaly, AnomalyConfig, AnomalyDetectionSystem};
use crate::brp_client::BrpClient;
use crate::brp_messages::{BrpRequest, BrpResponse, BrpResult};
use crate::error::Result;

/// Shared state for anomaly detection
pub struct AnomalyState {
    detection_system: AnomalyDetectionSystem,
    is_monitoring: bool,
}

impl AnomalyState {
    /// Create new anomaly detection state
    #[must_use]
    pub fn new() -> Self {
        let config = AnomalyConfig::default();
        Self {
            detection_system: AnomalyDetectionSystem::new(config),
            is_monitoring: false,
        }
    }

    /// Create with custom configuration
    #[must_use]
    pub fn with_config(config: AnomalyConfig) -> Self {
        Self {
            detection_system: AnomalyDetectionSystem::new(config),
            is_monitoring: false,
        }
    }
}

impl Default for AnomalyState {
    fn default() -> Self {
        Self::new()
    }
}

// Global anomaly state
static ANOMALY_STATE: std::sync::OnceLock<Arc<RwLock<AnomalyState>>> = std::sync::OnceLock::new();

fn get_anomaly_state() -> Arc<RwLock<AnomalyState>> {
    ANOMALY_STATE
        .get_or_init(|| Arc::new(RwLock::new(AnomalyState::new())))
        .clone()
}

/// Handle anomaly detection tool requests
///
/// # Errors
/// Returns error if BRP communication fails, configuration is invalid, or analysis fails
pub async fn handle(arguments: Value, brp_client: Arc<RwLock<BrpClient>>) -> Result<Value> {
    debug!("Anomaly tool called with arguments: {}", arguments);

    let action = arguments
        .get("action")
        .and_then(|a| a.as_str())
        .unwrap_or("detect");

    match action {
        "detect" => handle_detect(arguments, brp_client).await,
        "configure" => handle_configure(arguments).await,
        "start_monitoring" => handle_start_monitoring(arguments, brp_client).await,
        "stop_monitoring" => handle_stop_monitoring().await,
        "status" => handle_status().await,
        _ => Ok(json!({
            "error": "Invalid action",
            "message": format!("Unknown action: {}. Available actions: detect, configure, start_monitoring, stop_monitoring, status", action),
            "available_actions": ["detect", "configure", "start_monitoring", "stop_monitoring", "status"]
        })),
    }
}

/// Detect anomalies in current game state
async fn handle_detect(arguments: Value, brp_client: Arc<RwLock<BrpClient>>) -> Result<Value> {
    info!("Starting anomaly detection");

    // Get current game state
    let client_connected = {
        let client = brp_client.read().await;
        client.is_connected()
    };

    if !client_connected {
        warn!("BRP client not connected");
        return Ok(json!({
            "error": "BRP client not connected",
            "message": "Cannot detect anomalies - not connected to Bevy game"
        }));
    }

    // Query all entities
    let brp_request = BrpRequest::ListEntities { filter: None };
    let brp_response = {
        let mut client = brp_client.write().await;
        match client.send_request(&brp_request).await {
            Ok(response) => response,
            Err(e) => {
                error!("BRP request failed: {}", e);
                return Ok(json!({
                    "error": "BRP request failed",
                    "message": e.to_string()
                }));
            }
        }
    };

    let entities = match brp_response {
        BrpResponse::Success(boxed_result) => {
            if let BrpResult::Entities(entities) = boxed_result.as_ref() {
                entities.clone()
            } else {
                return Ok(json!({
                    "error": "Unexpected response type",
                    "message": "Expected entities list from BRP"
                }));
            }
        }
        BrpResponse::Error(error) => {
            warn!("BRP returned error: {}", error);
            return Ok(json!({
                "error": "BRP error",
                "code": error.code,
                "message": error.message,
                "details": error.details
            }));
        }
    };

    // Run anomaly detection
    let state = get_anomaly_state();
    let mut state_guard = state.write().await;

    let anomalies = match state_guard.detection_system.detect_anomalies(&entities) {
        Ok(anomalies) => anomalies,
        Err(e) => {
            error!("Anomaly detection failed: {}", e);
            return Ok(json!({
                "error": "Anomaly detection failed",
                "message": e.to_string()
            }));
        }
    };

    // Filter by severity if requested
    let min_severity = arguments
        .get("min_severity")
        .and_then(|s| s.as_f64())
        .unwrap_or(0.0) as f32;

    let filtered_anomalies: Vec<&Anomaly> = anomalies
        .iter()
        .filter(|a| a.severity >= min_severity)
        .collect();

    // Limit results if requested
    let limit = arguments
        .get("limit")
        .and_then(|l| l.as_u64())
        .unwrap_or(50) as usize;

    let limited_anomalies: Vec<&Anomaly> = filtered_anomalies.into_iter().take(limit).collect();

    info!(
        "Detected {} anomalies (showing {} after filtering)",
        anomalies.len(),
        limited_anomalies.len()
    );

    Ok(json!({
        "anomalies": limited_anomalies,
        "summary": {
            "total_detected": anomalies.len(),
            "after_filtering": limited_anomalies.len(),
            "entities_analyzed": entities.len(),
            "min_severity_filter": min_severity,
            "limit_applied": limit,
            "timestamp": chrono::Utc::now().to_rfc3339()
        },
        "severity_breakdown": calculate_severity_breakdown(&anomalies),
        "type_breakdown": calculate_type_breakdown(&anomalies)
    }))
}

/// Configure anomaly detection system
async fn handle_configure(arguments: Value) -> Result<Value> {
    info!("Configuring anomaly detection system");

    let mut config = AnomalyConfig::default();

    // Update configuration from arguments
    if let Some(window_size) = arguments.get("window_size").and_then(|w| w.as_u64()) {
        config.window_size = window_size as usize;
    }

    if let Some(z_threshold) = arguments.get("z_score_threshold").and_then(|z| z.as_f64()) {
        config.z_score_threshold = z_threshold as f32;
    }

    if let Some(iqr_multiplier) = arguments.get("iqr_multiplier").and_then(|i| i.as_f64()) {
        config.iqr_multiplier = iqr_multiplier as f32;
    }

    if let Some(min_samples) = arguments.get("min_samples").and_then(|m| m.as_u64()) {
        config.min_samples = min_samples as usize;
    }

    if let Some(perf_threshold) = arguments
        .get("performance_threshold")
        .and_then(|p| p.as_f64())
    {
        config.performance_threshold = perf_threshold as f32;
    }

    if let Some(entity_growth) = arguments
        .get("entity_growth_threshold")
        .and_then(|e| e.as_f64())
    {
        config.entity_growth_threshold = entity_growth as f32;
    }

    // Apply configuration
    let state = get_anomaly_state();
    let mut state_guard = state.write().await;
    state_guard.detection_system.update_config(config.clone());

    info!("Anomaly detection configuration updated");

    Ok(json!({
        "message": "Configuration updated successfully",
        "config": {
            "window_size": config.window_size,
            "z_score_threshold": config.z_score_threshold,
            "iqr_multiplier": config.iqr_multiplier,
            "min_samples": config.min_samples,
            "performance_threshold": config.performance_threshold,
            "entity_growth_threshold": config.entity_growth_threshold,
            "whitelist_count": config.whitelist.len()
        }
    }))
}

/// Start continuous monitoring (placeholder for future async monitoring)
async fn handle_start_monitoring(
    _arguments: Value,
    _brp_client: Arc<RwLock<BrpClient>>,
) -> Result<Value> {
    info!("Starting continuous anomaly monitoring");

    let state = get_anomaly_state();
    let mut state_guard = state.write().await;

    if state_guard.is_monitoring {
        return Ok(json!({
            "message": "Monitoring is already running",
            "is_monitoring": true
        }));
    }

    state_guard.is_monitoring = true;

    // In a real implementation, this would start a background task
    // that continuously monitors game state and reports anomalies

    Ok(json!({
        "message": "Continuous monitoring started",
        "is_monitoring": true,
        "note": "Monitoring implementation requires background task setup"
    }))
}

/// Stop continuous monitoring
async fn handle_stop_monitoring() -> Result<Value> {
    info!("Stopping continuous anomaly monitoring");

    let state = get_anomaly_state();
    let mut state_guard = state.write().await;

    if !state_guard.is_monitoring {
        return Ok(json!({
            "message": "Monitoring is not currently running",
            "is_monitoring": false
        }));
    }

    state_guard.is_monitoring = false;

    Ok(json!({
        "message": "Continuous monitoring stopped",
        "is_monitoring": false
    }))
}

/// Get monitoring status
async fn handle_status() -> Result<Value> {
    let state = get_anomaly_state();
    let state_guard = state.read().await;

    Ok(json!({
        "is_monitoring": state_guard.is_monitoring,
        "detectors": [
            "PhysicsDetector",
            "PerformanceDetector",
            "ConsistencyDetector"
        ],
        "supported_anomaly_types": [
            "PhysicsViolation",
            "PotentialMemoryLeak",
            "StateInconsistency",
            "PerformanceSpike",
            "EntityCountSpike",
            "RapidValueChange"
        ]
    }))
}

/// Calculate severity breakdown for anomalies
fn calculate_severity_breakdown(anomalies: &[Anomaly]) -> Value {
    let mut high = 0;
    let mut medium = 0;
    let mut low = 0;

    for anomaly in anomalies {
        if anomaly.severity >= 0.7 {
            high += 1;
        } else if anomaly.severity >= 0.4 {
            medium += 1;
        } else {
            low += 1;
        }
    }

    json!({
        "high_severity": high,
        "medium_severity": medium,
        "low_severity": low
    })
}

/// Calculate type breakdown for anomalies
fn calculate_type_breakdown(anomalies: &[Anomaly]) -> Value {
    let mut breakdown = std::collections::HashMap::new();

    for anomaly in anomalies {
        let type_name = format!("{:?}", anomaly.anomaly_type);
        *breakdown.entry(type_name).or_insert(0) += 1;
    }

    json!(breakdown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[tokio::test]
    async fn test_anomaly_configure() {
        let args = json!({
            "action": "configure",
            "window_size": 50,
            "z_score_threshold": 2.5
        });

        let result = handle_configure(args).await.unwrap();
        assert_eq!(result["config"]["window_size"], 50);
        assert_eq!(result["config"]["z_score_threshold"], 2.5);
    }

    #[tokio::test]
    async fn test_anomaly_status() {
        let result = handle_status().await.unwrap();
        assert!(result["detectors"].is_array());
        assert!(result["supported_anomaly_types"].is_array());
    }

    #[tokio::test]
    async fn test_anomaly_monitoring_control() {
        // Test start monitoring
        let start_result = handle_start_monitoring(json!({}), create_test_brp_client())
            .await
            .unwrap();
        assert_eq!(start_result["is_monitoring"], true);

        // Test stop monitoring
        let stop_result = handle_stop_monitoring().await.unwrap();
        assert_eq!(stop_result["is_monitoring"], false);
    }

    #[tokio::test]
    async fn test_anomaly_detect_no_connection() {
        let config = Config::default();
        let brp_client = Arc::new(RwLock::new(crate::brp_client::BrpClient::new(&config)));

        let args = json!({"action": "detect"});
        let result = handle_detect(args, brp_client).await.unwrap();

        assert!(result.get("error").is_some());
        assert_eq!(result["error"], "BRP client not connected");
    }

    fn create_test_brp_client() -> Arc<RwLock<BrpClient>> {
        let config = Config::default();
        Arc::new(RwLock::new(crate::brp_client::BrpClient::new(&config)))
    }
}
