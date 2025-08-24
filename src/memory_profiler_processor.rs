/// Memory Profiler Processor for handling memory profiling debug commands
/// 
/// This processor provides memory profiling capabilities through the debug command system,
/// integrating with the MemoryProfiler to offer leak detection, trend analysis, and allocation tracking.
use crate::brp_messages::{DebugCommand, DebugResponse};
use crate::brp_client::BrpClient;
use crate::debug_command_processor::DebugCommandProcessor;
use crate::memory_profiler::{MemoryProfiler, MemoryProfilerConfig};
use crate::error::{Error, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, info, warn};

/// Memory profiling session information
#[derive(Debug, Clone)]
struct MemoryProfilingSession {
    /// Session ID
    session_id: String,
    /// Target systems to profile (None means all systems)
    target_systems: Option<Vec<String>>,
    /// Whether to capture backtraces
    capture_backtraces: bool,
    /// Session start time
    start_time: Instant,
    /// Session duration limit
    duration: Option<Duration>,
}

/// Memory profiler processor for debug commands
pub struct MemoryProfilerProcessor {
    /// Memory profiler instance
    profiler: Arc<MemoryProfiler>,
    /// Active profiling sessions
    active_sessions: Arc<RwLock<Vec<MemoryProfilingSession>>>,
    /// Automatic snapshot task handle
    snapshot_task_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl MemoryProfilerProcessor {
    /// Create a new memory profiler processor
    pub fn new(_brp_client: Arc<RwLock<BrpClient>>) -> Self {
        let config = MemoryProfilerConfig::default();
        let profiler = Arc::new(MemoryProfiler::new(config));
        
        Self {
            profiler,
            active_sessions: Arc::new(RwLock::new(Vec::new())),
            snapshot_task_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        _brp_client: Arc<RwLock<BrpClient>>, 
        config: MemoryProfilerConfig
    ) -> Self {
        let profiler = Arc::new(MemoryProfiler::new(config));
        
        Self {
            profiler,
            active_sessions: Arc::new(RwLock::new(Vec::new())),
            snapshot_task_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Start memory profiling
    async fn handle_start_profiling(&self, params: Value) -> Result<DebugResponse> {
        debug!("Handling start memory profiling request: {:?}", params);

        let session_id = params.get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        let target_systems = params.get("target_systems")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<String>>()
            });

        let capture_backtraces = params.get("capture_backtraces")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let duration_secs = params.get("duration_seconds")
            .and_then(|v| v.as_u64());

        // Check if session already exists
        {
            let sessions = self.active_sessions.read().await;
            if sessions.iter().any(|s| s.session_id == session_id) {
                return Ok(DebugResponse::Success { 
                    message: format!("Memory profiling session '{}' is already active", session_id),
                    data: None,
                });
            }
        }

        // Create new session
        let session = MemoryProfilingSession {
            session_id: session_id.clone(),
            target_systems: target_systems.clone(),
            capture_backtraces,
            start_time: Instant::now(),
            duration: duration_secs.map(Duration::from_secs),
        };

        // Add session to active sessions
        {
            let mut sessions = self.active_sessions.write().await;
            sessions.push(session);
        }

        // Start automatic snapshot collection if not already running
        self.start_automatic_snapshots().await;

        info!("Started memory profiling session: {}", session_id);

        Ok(DebugResponse::Success {
            message: format!("Memory profiling started for session: {}", session_id),
            data: Some(serde_json::json!({
                "session_id": session_id,
                "target_systems": target_systems,
                "capture_backtraces": capture_backtraces,
                "duration_seconds": duration_secs,
            })),
        })
    }

    /// Stop memory profiling
    async fn handle_stop_profiling(&self, params: Value) -> Result<DebugResponse> {
        debug!("Handling stop memory profiling request: {:?}", params);

        let session_id = params.get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        // Remove session from active sessions
        let removed = {
            let mut sessions = self.active_sessions.write().await;
            let initial_len = sessions.len();
            sessions.retain(|s| s.session_id != session_id);
            initial_len != sessions.len()
        };

        if !removed {
            return Ok(DebugResponse::Success {
                message: format!("No active memory profiling session found: {}", session_id),
                data: None,
            });
        }

        // Stop automatic snapshots if no sessions remain
        {
            let sessions = self.active_sessions.read().await;
            if sessions.is_empty() {
                self.stop_automatic_snapshots().await;
            }
        }

        info!("Stopped memory profiling session: {}", session_id);

        Ok(DebugResponse::Success {
            message: format!("Memory profiling stopped for session: {}", session_id),
            data: Some(serde_json::json!({
                "session_id": session_id,
            })),
        })
    }

    /// Get current memory profile
    async fn handle_get_memory_profile(&self, _params: Value) -> Result<DebugResponse> {
        debug!("Handling get memory profile request");

        let profile = self.profiler.get_memory_profile().await?;

        Ok(DebugResponse::MemoryProfile { 
            total_allocated: profile.total_allocated,
            allocations_per_system: profile.allocations_per_system,
            top_allocations: profile.top_allocations,
        })
    }

    /// Detect memory leaks
    async fn handle_detect_leaks(&self, params: Value) -> Result<DebugResponse> {
        debug!("Handling detect memory leaks request: {:?}", params);

        let target_systems = params.get("target_systems")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<String>>()
            });

        let leaks = self.profiler.detect_leaks().await?;
        
        // Filter by target systems if specified
        let filtered_leaks = if let Some(targets) = target_systems {
            leaks.into_iter()
                .filter(|leak| targets.contains(&leak.system_name))
                .collect()
        } else {
            leaks
        };

        let leak_count = filtered_leaks.len();
        let total_leaked_memory: usize = filtered_leaks.iter()
            .map(|leak| leak.leaked_memory)
            .sum();

        info!("Detected {} memory leaks totaling {} bytes", leak_count, total_leaked_memory);

        Ok(DebugResponse::Success {
            message: format!("Detected {} potential memory leaks", leak_count),
            data: Some(serde_json::json!({
                "leak_count": leak_count,
                "total_leaked_memory": total_leaked_memory,
                "leaks": filtered_leaks,
            })),
        })
    }

