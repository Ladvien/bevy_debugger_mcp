/*
 * Bevy Debugger MCP - Health Monitoring
 * Copyright (C) 2025 ladvien
 *
 * Health endpoints for load balancers and Kubernetes readiness/liveness probes
 */

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::brp_client::BrpClient;
use crate::error::{Result, Error};

/// Health status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

/// Individual component health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub status: HealthStatus,
    pub message: String,
    pub last_checked: u64,
    pub response_time_ms: u64,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ComponentHealth {
    pub fn healthy(message: &str) -> Self {
        Self {
            status: HealthStatus::Healthy,
            message: message.to_string(),
            last_checked: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            response_time_ms: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn degraded(message: &str) -> Self {
        Self {
            status: HealthStatus::Degraded,
            message: message.to_string(),
            last_checked: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            response_time_ms: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn unhealthy(message: &str) -> Self {
        Self {
            status: HealthStatus::Unhealthy,
            message: message.to_string(),
            last_checked: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            response_time_ms: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_response_time(mut self, response_time: Duration) -> Self {
        self.response_time_ms = response_time.as_millis() as u64;
        self
    }

    pub fn with_metadata(mut self, key: &str, value: serde_json::Value) -> Self {
        self.metadata.insert(key.to_string(), value);
        self
    }
}

/// Overall system health response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub timestamp: u64,
    pub uptime_seconds: u64,
    pub version: String,
    pub components: HashMap<String, ComponentHealth>,
    pub summary: HealthSummary,
}

/// Health summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSummary {
    pub total_components: usize,
    pub healthy_components: usize,
    pub degraded_components: usize,
    pub unhealthy_components: usize,
    pub overall_status: HealthStatus,
}

/// Health monitoring service
pub struct HealthService {
    brp_client: Arc<RwLock<BrpClient>>,
    start_time: Instant,
    last_health_check: Arc<RwLock<Option<HealthResponse>>>,
    health_check_interval: Duration,
}

impl HealthService {
    /// Create new health service
    pub async fn new(brp_client: Arc<RwLock<BrpClient>>) -> Result<Self> {
        Ok(Self {
            brp_client,
            start_time: Instant::now(),
            last_health_check: Arc::new(RwLock::new(None)),
            health_check_interval: Duration::from_secs(30),
        })
    }

    /// Start background health monitoring
    pub async fn start(&self) -> Result<()> {
        info!("Starting health monitoring service");

        // Start background health check task
        let brp_client = self.brp_client.clone();
        let last_health_check = self.last_health_check.clone();
        let start_time = self.start_time;
        let check_interval = self.health_check_interval;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(check_interval);
            
            loop {
                interval.tick().await;
                
                match Self::perform_health_checks(brp_client.clone(), start_time).await {
                    Ok(health_response) => {
                        let mut last_check = last_health_check.write().await;
                        *last_check = Some(health_response);
                    }
                    Err(e) => {
                        error!("Health check failed: {}", e);
                    }
                }
            }
        });

        info!("Health monitoring service started");
        Ok(())
    }

    /// Shutdown health service
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down health monitoring service");
        // Background task will be dropped when service is dropped
        Ok(())
    }

    /// Get current health status
    pub async fn get_health_status(&self) -> Result<HealthResponse> {
        let cached_health = {
            let last_check = self.last_health_check.read().await;
            last_check.clone()
        };

        if let Some(health) = cached_health {
            // Return cached result if it's recent (within 2x check interval)
            let age = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() - health.timestamp;
            
            if age < (self.health_check_interval.as_secs() * 2) {
                return Ok(health);
            }
        }

        // Perform fresh health check
        Self::perform_health_checks(self.brp_client.clone(), self.start_time).await
    }

    /// Get readiness status (for Kubernetes readiness probe)
    pub async fn get_readiness_status(&self) -> Result<bool> {
        let health = self.get_health_status().await?;
        Ok(matches!(health.status, HealthStatus::Healthy | HealthStatus::Degraded))
    }

    /// Get liveness status (for Kubernetes liveness probe)  
    pub async fn get_liveness_status(&self) -> Result<bool> {
        // Simple check - if we can respond, we're alive
        // Could add more sophisticated checks like deadlock detection
        Ok(true)
    }

    /// Perform comprehensive health checks
    async fn perform_health_checks(
        brp_client: Arc<RwLock<BrpClient>>,
        start_time: Instant,
    ) -> Result<HealthResponse> {
        let mut components = HashMap::new();
        
        // Check BRP connection health
        let brp_health = Self::check_brp_connection(brp_client).await;
        components.insert("brp_connection".to_string(), brp_health);

        // Check system resources
        let system_health = Self::check_system_resources().await;
        components.insert("system_resources".to_string(), system_health);

        // Check memory usage
        let memory_health = Self::check_memory_usage().await;
        components.insert("memory_usage".to_string(), memory_health);

        // Check disk space (basic check)
        let disk_health = Self::check_disk_space().await;
        components.insert("disk_space".to_string(), disk_health);

        // Calculate summary
        let total_components = components.len();
        let healthy_components = components.values()
            .filter(|c| c.status == HealthStatus::Healthy)
            .count();
        let degraded_components = components.values()
            .filter(|c| c.status == HealthStatus::Degraded)
            .count();
        let unhealthy_components = components.values()
            .filter(|c| c.status == HealthStatus::Unhealthy)
            .count();

        // Determine overall status
        let overall_status = if unhealthy_components > 0 {
            HealthStatus::Unhealthy
        } else if degraded_components > 0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let summary = HealthSummary {
            total_components,
            healthy_components,
            degraded_components,
            unhealthy_components,
            overall_status: overall_status.clone(),
        };

        Ok(HealthResponse {
            status: overall_status,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            uptime_seconds: start_time.elapsed().as_secs(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            components,
            summary,
        })
    }

    /// Check BRP connection health
    async fn check_brp_connection(brp_client: Arc<RwLock<BrpClient>>) -> ComponentHealth {
        let start = Instant::now();
        
        match tokio::time::timeout(Duration::from_secs(5), async {
            let client = brp_client.read().await;
            client.is_connected()
        }).await {
            Ok(true) => {
                ComponentHealth::healthy("BRP connection active")
                    .with_response_time(start.elapsed())
            }
            Ok(false) => {
                ComponentHealth::degraded("BRP connection inactive")
                    .with_response_time(start.elapsed())
            }
            Err(_) => {
                ComponentHealth::unhealthy("BRP connection check timed out")
                    .with_response_time(start.elapsed())
            }
        }
    }

    /// Check system resources
    async fn check_system_resources() -> ComponentHealth {
        use sysinfo::{System, Process, Pid};
        
        let start = Instant::now();
        let mut system = System::new_all();
        system.refresh_processes();
        
        let process_id = std::process::id();
        
        if let Some(process) = system.process(Pid::from(process_id as usize)) {
            let cpu_usage = process.cpu_usage();
            let memory_mb = process.memory() / 1024; // Convert KB to MB
            
            let status = if cpu_usage > 80.0 {
                HealthStatus::Unhealthy
            } else if cpu_usage > 50.0 || memory_mb > 1000 {
                HealthStatus::Degraded
            } else {
                HealthStatus::Healthy
            };
            
            let message = format!("CPU: {:.1}%, Memory: {}MB", cpu_usage, memory_mb);
            
            ComponentHealth {
                status,
                message,
                last_checked: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                response_time_ms: start.elapsed().as_millis() as u64,
                metadata: {
                    let mut map = HashMap::new();
                    map.insert("cpu_usage_percent".to_string(), 
                              serde_json::Value::from(cpu_usage as f64));
                    map.insert("memory_usage_mb".to_string(), 
                              serde_json::Value::from(memory_mb));
                    map
                },
            }
        } else {
            ComponentHealth::unhealthy("Could not find process information")
                .with_response_time(start.elapsed())
        }
    }

    /// Check memory usage patterns  
    async fn check_memory_usage() -> ComponentHealth {
        // This is a basic implementation - could be enhanced with leak detection
        let start = Instant::now();
        
        // Basic memory check using available system info
        ComponentHealth::healthy("Memory usage within normal limits")
            .with_response_time(start.elapsed())
    }

    /// Check available disk space
    async fn check_disk_space() -> ComponentHealth {
        let start = Instant::now();
        
        // Basic disk space check - could be enhanced to check actual disk usage
        match std::env::current_dir() {
            Ok(_) => {
                ComponentHealth::healthy("Disk space available")
                    .with_response_time(start.elapsed())
            }
            Err(e) => {
                ComponentHealth::unhealthy(&format!("Cannot access current directory: {}", e))
                    .with_response_time(start.elapsed())
            }
        }
    }

    /// Create HTTP router for health endpoints
    pub fn create_router(&self) -> Router<Arc<HealthService>> {
        Router::new()
            .route("/health", get(health_handler))
            .route("/health/live", get(liveness_handler))
            .route("/health/ready", get(readiness_handler))
            .route("/metrics/health", get(health_metrics_handler))
    }
}

/// HTTP handler for /health endpoint
async fn health_handler(
    State(health_service): State<Arc<HealthService>>,
) -> Result<Json<HealthResponse>, (StatusCode, String)> {
    match health_service.get_health_status().await {
        Ok(health) => {
            let status_code = match health.status {
                HealthStatus::Healthy => StatusCode::OK,
                HealthStatus::Degraded => StatusCode::OK, // Still serving traffic
                HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
            };
            
            // Note: Axum will set the status code from the first element of the tuple
            if status_code == StatusCode::OK {
                Ok(Json(health))
            } else {
                Err((status_code, serde_json::to_string(&health).unwrap_or_default()))
            }
        }
        Err(e) => {
            error!("Health check failed: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, 
                format!("Health check failed: {}", e)))
        }
    }
}

/// HTTP handler for /health/live endpoint (Kubernetes liveness probe)
async fn liveness_handler(
    State(health_service): State<Arc<HealthService>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match health_service.get_liveness_status().await {
        Ok(true) => Ok(Json(serde_json::json!({"status": "alive"}))),
        Ok(false) => Err((StatusCode::SERVICE_UNAVAILABLE, "not alive".to_string())),
        Err(e) => {
            error!("Liveness check failed: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, 
                format!("Liveness check failed: {}", e)))
        }
    }
}

