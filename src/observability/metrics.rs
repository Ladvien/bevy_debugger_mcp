/*
 * Bevy Debugger MCP - Metrics Collection
 * Copyright (C) 2025 ladvien
 *
 * Prometheus-compatible metrics collection for MCP server monitoring
 */

use prometheus::{
    Counter, Gauge, Histogram, HistogramOpts, IntCounter, IntGauge, 
    Opts, Registry, TextEncoder, Encoder
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use sysinfo::{System, SystemExt, ProcessExt, Pid};

use crate::config::Config;
use crate::error::{Result, Error};

lazy_static::lazy_static! {
    /// Global metrics registry for the MCP server
    pub static ref MCP_METRICS: Arc<MetricsCollector> = {
        Arc::new(MetricsCollector::new_default())
    };
}

/// Comprehensive metrics collector for MCP server operations
pub struct MetricsCollector {
    registry: Registry,
    
    // Request metrics
    request_duration: Histogram,
    requests_total: IntCounter,
    requests_active: IntGauge,
    
    // Tool-specific metrics
    tool_requests_total: Counter,
    tool_errors_total: Counter,
    tool_duration: Histogram,
    
    // Connection metrics
    connections_active: IntGauge,
    connections_total: IntCounter,
    brp_connection_status: IntGauge,
    brp_reconnections_total: IntCounter,
    
    // System metrics
    memory_usage_bytes: Gauge,
    cpu_usage_percent: Gauge,
    process_uptime_seconds: Gauge,
    
    // Error metrics
    errors_total: IntCounter,
    panics_total: IntCounter,
    
    // Performance metrics
    gc_duration: Histogram,
    thread_count: IntGauge,
    
    // Runtime tracking
    start_time: Instant,
    system_info: Arc<RwLock<System>>,
    process_id: u32,
}

impl MetricsCollector {
    /// Create a new metrics collector with configuration
    pub fn new(config: &Config) -> Result<Self> {
        let registry = Registry::new();
        
        // Request metrics
        let request_duration = Histogram::with_opts(
            HistogramOpts::new(
                "mcp_request_duration_seconds",
                "Duration of MCP requests in seconds"
            ).buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0])
        ).map_err(|e| Error::Config(format!("Failed to create request duration metric: {}", e)))?;
        
        let requests_total = IntCounter::with_opts(
            Opts::new("mcp_requests_total", "Total number of MCP requests")
        ).map_err(|e| Error::Config(format!("Failed to create requests total metric: {}", e)))?;
        
        let requests_active = IntGauge::with_opts(
            Opts::new("mcp_requests_active", "Number of active MCP requests")
        ).map_err(|e| Error::Config(format!("Failed to create active requests metric: {}", e)))?;
        
        // Tool-specific metrics
        let tool_requests_total = Counter::with_opts(
            Opts::new("mcp_tool_requests_total", "Total number of tool requests by tool name")
                .variable_label("tool")
        ).map_err(|e| Error::Config(format!("Failed to create tool requests metric: {}", e)))?;
        
        let tool_errors_total = Counter::with_opts(
            Opts::new("mcp_tool_errors_total", "Total number of tool errors by tool name")
                .variable_label("tool")
        ).map_err(|e| Error::Config(format!("Failed to create tool errors metric: {}", e)))?;
        
        let tool_duration = Histogram::with_opts(
            HistogramOpts::new(
                "mcp_tool_duration_seconds",
                "Duration of tool execution in seconds"
            ).variable_label("tool")
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0, 30.0])
        ).map_err(|e| Error::Config(format!("Failed to create tool duration metric: {}", e)))?;
        
        // Connection metrics
        let connections_active = IntGauge::with_opts(
            Opts::new("mcp_connections_active", "Number of active MCP connections")
        ).map_err(|e| Error::Config(format!("Failed to create active connections metric: {}", e)))?;
        
        let connections_total = IntCounter::with_opts(
            Opts::new("mcp_connections_total", "Total number of MCP connections")
        ).map_err(|e| Error::Config(format!("Failed to create total connections metric: {}", e)))?;
        
        let brp_connection_status = IntGauge::with_opts(
            Opts::new("brp_connection_status", "BRP connection status (1=healthy, 0=unhealthy)")
        ).map_err(|e| Error::Config(format!("Failed to create BRP status metric: {}", e)))?;
        
        let brp_reconnections_total = IntCounter::with_opts(
            Opts::new("brp_reconnections_total", "Total number of BRP reconnection attempts")
        ).map_err(|e| Error::Config(format!("Failed to create BRP reconnections metric: {}", e)))?;
        
        // System metrics
        let memory_usage_bytes = Gauge::with_opts(
            Opts::new("process_memory_usage_bytes", "Memory usage in bytes")
        ).map_err(|e| Error::Config(format!("Failed to create memory usage metric: {}", e)))?;
        
        let cpu_usage_percent = Gauge::with_opts(
            Opts::new("process_cpu_usage_percent", "CPU usage percentage")
        ).map_err(|e| Error::Config(format!("Failed to create CPU usage metric: {}", e)))?;
        
        let process_uptime_seconds = Gauge::with_opts(
            Opts::new("process_uptime_seconds", "Process uptime in seconds")
        ).map_err(|e| Error::Config(format!("Failed to create uptime metric: {}", e)))?;
        
        // Error metrics
        let errors_total = IntCounter::with_opts(
            Opts::new("mcp_errors_total", "Total number of errors")
        ).map_err(|e| Error::Config(format!("Failed to create errors total metric: {}", e)))?;
        
        let panics_total = IntCounter::with_opts(
            Opts::new("mcp_panics_total", "Total number of panics")
        ).map_err(|e| Error::Config(format!("Failed to create panics total metric: {}", e)))?;
        
        // Performance metrics
        let gc_duration = Histogram::with_opts(
            HistogramOpts::new(
                "mcp_gc_duration_seconds",
                "Duration of garbage collection in seconds"
            ).buckets(vec![0.0001, 0.001, 0.01, 0.1, 1.0])
        ).map_err(|e| Error::Config(format!("Failed to create GC duration metric: {}", e)))?;
        
        let thread_count = IntGauge::with_opts(
            Opts::new("process_thread_count", "Number of threads")
        ).map_err(|e| Error::Config(format!("Failed to create thread count metric: {}", e)))?;
        
        // Register all metrics
        registry.register(Box::new(request_duration.clone()))?;
        registry.register(Box::new(requests_total.clone()))?;
        registry.register(Box::new(requests_active.clone()))?;
        registry.register(Box::new(tool_requests_total.clone()))?;
        registry.register(Box::new(tool_errors_total.clone()))?;
        registry.register(Box::new(tool_duration.clone()))?;
        registry.register(Box::new(connections_active.clone()))?;
        registry.register(Box::new(connections_total.clone()))?;
        registry.register(Box::new(brp_connection_status.clone()))?;
        registry.register(Box::new(brp_reconnections_total.clone()))?;
        registry.register(Box::new(memory_usage_bytes.clone()))?;
        registry.register(Box::new(cpu_usage_percent.clone()))?;
        registry.register(Box::new(process_uptime_seconds.clone()))?;
        registry.register(Box::new(errors_total.clone()))?;
        registry.register(Box::new(panics_total.clone()))?;
        registry.register(Box::new(gc_duration.clone()))?;
        registry.register(Box::new(thread_count.clone()))?;
        
        let process_id = std::process::id();
        
        Ok(Self {
            registry,
            request_duration,
            requests_total,
            requests_active,
            tool_requests_total,
            tool_errors_total,
            tool_duration,
            connections_active,
            connections_total,
            brp_connection_status,
            brp_reconnections_total,
            memory_usage_bytes,
            cpu_usage_percent,
            process_uptime_seconds,
            errors_total,
            panics_total,
            gc_duration,
            thread_count,
            start_time: Instant::now(),
            system_info: Arc::new(RwLock::new(System::new_all())),
            process_id,
        })
    }
    
    /// Create a default metrics collector for lazy initialization
    pub fn new_default() -> Self {
        Self::new(&Config::default()).expect("Failed to create default metrics collector")
    }
    
    /// Start metrics collection background task
    pub async fn start(&self) -> Result<()> {
        info!("Starting metrics collection");
        
        // Start system metrics collection task
        let system_info = self.system_info.clone();
        let memory_gauge = self.memory_usage_bytes.clone();
        let cpu_gauge = self.cpu_usage_percent.clone();
        let uptime_gauge = self.process_uptime_seconds.clone();
        let thread_gauge = self.thread_count.clone();
        let start_time = self.start_time;
        let process_id = self.process_id;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            
            loop {
                interval.tick().await;
                
                // Update system metrics
                {
                    let mut system = system_info.write().await;
                    system.refresh_processes();
                    system.refresh_cpu();
                    
                    if let Some(process) = system.process(Pid::from(process_id as usize)) {
                        memory_gauge.set(process.memory() as f64 * 1024.0); // Convert KB to bytes
                        cpu_gauge.set(process.cpu_usage() as f64);
                    }
                }
                
                // Update uptime
                let uptime = start_time.elapsed().as_secs_f64();
                uptime_gauge.set(uptime);
                
                // Update thread count (approximate)
                let thread_count = num_cpus::get() as i64;
                thread_gauge.set(thread_count);
            }
        });
        
        info!("Metrics collection started");
        Ok(())
    }
    
    /// Shutdown metrics collection
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down metrics collection");
        // TODO: Implement graceful shutdown of background tasks
        Ok(())
    }
    
    /// Record a request start
    pub fn record_request_start(&self) -> RequestTimer {
        self.requests_active.inc();
        self.requests_total.inc();
        RequestTimer::new(self.request_duration.clone(), self.requests_active.clone())
    }
    
    /// Record a tool request
    pub fn record_tool_request(&self, tool_name: &str) -> ToolTimer {
        self.tool_requests_total.with_label_values(&[tool_name]).inc();
        ToolTimer::new(tool_name, self.tool_duration.clone())
    }
    
    /// Record a tool error
    pub fn record_tool_error(&self, tool_name: &str) {
        self.tool_errors_total.with_label_values(&[tool_name]).inc();
    }
    
    /// Record a new connection
    pub fn record_connection_start(&self) -> ConnectionTracker {
        self.connections_active.inc();
        self.connections_total.inc();
        ConnectionTracker::new(self.connections_active.clone())
    }
    
    /// Set BRP connection status
    pub fn set_brp_connection_status(&self, healthy: bool) {
        self.brp_connection_status.set(if healthy { 1 } else { 0 });
    }
    
    /// Record BRP reconnection attempt
    pub fn record_brp_reconnection(&self) {
        self.brp_reconnections_total.inc();
    }
    
    /// Record an error
    pub fn record_error(&self) {
        self.errors_total.inc();
    }
    
    /// Record a panic
    pub fn record_panic(&self) {
        self.panics_total.inc();
    }
    
    /// Export metrics in Prometheus format
    pub fn export_prometheus(&self) -> Result<String> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder.encode_to_string(&metric_families)
            .map_err(|e| Error::Config(format!("Failed to encode metrics: {}", e)))
    }
    
    /// Get current metrics snapshot
    pub fn get_metrics_snapshot(&self) -> MetricsSnapshot {
        // Calculate percentiles from histogram
        let request_duration_samples = self.request_duration.get_sample_count();
        let request_latency_sum = self.request_duration.get_sample_sum();
        
        MetricsSnapshot {
            request_latency_p50: 0.0, // TODO: Calculate from histogram buckets
            request_latency_p95: 0.0, // TODO: Calculate from histogram buckets  
            request_latency_p99: 0.0, // TODO: Calculate from histogram buckets
            total_requests: self.requests_total.get(),
            active_requests: self.requests_active.get(),
            active_connections: self.connections_active.get(),
            total_connections: self.connections_total.get(),
            brp_connection_healthy: self.brp_connection_status.get() == 1,
            memory_usage_bytes: self.memory_usage_bytes.get() as u64,
            cpu_usage_percent: self.cpu_usage_percent.get(),
            uptime_seconds: self.process_uptime_seconds.get(),
            total_errors: self.errors_total.get(),
            total_panics: self.panics_total.get(),
        }
    }
}

