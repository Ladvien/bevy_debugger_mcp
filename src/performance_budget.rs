/// Performance Budget Monitor System
/// 
/// This module provides comprehensive performance budget monitoring for Bevy applications,
/// tracking frame time, memory usage, and system execution against defined budgets.
/// It integrates with existing metrics collection and provides real-time alerts via MCP.

use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Maximum number of violations to keep in history
const MAX_VIOLATION_HISTORY: usize = 1000;

/// Maximum number of compliance samples to keep
const MAX_COMPLIANCE_SAMPLES: usize = 10000;

/// Default violation detection latency (100ms as per requirements)
const VIOLATION_DETECTION_LATENCY_MS: u64 = 100;

/// Platform detection interval
const PLATFORM_DETECTION_INTERVAL: Duration = Duration::from_secs(60);

/// Performance budget configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    /// Frame time budget in milliseconds
    pub frame_time_ms: Option<f32>,
    
    /// Memory usage budget in megabytes
    pub memory_mb: Option<f32>,
    
    /// System execution time budgets (system name -> milliseconds)
    pub system_budgets: HashMap<String, f32>,
    
    /// CPU usage budget (percentage 0-100)
    pub cpu_percent: Option<f32>,
    
    /// GPU time budget in milliseconds
    pub gpu_time_ms: Option<f32>,
    
    /// Entity count budget
    pub entity_count: Option<usize>,
    
    /// Draw call budget
    pub draw_calls: Option<usize>,
    
    /// Network bandwidth budget in KB/s
    pub network_bandwidth_kbps: Option<f32>,
    
    /// Platform-specific overrides
    pub platform_overrides: HashMap<Platform, PlatformBudgetOverride>,
    
    /// Whether to auto-adjust budgets based on percentiles
    pub auto_adjust: bool,
    
    /// Percentile for auto-adjustment (e.g., 95 for p95)
    pub auto_adjust_percentile: f32,
    
    /// Violation threshold (how many times budget can be exceeded before alert)
    pub violation_threshold: usize,
    
    /// Time window for violation counting (seconds)
    pub violation_window_seconds: u64,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            frame_time_ms: Some(16.67), // 60 FPS
            memory_mb: Some(500.0),
            system_budgets: HashMap::new(),
            cpu_percent: Some(80.0),
            gpu_time_ms: Some(16.0),
            entity_count: Some(10000),
            draw_calls: Some(1000),
            network_bandwidth_kbps: Some(1000.0),
            platform_overrides: HashMap::new(),
            auto_adjust: false,
            auto_adjust_percentile: 95.0,
            violation_threshold: 3,
            violation_window_seconds: 10,
        }
    }
}

/// Platform-specific budget overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformBudgetOverride {
    /// Override frame time budget
    pub frame_time_ms: Option<f32>,
    
    /// Override memory budget
    pub memory_mb: Option<f32>,
    
    /// Override CPU budget
    pub cpu_percent: Option<f32>,
    
    /// Override GPU budget
    pub gpu_time_ms: Option<f32>,
}

/// Platform detection
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum Platform {
    Windows,
    MacOS,
    Linux,
    Web,
    Mobile,
    Console,
    Unknown,
}

impl Platform {
    /// Detect current platform
    pub fn detect() -> Self {
        #[cfg(target_os = "windows")]
        return Platform::Windows;
        
        #[cfg(target_os = "macos")]
        return Platform::MacOS;
        
        #[cfg(target_os = "linux")]
        return Platform::Linux;
        
        #[cfg(target_arch = "wasm32")]
        return Platform::Web;
        
        #[cfg(any(target_os = "ios", target_os = "android"))]
        return Platform::Mobile;
        
        #[cfg(not(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux",
            target_arch = "wasm32",
            target_os = "ios",
            target_os = "android"
        )))]
        return Platform::Unknown;
    }
}