    /// Analyze memory usage trends
    async fn handle_analyze_trends(&self, params: Value) -> Result<DebugResponse> {
        debug!("Handling analyze memory trends request: {:?}", params);

        let target_systems = params.get("target_systems")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<String>>()
            });

        let trends = self.profiler.analyze_trends().await?;
        
        // Filter by target systems if specified
        let filtered_trends = if let Some(targets) = target_systems {
            trends.into_iter()
                .filter(|trend| targets.contains(&trend.system_name))
                .collect()
        } else {
            trends
        };

        info!("Analyzed memory trends for {} systems", filtered_trends.len());

        Ok(DebugResponse::Success {
            message: format!("Analyzed memory trends for {} systems", filtered_trends.len()),
            data: Some(serde_json::json!({
                "trend_count": filtered_trends.len(),
                "trends": filtered_trends,
            })),
        })
    }

    /// Take a memory snapshot
    async fn handle_take_snapshot(&self, _params: Value) -> Result<DebugResponse> {
        debug!("Handling take memory snapshot request");

        let snapshot = self.profiler.take_snapshot().await?;

        Ok(DebugResponse::Success {
            message: "Memory snapshot captured successfully".to_string(),
            data: Some(serde_json::json!({
                "snapshot": snapshot,
            })),
        })
    }

    /// Get memory profiler statistics
    async fn handle_get_statistics(&self, _params: Value) -> Result<DebugResponse> {
        debug!("Handling get memory profiler statistics request");

        let mut stats = self.profiler.get_statistics().await;
        
        // Add session information
        let sessions = self.active_sessions.read().await;
        stats.insert("active_sessions".to_string(), 
                    serde_json::Value::Number(sessions.len().into()));
        
        let session_info: Vec<serde_json::Value> = sessions.iter()
            .map(|session| serde_json::json!({
                "session_id": session.session_id,
                "target_systems": session.target_systems,
                "capture_backtraces": session.capture_backtraces,
                "runtime_seconds": session.start_time.elapsed().as_secs(),
            }))
            .collect();
        
        stats.insert("sessions".to_string(), serde_json::Value::Array(session_info));

        Ok(DebugResponse::Success {
            message: "Memory profiler statistics retrieved successfully".to_string(),
            data: Some(serde_json::Value::Object(
                stats.into_iter()
                    .map(|(k, v)| (k, v))
                    .collect()
            )),
        })
    }

    /// Record a memory allocation (called by instrumentation)
    pub async fn record_allocation(
        &self,
        system_name: &str,
        size: usize,
        backtrace: Option<Vec<String>>,
    ) -> Result<u64> {
        // Check if this system is being profiled
        let should_record = {
            let sessions = self.active_sessions.read().await;
            sessions.iter().any(|session| {
                session.target_systems.as_ref()
                    .map(|targets| targets.contains(&system_name.to_string()))
                    .unwrap_or(true) // Record all systems if no filter specified
            })
        };

        if should_record {
            self.profiler.record_allocation(system_name, size, backtrace)
        } else {
            // Return a dummy ID if not recording
            Ok(0)
        }
    }

    /// Record a memory deallocation (called by instrumentation)
    pub async fn record_deallocation(&self, allocation_id: u64) -> Result<()> {
        if allocation_id > 0 {
            self.profiler.record_deallocation(allocation_id)
        } else {
            Ok(())
        }
    }

    /// Update entity count (called by instrumentation)
    pub async fn update_entity_count(&self, count: usize) {
        self.profiler.update_entity_count(count);
    }

    /// Update resource memory footprint (called by instrumentation)
    pub async fn update_resource_memory(&self, resource_name: &str, memory_size: usize) {
        self.profiler.update_resource_memory(resource_name, memory_size);
    }

    /// Start automatic snapshot collection
    async fn start_automatic_snapshots(&self) {
        let mut handle_guard = self.snapshot_task_handle.write().await;
        
        if handle_guard.is_some() {
            return; // Already running
        }

        let profiler = Arc::clone(&self.profiler);
        let sessions = Arc::clone(&self.active_sessions);
        
        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30)); // Take snapshots every 30 seconds
            
            loop {
                interval.tick().await;
                
                // Check if there are still active sessions
                {
                    let sessions_guard = sessions.read().await;
                    if sessions_guard.is_empty() {
                        break;
                    }
                    
                    // Clean up expired sessions
                    let mut expired_sessions = Vec::new();
                    for (index, session) in sessions_guard.iter().enumerate() {
                        if let Some(duration) = session.duration {
                            if session.start_time.elapsed() >= duration {
                                expired_sessions.push(index);
                            }
                        }
                    }
                    
                    drop(sessions_guard);
                    
                    // Remove expired sessions
                    if !expired_sessions.is_empty() {
                        let mut sessions_write = sessions.write().await;
                        for &index in expired_sessions.iter().rev() {
                            if index < sessions_write.len() {
                                let session = sessions_write.remove(index);
                                info!("Memory profiling session expired: {}", session.session_id);
                            }
                        }
                    }
                }
                
                // Take snapshot
                if let Err(e) = profiler.take_snapshot().await {
                    warn!("Failed to take automatic memory snapshot: {}", e);
                }
                
                // Check profiler overhead
                if !profiler.is_overhead_acceptable() {
                    warn!("Memory profiler overhead is too high: {:.2}%", 
                          profiler.get_overhead_percentage());
                }
            }
            
            info!("Automatic memory snapshot collection stopped");
        });
        
        *handle_guard = Some(handle);
        info!("Automatic memory snapshot collection started");
    }

    /// Stop automatic snapshot collection
    async fn stop_automatic_snapshots(&self) {
        let mut handle_guard = self.snapshot_task_handle.write().await;
        
        if let Some(handle) = handle_guard.take() {
            handle.abort();
            info!("Automatic memory snapshot collection stopped");
        }
    }

    /// Get profiler overhead percentage
    pub fn get_overhead_percentage(&self) -> f32 {
        self.profiler.get_overhead_percentage()
    }

    /// Check if profiler overhead is acceptable
    pub fn is_overhead_acceptable(&self) -> bool {
        self.profiler.is_overhead_acceptable()
    }
}

