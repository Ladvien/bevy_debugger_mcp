/// Automated Issue Detection System for Bevy Applications
/// 
/// This module provides intelligent pattern-based issue detection that continuously
/// monitors for common problems in Bevy applications and generates actionable alerts.
/// 
/// Key Features:
/// - Pattern-based detection for 15+ common issue types
/// - Configurable detection rules with hot-reload support
/// - Severity classification and alert throttling
/// - Remediation suggestions for each issue type
/// - ML-ready data collection for future enhancements

use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Configuration constants
pub mod constants {
    /// Maximum number of alerts to keep in history
    pub const MAX_ALERT_HISTORY: usize = 1000;
    /// Default alert throttle duration in seconds
    pub const DEFAULT_THROTTLE_SECONDS: u64 = 60;
    /// Maximum detection latency in milliseconds
    pub const MAX_DETECTION_LATENCY_MS: u64 = 500;
    /// Default false positive threshold percentage
    pub const FALSE_POSITIVE_THRESHOLD: f32 = 5.0;
    /// Maximum concurrent detectors
    pub const MAX_CONCURRENT_DETECTORS: usize = 20;
}

/// Issue severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum IssueSeverity {
    /// Low severity - performance suggestions
    Low,
    /// Medium severity - potential issues that should be addressed
    Medium,
    /// High severity - issues affecting functionality
    High,
    /// Critical severity - issues that may cause crashes or data loss
    Critical,
}

/// Common issue patterns that can be detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssuePattern {
    /// Transform contains NaN values
    TransformNaN { 
        entity_id: u32,
        component: String,
        values: Vec<f32>,
    },
    /// Memory leak detected
    MemoryLeak { 
        rate_mb_per_sec: f32,
        total_leaked_mb: f32,
        suspected_source: String,
    },
    /// Rendering stall detected
    RenderingStall { 
        duration_ms: f32,
        frame_number: u64,
        gpu_wait_time_ms: f32,
    },
    /// Entity count growing exponentially
    EntityExplosion { 
        growth_rate: f32,
        current_count: usize,
        time_window_sec: f32,
    },
    /// System execution taking too long
    SystemOverrun {
        system_name: String,
        execution_time_ms: f32,
        budget_ms: f32,
    },
    /// Excessive component additions/removals
    ComponentThrashing {
        entity_id: u32,
        component_type: String,
        changes_per_second: f32,
    },
    /// Resource contention detected
    ResourceContention {
        resource_name: String,
        wait_time_ms: f32,
        contending_systems: Vec<String>,
    },
    /// Frame time spike
    FrameSpike {
        frame_time_ms: f32,
        average_frame_time_ms: f32,
        spike_ratio: f32,
    },
    /// Asset loading failure
    AssetLoadFailure {
        asset_path: String,
        error_message: String,
        retry_count: u32,
    },
    /// Physics instability
    PhysicsInstability {
        entity_id: u32,
        velocity_magnitude: f32,
        position_delta: f32,
    },
    /// Render queue overflow
    RenderQueueOverflow {
        queue_size: usize,
        max_capacity: usize,
        dropped_items: usize,
    },
    /// Event queue backup
    EventQueueBackup {
        event_type: String,
        queue_depth: usize,
        processing_rate: f32,
    },
    /// Texture memory exhaustion
    TextureMemoryExhaustion {
        used_mb: f32,
        available_mb: f32,
        largest_texture_mb: f32,
    },
    /// Query performance degradation
    QueryPerformanceDegradation {
        query_description: String,
        execution_time_ms: f32,
        entity_count: usize,
    },
    /// State transition loop
    StateTransitionLoop {
        states: Vec<String>,
        transition_count: usize,
        time_window_sec: f32,
    },
    /// Audio buffer underrun
    AudioBufferUnderrun {
        underrun_count: u32,
        buffer_size: usize,
        sample_rate: u32,
    },
    /// Network latency spike
    NetworkLatencySpike {
        latency_ms: f32,
        average_latency_ms: f32,
        packet_loss_percent: f32,
    },
}