/// Timer for tracking request duration
pub struct RequestTimer {
    start: Instant,
    histogram: Histogram,
    active_gauge: IntGauge,
}

impl RequestTimer {
    fn new(histogram: Histogram, active_gauge: IntGauge) -> Self {
        Self {
            start: Instant::now(),
            histogram,
            active_gauge,
        }
    }
}

impl Drop for RequestTimer {
    fn drop(&mut self) {
        let duration = self.start.elapsed().as_secs_f64();
        self.histogram.observe(duration);
        self.active_gauge.dec();
    }
}

/// Timer for tracking tool execution duration
pub struct ToolTimer {
    start: Instant,
    tool_name: String,
    histogram: Histogram,
}

impl ToolTimer {
    fn new(tool_name: &str, histogram: Histogram) -> Self {
        Self {
            start: Instant::now(),
            tool_name: tool_name.to_string(),
            histogram,
        }
    }
}

impl Drop for ToolTimer {
    fn drop(&mut self) {
        let duration = self.start.elapsed().as_secs_f64();
        self.histogram.with_label_values(&[&self.tool_name]).observe(duration);
    }
}

/// Tracker for active connections
pub struct ConnectionTracker {
    active_gauge: IntGauge,
}

impl ConnectionTracker {
    fn new(active_gauge: IntGauge) -> Self {
        Self { active_gauge }
    }
}

