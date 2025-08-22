/// Issue Detector Processor for Debug Command Integration
/// 
/// This processor integrates the automated issue detection system with the debug command
/// infrastructure, providing MCP-accessible commands for issue monitoring and alerting.

use crate::brp_messages::{DebugCommand, DebugResponse};
use crate::brp_client::BrpClient;
use crate::debug_command_processor::DebugCommandProcessor;
use crate::issue_detector::{
    IssueDetector, IssueDetectorConfig, IssuePattern,
    DetectionRule, IssueAlert
};
use crate::error::{Error, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, watch};
use tokio::time::{interval, MissedTickBehavior};
use rand::prelude::*;
use rand::SeedableRng;
use tracing::{debug, info, warn};

/// Issue detection processor for debug commands
pub struct IssueDetectorProcessor {
    /// Issue detector instance
    detector: Arc<IssueDetector>,
    /// BRP client for Bevy interaction
    brp_client: Arc<RwLock<BrpClient>>,
    /// Background monitoring task handle
    monitoring_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Detection state
    detection_state: Arc<RwLock<DetectionState>>,
    /// Shutdown signal for graceful termination
    shutdown_sender: Arc<RwLock<Option<watch::Sender<bool>>>>,
    /// Random number generator for async-safe random values
    rng: Arc<RwLock<StdRng>>,
}

/// State tracking for detection system
#[derive(Debug, Default)]
struct DetectionState {
    /// Whether monitoring is active
    monitoring_active: bool,
    /// Last detection check time
    last_check: Option<Instant>,
    /// Entity count for explosion detection
    last_entity_count: usize,
    /// Memory usage samples for leak detection
    memory_samples: Vec<(f32, Instant)>,
    /// Frame time samples for spike detection
    frame_time_samples: Vec<f32>,
    /// System execution times
    system_times: HashMap<String, Vec<f32>>,
}

impl IssueDetectorProcessor {
    /// Create a new issue detector processor
    pub fn new(brp_client: Arc<RwLock<BrpClient>>) -> Self {
        let config = IssueDetectorConfig::default();
        let detector = Arc::new(IssueDetector::new(config));
        
        Self {
            detector,
            brp_client,
            monitoring_handle: Arc::new(RwLock::new(None)),
            detection_state: Arc::new(RwLock::new(DetectionState::default())),
            shutdown_sender: Arc::new(RwLock::new(None)),
            rng: Arc::new(RwLock::new(StdRng::seed_from_u64(42))),
        }
    }

    /// Start continuous monitoring
    pub async fn start_monitoring(&self) -> Result<()> {
        let mut handle_guard = self.monitoring_handle.write().await;
        
        if handle_guard.is_some() {
            return Ok(()); // Already monitoring
        }

        // Create shutdown channel for graceful termination
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        *self.shutdown_sender.write().await = Some(shutdown_tx);

        let detector = Arc::clone(&self.detector);
        let brp_client = Arc::clone(&self.brp_client);
        let detection_state = Arc::clone(&self.detection_state);
        let rng = Arc::clone(&self.rng);
        
        let handle = tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_millis(100));
            check_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
            