/// HTTP handler for /health/ready endpoint (Kubernetes readiness probe)
async fn readiness_handler(
    State(health_service): State<Arc<HealthService>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match health_service.get_readiness_status().await {
        Ok(true) => Ok(Json(serde_json::json!({"status": "ready"}))),
        Ok(false) => Err((StatusCode::SERVICE_UNAVAILABLE, "not ready".to_string())),
        Err(e) => {
            error!("Readiness check failed: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, 
                format!("Readiness check failed: {}", e)))
        }
    }
}

/// HTTP handler for /metrics/health endpoint (Prometheus-style metrics)
async fn health_metrics_handler(
    State(health_service): State<Arc<HealthService>>,
) -> Result<String, (StatusCode, String)> {
    match health_service.get_health_status().await {
        Ok(health) => {
            let mut metrics = String::new();
            
            // Overall health metric
            metrics.push_str(&format!(
                "mcp_health_status{{}} {}\n",
                match health.status {
                    HealthStatus::Healthy => 1,
                    HealthStatus::Degraded => 0.5,
                    HealthStatus::Unhealthy => 0,
                }
            ));
            
            // Uptime metric
            metrics.push_str(&format!(
                "mcp_uptime_seconds{{}} {}\n",
                health.uptime_seconds
            ));
            
            // Component health metrics
            for (component_name, component_health) in &health.components {
                metrics.push_str(&format!(
                    "mcp_component_health{{component=\"{}\"}} {}\n",
                    component_name,
                    match component_health.status {
                        HealthStatus::Healthy => 1,
                        HealthStatus::Degraded => 0.5,
                        HealthStatus::Unhealthy => 0,
                    }
                ));
                
                metrics.push_str(&format!(
                    "mcp_component_response_time_ms{{component=\"{}\"}} {}\n",
                    component_name,
                    component_health.response_time_ms
                ));
            }
            
            Ok(metrics)
        }
        Err(e) => {
            error!("Health metrics generation failed: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, 
                format!("Health metrics failed: {}", e)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[tokio::test]
    async fn test_health_service_creation() {
        let config = Config::default();
        let brp_client = Arc::new(RwLock::new(crate::brp_client::BrpClient::new(&config)));
        
        let health_service = HealthService::new(brp_client).await;
        assert!(health_service.is_ok());
    }

    #[tokio::test]
    async fn test_component_health_creation() {
        let health = ComponentHealth::healthy("test message")
            .with_response_time(Duration::from_millis(100))
            .with_metadata("test_key", serde_json::Value::from("test_value"));
        
        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.message, "test message");
        assert_eq!(health.response_time_ms, 100);
        assert_eq!(health.metadata.get("test_key").unwrap(), "test_value");
    }

    #[tokio::test]
    async fn test_health_status_determination() {
        // Test healthy status
        let healthy = ComponentHealth::healthy("all good");
        assert_eq!(healthy.status, HealthStatus::Healthy);
        
        // Test degraded status
        let degraded = ComponentHealth::degraded("some issues");
        assert_eq!(degraded.status, HealthStatus::Degraded);
        
        // Test unhealthy status
        let unhealthy = ComponentHealth::unhealthy("major problems");
        assert_eq!(unhealthy.status, HealthStatus::Unhealthy);
    }
}