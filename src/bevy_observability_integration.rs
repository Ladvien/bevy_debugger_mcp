/*
 * Bevy-Specific Observability Integration
 * 
 * This module provides integration points for observability systems
 * to capture Bevy-specific performance and behavioral metrics.
 */

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Bevy-specific metrics that should be captured by observability systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BevyMetrics {
    // BRP Connection Health
    pub brp_connection_status: ConnectionStatus,
    pub brp_connection_uptime: Duration,
    pub brp_reconnection_count: u64,
    pub brp_request_latency_ms: f64,
    pub brp_failed_requests: u64,
    
    // ECS Performance Metrics  
    pub entity_count: u64,
    pub component_count: HashMap<String, u64>,
    pub system_execution_times: HashMap<String, Duration>,
    pub world_memory_usage_bytes: u64,
    
    // Game Performance
    pub frame_time_ms: f64,
    pub fps: f64,
    pub memory_pressure_level: MemoryPressure,
    
    // Debugging Activity
    pub active_debug_sessions: u64,
    pub queries_executed_per_second: f64,
    pub debug_overhead_percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Reconnecting,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryPressure {
    Low,
    Medium,
    High,
    Critical,
}

/// Trait for observability systems to implement Bevy-specific metric collection
pub trait BevyObservabilityCollector {
    /// Collect current Bevy metrics snapshot
    fn collect_metrics(&self) -> Result<BevyMetrics>;
    
    /// Record a BRP operation for latency tracking
    fn record_brp_operation(&self, operation: &str, duration: Duration, success: bool);
    
    /// Record ECS system execution
    fn record_system_execution(&self, system_name: &str, duration: Duration);
    
    /// Record debug query execution
    fn record_debug_query(&self, query_type: &str, entity_count: u64, duration: Duration);
    
    /// Record memory usage change
    fn record_memory_usage(&self, component_type: &str, bytes: i64);
    
    /// Get health status for monitoring systems
    fn get_health_status(&self) -> BevyHealthStatus;
}

/// Health status for monitoring and alerting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BevyHealthStatus {
    pub overall_status: HealthLevel,
    pub brp_connection_healthy: bool,
    pub memory_usage_healthy: bool,
    pub performance_healthy: bool,
    pub issues: Vec<String>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthLevel {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

/// Default implementation that provides basic metric collection
pub struct DefaultBevyObservability {
    start_time: Instant,
    metrics: BevyMetrics,
    operation_history: Vec<OperationRecord>,
}

#[derive(Debug, Clone)]
struct OperationRecord {
    operation: String,
    duration: Duration,
    success: bool,
    timestamp: Instant,
}

impl DefaultBevyObservability {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            metrics: BevyMetrics::default(),
            operation_history: Vec::new(),
        }
    }
    
    /// Calculate connection uptime
    fn calculate_uptime(&self) -> Duration {
        self.start_time.elapsed()
    }
    
    /// Calculate average request latency from recent operations
    fn calculate_average_latency(&self) -> f64 {
        let recent_ops: Vec<_> = self.operation_history
            .iter()
            .filter(|op| op.timestamp.elapsed() < Duration::from_secs(60))
            .collect();
            
        if recent_ops.is_empty() {
            return 0.0;
        }
        
        let total_duration: Duration = recent_ops
            .iter()
            .map(|op| op.duration)
            .sum();
            
        total_duration.as_millis() as f64 / recent_ops.len() as f64
    }
}

impl Default for BevyMetrics {
    fn default() -> Self {
        Self {
            brp_connection_status: ConnectionStatus::Disconnected,
            brp_connection_uptime: Duration::from_secs(0),
            brp_reconnection_count: 0,
            brp_request_latency_ms: 0.0,
            brp_failed_requests: 0,
            entity_count: 0,
            component_count: HashMap::new(),
            system_execution_times: HashMap::new(),
            world_memory_usage_bytes: 0,
            frame_time_ms: 0.0,
            fps: 0.0,
            memory_pressure_level: MemoryPressure::Low,
            active_debug_sessions: 0,
            queries_executed_per_second: 0.0,
            debug_overhead_percentage: 0.0,
        }
    }
}