#[async_trait]
impl DebugCommandProcessor for MemoryProfilerProcessor {
    async fn process(&self, command: DebugCommand) -> Result<DebugResponse> {
        match command {
            DebugCommand::ProfileMemory { 
                target_systems, 
                capture_backtraces,
                duration_seconds,
            } => {
                let params = serde_json::json!({
                    "session_id": "default",
                    "target_systems": target_systems,
                    "capture_backtraces": capture_backtraces,
                    "duration_seconds": duration_seconds,
                });
                self.handle_start_profiling(params).await
            }
            DebugCommand::StopMemoryProfiling { session_id } => {
                let params = serde_json::json!({
                    "session_id": session_id.unwrap_or_else(|| "default".to_string()),
                });
                self.handle_stop_profiling(params).await
            }
            DebugCommand::GetMemoryProfile => {
                self.handle_get_memory_profile(serde_json::Value::Object(Default::default())).await
            }
            DebugCommand::DetectMemoryLeaks { target_systems } => {
                let params = serde_json::json!({
                    "target_systems": target_systems,
                });
                self.handle_detect_leaks(params).await
            }
            DebugCommand::AnalyzeMemoryTrends { target_systems } => {
                let params = serde_json::json!({
                    "target_systems": target_systems,
                });
                self.handle_analyze_trends(params).await
            }
            DebugCommand::TakeMemorySnapshot => {
                self.handle_take_snapshot(serde_json::Value::Object(Default::default())).await
            }
            DebugCommand::GetMemoryStatistics => {
                self.handle_get_statistics(serde_json::Value::Object(Default::default())).await
            }
            _ => Err(Error::DebugError(
                "Unsupported command for memory profiler processor".to_string(),
            )),
        }
    }