            loop {
                tokio::select! {
                    _ = check_interval.tick() => {
                        // Perform various checks
                        if let Err(e) = Self::perform_detection_checks(
                            &detector,
                            &brp_client,
                            &detection_state,
                            &rng
                        ).await {
                            warn!("Detection check failed: {}", e);
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            info!("Shutting down issue detection monitoring gracefully");
                            break;
                        }
                    }
                }
            }
        });

        *handle_guard = Some(handle);
        
        // Mark monitoring as active
        {
            let mut state = self.detection_state.write().await;
            state.monitoring_active = true;
        }

        info!("Issue detection monitoring started");
        Ok(())
    }

    /// Stop continuous monitoring
    pub async fn stop_monitoring(&self) -> Result<()> {
        let mut handle_guard = self.monitoring_handle.write().await;
        
        if let Some(handle) = handle_guard.take() {
            // Send shutdown signal for graceful termination
            if let Some(sender) = self.shutdown_sender.write().await.take() {
                let _ = sender.send(true);
            }
            
            // Wait for task to complete gracefully (with timeout)
            let _ = tokio::time::timeout(
                Duration::from_secs(5),
                handle
            ).await;
        }

        // Mark monitoring as inactive
        {
            let mut state = self.detection_state.write().await;
            state.monitoring_active = false;
        }

        info!("Issue detection monitoring stopped gracefully");
        Ok(())
    }

    /// Perform detection checks
    async fn perform_detection_checks(
        detector: &Arc<IssueDetector>,
        brp_client: &Arc<RwLock<BrpClient>>,
        detection_state: &Arc<RwLock<DetectionState>>,
        rng: &Arc<RwLock<StdRng>>,
    ) -> Result<()> {
        let mut state = detection_state.write().await;
        let now = Instant::now();
        
        // Update last check time
        state.last_check = Some(now);

        // Check for transform NaN issues
        // In a real implementation, this would query actual entity data
        // For now, we'll use simulated checks
        
        // Check for memory leaks
        if let Ok(memory_mb) = Self::get_current_memory_usage(brp_client, rng).await {
            state.memory_samples.push((memory_mb, now));
            
            // Keep only last 60 samples (1 minute at 100ms intervals)
            if state.memory_samples.len() > 60 {
                state.memory_samples.remove(0);
            }
            
            // Check for leak pattern
            if state.memory_samples.len() >= 10 {
                let rate = crate::issue_detector::calculate_memory_leak_rate(&state.memory_samples);
                
                if rate > 0.1 { // More than 0.1 MB/s growth
                    let pattern = IssuePattern::MemoryLeak {
                        rate_mb_per_sec: rate,
                        total_leaked_mb: memory_mb - state.memory_samples[0].0,
                        suspected_source: "Unknown".to_string(),
                    };
                    
                    let _ = detector.detect_issue(pattern).await;
                }
            }
        }

        // Check for frame spikes
        if let Ok(frame_time_ms) = Self::get_current_frame_time(brp_client, rng).await {
            state.frame_time_samples.push(frame_time_ms);
            
            // Keep only last 100 samples
            if state.frame_time_samples.len() > 100 {
                state.frame_time_samples.remove(0);
            }
            
            // Check for spike
            if state.frame_time_samples.len() >= 10 {
                let avg = state.frame_time_samples.iter().sum::<f32>() / state.frame_time_samples.len() as f32;
                let spike_ratio = frame_time_ms / avg;
                
                if spike_ratio > 2.0 {
                    let pattern = IssuePattern::FrameSpike {
                        frame_time_ms,
                        average_frame_time_ms: avg,
                        spike_ratio,
                    };
                    
                    let _ = detector.detect_issue(pattern).await;
                }
            }
        }

        // Check for entity explosion
        if let Ok(entity_count) = Self::get_entity_count(brp_client, rng).await {
            if state.last_entity_count > 0 {
                let growth_rate = entity_count as f32 / state.last_entity_count as f32;
                
                if growth_rate > 1.5 { // 50% growth between checks
                    let pattern = IssuePattern::EntityExplosion {
                        growth_rate,
                        current_count: entity_count,
                        time_window_sec: 0.1, // 100ms check interval
                    };
                    
                    let _ = detector.detect_issue(pattern).await;
                }
            }
            
            state.last_entity_count = entity_count;
        }

        Ok(())
    }

    /// Get current memory usage (simulated for now)
    async fn get_current_memory_usage(_brp_client: &Arc<RwLock<BrpClient>>, rng: &Arc<RwLock<StdRng>>) -> Result<f32> {
        // In a real implementation, this would query actual memory usage
        // For now, return a simulated value with async-safe RNG
        let mut rng = rng.write().await;
        Ok(100.0 + (rng.random::<f32>() * 10.0))
    }

    /// Get current frame time (simulated for now)
    async fn get_current_frame_time(_brp_client: &Arc<RwLock<BrpClient>>, rng: &Arc<RwLock<StdRng>>) -> Result<f32> {
        // In a real implementation, this would query actual frame time
        // For now, return a simulated value with async-safe RNG
        let mut rng = rng.write().await;
        Ok(16.0 + (rng.random::<f32>() * 5.0))
    }

    /// Get entity count (simulated for now)
    async fn get_entity_count(_brp_client: &Arc<RwLock<BrpClient>>, rng: &Arc<RwLock<StdRng>>) -> Result<usize> {
        // In a real implementation, this would query actual entity count
        // For now, return a simulated value with async-safe RNG
        let mut rng = rng.write().await;
        Ok(1000 + (rng.random::<f32>() * 100.0) as usize)
    }

    /// Manually trigger an issue detection
    pub async fn trigger_detection(&self, pattern: IssuePattern) -> Result<Option<IssueAlert>> {
        self.detector.detect_issue(pattern).await
    }

    /// Update a detection rule
    pub async fn update_detection_rule(&self, name: String, rule: DetectionRule) -> Result<()> {
        self.detector.update_rule(name, rule).await
    }

    /// Get all detection rules
    pub async fn get_detection_rules(&self) -> HashMap<String, DetectionRule> {
        self.detector.get_rules().await
    }

    /// Get alert history
    pub async fn get_alerts(&self, limit: Option<usize>) -> Vec<IssueAlert> {
        self.detector.get_alert_history(limit).await
    }

    /// Acknowledge an alert
    pub async fn acknowledge_alert(&self, alert_id: &str) -> Result<()> {
        self.detector.acknowledge_alert(alert_id).await
    }

    /// Report a false positive
    pub async fn report_false_positive(&self, alert_id: &str) -> Result<()> {
        self.detector.report_false_positive(alert_id).await
    }

    /// Get detection statistics
    pub async fn get_statistics(&self) -> HashMap<String, Value> {
        let mut stats = self.detector.get_statistics().await;
        
        // Add processor-specific stats
        let state = self.detection_state.read().await;
        stats.insert("monitoring_active".to_string(), serde_json::json!(state.monitoring_active));
        
        if let Some(last_check) = state.last_check {
            stats.insert(
                "last_check_seconds_ago".to_string(), 
                serde_json::json!(last_check.elapsed().as_secs())
            );
        }
        
        stats
    }

    /// Clear alert history
    pub async fn clear_alert_history(&self) -> Result<()> {
        self.detector.clear_history().await;
        Ok(())
    }

    /// Export ML training data
    pub async fn export_ml_data(&self) -> Vec<Value> {
        self.detector.export_ml_data().await
    }
}