impl IssuePattern {
    /// Get the severity of this issue pattern
    pub fn severity(&self) -> IssueSeverity {
        match self {
            IssuePattern::TransformNaN { .. } => IssueSeverity::Critical,
            IssuePattern::MemoryLeak { rate_mb_per_sec, .. } => {
                if *rate_mb_per_sec > 10.0 {
                    IssueSeverity::Critical
                } else if *rate_mb_per_sec > 1.0 {
                    IssueSeverity::High
                } else {
                    IssueSeverity::Medium
                }
            }
            IssuePattern::RenderingStall { duration_ms, .. } => {
                if *duration_ms > 100.0 {
                    IssueSeverity::Critical
                } else if *duration_ms > 33.0 {
                    IssueSeverity::High
                } else {
                    IssueSeverity::Medium
                }
            }
            IssuePattern::EntityExplosion { growth_rate, .. } => {
                if *growth_rate > 2.0 {
                    IssueSeverity::Critical
                } else {
                    IssueSeverity::High
                }
            }
            IssuePattern::SystemOverrun { .. } => IssueSeverity::High,
            IssuePattern::ComponentThrashing { .. } => IssueSeverity::Medium,
            IssuePattern::ResourceContention { wait_time_ms, .. } => {
                if *wait_time_ms > 50.0 {
                    IssueSeverity::High
                } else {
                    IssueSeverity::Medium
                }
            }
            IssuePattern::FrameSpike { spike_ratio, .. } => {
                if *spike_ratio > 3.0 {
                    IssueSeverity::High
                } else {
                    IssueSeverity::Medium
                }
            }
            IssuePattern::AssetLoadFailure { retry_count, .. } => {
                if *retry_count > 5 {
                    IssueSeverity::High
                } else {
                    IssueSeverity::Medium
                }
            }
            IssuePattern::PhysicsInstability { .. } => IssueSeverity::High,
            IssuePattern::RenderQueueOverflow { .. } => IssueSeverity::High,
            IssuePattern::EventQueueBackup { queue_depth, .. } => {
                if *queue_depth > 1000 {
                    IssueSeverity::High
                } else {
                    IssueSeverity::Medium
                }
            }
            IssuePattern::TextureMemoryExhaustion { .. } => IssueSeverity::Critical,
            IssuePattern::QueryPerformanceDegradation { .. } => IssueSeverity::Medium,
            IssuePattern::StateTransitionLoop { .. } => IssueSeverity::High,
            IssuePattern::AudioBufferUnderrun { .. } => IssueSeverity::Medium,
            IssuePattern::NetworkLatencySpike { packet_loss_percent, .. } => {
                if *packet_loss_percent > 10.0 {
                    IssueSeverity::High
                } else {
                    IssueSeverity::Medium
                }
            }
        }
    }

    /// Get a unique identifier for this issue pattern
    pub fn pattern_id(&self) -> String {
        match self {
            IssuePattern::TransformNaN { entity_id, .. } => format!("transform_nan_{}", entity_id),
            IssuePattern::MemoryLeak { .. } => "memory_leak".to_string(),
            IssuePattern::RenderingStall { .. } => "rendering_stall".to_string(),
            IssuePattern::EntityExplosion { .. } => "entity_explosion".to_string(),
            IssuePattern::SystemOverrun { system_name, .. } => format!("system_overrun_{}", system_name),
            IssuePattern::ComponentThrashing { entity_id, component_type, .. } => {
                format!("component_thrashing_{}_{}", entity_id, component_type)
            }
            IssuePattern::ResourceContention { resource_name, .. } => {
                format!("resource_contention_{}", resource_name)
            }
            IssuePattern::FrameSpike { .. } => "frame_spike".to_string(),
            IssuePattern::AssetLoadFailure { asset_path, .. } => {
                format!("asset_load_failure_{}", asset_path.replace('/', "_"))
            }
            IssuePattern::PhysicsInstability { entity_id, .. } => {
                format!("physics_instability_{}", entity_id)
            }
            IssuePattern::RenderQueueOverflow { .. } => "render_queue_overflow".to_string(),
            IssuePattern::EventQueueBackup { event_type, .. } => {
                format!("event_queue_backup_{}", event_type)
            }
            IssuePattern::TextureMemoryExhaustion { .. } => "texture_memory_exhaustion".to_string(),
            IssuePattern::QueryPerformanceDegradation { query_description, .. } => {
                format!("query_degradation_{}", query_description.replace(' ', "_"))
            }
            IssuePattern::StateTransitionLoop { .. } => "state_transition_loop".to_string(),
            IssuePattern::AudioBufferUnderrun { .. } => "audio_buffer_underrun".to_string(),
            IssuePattern::NetworkLatencySpike { .. } => "network_latency_spike".to_string(),
        }
    }
}

