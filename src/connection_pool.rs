/*
 * Production-Grade Connection Pool for BRP Connections
 * Copyright (C) 2025 ladvien
 *
 * Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
 *
 * Relicensed from GPL-3.0 when this crate was adopted into Ladvien/foundation_vs_slop: a GPL
 * crate in the Bevy ecosystem cannot be adopted, and being adoptable is why it is published.
 */

//! Connection pooling over the BRP HTTP transport.
//!
//! HTTP is stateless (reqwest pools TCP connections internally), so this pool
//! manages *logical endpoint leases*: acquisition is rate-limited by a
//! semaphore, and health is probed with an `rpc.discover` JSON-RPC call.

use crate::config::{Config, ConnectionPoolConfig};
use crate::error::{Error, Result};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::timeout;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::brp::{builtin_methods, BrpRequest, BrpResponse};

/// Connection metadata for pool management
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub id: Uuid,
    pub created_at: Instant,
    pub last_used: Instant,
    pub use_count: u64,
    pub is_healthy: bool,
    pub game_endpoint: String,
}

impl ConnectionInfo {
    pub fn new(game_endpoint: String) -> Self {
        let now = Instant::now();
        Self {
            id: Uuid::new_v4(),
            created_at: now,
            last_used: now,
            use_count: 0,
            is_healthy: true,
            game_endpoint,
        }
    }

    /// Check if connection has exceeded maximum lifetime
    pub fn is_expired(&self, max_lifetime: Duration) -> bool {
        self.created_at.elapsed() > max_lifetime
    }

    /// Check if connection has been idle too long
    pub fn is_idle(&self, idle_timeout: Duration) -> bool {
        self.last_used.elapsed() > idle_timeout
    }

    /// Mark connection as used
    pub fn mark_used(&mut self) {
        self.last_used = Instant::now();
        self.use_count += 1;
    }
}

/// Pooled connection: a logical lease over one logical BRP endpoint.
///
/// Each lease owns its own `reqwest::Client` (with its own TCP pool), so
/// concurrent leases execute fully in parallel.
#[derive(Debug, Clone)]
pub struct PooledConnection {
    pub info: ConnectionInfo,
    pub client: reqwest::Client,
}

impl PooledConnection {
    pub fn new(game_endpoint: String) -> Self {
        Self {
            info: ConnectionInfo::new(game_endpoint),
            client: reqwest::Client::new(),
        }
    }

    /// Send a BRP JSON-RPC request through this lease.
    pub async fn send_request(&self, request: &BrpRequest) -> Result<BrpResponse> {
        let response = self
            .client
            .post(&self.info.game_endpoint)
            .json(request)
            .send()
            .await
            .map_err(Error::from)?;

        if !response.status().is_success() {
            return Err(Error::Connection(format!(
                "BRP server returned HTTP {}",
                response.status()
            )));
        }

        response.json::<BrpResponse>().await.map_err(Error::from)
    }

    /// Test connection health with an `rpc.discover` probe
    pub async fn health_check(&mut self) -> bool {
        let probe = BrpRequest {
            method: builtin_methods::RPC_DISCOVER_METHOD.to_string(),
            id: Some(Value::from(self.info.id.to_string())),
            params: None,
        };

        match self
            .client
            .post(&self.info.game_endpoint)
            .json(&probe)
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                debug!("Health check passed for connection {}", self.info.id);
                self.info.is_healthy = true;
                true
            }
            Ok(response) => {
                warn!(
                    "Health check failed for connection {}: HTTP {}",
                    self.info.id,
                    response.status()
                );
                self.info.is_healthy = false;
                false
            }
            Err(e) => {
                warn!("Health check failed for connection {}: {}", self.info.id, e);
                self.info.is_healthy = false;
                false
            }
        }
    }
}

/// Production-grade connection pool for BRP HTTP endpoints
pub struct ConnectionPool {
    config: Config,
    pool_config: ConnectionPoolConfig,
    available_connections: Arc<Mutex<VecDeque<PooledConnection>>>,
    connection_semaphore: Arc<Semaphore>,
    cleanup_handle: Option<tokio::task::JoinHandle<()>>,
    health_check_handle: Option<tokio::task::JoinHandle<()>>,
    metrics: Arc<Mutex<ConnectionPoolMetrics>>,
}

