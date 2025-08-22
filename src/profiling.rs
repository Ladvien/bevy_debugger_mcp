use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Performance measurement point
#[derive(Debug, Clone)]
pub struct PerfMeasurement {
    pub operation: String,
    pub duration: Duration,
    pub timestamp: Instant,
    pub memory_delta: i64, // Memory change in bytes
    pub thread_id: std::thread::ThreadId,
}

/// Performance statistics for an operation
#[derive(Debug, Clone)]
pub struct PerfStats {
    pub operation: String,
    pub call_count: u64,
    pub total_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
    pub avg_duration: Duration,
    pub p95_duration: Duration,
    pub p99_duration: Duration,
    pub memory_allocations: i64,
    pub last_updated: Instant,
}

impl serde::Serialize for PerfStats {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        
        let mut state = serializer.serialize_struct("PerfStats", 9)?;
        state.serialize_field("operation", &self.operation)?;
        state.serialize_field("call_count", &self.call_count)?;
        state.serialize_field("total_duration_ms", &self.total_duration.as_millis())?;
        state.serialize_field("min_duration_ms", &self.min_duration.as_millis())?;
        state.serialize_field("max_duration_ms", &self.max_duration.as_millis())?;
        state.serialize_field("avg_duration_ms", &self.avg_duration.as_millis())?;
        state.serialize_field("p95_duration_ms", &self.p95_duration.as_millis())?;
        state.serialize_field("p99_duration_ms", &self.p99_duration.as_millis())?;
        state.serialize_field("memory_allocations", &self.memory_allocations)?;
        state.end()
    }
}

/// Hot path profiler for identifying performance bottlenecks
pub struct HotPathProfiler {
    measurements: Arc<RwLock<HashMap<String, Vec<PerfMeasurement>>>>,
    stats: Arc<RwLock<HashMap<String, PerfStats>>>,
    call_counts: Arc<RwLock<HashMap<String, AtomicU64>>>,
    enabled: Arc<std::sync::atomic::AtomicBool>,
    max_measurements_per_operation: usize,
}

