/*
 * Production-Grade Connection Pool for BRP Connections
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

use crate::config::{Config, ConnectionPoolConfig};
use crate::error::{Error, Result};
use futures_util::{SinkExt, StreamExt};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex, Semaphore};
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};
use url::Url;
use uuid::Uuid;

pub type WebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

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

/// Pooled connection wrapper
#[derive(Debug)]
pub struct PooledConnection {
    pub info: ConnectionInfo,
    pub websocket: WebSocket,
}

impl PooledConnection {
    pub fn new(websocket: WebSocket, game_endpoint: String) -> Self {
        Self {
            info: ConnectionInfo::new(game_endpoint),
            websocket,
        }
    }

    /// Test connection health by sending a ping
    pub async fn health_check(&mut self) -> bool {
        match self.websocket.send(Message::Ping(vec![])).await {
            Ok(_) => {
                debug!("Health check passed for connection {}", self.info.id);
                self.info.is_healthy = true;
                true
            }
            Err(e) => {
                warn!("Health check failed for connection {}: {}", self.info.id, e);
                self.info.is_healthy = false;
                false
            }
        }
    }
}

/// Production-grade connection pool for BRP connections
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
            if let Ok(connection) = self.create_connection().await {
                let mut pool = self.available_connections.lock().await;
                pool.push_back(connection);
            }
        }

        // Start cleanup task
        self.start_cleanup_task().await;
        
        // Start health check task
        self.start_health_check_task().await;

        Ok(())
    }

    /// Get a connection from the pool
    pub async fn get_connection(&self) -> Result<PooledConnection> {
        // Try to acquire semaphore permit
        let _permit = timeout(
            self.pool_config.connection_timeout,
            self.connection_semaphore.acquire()
        )
        .await
        .map_err(|_| {
            // Update metrics
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
                // Perform quick health check
                if connection.health_check().await {
                    connection.info.mark_used();
                    
                    // Update metrics
                    let mut metrics = self.metrics.lock().await;
                    metrics.available_connections = pool.len() as u32;
                    metrics.active_connections += 1;
                    
                    debug!("Reused pooled connection {}", connection.info.id);
                    return Ok(connection);
                } else {
                    // Connection is unhealthy, create new one
                    warn!("Removing unhealthy connection {} from pool", connection.info.id);
                    let mut metrics = self.metrics.lock().await;
                    metrics.total_connections_closed += 1;
                    metrics.health_check_failures += 1;
                }
            }
        }

        // Create new connection if pool is empty or connections are unhealthy
        let connection = self.create_connection().await?;
        
        // Update metrics
        let mut metrics = self.metrics.lock().await;
        metrics.active_connections += 1;
        
        info!("Created new pooled connection {}", connection.info.id);
        Ok(connection)
    }

    /// Return a connection to the pool
    pub async fn return_connection(&self, mut connection: PooledConnection) {
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

        // Perform health check before returning to pool
        if !connection.health_check().await {
            warn!("Connection {} failed health check, discarding", connection.info.id);
            let mut metrics = self.metrics.lock().await;
            metrics.active_connections = metrics.active_connections.saturating_sub(1);
            metrics.total_connections_closed += 1;
            metrics.health_check_failures += 1;
            return;
        }

        // Return healthy connection to pool
        {
            let mut pool = self.available_connections.lock().await;
            pool.push_back(connection);
            
            // Update metrics
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

        // Cancel background tasks
        if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
        }
        if let Some(handle) = self.health_check_handle.take() {
            handle.abort();
        }

        // Close all connections
        let mut pool = self.available_connections.lock().await;
        while let Some(mut connection) = pool.pop_front() {
            let _ = connection.websocket.close(None).await;
        }

        info!("Connection pool shutdown complete");
    }

    /// Create a new connection to the BRP endpoint
    async fn create_connection(&self) -> Result<PooledConnection> {
        let url_str = self.config.brp_url();
        let url = Url::parse(&url_str)
            .map_err(|e| Error::Connection(format!("Invalid BRP URL: {}", e)))?;

        debug!("Creating new connection to {}", url_str);
        
        let (websocket, _) = timeout(
            self.pool_config.connection_timeout,
            connect_async(&url_str)
        )
        .await
        .map_err(|_| {
            let mut metrics = self.metrics.try_lock().unwrap();
            metrics.connection_timeouts += 1;
            Error::Connection("Connection timeout".to_string())
        })?
        .map_err(|e| Error::WebSocket(Box::new(e)))?;

        let connection = PooledConnection::new(websocket, url_str);
        
        // Update metrics
        let mut metrics = self.metrics.lock().await;
        metrics.total_connections_created += 1;

        Ok(connection)
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
                
                let mut connections_to_close = Vec::new();
                let mut remaining_connections = VecDeque::new();
                
                {
                    let mut pool_guard = pool.lock().await;
                    
                    while let Some(connection) = pool_guard.pop_front() {
                        if connection.info.is_expired(config.max_connection_lifetime) 
                            || connection.info.is_idle(config.idle_timeout) 
                        {
                            connections_to_close.push(connection);
                        } else {
                            remaining_connections.push_back(connection);
                        }
                    }
                    
                    *pool_guard = remaining_connections;
                }

                // Close expired connections
                let closed_count = connections_to_close.len();
                for mut connection in connections_to_close {
                    let _ = connection.websocket.close(None).await;
                }

                if closed_count > 0 {
                    debug!("Cleaned up {} expired/idle connections", closed_count);
                    let mut metrics_guard = metrics.lock().await;
                    metrics_guard.total_connections_closed += closed_count as u64;
                    metrics_guard.available_connections = metrics_guard.available_connections.saturating_sub(closed_count as u32);
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
                let mut unhealthy_count = 0;
                
                {
                    let mut pool_guard = pool.lock().await;
                    
                    while let Some(mut connection) = pool_guard.pop_front() {
                        if connection.health_check().await {
                            healthy_connections.push_back(connection);
                        } else {
                            unhealthy_count += 1;
                            let _ = connection.websocket.close(None).await;
                        }
                    }
                    
                    *pool_guard = healthy_connections;
                }

                if unhealthy_count > 0 {
                    warn!("Removed {} unhealthy connections during health check", unhealthy_count);
                    let mut metrics_guard = metrics.lock().await;
                    metrics_guard.total_connections_closed += unhealthy_count;
                    metrics_guard.health_check_failures += unhealthy_count;
                    metrics_guard.available_connections = metrics_guard.available_connections.saturating_sub(unhealthy_count as u32);
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
        let mut info = ConnectionInfo::new("ws://localhost:15702".to_string());
        
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