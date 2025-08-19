/// System Profiler Processor for debug command integration
use crate::brp_messages::{DebugCommand, DebugResponse};
use crate::debug_command_processor::DebugCommandProcessor;
use crate::error::{Error, Result};
use crate::system_profiler::{SystemProfiler, ExportFormat};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

/// System profiler processor for debug commands
pub struct SystemProfilerProcessor {
    /// System profiler instance
    profiler: Arc<SystemProfiler>,
}

impl SystemProfilerProcessor {
    /// Create new system profiler processor
    pub fn new(profiler: Arc<SystemProfiler>) -> Self {
        Self { profiler }
    }
}

#[async_trait]
impl DebugCommandProcessor for SystemProfilerProcessor {
    async fn process(&self, command: DebugCommand) -> Result<DebugResponse> {
        match command {
            DebugCommand::ProfileSystem {
                system_name,
                duration_ms,
                track_allocations,
            } => {
                info!("Processing ProfileSystem command for: {}", system_name);
                
                // Start profiling
                self.profiler
                    .start_profiling(system_name.clone(), duration_ms, track_allocations)
                    .await?;
                
                // If duration specified, wait and collect results
                if let Some(duration) = duration_ms {
                    // For short durations, wait synchronously
                    if duration <= 5000 {
                        tokio::time::sleep(Duration::from_millis(duration)).await;
                        let profile = self.profiler.stop_profiling(&system_name).await?;
                        return Ok(DebugResponse::SystemProfile(profile));
                    }
                }
                
                // For long durations or no duration, return started response
                Ok(DebugResponse::ProfilingStarted {
                    system_name,
                    duration_ms,
                })
            }
            _ => Err(Error::DebugError("Unsupported command for SystemProfilerProcessor".to_string())),
        }
    }
    
    async fn validate(&self, command: &DebugCommand) -> Result<()> {
        match command {
            DebugCommand::ProfileSystem { system_name, duration_ms, .. } => {
                // Validate system name
                if system_name.is_empty() {
                    return Err(Error::DebugError("System name cannot be empty".to_string()));
                }
                
                if system_name.len() > 256 {
                    return Err(Error::DebugError("System name too long (max 256 chars)".to_string()));
                }
                
                // Validate duration
                if let Some(duration) = duration_ms {
                    if *duration == 0 {
                        return Err(Error::DebugError("Duration must be greater than 0".to_string()));
                    }
                    if *duration > 300_000 {  // 5 minutes max
                        return Err(Error::DebugError("Duration too long (max 5 minutes)".to_string()));
                    }
                }
                
                Ok(())
            }
            _ => Ok(()),
        }
    }
    
    fn estimate_processing_time(&self, command: &DebugCommand) -> Duration {
        match command {
            DebugCommand::ProfileSystem { duration_ms, .. } => {
                // Processing time is roughly the profiling duration
                Duration::from_millis(duration_ms.unwrap_or(5000))
            }
            _ => Duration::from_millis(10),
        }
    }
    
    fn supports_command(&self, command: &DebugCommand) -> bool {
        matches!(command, DebugCommand::ProfileSystem { .. })
    }
}

/// Extended profiler commands processor for additional functionality
pub struct ExtendedProfilerProcessor {
    /// System profiler instance
    profiler: Arc<SystemProfiler>,
}

impl ExtendedProfilerProcessor {
    /// Create new extended profiler processor
    pub fn new(profiler: Arc<SystemProfiler>) -> Self {
        Self { profiler }
    }
    
    /// Stop profiling for a system
    pub async fn stop_profiling(&self, system_name: &str) -> Result<DebugResponse> {
        let profile = self.profiler.stop_profiling(system_name).await?;
        Ok(DebugResponse::SystemProfile(profile))
    }
    
    /// Get profiling history
    pub async fn get_history(&self, system_name: &str) -> Result<DebugResponse> {
        let samples = self.profiler.get_system_history(system_name).await;
        
        // Convert to response format
        let frame_count = samples.len();
        Ok(DebugResponse::ProfileHistory {
            system_name: system_name.to_string(),
            samples,
            frame_count,
        })
    }
    
    /// Get detected anomalies
    pub async fn get_anomalies(&self) -> Result<DebugResponse> {
        let anomalies = self.profiler.get_anomalies().await;
        
        Ok(DebugResponse::PerformanceAnomalies {
            count: anomalies.len(),
            anomalies: anomalies.into_iter().map(|a| {
                serde_json::json!({
                    "frame": a.frame_number,
                    "system": a.system_name,
                    "execution_ms": a.execution_time_ms,
                    "expected_ms": a.expected_time_ms,
                    "severity": a.severity,
                    "detected_at_us": a.detected_at_us,
                })
            }).collect(),
        })
    }
    
    /// Export profiling data
    pub async fn export_data(&self, format: ExportFormat) -> Result<String> {
        self.profiler.export_profiling_data(format).await
    }
    
    /// Clear all profiling data
    pub async fn clear_data(&self) -> Result<()> {
        self.profiler.clear_profiling_data().await;
        Ok(())
    }
    
    /// Update system dependencies
    pub async fn update_dependencies(
        &self,
        system_name: String,
        dependencies: Vec<String>,
    ) -> Result<()> {
        self.profiler.update_dependency_graph(system_name, dependencies).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brp_client::BrpClient;
    use crate::config::Config;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_profiler_processor_creation() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        let profiler = Arc::new(SystemProfiler::new(brp_client));
        let processor = SystemProfilerProcessor::new(profiler);
        
        let command = DebugCommand::ProfileSystem {
            system_name: "test".to_string(),
            duration_ms: Some(1000),
            track_allocations: Some(false),
        };
        
        assert!(processor.supports_command(&command));
    }
    
    #[tokio::test]
    async fn test_command_validation() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        let profiler = Arc::new(SystemProfiler::new(brp_client));
        let processor = SystemProfilerProcessor::new(profiler);
        
        // Valid command
        let valid_command = DebugCommand::ProfileSystem {
            system_name: "test_system".to_string(),
            duration_ms: Some(1000),
            track_allocations: Some(false),
        };
        assert!(processor.validate(&valid_command).await.is_ok());
        
        // Invalid: empty name
        let invalid_command = DebugCommand::ProfileSystem {
            system_name: "".to_string(),
            duration_ms: Some(1000),
            track_allocations: Some(false),
        };
        assert!(processor.validate(&invalid_command).await.is_err());
        
        // Invalid: duration too long
        let invalid_command = DebugCommand::ProfileSystem {
            system_name: "test".to_string(),
            duration_ms: Some(400_000),
            track_allocations: Some(false),
        };
        assert!(processor.validate(&invalid_command).await.is_err());
    }
    
    #[test]
    fn test_processing_time_estimation() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        let profiler = Arc::new(SystemProfiler::new(brp_client));
        let processor = SystemProfilerProcessor::new(profiler);
        
        let command = DebugCommand::ProfileSystem {
            system_name: "test".to_string(),
            duration_ms: Some(3000),
            track_allocations: Some(false),
        };
        
        let estimated_time = processor.estimate_processing_time(&command);
        assert_eq!(estimated_time.as_millis(), 3000);
    }
}