/// Connection pool metrics for monitoring
#[derive(Debug, Clone, Default)]
pub struct ConnectionPoolMetrics {
    pub active_connections: u32,
    pub available_connections: u32,
    pub total_connections_created: u64,
    pub total_connections_closed: u64,
    pub connection_timeouts: u64,
    pub health_check_failures: u64,
    pub pool_exhausted_events: u64,
}

impl ConnectionPoolMetrics {
    pub fn connection_utilization_rate(&self) -> f64 {
        if self.active_connections + self.available_connections == 0 {
            0.0
        } else {
            self.active_connections as f64
                / (self.active_connections + self.available_connections) as f64
        }
    }
}

static PROBE_SEQ: AtomicU64 = AtomicU64::new(1);

impl ConnectionPool {
    pub fn new(config: Config) -> Self {
        let pool_config = config.resilience.connection_pool.clone();
        let semaphore = Arc::new(Semaphore::new(pool_config.max_connections as usize));

        Self {
            config,
            pool_config,
            available_connections: Arc::new(Mutex::new(VecDeque::new())),
            connection_semaphore: semaphore,
            cleanup_handle: None,
            health_check_handle: None,
            metrics: Arc::new(Mutex::new(ConnectionPoolMetrics::default())),
        }
    }

    /// Start background tasks for connection pool maintenance
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting connection pool with {} max connections", self.pool_config.max_connections);

        // Pre-populate pool with minimum connections
        for _ in 0..self.pool_config.min_connections {
            let connection = self.create_connection();
            let mut pool = self.available_connections.lock().await;
            pool.push_back(connection);
        }

        // Start cleanup task
        self.start_cleanup_task().await;

        // Start health check task
        self.start_health_check_task().await;