/// Performance metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Frame time in milliseconds
    pub frame_time_ms: f32,
    
    /// Memory usage in megabytes
    pub memory_mb: f32,
    
    /// System execution times (system name -> milliseconds)
    pub system_times: HashMap<String, f32>,
    
    /// CPU usage percentage
    pub cpu_percent: f32,
    
    /// GPU time in milliseconds
    pub gpu_time_ms: f32,
    
    /// Entity count
    pub entity_count: usize,
    
    /// Draw call count
    pub draw_calls: usize,
    
    /// Network bandwidth in KB/s
    pub network_bandwidth_kbps: f32,
    
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Budget violation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetViolation {
    /// Unique violation ID
    pub id: String,
    
    /// Metric that violated budget
    pub metric: ViolatedMetric,
    
    /// Actual value
    pub actual_value: f32,
    
    /// Budget value
    pub budget_value: f32,
    
    /// Violation percentage (how much over budget)
    pub violation_percent: f32,
    
    /// When the violation occurred
    pub timestamp: DateTime<Utc>,
    
    /// Duration of violation
    pub duration_ms: u64,
    
    /// Severity
    pub severity: ViolationSeverity,
    
    /// Context information
    pub context: HashMap<String, String>,
}

/// Violated metric type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViolatedMetric {
    FrameTime,
    Memory,
    SystemExecution(String),
    CpuUsage,
    GpuTime,
    EntityCount,
    DrawCalls,
    NetworkBandwidth,
}

/// Violation severity
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ViolationSeverity {
    Warning,  // 0-25% over budget
    Minor,    // 25-50% over budget
    Major,    // 50-100% over budget
    Critical, // >100% over budget
}

impl ViolationSeverity {
    fn from_violation_percent(percent: f32) -> Self {
        match percent {
            p if p <= 25.0 => ViolationSeverity::Warning,
            p if p <= 50.0 => ViolationSeverity::Minor,
            p if p <= 100.0 => ViolationSeverity::Major,
            _ => ViolationSeverity::Critical,
        }
    }
}

/// Budget compliance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    /// Time period of report
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    
    /// Total samples analyzed
    pub total_samples: usize,
    
    /// Compliance by metric
    pub metric_compliance: HashMap<String, MetricCompliance>,
    
    /// Overall compliance percentage
    pub overall_compliance_percent: f32,
    
    /// Top violations
    pub top_violations: Vec<BudgetViolation>,
    
    /// Recommendations
    pub recommendations: Vec<BudgetRecommendation>,
}

/// Metric compliance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricCompliance {
    /// Metric name
    pub metric_name: String,
    
    /// Number of compliant samples
    pub compliant_samples: usize,
    
    /// Number of violations
    pub violation_count: usize,
    
    /// Compliance percentage
    pub compliance_percent: f32,
    
    /// Average value
    pub avg_value: f32,
    
    /// P50 value
    pub p50_value: f32,
    
    /// P95 value
    pub p95_value: f32,
    
    /// P99 value
    pub p99_value: f32,
    
    /// Max value
    pub max_value: f32,
}

/// Budget recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetRecommendation {
    /// Metric to adjust
    pub metric: String,
    
    /// Current budget value
    pub current_budget: f32,
    
    /// Recommended budget value
    pub recommended_budget: f32,
    
    /// Reason for recommendation
    pub reason: String,
    
    /// Confidence level (0-1)
    pub confidence: f32,
}

/// Performance budget monitor
pub struct PerformanceBudgetMonitor {
    /// Inner state protected by RwLock
    inner: Arc<RwLock<PerformanceBudgetMonitorInner>>,
}

/// Inner state for the performance budget monitor
struct PerformanceBudgetMonitorInner {
    /// Budget configuration
    config: BudgetConfig,
    
    /// Current platform
    platform: Platform,
    
    /// Violation history
    violation_history: VecDeque<BudgetViolation>,
    
    /// Compliance samples
    compliance_samples: VecDeque<PerformanceMetrics>,
    
    /// Active violations (metric -> violation start time)
    active_violations: HashMap<String, Instant>,
    
    /// Monitoring state
    monitoring_active: bool,
    
    /// Last detection check
    last_detection: Instant,
    
    /// Statistics
    stats: BudgetStatistics,
}

/// Budget monitoring statistics
#[derive(Debug, Default, Serialize, Deserialize)]
struct BudgetStatistics {
    /// Total violations detected
    total_violations: u64,
    
    /// Violations by metric
    violations_by_metric: HashMap<String, u64>,
    
    /// Total samples processed
    total_samples: u64,
    
    /// Average detection latency
    avg_detection_latency_ms: f32,
    
    /// Last platform detection
    last_platform_detection: Option<DateTime<Utc>>,
}