impl HotPathProfiler {
    /// Create a new hot path profiler
    pub fn new(max_measurements_per_operation: usize) -> Self {
        Self {
            measurements: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(HashMap::new())),
            call_counts: Arc::new(RwLock::new(HashMap::new())),
            enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            max_measurements_per_operation,
        }
    }
    
    /// Enable or disable profiling
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
        if enabled {
            info!("Hot path profiling enabled");
        } else {
            info!("Hot path profiling disabled");
        }
    }
    
    /// Check if profiling is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
    
    /// Record a performance measurement
    pub async fn record(&self, measurement: PerfMeasurement) {
        if !self.is_enabled() {
            return;
        }
        
        let operation = measurement.operation.clone();
        
        // Update call count
        {
            let mut call_counts = self.call_counts.write().await;
            call_counts.entry(operation.clone())
                .or_insert_with(|| AtomicU64::new(0))
                .fetch_add(1, Ordering::Relaxed);
        }
        
        // Store measurement
        {
            let mut measurements = self.measurements.write().await;
            let op_measurements = measurements.entry(operation.clone()).or_insert_with(Vec::new);
            
            op_measurements.push(measurement.clone());
            
            // Keep only the most recent measurements
            if op_measurements.len() > self.max_measurements_per_operation {
                op_measurements.drain(0..op_measurements.len() - self.max_measurements_per_operation);
            }
        }
        
        // Update statistics
        self.update_stats(&operation).await;
        
        // Log slow operations
        if measurement.duration > Duration::from_millis(100) {
            warn!("Slow operation detected: {} took {:?}", operation, measurement.duration);
        }
    }
    
    /// Update statistics for an operation
    async fn update_stats(&self, operation: &str) {
        let measurements = {
            let measurements_guard = self.measurements.read().await;
            measurements_guard.get(operation).cloned().unwrap_or_default()
        };
        
        if measurements.is_empty() {
            return;
        }
        
        let call_count = {
            let call_counts = self.call_counts.read().await;
            call_counts.get(operation)
                .map(|c| c.load(Ordering::Relaxed))
                .unwrap_or(0)
        };
        
        let durations: Vec<Duration> = measurements.iter().map(|m| m.duration).collect();
        let total_duration: Duration = durations.iter().sum();
        let min_duration = durations.iter().min().copied().unwrap_or(Duration::ZERO);
        let max_duration = durations.iter().max().copied().unwrap_or(Duration::ZERO);
        let avg_duration = if durations.is_empty() {
            Duration::ZERO
        } else {
            total_duration / durations.len() as u32
        };
        
        // Calculate percentiles
        let mut sorted_durations = durations.clone();
        sorted_durations.sort();
        
        let p95_duration = if sorted_durations.is_empty() {
            Duration::ZERO
        } else {
            let index = ((sorted_durations.len() as f64 * 0.95) as usize).min(sorted_durations.len() - 1);
            sorted_durations[index]
        };
        
        let p99_duration = if sorted_durations.is_empty() {
            Duration::ZERO
        } else {
            let index = ((sorted_durations.len() as f64 * 0.99) as usize).min(sorted_durations.len() - 1);
            sorted_durations[index]
        };
        
        let memory_allocations: i64 = measurements.iter().map(|m| m.memory_delta).sum();
        
        let stats = PerfStats {
            operation: operation.to_string(),
            call_count,
            total_duration,
            min_duration,
            max_duration,
            avg_duration,
            p95_duration,
            p99_duration,
            memory_allocations,
            last_updated: Instant::now(),
        };
        
        let mut stats_guard = self.stats.write().await;
        stats_guard.insert(operation.to_string(), stats);
    }
    
    /// Get statistics for all operations
    pub async fn get_all_stats(&self) -> HashMap<String, PerfStats> {
        self.stats.read().await.clone()
    }
    
    /// Get statistics for a specific operation
    pub async fn get_stats(&self, operation: &str) -> Option<PerfStats> {
        self.stats.read().await.get(operation).cloned()
    }
    
    /// Get hot path operations (top N by call count)
    pub async fn get_hot_paths(&self, limit: usize) -> Vec<PerfStats> {
        let stats = self.stats.read().await;
        let mut operations: Vec<PerfStats> = stats.values().cloned().collect();
        
        // Sort by call count descending
        operations.sort_by(|a, b| b.call_count.cmp(&a.call_count));
        
        operations.into_iter().take(limit).collect()
    }
    
    /// Get slow operations (operations exceeding duration threshold)
    pub async fn get_slow_operations(&self, threshold: Duration) -> Vec<PerfStats> {
        let stats = self.stats.read().await;
        stats.values()
            .filter(|stat| stat.avg_duration > threshold)
            .cloned()
            .collect()
    }
    
    /// Clear all measurements and statistics
    pub async fn clear(&self) {
        let mut measurements = self.measurements.write().await;
        let mut stats = self.stats.write().await;
        let mut call_counts = self.call_counts.write().await;
        
        measurements.clear();
        stats.clear();
        call_counts.clear();
        
        info!("Hot path profiler data cleared");
    }
    
    /// Generate a performance report
    pub async fn generate_report(&self) -> String {
        let stats = self.get_all_stats().await;
        let hot_paths = self.get_hot_paths(10).await;
        let slow_operations = self.get_slow_operations(Duration::from_millis(10)).await;
        
        let mut report = String::new();
        report.push_str("=== Hot Path Performance Report ===\n\n");
        
        report.push_str(&format!("Total Operations Tracked: {}\n", stats.len()));
        report.push_str(&format!("Profiling Enabled: {}\n\n", self.is_enabled()));
        
        // Hot paths section
        report.push_str("Top 10 Hot Paths (by call count):\n");
        for (i, stat) in hot_paths.iter().enumerate() {
            report.push_str(&format!(
                "{}. {} - {} calls, avg: {:?}, p95: {:?}, p99: {:?}\n",
                i + 1, stat.operation, stat.call_count, stat.avg_duration, stat.p95_duration, stat.p99_duration
            ));
        }
        
        // Slow operations section
        if !slow_operations.is_empty() {
            report.push_str("\nSlow Operations (> 10ms avg):\n");
            for stat in &slow_operations {
                report.push_str(&format!(
                    "- {} - avg: {:?}, max: {:?}, calls: {}\n",
                    stat.operation, stat.avg_duration, stat.max_duration, stat.call_count
                ));
            }
        }
        
        // Memory allocation section
        let total_allocations: i64 = stats.values().map(|s| s.memory_allocations).sum();
        if total_allocations != 0 {
            report.push_str(&format!("\nTotal Memory Delta: {} bytes\n", total_allocations));
            
            let high_allocation_ops: Vec<_> = stats.values()
                .filter(|s| s.memory_allocations.abs() > 1024 * 1024) // > 1MB
                .collect();
            
            if !high_allocation_ops.is_empty() {
                report.push_str("\nHigh Memory Allocation Operations (> 1MB):\n");
                for stat in high_allocation_ops {
                    report.push_str(&format!(
                        "- {} - {} bytes (calls: {})\n",
                        stat.operation, stat.memory_allocations, stat.call_count
                    ));
                }
            }
        }
        
        report
    }
}

/// Global profiler instance
static mut GLOBAL_PROFILER: Option<HotPathProfiler> = None;
static INIT: std::sync::Once = std::sync::Once::new();

/// Initialize the global profiler
pub fn init_profiler() -> &'static HotPathProfiler {
    unsafe {
        INIT.call_once(|| {
            GLOBAL_PROFILER = Some(HotPathProfiler::new(1000)); // Keep 1000 measurements per operation
        });
        GLOBAL_PROFILER.as_ref().unwrap()
    }
}

/// Get the global profiler instance
pub fn get_profiler() -> &'static HotPathProfiler {
    unsafe {
        GLOBAL_PROFILER.as_ref().unwrap_or_else(|| init_profiler())
    }
}