impl BevyObservabilityCollector for DefaultBevyObservability {
    fn collect_metrics(&self) -> Result<BevyMetrics> {
        let mut metrics = self.metrics.clone();
        metrics.brp_connection_uptime = self.calculate_uptime();
        metrics.brp_request_latency_ms = self.calculate_average_latency();
        Ok(metrics)
    }
    
    fn record_brp_operation(&self, operation: &str, duration: Duration, success: bool) {
        // In a real implementation, this would be thread-safe
        println!("BRP Operation: {} took {:?}, success: {}", operation, duration, success);
    }
    
    fn record_system_execution(&self, system_name: &str, duration: Duration) {
        println!("System '{}' executed in {:?}", system_name, duration);
    }
    
    fn record_debug_query(&self, query_type: &str, entity_count: u64, duration: Duration) {
        println!("Debug query '{}' processed {} entities in {:?}", 
                query_type, entity_count, duration);
    }
    
    fn record_memory_usage(&self, component_type: &str, bytes: i64) {
        println!("Memory usage for '{}': {} bytes change", component_type, bytes);
    }
    
    fn get_health_status(&self) -> BevyHealthStatus {
        let mut status = BevyHealthStatus {
            overall_status: HealthLevel::Healthy,
            brp_connection_healthy: true,
            memory_usage_healthy: true,
            performance_healthy: true,
            issues: Vec::new(),
            recommendations: Vec::new(),
        };
        
        // Check BRP connection health
        if matches!(self.metrics.brp_connection_status, ConnectionStatus::Failed | ConnectionStatus::Disconnected) {
            status.brp_connection_healthy = false;
            status.issues.push("BRP connection is not healthy".to_string());
            status.overall_status = HealthLevel::Critical;
        }
        
        // Check memory pressure
        if matches!(self.metrics.memory_pressure_level, MemoryPressure::High | MemoryPressure::Critical) {
            status.memory_usage_healthy = false;
            status.issues.push("Memory pressure is high".to_string());
            if status.overall_status == HealthLevel::Healthy {
                status.overall_status = HealthLevel::Warning;
            }
        }
        
        // Check performance
        if self.metrics.frame_time_ms > 16.0 { // Below 60 FPS
            status.performance_healthy = false;
            status.issues.push("Frame time exceeds 16ms target".to_string());
            status.recommendations.push("Consider optimizing system execution times".to_string());
            if status.overall_status == HealthLevel::Healthy {
                status.overall_status = HealthLevel::Warning;
            }
        }
        
        status
    }
}

/// Integration points for Prometheus metrics
pub mod prometheus_integration {
    use super::*;
    
    /// Metric names that should be registered with Prometheus
    pub const BEVY_METRIC_NAMES: &[&str] = &[
        "bevy_brp_connection_status",
        "bevy_brp_connection_uptime_seconds",
        "bevy_brp_reconnection_total",
        "bevy_brp_request_duration_seconds",
        "bevy_brp_failed_requests_total",
        "bevy_entity_count",
        "bevy_component_count",
        "bevy_system_execution_duration_seconds",
        "bevy_world_memory_usage_bytes",
        "bevy_frame_time_seconds",
        "bevy_fps",
        "bevy_memory_pressure_level",
        "bevy_active_debug_sessions",
        "bevy_debug_queries_per_second",
        "bevy_debug_overhead_percentage",
    ];
    