    async fn validate(&self, command: &DebugCommand) -> Result<()> {
        match command {
            DebugCommand::ProfileMemory { target_systems, duration_seconds, .. } => {
                if let Some(systems) = target_systems {
                    if systems.len() > 100 {
                        return Err(Error::Validation(
                            "Too many target systems specified (max 100)".to_string()
                        ));
                    }
                }
                
                if let Some(duration) = duration_seconds {
                    if *duration > 86400 { // 24 hours max
                        return Err(Error::Validation(
                            "Duration too long (max 24 hours)".to_string()
                        ));
                    }
                }
                
                Ok(())
            }
            DebugCommand::StopMemoryProfiling { .. } |
            DebugCommand::GetMemoryProfile |
            DebugCommand::DetectMemoryLeaks { .. } |
            DebugCommand::AnalyzeMemoryTrends { .. } |
            DebugCommand::TakeMemorySnapshot |
            DebugCommand::GetMemoryStatistics => Ok(()),
            _ => Err(Error::DebugError("Command not supported by memory profiler processor".to_string())),
        }
    }

    fn estimate_processing_time(&self, command: &DebugCommand) -> Duration {
        match command {
            DebugCommand::ProfileMemory { .. } => Duration::from_millis(50),
            DebugCommand::StopMemoryProfiling { .. } => Duration::from_millis(20),
            DebugCommand::GetMemoryProfile => Duration::from_millis(100),
            DebugCommand::DetectMemoryLeaks { .. } => Duration::from_millis(500),
            DebugCommand::AnalyzeMemoryTrends { .. } => Duration::from_millis(300),
            DebugCommand::TakeMemorySnapshot => Duration::from_millis(150),
            DebugCommand::GetMemoryStatistics => Duration::from_millis(30),
            _ => Duration::from_millis(1),
        }
    }

    fn supports_command(&self, command: &DebugCommand) -> bool {
        matches!(
            command,
            DebugCommand::ProfileMemory { .. } |
            DebugCommand::StopMemoryProfiling { .. } |
            DebugCommand::GetMemoryProfile |
            DebugCommand::DetectMemoryLeaks { .. } |
            DebugCommand::AnalyzeMemoryTrends { .. } |
            DebugCommand::TakeMemorySnapshot |
            DebugCommand::GetMemoryStatistics
        )
    }
}