/// Alert generated when an issue is detected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueAlert {
    /// Unique alert ID
    pub id: String,
    /// The detected issue pattern
    pub pattern: IssuePattern,
    /// Severity of the issue
    pub severity: IssueSeverity,
    /// When the issue was detected
    pub detected_at: DateTime<Utc>,
    /// Detection latency in milliseconds
    pub detection_latency_ms: u64,
    /// Suggested remediation steps
    pub remediation: Vec<String>,
    /// Additional context data
    pub context: HashMap<String, serde_json::Value>,
    /// Whether this alert has been acknowledged
    pub acknowledged: bool,
}

/// Detection rule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionRule {
    /// Rule name
    pub name: String,
    /// Rule description
    pub description: String,
    /// Whether the rule is enabled
    pub enabled: bool,
    /// Sensitivity level (0.0 = least sensitive, 1.0 = most sensitive)
    pub sensitivity: f32,
    /// Minimum occurrences before alerting
    pub min_occurrences: u32,
    /// Time window for occurrence counting (seconds)
    pub time_window_seconds: u64,
    /// Custom parameters for the rule
    pub parameters: HashMap<String, serde_json::Value>,
}

impl Default for DetectionRule {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            enabled: true,
            sensitivity: 0.5,
            min_occurrences: 1,
            time_window_seconds: 60,
            parameters: HashMap::new(),
        }
    }
}

/// Configuration for the issue detector
#[derive(Debug, Clone)]
pub struct IssueDetectorConfig {
    /// Maximum number of concurrent detectors
    pub max_concurrent_detectors: usize,
    /// Alert history size
    pub max_alert_history: usize,
    /// Default throttle duration for alerts
    pub default_throttle_duration: Duration,
    /// Enable ML data collection
    pub enable_ml_collection: bool,
    /// Detection check interval
    pub detection_interval: Duration,
    /// Maximum detection latency allowed
    pub max_detection_latency: Duration,
}

impl Default for IssueDetectorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_detectors: constants::MAX_CONCURRENT_DETECTORS,
            max_alert_history: constants::MAX_ALERT_HISTORY,
            default_throttle_duration: Duration::from_secs(constants::DEFAULT_THROTTLE_SECONDS),
            enable_ml_collection: false,
            detection_interval: Duration::from_millis(100),
            max_detection_latency: Duration::from_millis(constants::MAX_DETECTION_LATENCY_MS),
        }
    }
}

/// Main issue detector that coordinates detection and alerting
pub struct IssueDetector {
    /// Configuration
    config: IssueDetectorConfig,
    /// Detection rules
    rules: Arc<RwLock<HashMap<String, DetectionRule>>>,
    /// Alert history
    alert_history: Arc<RwLock<VecDeque<IssueAlert>>>,
    /// Alert throttling map (pattern_id -> last_alert_time)
    throttle_map: Arc<RwLock<HashMap<String, Instant>>>,
    /// Active detectors
    active_detectors: Arc<RwLock<HashSet<String>>>,
    /// ML data collection buffer
    ml_data_buffer: Arc<RwLock<Vec<serde_json::Value>>>,
    /// Detection statistics
    stats: Arc<RwLock<DetectionStatistics>>,
}