    /// Convert BevyMetrics to Prometheus-compatible format
    pub fn metrics_to_prometheus_format(metrics: &BevyMetrics) -> HashMap<String, f64> {
        let mut prometheus_metrics = HashMap::new();
        
        prometheus_metrics.insert("bevy_brp_connection_status".to_string(), 
            match metrics.brp_connection_status {
                ConnectionStatus::Connected => 1.0,
                ConnectionStatus::Reconnecting => 0.5,
                _ => 0.0,
            });
        
        prometheus_metrics.insert("bevy_brp_connection_uptime_seconds".to_string(), 
            metrics.brp_connection_uptime.as_secs_f64());
        
        prometheus_metrics.insert("bevy_brp_reconnection_total".to_string(), 
            metrics.brp_reconnection_count as f64);
        
        prometheus_metrics.insert("bevy_brp_request_duration_seconds".to_string(), 
            metrics.brp_request_latency_ms / 1000.0);
        
        prometheus_metrics.insert("bevy_brp_failed_requests_total".to_string(), 
            metrics.brp_failed_requests as f64);
        
        prometheus_metrics.insert("bevy_entity_count".to_string(), 
            metrics.entity_count as f64);
        
        prometheus_metrics.insert("bevy_world_memory_usage_bytes".to_string(), 
            metrics.world_memory_usage_bytes as f64);
        
        prometheus_metrics.insert("bevy_frame_time_seconds".to_string(), 
            metrics.frame_time_ms / 1000.0);
        
        prometheus_metrics.insert("bevy_fps".to_string(), metrics.fps);
        
        prometheus_metrics.insert("bevy_memory_pressure_level".to_string(), 
            match metrics.memory_pressure_level {
                MemoryPressure::Low => 1.0,
                MemoryPressure::Medium => 2.0,
                MemoryPressure::High => 3.0,
                MemoryPressure::Critical => 4.0,
            });
        
        prometheus_metrics.insert("bevy_active_debug_sessions".to_string(), 
            metrics.active_debug_sessions as f64);
        
        prometheus_metrics.insert("bevy_debug_queries_per_second".to_string(), 
            metrics.queries_executed_per_second);
        
        prometheus_metrics.insert("bevy_debug_overhead_percentage".to_string(), 
            metrics.debug_overhead_percentage);
        
        prometheus_metrics
    }
}

/// Integration points for OpenTelemetry tracing
pub mod opentelemetry_integration {
    use super::*;
    
    /// Span names for distributed tracing
    pub const BEVY_SPAN_NAMES: &[&str] = &[
        "bevy.brp.connect",
        "bevy.brp.request", 
        "bevy.brp.reconnect",
        "bevy.debug.observe",
        "bevy.debug.experiment",
        "bevy.debug.stress_test",
        "bevy.debug.hypothesis",
        "bevy.ecs.system_execution",
        "bevy.ecs.query_execution",
        "bevy.memory.allocation",
        "bevy.frame.render",
    ];
    
    /// Create span attributes for Bevy operations
    pub fn create_bevy_span_attributes(
        operation: &str, 
        entity_count: Option<u64>,
        component_types: Option<&[String]>
    ) -> HashMap<String, String> {
        let mut attributes = HashMap::new();
        
        attributes.insert("bevy.operation".to_string(), operation.to_string());
        
        if let Some(count) = entity_count {
            attributes.insert("bevy.entity_count".to_string(), count.to_string());
        }
        
        if let Some(types) = component_types {
            attributes.insert("bevy.component_types".to_string(), types.join(","));
        }
        
        attributes.insert("bevy.debugger.version".to_string(), 
            env!("CARGO_PKG_VERSION").to_string());
        
        attributes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_bevy_observability() {
        let observability = DefaultBevyObservability::new();
        let metrics = observability.collect_metrics().unwrap();
        
        assert_eq!(metrics.entity_count, 0);
        assert!(matches!(metrics.brp_connection_status, ConnectionStatus::Disconnected));
    }
    
    #[test]
    fn test_prometheus_metrics_conversion() {
        let metrics = BevyMetrics {
            brp_connection_status: ConnectionStatus::Connected,
            entity_count: 100,
            fps: 60.0,
            ..Default::default()
        };
        
        let prometheus_metrics = prometheus_integration::metrics_to_prometheus_format(&metrics);
        
        assert_eq!(prometheus_metrics["bevy_brp_connection_status"], 1.0);
        assert_eq!(prometheus_metrics["bevy_entity_count"], 100.0);
        assert_eq!(prometheus_metrics["bevy_fps"], 60.0);
    }
    
    #[test]
    fn test_health_status_calculation() {
        let mut observability = DefaultBevyObservability::new();
        observability.metrics.memory_pressure_level = MemoryPressure::High;
        
        let health = observability.get_health_status();
        
        assert_eq!(health.overall_status, HealthLevel::Warning);
        assert!(!health.memory_usage_healthy);
        assert!(health.issues.len() > 0);
    }
}