        Ok(())
    }

    /// Get a connection lease from the pool
    pub async fn get_connection(&self) -> Result<PooledConnection> {
        // Try to acquire semaphore permit
        let _permit = timeout(
            self.pool_config.connection_timeout,
            self.connection_semaphore.acquire()
        )
        .await
        .map_err(|_| {
            {
                let mut metrics = self.metrics.try_lock().unwrap();
                metrics.connection_timeouts += 1;
                metrics.pool_exhausted_events += 1;
            }
            Error::Connection("Connection pool exhausted - timeout acquiring permit".to_string())
        })?
        .map_err(|_| Error::Connection("Semaphore closed".to_string()))?;

        // Try to get existing connection from pool
        {
            let mut pool = self.available_connections.lock().await;
            if let Some(mut connection) = pool.pop_front() {
                connection.info.mark_used();

                let mut metrics = self.metrics.lock().await;
                metrics.available_connections = pool.len() as u32;
                metrics.active_connections += 1;

                debug!("Reused pooled connection {}", connection.info.id);
                return Ok(connection);
            }
        }

        // Create new connection if pool is empty
        let connection = self.create_connection();

        let mut metrics = self.metrics.lock().await;
        metrics.active_connections += 1;

        info!("Created new pooled connection {}", connection.info.id);
        Ok(connection)
    }

    /// Return a connection lease to the pool
    pub async fn return_connection(&self, connection: PooledConnection) {
        // Check if connection should be discarded
        if connection.info.is_expired(self.pool_config.max_connection_lifetime)
            || !connection.info.is_healthy
        {
            debug!("Discarding connection {} (expired or unhealthy)", connection.info.id);
            let mut metrics = self.metrics.lock().await;
            metrics.active_connections = metrics.active_connections.saturating_sub(1);
            metrics.total_connections_closed += 1;
            return;
        }

        {
            let mut pool = self.available_connections.lock().await;
            pool.push_back(connection);

            let mut metrics = self.metrics.lock().await;
            metrics.available_connections = pool.len() as u32;
            metrics.active_connections = metrics.active_connections.saturating_sub(1);
        }

        debug!("Returned connection to pool");
    }

    /// Get current pool metrics
    pub async fn get_metrics(&self) -> ConnectionPoolMetrics {
        self.metrics.lock().await.clone()
    }

    /// Shutdown the connection pool
    pub async fn shutdown(&mut self) {
        info!("Shutting down connection pool");

        if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
        }
        if let Some(handle) = self.health_check_handle.take() {
            handle.abort();
        }

        let mut pool = self.available_connections.lock().await;
        pool.clear();

        info!("Connection pool shutdown complete");
    }

    /// Create a new lease to the BRP endpoint
    fn create_connection(&self) -> PooledConnection {
        let url_str = format!(
            "http://{}:{}",
            self.config.bevy_brp_host, self.config.bevy_brp_port
        );

        let connection = PooledConnection::new(url_str);
        PROBE_SEQ.fetch_add(1, Ordering::Relaxed);
        connection
    }

    /// Start cleanup task to remove expired/idle connections
    async fn start_cleanup_task(&mut self) {
        let pool = self.available_connections.clone();
        let config = self.pool_config.clone();
        let metrics = self.metrics.clone();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                let mut closed_count = 0u32;
                let mut remaining_connections = VecDeque::new();

                {
                    let mut pool_guard = pool.lock().await;

                    while let Some(connection) = pool_guard.pop_front() {
                        if connection.info.is_expired(config.max_connection_lifetime)
                            || connection.info.is_idle(config.idle_timeout)
                        {
                            closed_count += 1;
                        } else {
                            remaining_connections.push_back(connection);
                        }
                    }

                    *pool_guard = remaining_connections;
                }

                if closed_count > 0 {
                    debug!("Cleaned up {} expired/idle connections", closed_count);
                    let mut metrics_guard = metrics.lock().await;
                    metrics_guard.total_connections_closed += closed_count as u64;
                    metrics_guard.available_connections = metrics_guard.available_connections.saturating_sub(closed_count);
                }
            }
        });

        self.cleanup_handle = Some(handle);
    }

    /// Start health check task for proactive connection monitoring
    async fn start_health_check_task(&mut self) {
        let pool = self.available_connections.clone();
        let metrics = self.metrics.clone();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(120)); // Check every 2 minutes

            loop {
                interval.tick().await;

                let mut healthy_connections = VecDeque::new();
                let mut unhealthy_count = 0u32;

                {
                    let mut pool_guard = pool.lock().await;

                    while let Some(mut connection) = pool_guard.pop_front() {
                        if connection.health_check().await {
                            healthy_connections.push_back(connection);
                        } else {
                            unhealthy_count += 1;
                        }
                    }

                    *pool_guard = healthy_connections;
                }

                if unhealthy_count > 0 {
                    warn!("Removed {} unhealthy connections during health check", unhealthy_count);
                    let mut metrics_guard = metrics.lock().await;
                    metrics_guard.total_connections_closed += unhealthy_count as u64;
                    metrics_guard.health_check_failures += unhealthy_count as u64;
                    metrics_guard.available_connections = metrics_guard.available_connections.saturating_sub(unhealthy_count);
                }
            }
        });

        self.health_check_handle = Some(handle);
    }
}

impl Drop for ConnectionPool {
    fn drop(&mut self) {
        if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
        }
        if let Some(handle) = self.health_check_handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_info_expiry() {
        let mut info = ConnectionInfo::new("http://localhost:15702".to_string());

        // Should not be expired immediately
        assert!(!info.is_expired(Duration::from_secs(60)));

        // Should not be idle immediately
        assert!(!info.is_idle(Duration::from_secs(60)));

        // Mark as used and check use count
        info.mark_used();
        assert_eq!(info.use_count, 1);
    }

    #[test]
    fn test_metrics_utilization_rate() {
        let mut metrics = ConnectionPoolMetrics::default();
        metrics.active_connections = 3;
        metrics.available_connections = 7;

        assert_eq!(metrics.connection_utilization_rate(), 0.3);

        // Edge case: no connections
        metrics.active_connections = 0;
        metrics.available_connections = 0;
        assert_eq!(metrics.connection_utilization_rate(), 0.0);
    }
}