#[async_trait]
impl DebugCommandProcessor for IssueDetectorProcessor {
    async fn process(&self, command: DebugCommand) -> Result<DebugResponse> {
        match command {
            DebugCommand::StartIssueDetection => {
                debug!("Starting issue detection monitoring");
                self.start_monitoring().await?;
                
                Ok(DebugResponse::Success {
                    message: "Issue detection monitoring started".to_string(),
                    data: None,
                })
            }
            
            DebugCommand::StopIssueDetection => {
                debug!("Stopping issue detection monitoring");
                self.stop_monitoring().await?;
                
                Ok(DebugResponse::Success {
                    message: "Issue detection monitoring stopped".to_string(),
                    data: None,
                })
            }
            
            DebugCommand::GetDetectedIssues { limit } => {
                debug!("Getting detected issues");
                let alerts = self.get_alerts(limit).await;
                
                Ok(DebugResponse::Success {
                    message: format!("Retrieved {} alerts", alerts.len()),
                    data: Some(serde_json::to_value(alerts)?),
                })
            }
            
            DebugCommand::AcknowledgeIssue { alert_id } => {
                debug!("Acknowledging alert: {}", alert_id);
                self.acknowledge_alert(&alert_id).await?;
                
                Ok(DebugResponse::Success {
                    message: format!("Alert {} acknowledged", alert_id),
                    data: None,
                })
            }
            
            DebugCommand::ReportFalsePositive { alert_id } => {
                debug!("Reporting false positive: {}", alert_id);
                self.report_false_positive(&alert_id).await?;
                
                Ok(DebugResponse::Success {
                    message: format!("False positive reported for alert {}", alert_id),
                    data: None,
                })
            }
            
            DebugCommand::GetIssueDetectionStats => {
                debug!("Getting issue detection statistics");
                let stats = self.get_statistics().await;
                
                Ok(DebugResponse::Success {
                    message: "Issue detection statistics retrieved".to_string(),
                    data: Some(Value::Object(stats.into_iter().collect())),
                })
            }
            
            DebugCommand::UpdateDetectionRule { name, enabled, sensitivity } => {
                debug!("Updating detection rule: {}", name);
                
                let mut current_rules = self.get_detection_rules().await;
                
                if let Some(mut rule) = current_rules.get(&name).cloned() {
                    if let Some(enabled) = enabled {
                        rule.enabled = enabled;
                    }
                    if let Some(sensitivity) = sensitivity {
                        rule.sensitivity = sensitivity.clamp(0.0, 1.0);
                    }
                    
                    self.update_detection_rule(name.clone(), rule).await?;
                    
                    Ok(DebugResponse::Success {
                        message: format!("Detection rule '{}' updated", name),
                        data: None,
                    })
                } else {
                    Err(Error::Validation(format!("Detection rule '{}' not found", name)))
                }
            }
            
            DebugCommand::ClearIssueHistory => {
                debug!("Clearing issue alert history");
                self.clear_alert_history().await?;
                
                Ok(DebugResponse::Success {
                    message: "Alert history cleared".to_string(),
                    data: None,
                })
            }
            
            _ => Err(Error::DebugError(
                "Unsupported command for issue detector processor".to_string(),
            )),
        }
    }

