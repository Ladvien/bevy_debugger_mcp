/*
 * Bevy Debugger MCP - Observability Stack
 * Copyright (C) 2025 ladvien
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

//! Comprehensive observability stack for Bevy Debugger MCP Server
//! 
//! This module provides production-grade monitoring, metrics, and tracing capabilities:
//! - OpenTelemetry integration for distributed tracing
//! - Prometheus metrics export for monitoring
//! - Jaeger-compatible distributed tracing
//! - Health endpoints for load balancers and Kubernetes
//! - Alert rules and Grafana dashboards

pub mod metrics;
pub mod tracing;
pub mod health;
pub mod telemetry;
pub mod alerts;

use std::sync::Arc;
use tokio::sync::RwLock;
// Re-export tracing macros since they're used throughout the module
use crate::config::Config;
use crate::brp_client::BrpClient;
use crate::error::Result;

pub use metrics::{MetricsCollector, MCP_METRICS};
pub use tracing::TracingService;
pub use health::{HealthService, HealthStatus};
pub use telemetry::TelemetryService;

/// Main observability service that coordinates all monitoring components
pub struct ObservabilityService {
    config: Config,
    metrics: Arc<MetricsCollector>,
    tracing_service: Arc<TracingService>,
    health_service: Arc<HealthService>,
    telemetry_service: Arc<TelemetryService>,
}

impl ObservabilityService {
    /// Create a new observability service with all monitoring components
    pub async fn new(config: Config, brp_client: Arc<RwLock<BrpClient>>) -> Result<Self> {
        println!("Initializing observability stack");

        // Initialize metrics collection
        let metrics = Arc::new(MetricsCollector::new(&config)?);
        
        // Initialize distributed tracing
        let tracing_service = Arc::new(TracingService::new(&config).await?);
        
        // Initialize health monitoring
        let health_service = Arc::new(HealthService::new(brp_client.clone()).await?);
        
        // Initialize telemetry service
        let telemetry_service = Arc::new(TelemetryService::new(&config).await?);

        println!("Observability stack initialized successfully");

        Ok(Self {
            config,
            metrics,
            tracing_service,
            health_service,
            telemetry_service,
        })
    }

    /// Start all observability services
    pub async fn start(&self) -> Result<()> {
        println!("Starting observability services");

        // Start metrics collection
        self.metrics.start().await?;
        
        // Start tracing service
        self.tracing_service.start().await?;
        
        // Start health monitoring
        self.health_service.start().await?;
        
        // Start telemetry service
        self.telemetry_service.start().await?;

        println!("All observability services started");
        Ok(())
    }

    /// Shutdown all observability services gracefully
    pub async fn shutdown(&self) -> Result<()> {
        println!("Shutting down observability services");

        // Shutdown in reverse order
        if let Err(e) = self.telemetry_service.shutdown().await {
            println!("Error shutting down telemetry service: {}", e);
        }

        if let Err(e) = self.health_service.shutdown().await {
            println!("Error shutting down health service: {}", e);
        }

        if let Err(e) = self.tracing_service.shutdown().await {
            println!("Error shutting down tracing service: {}", e);
        }

        if let Err(e) = self.metrics.shutdown().await {
            println!("Error shutting down metrics service: {}", e);
        }

        println!("Observability services shutdown complete");
        Ok(())
    }

    /// Get metrics collector for recording measurements
    pub fn metrics(&self) -> Arc<MetricsCollector> {
        self.metrics.clone()
    }

    /// Get tracing service for creating spans
    pub fn tracing(&self) -> Arc<TracingService> {
        self.tracing_service.clone()
    }

    /// Get health service for status checks
    pub fn health(&self) -> Arc<HealthService> {
        self.health_service.clone()
    }

    /// Get telemetry service for custom telemetry
    pub fn telemetry(&self) -> Arc<TelemetryService> {
        self.telemetry_service.clone()
    }
}

/// Key metrics that need to be tracked according to BEVDBG-016 requirements
pub struct ObservabilityMetrics {
    /// Request latency percentiles (p50, p95, p99)
    pub request_latency_p50: f64,
    pub request_latency_p95: f64,
    pub request_latency_p99: f64,
    
    /// Error rate by tool
    pub error_rate_by_tool: std::collections::HashMap<String, f64>,
    
    /// Active connections
    pub active_connections: u64,
    
    /// Memory usage in bytes
    pub memory_usage_bytes: u64,
    
    /// CPU usage percentage
    pub cpu_usage_percent: f64,
    
    /// BRP connection health status
    pub brp_connection_healthy: bool,
}

impl Default for ObservabilityMetrics {
    fn default() -> Self {
        Self {
            request_latency_p50: 0.0,
            request_latency_p95: 0.0,
            request_latency_p99: 0.0,
            error_rate_by_tool: std::collections::HashMap::new(),
            active_connections: 0,
            memory_usage_bytes: 0,
            cpu_usage_percent: 0.0,
            brp_connection_healthy: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_observability_service_creation() {
        let config = Config::default();
        let brp_client = Arc::new(RwLock::new(crate::brp_client::BrpClient::new(&config)));
        
        let observability = ObservabilityService::new(config, brp_client).await;
        assert!(observability.is_ok());
    }
}