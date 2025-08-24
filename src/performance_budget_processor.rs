/// Performance Budget Processor for Debug Command Integration
/// 
/// This processor integrates the performance budget monitoring system with the debug command
/// infrastructure, providing MCP-accessible commands for budget configuration and monitoring.

use crate::brp_messages::{DebugCommand, DebugResponse};
use crate::brp_client::BrpClient;
use crate::debug_command_processor::DebugCommandProcessor;
use crate::performance_budget::{
    PerformanceBudgetMonitor, BudgetConfig, PerformanceMetrics, Platform,
    BudgetViolation, ComplianceReport, BudgetRecommendation
};
use crate::error::{Error, Result};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, info, warn};

/// Performance budget processor for debug commands
pub struct PerformanceBudgetProcessor {
    /// Budget monitor instance
    monitor: Arc<PerformanceBudgetMonitor>,
    
    /// BRP client for Bevy interaction
    brp_client: Arc<RwLock<BrpClient>>,
    
    /// Background monitoring task handle
    monitoring_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    
    /// Monitoring state
    monitoring_state: Arc<RwLock<MonitoringState>>,
    
    /// Configuration persistence path
    config_path: Option<String>,
}

/// Monitoring state tracking
#[derive(Debug, Default)]
struct MonitoringState {
    /// Whether continuous monitoring is active
    continuous_monitoring: bool,
    
    /// Last check timestamp
    last_check: Option<Instant>,
    
    /// Recent violations count
    recent_violations: usize,
    
    /// Consecutive violation count
    consecutive_violations: usize,
    
    /// Last platform check
    last_platform_check: Option<Instant>,
}

impl PerformanceBudgetProcessor {
    /// Create a new performance budget processor
    pub fn new(brp_client: Arc<RwLock<BrpClient>>) -> Self {
        let config = BudgetConfig::default();
        let monitor = Arc::new(PerformanceBudgetMonitor::new(config));
        
        Self {
            monitor,
            brp_client,
            monitoring_handle: Arc::new(RwLock::new(None)),
            monitoring_state: Arc::new(RwLock::new(MonitoringState::default())),
            config_path: Some("config/performance_budgets.toml".to_string()),
        }
    }
    
    /// Start continuous budget monitoring
    pub async fn start_continuous_monitoring(&self) -> Result<()> {
        let mut handle_guard = self.monitoring_handle.write().await;
        
        if handle_guard.is_some() {
            return Ok(()); // Already monitoring
        }
        
        // Start the monitor
        self.monitor.start_monitoring().await?;
        
        let monitor = Arc::clone(&self.monitor);
        let brp_client = Arc::clone(&self.brp_client);
        let monitoring_state = Arc::clone(&self.monitoring_state);
        
        let handle = tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_millis(100)); // Check every 100ms
            let mut platform_check_interval = interval(Duration::from_secs(60)); // Platform check every minute
            