    async fn validate(&self, command: &DebugCommand) -> Result<()> {
        match command {
            DebugCommand::StartIssueDetection
            | DebugCommand::StopIssueDetection
            | DebugCommand::GetIssueDetectionStats
            | DebugCommand::ClearIssueHistory => Ok(()),
            
            DebugCommand::GetDetectedIssues { limit } => {
                if let Some(limit) = limit {
                    if *limit == 0 || *limit > 1000 {
                        return Err(Error::Validation(
                            "Limit must be between 1 and 1000".to_string()
                        ));
                    }
                }
                Ok(())
            }
            
            DebugCommand::AcknowledgeIssue { alert_id }
            | DebugCommand::ReportFalsePositive { alert_id } => {
                if alert_id.is_empty() {
                    return Err(Error::Validation("Alert ID cannot be empty".to_string()));
                }
                Ok(())
            }
            
            DebugCommand::UpdateDetectionRule { name, sensitivity, .. } => {
                if name.is_empty() {
                    return Err(Error::Validation("Rule name cannot be empty".to_string()));
                }
                if let Some(sensitivity) = sensitivity {
                    if !(*sensitivity >= 0.0 && *sensitivity <= 1.0) {
                        return Err(Error::Validation(
                            "Sensitivity must be between 0.0 and 1.0".to_string()
                        ));
                    }
                }
                Ok(())
            }
            
            _ => Err(Error::DebugError(
                "Command not supported by issue detector processor".to_string()
            )),
        }
    }

    fn estimate_processing_time(&self, command: &DebugCommand) -> Duration {
        match command {
            DebugCommand::StartIssueDetection => Duration::from_millis(50),
            DebugCommand::StopIssueDetection => Duration::from_millis(20),
            DebugCommand::GetDetectedIssues { .. } => Duration::from_millis(30),
            DebugCommand::AcknowledgeIssue { .. } => Duration::from_millis(10),
            DebugCommand::ReportFalsePositive { .. } => Duration::from_millis(15),
            DebugCommand::GetIssueDetectionStats => Duration::from_millis(20),
            DebugCommand::UpdateDetectionRule { .. } => Duration::from_millis(25),
            DebugCommand::ClearIssueHistory => Duration::from_millis(10),
            _ => Duration::from_millis(1),
        }
    }