impl Drop for ConnectionTracker {
    fn drop(&mut self) {
        self.active_gauge.dec();
    }
}

/// Snapshot of current metrics
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub request_latency_p50: f64,
    pub request_latency_p95: f64,
    pub request_latency_p99: f64,
    pub total_requests: u64,
    pub active_requests: i64,
    pub active_connections: i64,
    pub total_connections: u64,
    pub brp_connection_healthy: bool,
    pub memory_usage_bytes: u64,
    pub cpu_usage_percent: f64,
    pub uptime_seconds: f64,
    pub total_errors: u64,
    pub total_panics: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collection() {
        let config = Config::default();
        let collector = MetricsCollector::new(&config).unwrap();
        
        // Test request tracking
        let timer = collector.record_request_start();
        drop(timer);
        
        // Test tool tracking
        let tool_timer = collector.record_tool_request("observe");
        drop(tool_timer);
        
        // Test error recording
        collector.record_error();
        collector.record_tool_error("observe");
        
        // Get snapshot
        let snapshot = collector.get_metrics_snapshot();
        assert_eq!(snapshot.total_requests, 1);
        assert_eq!(snapshot.total_errors, 1);
    }

    #[test]
    fn test_prometheus_export() {
        let config = Config::default();
        let collector = MetricsCollector::new(&config).unwrap();
        
        let export = collector.export_prometheus();
        assert!(export.is_ok());
        
        let output = export.unwrap();
        assert!(output.contains("mcp_requests_total"));
        assert!(output.contains("mcp_errors_total"));
    }
}