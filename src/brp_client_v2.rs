/*
 * Production-Grade BRP Client v2 with Resilience Features
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

use crate::brp_messages::{BrpRequest, BrpResponse};
use crate::circuit_breaker::{CircuitBreaker, CircuitState};
use crate::config::Config;
use crate::connection_pool::{ConnectionPool, PooledConnection};
use crate::error::{Error, Result};
use crate::heartbeat::{HeartbeatService};
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{sleep, timeout};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Enhanced BRP client with production-grade resilience features
pub struct BrpClientV2 {
    config: Config,
    connection_pool: Arc<Mutex<ConnectionPool>>,
    circuit_breaker: Arc<CircuitBreaker>,
    is_running: Arc<AtomicBool>,
    retry_count: Arc<AtomicU32>,
    metrics: Arc<RwLock<BrpClientMetrics>>,
    request_id_counter: Arc<AtomicU32>,
}

/// Comprehensive metrics for monitoring the BRP client
#[derive(Debug, Clone, Default)]
pub struct BrpClientMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub circuit_breaker_trips: u64,
    pub connection_timeouts: u64,
    pub retry_attempts: u64,
    pub average_response_time: Duration,
    pub uptime_start: Option<Instant>,
}

impl BrpClientMetrics {
    /// Calculate success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            100.0
        } else {
            (self.successful_requests as f64 / self.total_requests as f64) * 100.0
        }
    }

    /// Calculate uptime duration
    pub fn uptime(&self) -> Duration {
        self.uptime_start.map(|start| start.elapsed()).unwrap_or_default()
    }

    /// Check if client is meeting 99.9% uptime requirement
    pub fn meets_uptime_sla(&self, target_uptime_percentage: f64) -> bool {
        self.success_rate() >= target_uptime_percentage
    }
}

/// Request wrapper with metadata for enhanced processing
#[derive(Debug)]
struct RequestWrapper {
    id: Uuid,
    request: BrpRequest,
    response_tx: mpsc::Sender<Result<BrpResponse>>,
    created_at: Instant,
    retry_count: u32,
}

impl BrpClientV2 {
    pub fn new(config: Config) -> Result<Self> {
        // Validate configuration
        config.validate()?;
        
        let connection_pool = ConnectionPool::new(config.clone());
        let circuit_breaker = CircuitBreaker::new(config.resilience.circuit_breaker.clone());

        Ok(Self {
            config,
            connection_pool: Arc::new(Mutex::new(connection_pool)),
            circuit_breaker: Arc::new(circuit_breaker),
            is_running: Arc::new(AtomicBool::new(false)),
            retry_count: Arc::new(AtomicU32::new(0)),
            metrics: Arc::new(RwLock::new(BrpClientMetrics::default())),
            request_id_counter: Arc::new(AtomicU32::new(0)),
        })
    }

    /// Start the BRP client and its background services
    pub async fn start(&self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            return Ok(()); // Already running
        }

        info!("Starting production-grade BRP client v2");

        // Initialize metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.uptime_start = Some(Instant::now());
        }

        // Start connection pool
        {
            let mut pool = self.connection_pool.lock().await;
            pool.start().await?;
        }

        self.is_running.store(true, Ordering::Relaxed);
        
        info!("BRP client v2 started successfully");
        Ok(())
    }

    /// Shutdown the BRP client gracefully
    pub async fn shutdown(&self) {
        info!("Shutting down BRP client v2");

        self.is_running.store(false, Ordering::Relaxed);

        // Shutdown connection pool
        {
            let mut pool = self.connection_pool.lock().await;
            pool.shutdown().await;
        }

        info!("BRP client v2 shutdown complete");
    }

    /// Send a request with full resilience features
    pub async fn send_request(&self, request: BrpRequest) -> Result<BrpResponse> {
        let request_id = Uuid::new_v4();
        let start_time = Instant::now();

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_requests += 1;
        }

        // Check circuit breaker
        if !self.circuit_breaker.can_execute() {
            let mut metrics = self.metrics.write().await;
            metrics.failed_requests += 1;
            metrics.circuit_breaker_trips += 1;
            
            return Err(Error::Connection(
                "Circuit breaker is open - service unavailable".to_string(),
            ));
        }

        // Attempt request with retry logic
        let result = self.send_request_with_retry(request_id, request).await;

        // Update circuit breaker and metrics based on result
        let duration = start_time.elapsed();
        match &result {
            Ok(_) => {
                self.circuit_breaker.record_success();
                let mut metrics = self.metrics.write().await;
                metrics.successful_requests += 1;
                self.update_average_response_time(&mut metrics, duration);
            }
            Err(_) => {
                self.circuit_breaker.record_failure();
                let mut metrics = self.metrics.write().await;
                metrics.failed_requests += 1;
            }
        }

        debug!("Request {} completed in {:?}", request_id, duration);
        result
    }

    /// Send request with exponential backoff retry
    async fn send_request_with_retry(&self, request_id: Uuid, request: BrpRequest) -> Result<BrpResponse> {
        let max_attempts = self.config.resilience.retry.max_attempts;
        let mut last_error = Error::Connection("No attempts made".to_string());

        for attempt in 1..=max_attempts {
            match self.send_request_attempt(request_id, &request).await {
                Ok(response) => {
                    if attempt > 1 {
                        info!("Request {} succeeded on attempt {}/{}", request_id, attempt, max_attempts);
                    }
                    return Ok(response);
                }
                Err(e) => {
                    last_error = e;
                    
                    if attempt < max_attempts {
                        let delay = self.calculate_backoff_delay(attempt - 1);
                        
                        warn!(
                            "Request {} failed on attempt {}/{}: {}. Retrying in {:?}",
                            request_id, attempt, max_attempts, last_error, delay
                        );

                        // Update retry metrics
                        {
                            let mut metrics = self.metrics.write().await;
                            metrics.retry_attempts += 1;
                        }

                        sleep(delay).await;
                    }
                }
            }
        }

        error!("Request {} failed after {} attempts: {}", request_id, max_attempts, last_error);
        Err(last_error)
    }

    /// Single request attempt
    async fn send_request_attempt(&self, request_id: Uuid, request: &BrpRequest) -> Result<BrpResponse> {
        // Get connection from pool
        let mut connection = {
            let pool = self.connection_pool.lock().await;
            timeout(
                self.config.resilience.connection_pool.connection_timeout,
                pool.get_connection()
            )
            .await
            .map_err(|_| {
                let mut metrics = self.metrics.try_write().unwrap();
                metrics.connection_timeouts += 1;
                Error::Connection("Timeout acquiring connection from pool".to_string())
            })?
        }?;

        debug!("Sending request {} using connection {}", request_id, connection.info.id);

        // Send request
        let request_json = serde_json::to_string(request)
            .map_err(Error::Json)?;
            
        connection.websocket
            .send(Message::Text(request_json))
            .await
            .map_err(|e| Error::WebSocket(Box::new(e)))?;

        // Wait for response with timeout
        let response = timeout(
            self.config.resilience.request_timeout,
            connection.websocket.next()
        )
        .await
        .map_err(|_| Error::Connection("Request timeout".to_string()))?;

        // Process response
        let response_text = match response {
            Some(Ok(Message::Text(text))) => text,
            Some(Ok(Message::Close(_))) => {
                return Err(Error::Connection("Connection closed during request".to_string()));
            }
            Some(Err(e)) => {
                return Err(Error::WebSocket(Box::new(e)));
            }
            None => {
                return Err(Error::Connection("No response received".to_string()));
            }
            _ => {
                return Err(Error::Connection("Unexpected message type".to_string()));
            }
        };

        // Parse response
        let brp_response: BrpResponse = serde_json::from_str(&response_text)
            .map_err(Error::Json)?;

        // Return connection to pool
        {
            let pool = self.connection_pool.lock().await;
            pool.return_connection(connection).await;
        }

        debug!("Request {} completed successfully", request_id);
        Ok(brp_response)
    }

    /// Calculate exponential backoff delay with jitter
    fn calculate_backoff_delay(&self, attempt: u32) -> Duration {
        let base_delay = self.config.resilience.retry.initial_delay;
        let multiplier = self.config.resilience.retry.multiplier;
        let max_delay = self.config.resilience.retry.max_delay;

        // Calculate exponential backoff
        let delay_ms = (base_delay.as_millis() as f64 * (multiplier as f64).powi(attempt as i32)) as u64;
        let delay = Duration::from_millis(delay_ms.min(max_delay.as_millis() as u64));

        // Add jitter if enabled
        if self.config.resilience.retry.jitter {
            let jitter_factor = rand::thread_rng().gen_range(0.0..0.1); // 0-10% jitter
            let jitter_ms = (delay.as_millis() as f64 * jitter_factor) as u64;
            delay + Duration::from_millis(jitter_ms)
        } else {
            delay
        }
    }

    /// Update moving average response time
    fn update_average_response_time(&self, metrics: &mut BrpClientMetrics, new_duration: Duration) {
        if metrics.successful_requests == 1 {
            metrics.average_response_time = new_duration;
        } else {
            let current_avg = metrics.average_response_time;
            let count = metrics.successful_requests;
            
            // Moving average calculation
            metrics.average_response_time = Duration::from_nanos(
                ((current_avg.as_nanos() as u64 * (count - 1)) + new_duration.as_nanos() as u64) / count
            );
        }
    }

    /// Get current client metrics
    pub async fn get_metrics(&self) -> BrpClientMetrics {
        self.metrics.read().await.clone()
    }

    /// Get circuit breaker state
    pub fn get_circuit_breaker_state(&self) -> CircuitState {
        self.circuit_breaker.get_state()
    }

    /// Check if client is healthy
    pub async fn is_healthy(&self) -> bool {
        if !self.is_running.load(Ordering::Relaxed) {
            return false;
        }

        let metrics = self.metrics.read().await;
        let circuit_metrics = self.circuit_breaker.get_metrics();
        
        // Healthy if circuit breaker is closed and success rate is good
        circuit_metrics.is_healthy() && metrics.success_rate() >= 95.0
    }

    /// Force circuit breaker reset (for testing/recovery)
    pub fn reset_circuit_breaker(&self) {
        self.circuit_breaker.reset();
    }

    /// Get connection pool metrics
    pub async fn get_pool_metrics(&self) -> crate::connection_pool::ConnectionPoolMetrics {
        let pool = self.connection_pool.lock().await;
        pool.get_metrics().await
    }
}

impl Drop for BrpClientV2 {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_calculations() {
        let mut metrics = BrpClientMetrics::default();
        metrics.total_requests = 100;
        metrics.successful_requests = 95;
        
        assert_eq!(metrics.success_rate(), 95.0);
        assert!(metrics.meets_uptime_sla(90.0));
        assert!(!metrics.meets_uptime_sla(99.0));
    }

    #[tokio::test]
    async fn test_client_creation_and_validation() {
        let mut config = Config::default();
        config.resilience.circuit_breaker.failure_threshold = 0; // Invalid
        
        let result = BrpClientV2::new(config);
        assert!(result.is_err());
        
        let valid_config = Config::default();
        let client = BrpClientV2::new(valid_config);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_backoff_calculation() {
        let config = Config::default();
        let client = BrpClientV2::new(config).unwrap();
        
        let delay1 = client.calculate_backoff_delay(0);
        let delay2 = client.calculate_backoff_delay(1);
        
        assert!(delay2 > delay1);
        assert!(delay2 <= client.config.resilience.retry.max_delay);
    }
}