    fn supports_command(&self, command: &DebugCommand) -> bool {
        matches!(
            command,
            DebugCommand::StartIssueDetection
                | DebugCommand::StopIssueDetection
                | DebugCommand::GetDetectedIssues { .. }
                | DebugCommand::AcknowledgeIssue { .. }
                | DebugCommand::ReportFalsePositive { .. }
                | DebugCommand::GetIssueDetectionStats
                | DebugCommand::UpdateDetectionRule { .. }
                | DebugCommand::ClearIssueHistory
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    async fn create_test_processor() -> IssueDetectorProcessor {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        IssueDetectorProcessor::new(brp_client)
    }

    #[tokio::test]
    async fn test_supports_issue_detection_commands() {
        let processor = create_test_processor().await;
        
        let commands = vec![
            DebugCommand::StartIssueDetection,
            DebugCommand::StopIssueDetection,
            DebugCommand::GetDetectedIssues { limit: Some(10) },
            DebugCommand::GetIssueDetectionStats,
        ];
        
        for cmd in commands {
            assert!(processor.supports_command(&cmd), "Should support: {:?}", cmd);
        }
    }

    #[tokio::test]
    async fn test_start_stop_monitoring() {
        let processor = create_test_processor().await;
        
        // Start monitoring
        let result = processor.process(DebugCommand::StartIssueDetection).await;
        assert!(result.is_ok());
        
        // Check status
        let stats = processor.get_statistics().await;
        assert_eq!(stats.get("monitoring_active"), Some(&serde_json::json!(true)));
        
        // Stop monitoring
        let result = processor.process(DebugCommand::StopIssueDetection).await;
        assert!(result.is_ok());
        
        // Check status again
        let stats = processor.get_statistics().await;
        assert_eq!(stats.get("monitoring_active"), Some(&serde_json::json!(false)));
    }

    #[tokio::test]
    async fn test_manual_issue_detection() {
        let processor = create_test_processor().await;
        
        let pattern = IssuePattern::TransformNaN {
            entity_id: 456,
            component: "GlobalTransform".to_string(),
            values: vec![f32::NAN, 0.0, 0.0],
        };
        
        let alert = processor.trigger_detection(pattern).await.unwrap();
        assert!(alert.is_some());
        
        // Check that alert appears in history
        let alerts = processor.get_alerts(Some(10)).await;
        assert!(!alerts.is_empty());
    }

    #[tokio::test]
    async fn test_alert_acknowledgment() {
        let processor = create_test_processor().await;
        
        // Create an alert
        let pattern = IssuePattern::MemoryLeak {
            rate_mb_per_sec: 5.0,
            total_leaked_mb: 100.0,
            suspected_source: "TestSystem".to_string(),
        };
        
        let alert = processor.trigger_detection(pattern).await.unwrap().unwrap();
        
        // Acknowledge it
        let result = processor.acknowledge_alert(&alert.id).await;
        assert!(result.is_ok());
        
        // Check that it's acknowledged
        let alerts = processor.get_alerts(Some(10)).await;
        assert!(alerts.iter().any(|a| a.id == alert.id && a.acknowledged));
    }

    #[tokio::test]
    async fn test_false_positive_reporting() {
        let processor = create_test_processor().await;
        
        // Create an alert
        let pattern = IssuePattern::FrameSpike {
            frame_time_ms: 50.0,
            average_frame_time_ms: 16.0,
            spike_ratio: 3.1,
        };
        
        let alert = processor.trigger_detection(pattern).await.unwrap().unwrap();
        
        // Report as false positive
        let result = processor.report_false_positive(&alert.id).await;
        assert!(result.is_ok());
        
        // Check statistics
        let stats = processor.get_statistics().await;
        assert!(stats.contains_key("false_positives"));
    }

    #[tokio::test]
    async fn test_validation() {
        let processor = create_test_processor().await;
        
        // Valid commands
        let valid_commands = vec![
            DebugCommand::StartIssueDetection,
            DebugCommand::GetDetectedIssues { limit: Some(50) },
            DebugCommand::AcknowledgeIssue { alert_id: "test-id".to_string() },
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
            DebugCommand::AcknowledgeIssue { alert_id: "".to_string() },
            DebugCommand::UpdateDetectionRule {
                name: "".to_string(),
                enabled: None,
                sensitivity: None,
            },
            DebugCommand::UpdateDetectionRule {
                name: "test".to_string(),
                enabled: None,
                sensitivity: Some(1.5), // Out of range
            },
        ];
        
        for cmd in invalid_commands {
            assert!(processor.validate(&cmd).await.is_err(), "Should be invalid: {:?}", cmd);
        }
    }

    #[tokio::test]
    async fn test_processing_time_estimates() {
        let processor = create_test_processor().await;
        
        let test_cases = vec![
            (DebugCommand::StartIssueDetection, Duration::from_millis(50)),
            (DebugCommand::StopIssueDetection, Duration::from_millis(20)),
            (DebugCommand::GetDetectedIssues { limit: Some(10) }, Duration::from_millis(30)),
            (DebugCommand::GetIssueDetectionStats, Duration::from_millis(20)),
        ];
        
        for (cmd, expected) in test_cases {
            let estimated = processor.estimate_processing_time(&cmd);
            assert_eq!(estimated, expected, "Time mismatch for: {:?}", cmd);
        }
    }
}