/// Statistics for detection system
#[derive(Debug, Default, Clone)]
struct DetectionStatistics {
    /// Total issues detected
    total_detected: u64,
    /// False positives reported
    false_positives: u64,
    /// Average detection latency
    avg_detection_latency_ms: f64,
    /// Detection rule hit counts
    rule_hits: HashMap<String, u64>,
    /// Last detection time
    last_detection: Option<Instant>,
}

impl IssueDetector {
    /// Create a new issue detector
    pub fn new(config: IssueDetectorConfig) -> Self {
        Self {
            config,
            rules: Arc::new(RwLock::new(Self::default_rules())),
            alert_history: Arc::new(RwLock::new(VecDeque::with_capacity(constants::MAX_ALERT_HISTORY))),
            throttle_map: Arc::new(RwLock::new(HashMap::new())),
            active_detectors: Arc::new(RwLock::new(HashSet::new())),
            ml_data_buffer: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(DetectionStatistics::default())),
        }
    }

    /// Get default detection rules
    fn default_rules() -> HashMap<String, DetectionRule> {
        let mut rules = HashMap::new();

        // Transform NaN detection
        rules.insert("transform_nan".to_string(), DetectionRule {
            name: "Transform NaN Detection".to_string(),
            description: "Detects NaN values in transform components".to_string(),
            enabled: true,
            sensitivity: 1.0, // Always detect NaN
            min_occurrences: 1,
            time_window_seconds: 1,
            parameters: HashMap::new(),
        });

        // Memory leak detection
        rules.insert("memory_leak".to_string(), DetectionRule {
            name: "Memory Leak Detection".to_string(),
            description: "Detects continuous memory growth patterns".to_string(),
            enabled: true,
            sensitivity: 0.7,
            min_occurrences: 5,
            time_window_seconds: 60,
            parameters: {
                let mut params = HashMap::new();
                params.insert("min_rate_mb_per_sec".to_string(), serde_json::json!(0.1));
                params
            },
        });

        // Frame spike detection
        rules.insert("frame_spike".to_string(), DetectionRule {
            name: "Frame Spike Detection".to_string(),
            description: "Detects sudden frame time increases".to_string(),
            enabled: true,
            sensitivity: 0.5,
            min_occurrences: 3,
            time_window_seconds: 10,
            parameters: {
                let mut params = HashMap::new();
                params.insert("spike_ratio_threshold".to_string(), serde_json::json!(2.0));
                params
            },
        });

        // Entity explosion detection
        rules.insert("entity_explosion".to_string(), DetectionRule {
            name: "Entity Explosion Detection".to_string(),
            description: "Detects rapid entity count growth".to_string(),
            enabled: true,
            sensitivity: 0.6,
            min_occurrences: 1,
            time_window_seconds: 5,
            parameters: {
                let mut params = HashMap::new();
                params.insert("growth_rate_threshold".to_string(), serde_json::json!(1.5));
                params
            },
        });

        // Add more default rules...
        rules
    }

    /// Detect an issue pattern
    pub async fn detect_issue(&self, pattern: IssuePattern) -> Result<Option<IssueAlert>> {
        let detection_start = Instant::now();
        let pattern_id = pattern.pattern_id();

        // Check if detection should be throttled
        if self.is_throttled(&pattern_id).await {
            debug!("Issue detection throttled for pattern: {}", pattern_id);
            return Ok(None);
        }

        // Check if rule is enabled and meets criteria
        let should_alert = self.check_detection_criteria(&pattern).await?;
        
        if !should_alert {
            return Ok(None);
        }

        // Generate remediation suggestions
        let remediation = self.generate_remediation(&pattern);

        // Create alert
        let alert = IssueAlert {
            id: uuid::Uuid::new_v4().to_string(),
            pattern: pattern.clone(),
            severity: pattern.severity(),
            detected_at: Utc::now(),
            detection_latency_ms: detection_start.elapsed().as_millis() as u64,
            remediation,
            context: HashMap::new(),
            acknowledged: false,
        };

        // Record alert
        self.record_alert(alert.clone()).await?;

        // Update throttle map
        {
            let mut throttle_map = self.throttle_map.write().await;
            throttle_map.insert(pattern_id.clone(), Instant::now());
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_detected += 1;
            stats.last_detection = Some(Instant::now());
            
            let latency = detection_start.elapsed().as_millis() as f64;
            stats.avg_detection_latency_ms = 
                (stats.avg_detection_latency_ms * (stats.total_detected - 1) as f64 + latency) 
                / stats.total_detected as f64;
        }

        // Collect ML data if enabled
        if self.config.enable_ml_collection {
            self.collect_ml_data(&alert).await;
        }

        info!("Issue detected: {:?} with severity {:?}", pattern_id, alert.severity);
        Ok(Some(alert))
    }

    /// Check if an issue should be throttled
    async fn is_throttled(&self, pattern_id: &str) -> bool {
        let throttle_map = self.throttle_map.read().await;
        
        if let Some(last_alert_time) = throttle_map.get(pattern_id) {
            last_alert_time.elapsed() < self.config.default_throttle_duration
        } else {
            false
        }
    }

    /// Check if detection criteria are met
    async fn check_detection_criteria(&self, pattern: &IssuePattern) -> Result<bool> {
        let rules = self.rules.read().await;
        
        // Find applicable rule
        let rule_key = match pattern {
            IssuePattern::TransformNaN { .. } => "transform_nan",
            IssuePattern::MemoryLeak { .. } => "memory_leak",
            IssuePattern::FrameSpike { .. } => "frame_spike",
            IssuePattern::EntityExplosion { .. } => "entity_explosion",
            _ => return Ok(true), // Default to alerting if no specific rule
        };

        if let Some(rule) = rules.get(rule_key) {
            if !rule.enabled {
                return Ok(false);
            }

            // Apply sensitivity threshold
            // This is simplified - in production you'd have more sophisticated logic
            Ok(rule.sensitivity > 0.0)
        } else {
            Ok(true)
        }
    }

    /// Generate remediation suggestions for an issue
    fn generate_remediation(&self, pattern: &IssuePattern) -> Vec<String> {
        match pattern {
            IssuePattern::TransformNaN { entity_id, component, .. } => vec![
                format!("Check calculations affecting entity {} transform", entity_id),
                format!("Validate input data for {} component", component),
                "Add NaN checks in transform update systems".to_string(),
                "Consider using safe math operations (e.g., clamping)".to_string(),
            ],
            IssuePattern::MemoryLeak { suspected_source, .. } => vec![
                format!("Review memory allocations in {}", suspected_source),
                "Check for circular references or retained collections".to_string(),
                "Ensure resources are properly cleaned up".to_string(),
                "Profile memory allocations with detailed tracking".to_string(),
            ],
            IssuePattern::RenderingStall { .. } => vec![
                "Check GPU synchronization points".to_string(),
                "Review render pipeline for bottlenecks".to_string(),
                "Consider using async compute for heavy operations".to_string(),
                "Optimize draw call batching".to_string(),
            ],
            IssuePattern::EntityExplosion { .. } => vec![
                "Review entity spawning logic for uncontrolled loops".to_string(),
                "Add entity count limits or pooling".to_string(),
                "Check for recursive entity spawning".to_string(),
                "Implement entity lifecycle management".to_string(),
            ],
            IssuePattern::SystemOverrun { system_name, budget_ms, .. } => vec![
                format!("Optimize {} system implementation", system_name),
                format!("Consider increasing budget from {}ms", budget_ms),
                "Profile system to identify bottlenecks".to_string(),
                "Consider parallelizing or splitting the system".to_string(),
            ],
            IssuePattern::ComponentThrashing { entity_id, component_type, .. } => vec![
                format!("Review component lifecycle for entity {}", entity_id),
                format!("Consider using a different pattern than adding/removing {}", component_type),
                "Use component flags instead of add/remove".to_string(),
                "Batch component operations".to_string(),
            ],
            _ => vec![
                "Review relevant system logs for more details".to_string(),
                "Check documentation for best practices".to_string(),
                "Consider filing a bug report if issue persists".to_string(),
            ],
        }
    }

    /// Record an alert in history
    async fn record_alert(&self, alert: IssueAlert) -> Result<()> {
        let mut history = self.alert_history.write().await;
        
        // Strictly enforce max history size - never exceed it
        if history.len() >= self.config.max_alert_history {
            history.pop_front();
        }
        
        history.push_back(alert);
        
        // Debug assertion to ensure we never exceed the limit
        debug_assert!(history.len() <= self.config.max_alert_history, 
                     "Alert history exceeded maximum size");
        
        Ok(())
    }

    /// Collect data for ML training
    async fn collect_ml_data(&self, alert: &IssueAlert) {
        const MAX_ML_BUFFER_SIZE: usize = 10000;
        const DRAIN_TO_SIZE: usize = 5000;
        
        let mut buffer = self.ml_data_buffer.write().await;
        
        // Strictly enforce buffer limit - check before insertion
        if buffer.len() >= MAX_ML_BUFFER_SIZE {
            // Drain older records to make room
            let drain_count = buffer.len() - DRAIN_TO_SIZE;
            buffer.drain(0..drain_count);
        }
        
        let ml_record = serde_json::json!({
            "timestamp": alert.detected_at.to_rfc3339(),
            "pattern_type": format!("{:?}", std::mem::discriminant(&alert.pattern)),
            "severity": alert.severity,
            "detection_latency_ms": alert.detection_latency_ms,
            "pattern_data": alert.pattern,
        });
        
        buffer.push(ml_record);
        
        // Debug assertion to ensure we never exceed the limit
        debug_assert!(buffer.len() <= MAX_ML_BUFFER_SIZE,
                     "ML buffer exceeded maximum size");
    }

    /// Update a detection rule
    pub async fn update_rule(&self, name: String, rule: DetectionRule) -> Result<()> {
        let mut rules = self.rules.write().await;
        let name_clone = name.clone();
        rules.insert(name, rule);
        info!("Updated detection rule: {}", name_clone);
        Ok(())
    }

    /// Get all detection rules
    pub async fn get_rules(&self) -> HashMap<String, DetectionRule> {
        let rules = self.rules.read().await;
        rules.clone()
    }

    /// Get alert history
    pub async fn get_alert_history(&self, limit: Option<usize>) -> Vec<IssueAlert> {
        let history = self.alert_history.read().await;
        let limit = limit.unwrap_or(100).min(history.len());
        
        history.iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    /// Mark an alert as acknowledged
    pub async fn acknowledge_alert(&self, alert_id: &str) -> Result<()> {
        let mut history = self.alert_history.write().await;
        
        for alert in history.iter_mut() {
            if alert.id == alert_id {
                alert.acknowledged = true;
                return Ok(());
            }
        }
        
        Err(Error::Validation(format!("Alert not found: {}", alert_id)))
    }

    /// Report a false positive
    pub async fn report_false_positive(&self, alert_id: &str) -> Result<()> {
        let mut stats = self.stats.write().await;
        stats.false_positives += 1;
        
        // Mark alert as acknowledged
        self.acknowledge_alert(alert_id).await?;
        
        info!("False positive reported for alert: {}", alert_id);
        Ok(())
    }

    /// Get detection statistics
    pub async fn get_statistics(&self) -> HashMap<String, serde_json::Value> {
        let stats = self.stats.read().await;
        let mut result = HashMap::new();
        
        result.insert("total_detected".to_string(), serde_json::json!(stats.total_detected));
        result.insert("false_positives".to_string(), serde_json::json!(stats.false_positives));
        result.insert("avg_detection_latency_ms".to_string(), serde_json::json!(stats.avg_detection_latency_ms));
        
        let false_positive_rate = if stats.total_detected > 0 {
            (stats.false_positives as f64 / stats.total_detected as f64) * 100.0
        } else {
            0.0
        };
        result.insert("false_positive_rate".to_string(), serde_json::json!(false_positive_rate));
        
        result
    }

    /// Clear alert history
    pub async fn clear_history(&self) {
        let mut history = self.alert_history.write().await;
        history.clear();
        info!("Alert history cleared");
    }

    /// Export ML training data
    pub async fn export_ml_data(&self) -> Vec<serde_json::Value> {
        let buffer = self.ml_data_buffer.read().await;
        buffer.clone()
    }
}