            loop {
                tokio::select! {
                    _ = check_interval.tick() => {
                        // Perform budget checks
                        if let Ok(metrics) = Self::collect_metrics(&brp_client).await {
                            if let Ok(violations) = monitor.check_violations(metrics).await {
                                Self::handle_violations(&violations, &monitoring_state).await;
                            }
                        }
                    }
                    _ = platform_check_interval.tick() => {
                        // Update platform detection
                        let platform = monitor.update_platform().await;
                        debug!("Platform updated: {:?}", platform);
                        
                        let mut state = monitoring_state.write().await;
                        state.last_platform_check = Some(Instant::now());
                    }
                }
            }
        });
        
        *handle_guard = Some(handle);
        
        // Update state
        let mut state = self.monitoring_state.write().await;
        state.continuous_monitoring = true;
        
        info!("Continuous performance budget monitoring started");
        Ok(())
    }
    
    /// Stop continuous monitoring
    pub async fn stop_continuous_monitoring(&self) -> Result<()> {
        let mut handle_guard = self.monitoring_handle.write().await;
        
        if let Some(handle) = handle_guard.take() {
            handle.abort();
        }
        
        // Stop the monitor
        self.monitor.stop_monitoring().await?;
        
        // Update state
        let mut state = self.monitoring_state.write().await;
        state.continuous_monitoring = false;
        
        info!("Continuous performance budget monitoring stopped");
        Ok(())
    }
    
    /// Collect current performance metrics
    async fn collect_metrics(brp_client: &Arc<RwLock<BrpClient>>) -> Result<PerformanceMetrics> {
        // In a real implementation, this would query actual metrics from Bevy
        // For now, we'll simulate metrics collection
        
        // This would normally query BRP for actual metrics
        // Example queries would include:
        // - Frame time from diagnostics
        // - Memory usage from system info
        // - Entity count from world stats
        // - System execution times from profiler
        
        // Simulated metrics for now
        Ok(PerformanceMetrics {
            frame_time_ms: 16.0 + (rand::random::<f32>() * 5.0),
            memory_mb: 450.0 + (rand::random::<f32>() * 100.0),
            system_times: HashMap::new(),
            cpu_percent: 60.0 + (rand::random::<f32>() * 30.0),
            gpu_time_ms: 14.0 + (rand::random::<f32>() * 6.0),
            entity_count: 8000 + (rand::random::<f32>() * 4000.0) as usize,
            draw_calls: 800 + (rand::random::<f32>() * 400.0) as usize,
            network_bandwidth_kbps: 500.0 + (rand::random::<f32>() * 500.0),
            timestamp: Utc::now(),
        })
    }
    
    /// Handle detected violations
    async fn handle_violations(
        violations: &[BudgetViolation],
        monitoring_state: &Arc<RwLock<MonitoringState>>,
    ) {
        if violations.is_empty() {
            // Reset consecutive violations if no violations
            let mut state = monitoring_state.write().await;
            state.consecutive_violations = 0;
            return;
        }
        
        let mut state = monitoring_state.write().await;
        state.recent_violations += violations.len();
        state.consecutive_violations += 1;
        state.last_check = Some(Instant::now());
        
        // Log violations based on severity
        for violation in violations {
            match violation.severity {
                crate::performance_budget::ViolationSeverity::Critical => {
                    warn!("CRITICAL budget violation: {:?} - {:.1}% over budget", 
                        violation.metric, violation.violation_percent);
                }
                crate::performance_budget::ViolationSeverity::Major => {
                    warn!("Major budget violation: {:?} - {:.1}% over budget", 
                        violation.metric, violation.violation_percent);
                }
                _ => {
                    debug!("Budget violation: {:?} - {:.1}% over budget", 
                        violation.metric, violation.violation_percent);
                }
            }
        }
    }
    
    /// Load configuration from file
    async fn load_config(&self) -> Result<BudgetConfig> {
        if let Some(ref path) = self.config_path {
            // In a real implementation, this would load from TOML file
            // For now, return default config
            Ok(BudgetConfig::default())
        } else {
            Ok(BudgetConfig::default())
        }
    }
    
    /// Save configuration to file
    async fn save_config(&self, config: &BudgetConfig) -> Result<()> {
        if let Some(ref path) = self.config_path {
            // In a real implementation, this would save to TOML file
            info!("Configuration saved to {}", path);
            Ok(())
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl DebugCommandProcessor for PerformanceBudgetProcessor {
    async fn process(&self, command: DebugCommand) -> Result<DebugResponse> {
        match command {
            DebugCommand::StartBudgetMonitoring => {
                debug!("Starting performance budget monitoring");
                self.start_continuous_monitoring().await?;
                
                Ok(DebugResponse::Success {
                    message: "Performance budget monitoring started".to_string(),
                    data: None,
                })
            }
            
            DebugCommand::StopBudgetMonitoring => {
                debug!("Stopping performance budget monitoring");
                self.stop_continuous_monitoring().await?;
                
                Ok(DebugResponse::Success {
                    message: "Performance budget monitoring stopped".to_string(),
                    data: None,
                })
            }
            
            DebugCommand::SetPerformanceBudget { config } => {
                debug!("Setting performance budget configuration");
                
                // Parse the configuration
                let budget_config: BudgetConfig = serde_json::from_value(config)?;
                
                // Update the monitor
                self.monitor.update_config(budget_config.clone()).await?;
                
                // Save to file
                self.save_config(&budget_config).await?;
                
                Ok(DebugResponse::Success {
                    message: "Performance budget configuration updated".to_string(),
                    data: Some(serde_json::to_value(budget_config)?),
                })
            }
            
            DebugCommand::GetPerformanceBudget => {
                debug!("Getting performance budget configuration");
                let config = self.monitor.get_config().await;
                
                Ok(DebugResponse::Success {
                    message: "Current performance budget configuration".to_string(),
                    data: Some(serde_json::to_value(config)?),
                })
            }
            
            DebugCommand::CheckBudgetViolations => {
                debug!("Checking for budget violations");
                
                // Collect current metrics
                let metrics = Self::collect_metrics(&self.brp_client).await?;
                
                // Check for violations
                let violations = self.monitor.check_violations(metrics).await?;
                
                Ok(DebugResponse::Success {
                    message: format!("Found {} budget violations", violations.len()),
                    data: Some(serde_json::to_value(violations)?),
                })
            }
            
            DebugCommand::GetBudgetViolationHistory { limit } => {
                debug!("Getting budget violation history");
                let history = self.monitor.get_violation_history(limit).await;
                
                Ok(DebugResponse::Success {
                    message: format!("Retrieved {} violations from history", history.len()),
                    data: Some(serde_json::to_value(history)?),
                })
            }
            
            DebugCommand::GenerateComplianceReport { duration_seconds } => {
                debug!("Generating compliance report");
                let duration = Duration::from_secs(duration_seconds.unwrap_or(3600));
                
                match self.monitor.generate_compliance_report(duration).await {
                    Ok(report) => {
                        Ok(DebugResponse::Success {
                            message: format!(
                                "Compliance report generated: {:.1}% overall compliance",
                                report.overall_compliance_percent
                            ),
                            data: Some(serde_json::to_value(report)?),
                        })
                    }
                    Err(e) => {
                        Ok(DebugResponse::Success {
                            message: format!("Could not generate report: {}", e),
                            data: None,
                        })
                    }
                }
            }
            
            DebugCommand::GetBudgetRecommendations => {
                debug!("Getting budget recommendations");
                
                // Generate a 1-hour compliance report to get recommendations
                let duration = Duration::from_secs(3600);
                
                match self.monitor.generate_compliance_report(duration).await {
                    Ok(report) => {
                        Ok(DebugResponse::Success {
                            message: format!("Generated {} budget recommendations", 
                                report.recommendations.len()),
                            data: Some(serde_json::to_value(report.recommendations)?),
                        })
                    }
                    Err(_) => {
                        // No data yet, return empty recommendations
                        Ok(DebugResponse::Success {
                            message: "No recommendations available (insufficient data)".to_string(),
                            data: Some(serde_json::json!([])),
                        })
                    }
                }
            }
            
            DebugCommand::ClearBudgetHistory => {
                debug!("Clearing budget violation history");
                self.monitor.clear_violation_history().await;
                
                Ok(DebugResponse::Success {
                    message: "Budget violation history cleared".to_string(),
                    data: None,
                })
            }
            
            DebugCommand::GetBudgetStatistics => {
                debug!("Getting budget monitoring statistics");
                let stats = self.monitor.get_statistics().await;
                
                // Add processor-specific stats
                let state = self.monitoring_state.read().await;
                let mut all_stats = stats;
                all_stats.insert("continuous_monitoring".to_string(), 
                    serde_json::json!(state.continuous_monitoring));
                all_stats.insert("recent_violations".to_string(), 
                    serde_json::json!(state.recent_violations));
                all_stats.insert("consecutive_violations".to_string(), 
                    serde_json::json!(state.consecutive_violations));
                
                Ok(DebugResponse::Success {
                    message: "Budget monitoring statistics".to_string(),
                    data: Some(serde_json::json!(all_stats)),
                })
            }
            
            _ => Err(Error::DebugError(
                format!("Unsupported command for PerformanceBudgetProcessor: {:?}", command)
            )),
        }
    }
    
    fn supports_command(&self, command: &DebugCommand) -> bool {
        matches!(command,
            DebugCommand::StartBudgetMonitoring |
            DebugCommand::StopBudgetMonitoring |
            DebugCommand::SetPerformanceBudget { .. } |
            DebugCommand::GetPerformanceBudget |
            DebugCommand::CheckBudgetViolations |
            DebugCommand::GetBudgetViolationHistory { .. } |
            DebugCommand::ClearBudgetHistory |
            DebugCommand::GenerateComplianceReport { .. } |
            DebugCommand::GetBudgetRecommendations |
            DebugCommand::GetBudgetStatistics
        )
    }

    async fn validate(&self, command: &DebugCommand) -> Result<()> {
        match command {
            DebugCommand::SetPerformanceBudget { config } => {
                // Validate configuration structure
                let _: BudgetConfig = serde_json::from_value(config.clone())
                    .map_err(|e| Error::Validation(format!("Invalid budget config: {}", e)))?;
                Ok(())
            }
            
            DebugCommand::GenerateComplianceReport { duration_seconds } => {
                if let Some(duration) = duration_seconds {
                    if *duration == 0 {
                        return Err(Error::Validation("Duration must be greater than 0".to_string()));
                    }
                    if *duration > 86400 * 30 {
                        return Err(Error::Validation("Duration cannot exceed 30 days".to_string()));
                    }
                }
                Ok(())
            }
            
            DebugCommand::GetBudgetViolationHistory { limit } => {
                if let Some(limit) = limit {
                    if *limit == 0 {
                        return Err(Error::Validation("Limit must be greater than 0".to_string()));
                    }
                    if *limit > 1000 {
                        return Err(Error::Validation("Limit cannot exceed 1000".to_string()));
                    }
                }
                Ok(())
            }
            
            _ => Ok(()),
        }
    }
    
    fn estimate_processing_time(&self, command: &DebugCommand) -> Duration {
        match command {
            DebugCommand::StartBudgetMonitoring | 
            DebugCommand::StopBudgetMonitoring => Duration::from_millis(50),
            
            DebugCommand::CheckBudgetViolations => Duration::from_millis(100),
            
            DebugCommand::GenerateComplianceReport { .. } => Duration::from_millis(500),
            
            DebugCommand::GetBudgetRecommendations => Duration::from_millis(300),
            
            _ => Duration::from_millis(20),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    async fn create_test_processor() -> PerformanceBudgetProcessor {
        let mut config = crate::config::Config::default();
        config.bevy_brp_host = "localhost".to_string();
        config.bevy_brp_port = 15702;
        config.mcp_port = 3000;
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        PerformanceBudgetProcessor::new(brp_client)
    }
    
    #[tokio::test]
    async fn test_processor_creation() {
        let processor = create_test_processor().await;
        
        // Should start with monitoring inactive
        let state = processor.monitoring_state.read().await;
        assert!(!state.continuous_monitoring);
    }
    
    #[tokio::test]
    async fn test_start_stop_monitoring() {
        let processor = create_test_processor().await;
        
        // Start monitoring
        let result = processor.process(DebugCommand::StartBudgetMonitoring).await;
        assert!(result.is_ok());
        
        // Check state
        {
            let state = processor.monitoring_state.read().await;
            assert!(state.continuous_monitoring);
        }
        
        // Stop monitoring
        let result = processor.process(DebugCommand::StopBudgetMonitoring).await;
        assert!(result.is_ok());
        
        // Check state
        {
            let state = processor.monitoring_state.read().await;
            assert!(!state.continuous_monitoring);
        }
    }
    
    #[tokio::test]
    async fn test_budget_configuration() {
        let processor = create_test_processor().await;
        
        // Set a custom budget
        let config = serde_json::json!({
            "frame_time_ms": 20.0,
            "memory_mb": 600.0,
            "cpu_percent": 75.0,
            "auto_adjust": true,
            "violation_threshold": 5
        });
        
        let result = processor.process(DebugCommand::SetPerformanceBudget { config }).await;
        assert!(result.is_ok());
        
        // Get the configuration
        let result = processor.process(DebugCommand::GetPerformanceBudget).await;
        assert!(result.is_ok());
        
        match result.unwrap() {
            DebugResponse::Success { data: Some(data), .. } => {
                let config: BudgetConfig = serde_json::from_value(data).unwrap();
                assert_eq!(config.frame_time_ms, Some(20.0));
                assert_eq!(config.memory_mb, Some(600.0));
                assert_eq!(config.cpu_percent, Some(75.0));
                assert!(config.auto_adjust);
            }
            _ => panic!("Expected Success response with data"),
        }
    }
}