impl PerformanceBudgetMonitor {
    /// Create a new performance budget monitor
    pub fn new(config: BudgetConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(PerformanceBudgetMonitorInner {
                config,
                platform: Platform::detect(),
                violation_history: VecDeque::with_capacity(MAX_VIOLATION_HISTORY),
                compliance_samples: VecDeque::with_capacity(MAX_COMPLIANCE_SAMPLES),
                active_violations: HashMap::new(),
                monitoring_active: false,
                last_detection: Instant::now(),
                stats: BudgetStatistics::default(),
            })),
        }
    }
    
    /// Start monitoring
    pub async fn start_monitoring(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        if inner.monitoring_active {
            return Err(Error::Validation("Monitoring already active".to_string()));
        }
        
        inner.monitoring_active = true;
        info!("Performance budget monitoring started");
        Ok(())
    }
    
    /// Stop monitoring
    pub async fn stop_monitoring(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        if !inner.monitoring_active {
            return Err(Error::Validation("Monitoring not active".to_string()));
        }
        
        inner.monitoring_active = false;
        info!("Performance budget monitoring stopped");
        Ok(())
    }
    
    /// Check for budget violations
    pub async fn check_violations(&self, metrics: PerformanceMetrics) -> Result<Vec<BudgetViolation>> {
        let start = Instant::now();
        let mut inner = self.inner.write().await;
        
        if !inner.monitoring_active {
            return Ok(Vec::new());
        }
        
        let mut violations = Vec::new();
        
        // Apply platform-specific overrides
        let effective_config = self.apply_platform_overrides(&inner.config, inner.platform);
        
        // Check frame time budget
        if let Some(budget) = effective_config.frame_time_ms {
            if metrics.frame_time_ms > budget {
                violations.push(self.create_violation_inner(
                    &mut inner,
                    ViolatedMetric::FrameTime,
                    metrics.frame_time_ms,
                    budget,
                    &metrics,
                ));
            }
        }
        
        // Check memory budget
        if let Some(budget) = effective_config.memory_mb {
            if metrics.memory_mb > budget {
                violations.push(self.create_violation_inner(
                    &mut inner,
                    ViolatedMetric::Memory,
                    metrics.memory_mb,
                    budget,
                    &metrics,
                ));
            }
        }
        
        // Check CPU budget
        if let Some(budget) = effective_config.cpu_percent {
            if metrics.cpu_percent > budget {
                violations.push(self.create_violation_inner(
                    &mut inner,
                    ViolatedMetric::CpuUsage,
                    metrics.cpu_percent,
                    budget,
                    &metrics,
                ));
            }
        }
        
        // Check GPU budget
        if let Some(budget) = effective_config.gpu_time_ms {
            if metrics.gpu_time_ms > budget {
                violations.push(self.create_violation_inner(
                    &mut inner,
                    ViolatedMetric::GpuTime,
                    metrics.gpu_time_ms,
                    budget,
                    &metrics,
                ));
            }
        }
        
        // Check entity count budget
        if let Some(budget) = effective_config.entity_count {
            if metrics.entity_count > budget {
                violations.push(self.create_violation_inner(
                    &mut inner,
                    ViolatedMetric::EntityCount,
                    metrics.entity_count as f32,
                    budget as f32,
                    &metrics,
                ));
            }
        }
        
        // Check draw calls budget
        if let Some(budget) = effective_config.draw_calls {
            if metrics.draw_calls > budget {
                violations.push(self.create_violation_inner(
                    &mut inner,
                    ViolatedMetric::DrawCalls,
                    metrics.draw_calls as f32,
                    budget as f32,
                    &metrics,
                ));
            }
        }
        
        // Check network bandwidth budget
        if let Some(budget) = effective_config.network_bandwidth_kbps {
            if metrics.network_bandwidth_kbps > budget {
                violations.push(self.create_violation_inner(
                    &mut inner,
                    ViolatedMetric::NetworkBandwidth,
                    metrics.network_bandwidth_kbps,
                    budget,
                    &metrics,
                ));
            }
        }
        
        // Check system-specific budgets
        for (system_name, &budget) in &effective_config.system_budgets {
            if let Some(&actual) = metrics.system_times.get(system_name) {
                if actual > budget {
                    violations.push(self.create_violation_inner(
                        &mut inner,
                        ViolatedMetric::SystemExecution(system_name.clone()),
                        actual,
                        budget,
                        &metrics,
                    ));
                }
            }
        }
        
        // Record the sample for compliance tracking
        Self::record_sample_inner(&mut inner, metrics);
        
        // Record violations
        for violation in &violations {
            Self::record_violation_inner(&mut inner, violation.clone());
        }
        
        // Update statistics
        let detection_latency = start.elapsed().as_millis() as f32;
        Self::update_statistics_inner(&mut inner, violations.len(), detection_latency);
        
        // Check detection latency requirement (< 100ms)
        if detection_latency > VIOLATION_DETECTION_LATENCY_MS as f32 {
            warn!("Violation detection exceeded latency budget: {}ms", detection_latency);
        }
        
        Ok(violations)
    }
    
    /// Apply platform-specific overrides to configuration
    fn apply_platform_overrides(&self, config: &BudgetConfig, platform: Platform) -> BudgetConfig {
        let mut effective = config.clone();
        
        if let Some(overrides) = config.platform_overrides.get(&platform) {
            if let Some(frame_time) = overrides.frame_time_ms {
                effective.frame_time_ms = Some(frame_time);
            }
            if let Some(memory) = overrides.memory_mb {
                effective.memory_mb = Some(memory);
            }
            if let Some(cpu) = overrides.cpu_percent {
                effective.cpu_percent = Some(cpu);
            }
            if let Some(gpu) = overrides.gpu_time_ms {
                effective.gpu_time_ms = Some(gpu);
            }
        }
        
        effective
    }
    
    /// Create a violation record (internal helper)
    fn create_violation_inner(
        &self,
        inner: &mut PerformanceBudgetMonitorInner,
        metric: ViolatedMetric,
        actual: f32,
        budget: f32,
        metrics: &PerformanceMetrics,
    ) -> BudgetViolation {
        let violation_percent = ((actual - budget) / budget) * 100.0;
        let severity = ViolationSeverity::from_violation_percent(violation_percent);
        
        // Use enum variant discriminant as key to avoid string allocation
        let metric_key = format!("{:?}", std::mem::discriminant(&metric));
        let duration_ms = if let Some(start_time) = inner.active_violations.get(&metric_key) {
            start_time.elapsed().as_millis() as u64
        } else {
            inner.active_violations.insert(metric_key.clone(), Instant::now());
            0
        };
        
        BudgetViolation {
            id: uuid::Uuid::new_v4().to_string(),
            metric,
            actual_value: actual,
            budget_value: budget,
            violation_percent,
            timestamp: metrics.timestamp,
            duration_ms,
            severity,
            context: HashMap::new(),
        }
    }
    
    /// Record a violation (internal helper)
    fn record_violation_inner(
        inner: &mut PerformanceBudgetMonitorInner,
        violation: BudgetViolation,
    ) {
        // Maintain max history size
        if inner.violation_history.len() >= MAX_VIOLATION_HISTORY {
            inner.violation_history.pop_front();
        }
        
        inner.violation_history.push_back(violation);
    }
    
    /// Record a metrics sample (internal helper)
    fn record_sample_inner(
        inner: &mut PerformanceBudgetMonitorInner,
        metrics: PerformanceMetrics,
    ) {
        // Maintain max sample size
        if inner.compliance_samples.len() >= MAX_COMPLIANCE_SAMPLES {
            inner.compliance_samples.pop_front();
        }
        
        inner.compliance_samples.push_back(metrics);
    }
    
    /// Update statistics (internal helper)
    fn update_statistics_inner(
        inner: &mut PerformanceBudgetMonitorInner,
        violation_count: usize,
        detection_latency_ms: f32,
    ) {
        inner.stats.total_violations += violation_count as u64;
        inner.stats.total_samples += 1;
        
        // Update moving average for detection latency
        let alpha = 0.1; // Smoothing factor
        inner.stats.avg_detection_latency_ms = 
            (1.0 - alpha) * inner.stats.avg_detection_latency_ms + alpha * detection_latency_ms;
    }
    
    /// Generate compliance report
    pub async fn generate_compliance_report(&self, duration: Duration) -> Result<ComplianceReport> {
        let inner = self.inner.read().await;
        let now = Utc::now();
        let period_start = now - chrono::Duration::from_std(duration).unwrap();
        
        // Filter samples within the period
        let period_samples: Vec<_> = inner.compliance_samples
            .iter()
            .filter(|s| s.timestamp >= period_start)
            .cloned()
            .collect();
        
        if period_samples.is_empty() {
            return Err(Error::Validation("No samples in specified period".to_string()));
        }
        
        // Calculate compliance by metric
        let mut metric_compliance = HashMap::new();
        
        // Frame time compliance
        if let Some(budget) = inner.config.frame_time_ms {
            let compliance = self.calculate_metric_compliance(
                &period_samples,
                "frame_time",
                |m| m.frame_time_ms,
                budget,
            );
            metric_compliance.insert("frame_time".to_string(), compliance);
        }
        
        // Memory compliance
        if let Some(budget) = inner.config.memory_mb {
            let compliance = self.calculate_metric_compliance(
                &period_samples,
                "memory",
                |m| m.memory_mb,
                budget,
            );
            metric_compliance.insert("memory".to_string(), compliance);
        }
        
        // CPU compliance
        if let Some(budget) = inner.config.cpu_percent {
            let compliance = self.calculate_metric_compliance(
                &period_samples,
                "cpu",
                |m| m.cpu_percent,
                budget,
            );
            metric_compliance.insert("cpu".to_string(), compliance);
        }
        
        // Calculate overall compliance
        let total_compliant: usize = metric_compliance.values()
            .map(|c| c.compliant_samples)
            .sum();
        let total_violations: usize = metric_compliance.values()
            .map(|c| c.violation_count)
            .sum();
        let overall_compliance_percent = 
            (total_compliant as f32 / (total_compliant + total_violations) as f32) * 100.0;
        
        // Get top violations
        let top_violations: Vec<_> = inner.violation_history
            .iter()
            .filter(|v| v.timestamp >= period_start)
            .take(10)
            .cloned()
            .collect();
        
        // Generate recommendations
        let recommendations = Self::generate_recommendations(&inner.config, &metric_compliance);
        
        Ok(ComplianceReport {
            period_start,
            period_end: now,
            total_samples: period_samples.len(),
            metric_compliance,
            overall_compliance_percent,
            top_violations,
            recommendations,
        })
    }
    
    /// Calculate compliance for a specific metric
    fn calculate_metric_compliance<F>(
        &self,
        samples: &[PerformanceMetrics],
        metric_name: &str,
        extractor: F,
        budget: f32,
    ) -> MetricCompliance
    where
        F: Fn(&PerformanceMetrics) -> f32,
    {
        let values: Vec<f32> = samples.iter().map(&extractor).collect();
        let mut sorted_values = values.clone();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let compliant_count = values.iter().filter(|&&v| v <= budget).count();
        let violation_count = values.len() - compliant_count;
        
        let avg_value = values.iter().sum::<f32>() / values.len() as f32;
        let p50_value = sorted_values[sorted_values.len() / 2];
        let p95_value = sorted_values[(sorted_values.len() as f32 * 0.95) as usize];
        let p99_value = sorted_values[(sorted_values.len() as f32 * 0.99) as usize];
        let max_value = *sorted_values.last().unwrap();
        
        MetricCompliance {
            metric_name: metric_name.to_string(),
            compliant_samples: compliant_count,
            violation_count,
            compliance_percent: (compliant_count as f32 / values.len() as f32) * 100.0,
            avg_value,
            p50_value,
            p95_value,
            p99_value,
            max_value,
        }
    }
    
    /// Generate budget recommendations
    fn generate_recommendations(
        config: &BudgetConfig,
        compliance: &HashMap<String, MetricCompliance>,
    ) -> Vec<BudgetRecommendation> {
        let mut recommendations = Vec::new();
        
        for (metric_name, stats) in compliance {
            // Recommend budget adjustment if compliance is low
            if stats.compliance_percent < 90.0 {
                let percentile = config.auto_adjust_percentile;
                let recommended_value = if percentile <= 50.0 {
                    stats.p50_value
                } else if percentile <= 95.0 {
                    stats.p95_value
                } else {
                    stats.p99_value
                };
                
                let current_budget = match metric_name.as_str() {
                    "frame_time" => config.frame_time_ms.unwrap_or(16.67),
                    "memory" => config.memory_mb.unwrap_or(500.0),
                    "cpu" => config.cpu_percent.unwrap_or(80.0),
                    "gpu_time" => config.gpu_time_ms.unwrap_or(16.0),
                    _ => continue, // Skip unknown metrics
                };
                
                // Only recommend if there's a meaningful difference
                if (recommended_value - current_budget).abs() > current_budget * 0.05 {
                    recommendations.push(BudgetRecommendation {
                        metric: metric_name.clone(),
                        current_budget,
                        recommended_budget: recommended_value * 1.1, // Add 10% buffer
                        reason: format!(
                            "Current compliance is {:.1}%. Adjusting to P{} would improve compliance.",
                            stats.compliance_percent, percentile
                        ),
                        confidence: 0.8,
                    });
                }
            }
        }
        
        recommendations
    }
    
    /// Update configuration
    pub async fn update_config(&self, config: BudgetConfig) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.config = config;
        info!("Performance budget configuration updated");
        Ok(())
    }
    
    /// Get current configuration
    pub async fn get_config(&self) -> BudgetConfig {
        self.inner.read().await.config.clone()
    }
    
    /// Get violation history
    pub async fn get_violation_history(&self, limit: Option<usize>) -> Vec<BudgetViolation> {
        let inner = self.inner.read().await;
        let limit = limit.unwrap_or(100).min(inner.violation_history.len());
        
        inner.violation_history.iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }
    
    /// Clear violation history
    pub async fn clear_violation_history(&self) {
        let mut inner = self.inner.write().await;
        inner.violation_history.clear();
        inner.active_violations.clear();
        
        info!("Violation history cleared");
    }
    
    /// Detect and update platform
    pub async fn update_platform(&self) -> Platform {
        let detected = Platform::detect();
        let mut inner = self.inner.write().await;
        inner.platform = detected;
        inner.stats.last_platform_detection = Some(Utc::now());
        
        info!("Platform detected: {:?}", detected);
        detected
    }
    
    /// Get monitoring statistics
    pub async fn get_statistics(&self) -> HashMap<String, serde_json::Value> {
        let inner = self.inner.read().await;
        let mut result = HashMap::new();
        
        result.insert("total_violations".to_string(), 
            serde_json::json!(inner.stats.total_violations));
        result.insert("total_samples".to_string(), 
            serde_json::json!(inner.stats.total_samples));
        result.insert("avg_detection_latency_ms".to_string(), 
            serde_json::json!(inner.stats.avg_detection_latency_ms));
        result.insert("monitoring_active".to_string(), 
            serde_json::json!(inner.monitoring_active));
        result.insert("current_platform".to_string(), 
            serde_json::json!(format!("{:?}", inner.platform)));
        
        if let Some(last_detection) = inner.stats.last_platform_detection {
            result.insert("last_platform_detection".to_string(), 
                serde_json::json!(last_detection.to_rfc3339()));
        }
        
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_platform_detection() {
        let platform = Platform::detect();
        // Should detect something
        assert_ne!(platform, Platform::Unknown);
    }
    
    #[test]
    fn test_violation_severity() {
        assert!(matches!(ViolationSeverity::from_violation_percent(10.0), ViolationSeverity::Warning));
        assert!(matches!(ViolationSeverity::from_violation_percent(30.0), ViolationSeverity::Minor));
        assert!(matches!(ViolationSeverity::from_violation_percent(75.0), ViolationSeverity::Major));
        assert!(matches!(ViolationSeverity::from_violation_percent(150.0), ViolationSeverity::Critical));
    }
    
    #[tokio::test]
    async fn test_budget_monitor_creation() {
        let config = BudgetConfig::default();
        let monitor = PerformanceBudgetMonitor::new(config);
        
        // Should start inactive
        assert!(!monitor.inner.read().await.monitoring_active);
        
        // Should be able to start monitoring
        assert!(monitor.start_monitoring().await.is_ok());
        assert!(monitor.inner.read().await.monitoring_active);
        
        // Should not be able to start again
        assert!(monitor.start_monitoring().await.is_err());
        
        // Should be able to stop
        assert!(monitor.stop_monitoring().await.is_ok());
        assert!(!monitor.inner.read().await.monitoring_active);
    }
}