/// Macro for easy profiling of code blocks
#[macro_export]
macro_rules! profile_block {
    ($operation:expr, $block:block) => {{
        let _profiler = $crate::profiling::get_profiler();
        let _enabled = _profiler.is_enabled();
        
        let (_result, _duration) = if _enabled {
            let _start = std::time::Instant::now();
            let _result = $block;
            let _duration = _start.elapsed();
            (_result, _duration)
        } else {
            let _result = $block;
            (_result, std::time::Duration::ZERO)
        };
        
        if _enabled {
            let _measurement = $crate::profiling::PerfMeasurement {
                operation: $operation.to_string(),
                duration: _duration,
                timestamp: std::time::Instant::now(),
                memory_delta: 0, // TODO: Implement memory tracking
                thread_id: std::thread::current().id(),
            };
            
            // Record in background to avoid blocking
            let _profiler_clone = _profiler;
            tokio::spawn(async move {
                _profiler_clone.record(_measurement).await;
            });
        }
        
        _result
    }};
}

/// Async version of profile_block macro
#[macro_export]
macro_rules! profile_async_block {
    ($operation:expr, $block:expr) => {{
        let _profiler = $crate::profiling::get_profiler();
        let _enabled = _profiler.is_enabled();
        
        if _enabled {
            let _start = std::time::Instant::now();
            let _result = $block.await;
            let _duration = _start.elapsed();
            
            let _measurement = $crate::profiling::PerfMeasurement {
                operation: $operation.to_string(),
                duration: _duration,
                timestamp: std::time::Instant::now(),
                memory_delta: 0, // TODO: Implement memory tracking
                thread_id: std::thread::current().id(),
            };
            
            _profiler.record(_measurement).await;
            _result
        } else {
            $block.await
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;
    
    #[tokio::test]
    async fn test_profiler_basic_operations() {
        let profiler = HotPathProfiler::new(100);
        
        let measurement = PerfMeasurement {
            operation: "test_operation".to_string(),
            duration: Duration::from_millis(50),
            timestamp: Instant::now(),
            memory_delta: 1024,
            thread_id: std::thread::current().id(),
        };
        
        profiler.record(measurement).await;
        
        let stats = profiler.get_stats("test_operation").await;
        assert!(stats.is_some());
        
        let stats = stats.unwrap();
        assert_eq!(stats.call_count, 1);
        assert_eq!(stats.total_duration, Duration::from_millis(50));
    }
    
    #[tokio::test]
    async fn test_hot_paths_identification() {
        let profiler = HotPathProfiler::new(100);
        
        // Record multiple operations with different call counts
        for i in 0..10 {
            let measurement = PerfMeasurement {
                operation: "frequent_op".to_string(),
                duration: Duration::from_millis(10),
                timestamp: Instant::now(),
                memory_delta: 0,
                thread_id: std::thread::current().id(),
            };
            profiler.record(measurement).await;
        }
        
        for i in 0..5 {
            let measurement = PerfMeasurement {
                operation: "less_frequent_op".to_string(),
                duration: Duration::from_millis(20),
                timestamp: Instant::now(),
                memory_delta: 0,
                thread_id: std::thread::current().id(),
            };
            profiler.record(measurement).await;
        }
        
        let hot_paths = profiler.get_hot_paths(2).await;
        assert_eq!(hot_paths.len(), 2);
        assert_eq!(hot_paths[0].operation, "frequent_op");
        assert_eq!(hot_paths[0].call_count, 10);
    }
    
    #[tokio::test]
    async fn test_slow_operations_detection() {
        let profiler = HotPathProfiler::new(100);
        
        let slow_measurement = PerfMeasurement {
            operation: "slow_operation".to_string(),
            duration: Duration::from_millis(100),
            timestamp: Instant::now(),
            memory_delta: 0,
            thread_id: std::thread::current().id(),
        };
        profiler.record(slow_measurement).await;
        
        let fast_measurement = PerfMeasurement {
            operation: "fast_operation".to_string(),
            duration: Duration::from_millis(5),
            timestamp: Instant::now(),
            memory_delta: 0,
            thread_id: std::thread::current().id(),
        };
        profiler.record(fast_measurement).await;
        
        let slow_ops = profiler.get_slow_operations(Duration::from_millis(50)).await;
        assert_eq!(slow_ops.len(), 1);
        assert_eq!(slow_ops[0].operation, "slow_operation");
    }
    
    #[test]
    fn test_profile_block_macro() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let _profiler = init_profiler();
            
            let result = profile_block!("test_macro", {
                std::thread::sleep(std::time::Duration::from_millis(10));
                42
            });
            
            assert_eq!(result, 42);
            
            // Give some time for async recording
            sleep(Duration::from_millis(100)).await;
            
            let stats = get_profiler().get_stats("test_macro").await;
            assert!(stats.is_some());
        });
    }
}