impl Drop for MemoryProfilerProcessor {
    fn drop(&mut self) {
        // Clean up any running tasks
        if let Ok(mut handle_guard) = self.snapshot_task_handle.try_write() {
            if let Some(handle) = handle_guard.take() {
                handle.abort();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use serde_json::json;

    async fn create_test_processor() -> MemoryProfilerProcessor {
        let mut config = Config::default();
        config.bevy_brp_host = "localhost".to_string();
        config.bevy_brp_port = 15702;
        config.mcp_port = 3000;
        let brp_client = Arc::new(RwLock::new(crate::brp_client::BrpClient::new(&config)));
        MemoryProfilerProcessor::new(brp_client)
    }

    #[tokio::test]
    async fn test_supports_memory_commands() {
        let processor = create_test_processor().await;
        
        let profile_cmd = DebugCommand::ProfileMemory { 
            target_systems: Some(vec!["TestSystem".to_string()]),
            capture_backtraces: Some(true),
            duration_seconds: Some(300),
        };
        let stop_cmd = DebugCommand::StopMemoryProfiling { 
            session_id: Some("test".to_string())
        };
        let get_profile_cmd = DebugCommand::GetMemoryProfile;
        
        assert!(processor.supports_command(&profile_cmd));
        assert!(processor.supports_command(&stop_cmd));
        assert!(processor.supports_command(&get_profile_cmd));
    }

    #[tokio::test]
    async fn test_start_stop_profiling() {
        let processor = create_test_processor().await;
        
        let start_params = json!({
            "session_id": "test_session",
            "target_systems": ["TestSystem"],
            "capture_backtraces": true
        });
        
        let result = processor.handle_start_profiling(start_params).await;
        assert!(result.is_ok());
        
        // Check that session was created
        let sessions = processor.active_sessions.read().await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "test_session");
        drop(sessions);
        
        let stop_params = json!({
            "session_id": "test_session"
        });
        
        let result = processor.handle_stop_profiling(stop_params).await;
        assert!(result.is_ok());
        
        // Check that session was removed
        let sessions = processor.active_sessions.read().await;
        assert_eq!(sessions.len(), 0);
    }

    #[tokio::test]
    async fn test_allocation_recording() {
        let processor = create_test_processor().await;
        
        // Start profiling session first
        let start_params = json!({
            "session_id": "test",
            "target_systems": ["TestSystem"]
        });
        processor.handle_start_profiling(start_params).await.unwrap();
        
        // Record allocations
        let allocation_id = processor.record_allocation(
            "TestSystem", 
            1024, 
            Some(vec!["test_function".to_string()])
        ).await.unwrap();
        
        assert!(allocation_id > 0);
        
        // Check memory profile
        let profile_result = processor.handle_get_memory_profile(json!({})).await.unwrap();
        match profile_result {
            DebugResponse::MemoryProfile { total_allocated, allocations_per_system, .. } => {
                assert_eq!(total_allocated, 1024);
                assert!(allocations_per_system.contains_key("TestSystem"));
            }
            _ => panic!("Expected MemoryProfile response"),
        }
    }

    #[tokio::test]
    async fn test_memory_snapshot() {
        let processor = create_test_processor().await;
        
        // Start profiling and record some allocations
        processor.handle_start_profiling(json!({"session_id": "test"})).await.unwrap();
        processor.record_allocation("TestSystem", 2048, None).await.unwrap();
        processor.update_entity_count(50).await;
        
        let result = processor.handle_take_snapshot(json!({})).await;
        assert!(result.is_ok());
        
        match result.unwrap() {
            DebugResponse::Success { data: Some(data), .. } => {
                let snapshot = data.get("snapshot").unwrap();
                assert!(snapshot.get("total_allocated").unwrap().as_u64().unwrap() > 0);
                assert_eq!(snapshot.get("entity_count").unwrap().as_u64().unwrap(), 50);
            }
            _ => panic!("Expected Success response with snapshot data"),
        }
    }

    #[tokio::test]
    async fn test_leak_detection() {
        let processor = create_test_processor().await;
        
        // Start profiling
        processor.handle_start_profiling(json!({"session_id": "test"})).await.unwrap();
        
        let result = processor.handle_detect_leaks(json!({})).await;
        assert!(result.is_ok());
        
        match result.unwrap() {
            DebugResponse::Success { data: Some(data), .. } => {
                assert!(data.get("leak_count").is_some());
                assert!(data.get("leaks").is_some());
            }
            _ => panic!("Expected Success response with leak data"),
        }
    }

    #[tokio::test]
    async fn test_statistics() {
        let processor = create_test_processor().await;
        
        let result = processor.handle_get_statistics(json!({})).await;
        assert!(result.is_ok());
        
        match result.unwrap() {
            DebugResponse::Success { data: Some(data), .. } => {
                assert!(data.get("active_sessions").is_some());
                assert!(data.get("total_allocations_tracked").is_some());
            }
            _ => panic!("Expected Success response with statistics"),
        }
    }

    #[tokio::test]
    async fn test_validation() {
        let processor = create_test_processor().await;
        
        let valid_cmd = DebugCommand::ProfileMemory { 
            target_systems: Some(vec!["Test".to_string()]),
            capture_backtraces: Some(true),
            duration_seconds: Some(300),
        };
        
        let invalid_cmd = DebugCommand::ProfileMemory { 
            target_systems: Some((0..150).map(|i| format!("System{}", i)).collect()),
            capture_backtraces: Some(true),
            duration_seconds: Some(100000), // Too long
        };
        
        assert!(processor.validate(&valid_cmd).await.is_ok());
        assert!(processor.validate(&invalid_cmd).await.is_err());
    }
}