/// Helper function to check for NaN in transform values
pub fn check_transform_nan(values: &[f32]) -> bool {
    values.iter().any(|v| v.is_nan())
}

/// Helper function to calculate memory leak rate
pub fn calculate_memory_leak_rate(samples: &[(f32, Instant)]) -> f32 {
    if samples.len() < 2 {
        return 0.0;
    }
    
    let first = &samples[0];
    let last = &samples[samples.len() - 1];
    
    let memory_diff = last.0 - first.0;
    let time_diff = last.1.duration_since(first.1).as_secs_f32();
    
    if time_diff > 0.0 {
        memory_diff / time_diff
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_issue_detection() {
        let config = IssueDetectorConfig::default();
        let detector = IssueDetector::new(config);

        let pattern = IssuePattern::TransformNaN {
            entity_id: 123,
            component: "Transform".to_string(),
            values: vec![f32::NAN, 1.0, 2.0],
        };

        let alert = detector.detect_issue(pattern).await.unwrap();
        assert!(alert.is_some());
        
        let alert = alert.unwrap();
        assert_eq!(alert.severity, IssueSeverity::Critical);
        assert!(!alert.remediation.is_empty());
    }

    #[tokio::test]
    async fn test_alert_throttling() {
        let config = IssueDetectorConfig {
            default_throttle_duration: Duration::from_millis(100),
            ..Default::default()
        };
        let detector = IssueDetector::new(config);

        let pattern = IssuePattern::MemoryLeak {
            rate_mb_per_sec: 2.0,
            total_leaked_mb: 100.0,
            suspected_source: "TestSystem".to_string(),
        };

        // First detection should succeed
        let alert1 = detector.detect_issue(pattern.clone()).await.unwrap();
        assert!(alert1.is_some());

        // Immediate second detection should be throttled
        let alert2 = detector.detect_issue(pattern.clone()).await.unwrap();
        assert!(alert2.is_none());

        // After throttle duration, detection should succeed again
        tokio::time::sleep(Duration::from_millis(150)).await;
        let alert3 = detector.detect_issue(pattern).await.unwrap();
        assert!(alert3.is_some());
    }

    #[tokio::test]
    async fn test_severity_classification() {
        let patterns = vec![
            (IssuePattern::TransformNaN {
                entity_id: 1,
                component: "Transform".to_string(),
                values: vec![f32::NAN],
            }, IssueSeverity::Critical),
            (IssuePattern::MemoryLeak {
                rate_mb_per_sec: 0.5,
                total_leaked_mb: 10.0,
                suspected_source: "Test".to_string(),
            }, IssueSeverity::Medium),
            (IssuePattern::FrameSpike {
                frame_time_ms: 50.0,
                average_frame_time_ms: 16.0,
                spike_ratio: 3.1,
            }, IssueSeverity::High),
        ];

        for (pattern, expected_severity) in patterns {
            assert_eq!(pattern.severity(), expected_severity);
        }
    }

    #[test]
    fn test_nan_detection() {
        assert!(check_transform_nan(&[1.0, f32::NAN, 3.0]));
        assert!(!check_transform_nan(&[1.0, 2.0, 3.0]));
    }

    #[test]
    fn test_memory_leak_calculation() {
        let now = Instant::now();
        let samples = vec![
            (100.0, now),
            (110.0, now + Duration::from_secs(1)),
            (120.0, now + Duration::from_secs(2)),
        ];
        
        let rate = calculate_memory_leak_rate(&samples);
        assert!((rate - 10.0).abs() < 0.1); // ~10 MB/s
    }
}