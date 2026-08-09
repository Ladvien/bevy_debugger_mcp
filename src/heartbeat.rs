/*
 * Production-Grade Heartbeat Service for BRP Connections
 * Copyright (C) 2025 ladvien
 *
 * Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
 *
 * Relicensed from GPL-3.0 when this crate was adopted into Ladvien/foundation_vs_slop: a GPL
 * crate in the Bevy ecosystem cannot be adopted, and being adoptable is why it is published.
 */

//! Heartbeat monitoring over the BRP HTTP transport.
//!
//! Bevy's BRP is request/response JSON-RPC over HTTP: there is no persistent
//! socket to ping and no async pong stream to read. A heartbeat is therefore
//! one probe of the server (`rpc.discover` by default) per interval; round-trip
//! time is measured directly on the probe call.

use crate::config::{Config, HeartbeatConfig};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::Mutex;
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::brp::{builtin_methods, BrpRequest, BrpResponse};
use crate::error::{Error, Result};

/// Heartbeat message types
#[derive(Debug, Clone)]
pub enum HeartbeatMessage {
    Ping { id: Uuid, timestamp: u64 },
    Pong { id: Uuid, timestamp: u64 },
}

/// Heartbeat statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct HeartbeatStats {
    pub total_pings_sent: u64,
    pub total_pongs_received: u64,
    pub missed_heartbeats: u32,
    pub avg_round_trip_time: Duration,
    pub max_round_trip_time: Duration,
    pub consecutive_failures: u32,
    pub last_successful_heartbeat: Option<SystemTime>,
}

impl HeartbeatStats {
    /// Calculate heartbeat success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_pings_sent == 0 {
            100.0
        } else {
            (self.total_pongs_received as f64 / self.total_pings_sent as f64) * 100.0
        }
    }

    /// Check if connection is considered healthy based on heartbeat
    pub fn is_healthy(&self, max_missed: u32) -> bool {
        self.consecutive_failures < max_missed
    }
}

/// Transport abstraction for heartbeat probes.
///
/// BRP is HTTP JSON-RPC, so implementors send a single `BrpRequest` and
/// return the HTTP response; round-trip time is measured by the service.
#[async_trait]
pub trait HeartbeatTransport: Send + Sync {
    async fn send_request(&self, request: &BrpRequest) -> Result<BrpResponse>;
}

/// Default `HeartbeatTransport` that POSTs JSON-RPC to the BRP HTTP endpoint.
pub struct HttpHeartbeatTransport {
    http: reqwest::Client,
    url: String,
}

impl HttpHeartbeatTransport {
    pub fn new(config: &Config) -> Self {
        Self {
            http: reqwest::Client::new(),
            url: format!(
                "http://{}:{}",
                config.bevy_brp_host, config.bevy_brp_port
            ),
        }
    }
}

#[async_trait]
impl HeartbeatTransport for HttpHeartbeatTransport {
    async fn send_request(&self, request: &BrpRequest) -> Result<BrpResponse> {
        let response = self
            .http
            .post(&self.url)
            .json(request)
            .send()
            .await
            .map_err(|e| Error::Connection(format!("Heartbeat probe failed: {e}")))?;

        if !response.status().is_success() {
            return Err(Error::Connection(format!(
                "Heartbeat probe returned HTTP {}",
                response.status()
            )));
        }

        response.json::<BrpResponse>().await.map_err(Error::from)
    }
}

/// Production-grade heartbeat service for monitoring BRP connection health.
pub struct HeartbeatService {
    config: HeartbeatConfig,
    transport: Arc<dyn HeartbeatTransport>,
    stats: Arc<Mutex<HeartbeatStats>>,
    is_running: Arc<AtomicBool>,
    heartbeat_handle: Option<tokio::task::JoinHandle<()>>,
    failure_callback: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl std::fmt::Debug for HeartbeatService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeartbeatService")
            .field("config", &self.config)
            .field("is_running", &self.is_running.load(Ordering::Relaxed))
            .finish()
    }
}

impl HeartbeatService {
    pub fn new(transport: Arc<dyn HeartbeatTransport>, config: HeartbeatConfig) -> Self {
        Self {
            config,
            transport,
            stats: Arc::new(Mutex::new(HeartbeatStats::default())),
            is_running: Arc::new(AtomicBool::new(false)),
            heartbeat_handle: None,
            failure_callback: None,
        }
    }

    /// Convenience constructor bound to the BRP HTTP endpoint from `Config`.
    pub fn from_http_config(config: &Config, heartbeat_config: HeartbeatConfig) -> Self {
        Self::new(Arc::new(HttpHeartbeatTransport::new(config)), heartbeat_config)
    }

    /// Set callback to be called when connection is considered failed
    pub fn set_failure_callback<F>(&mut self, callback: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.failure_callback = Some(Arc::new(callback));
    }

    /// Start the heartbeat service
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            return Ok(()); // Already running
        }

        info!("Starting heartbeat service (interval: {:?}, timeout: {:?})",
              self.config.interval, self.config.timeout);

        self.is_running.store(true, Ordering::Relaxed);

        self.start_heartbeat_task().await;

        Ok(())
    }

    /// Stop the heartbeat service
    pub async fn stop(&mut self) {
        info!("Stopping heartbeat service");

        self.is_running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.heartbeat_handle.take() {
            handle.abort();
        }
    }

    /// Get current heartbeat statistics
    pub async fn get_stats(&self) -> HeartbeatStats {
        self.stats.lock().await.clone()
    }

    /// Check if heartbeat service considers the connection healthy
    pub async fn is_healthy(&self) -> bool {
        let stats = self.stats.lock().await;
        stats.is_healthy(self.config.max_missed)
    }

    /// Manually trigger a heartbeat (useful for testing)
    pub async fn trigger_heartbeat(&self) -> Result<()> {
        if !self.is_running.load(Ordering::Relaxed) {
            return Err(Error::Connection("Heartbeat service not running".to_string()));
        }

        self.send_probe().await
    }

    /// Start the heartbeat sender task
    async fn start_heartbeat_task(&mut self) {
        let transport = self.transport.clone();
        let config = self.config.clone();
        let is_running = self.is_running.clone();
        let stats = self.stats.clone();
        let failure_callback = self.failure_callback.clone();

        let handle = tokio::spawn(async move {
            let mut heartbeat_interval = interval(config.interval);

            while is_running.load(Ordering::Relaxed) {
                heartbeat_interval.tick().await;

                let sent_at = Instant::now();
                let probe = BrpRequest {
                    method: builtin_methods::RPC_DISCOVER_METHOD.to_string(),
                    id: Some(Value::from(Uuid::new_v4().to_string())),
                    params: None,
                };

                {
                    let mut stats_guard = stats.lock().await;
                    stats_guard.total_pings_sent += 1;
                }

                match transport.send_request(&probe).await {
                    Ok(_response) => {
                        let rtt = sent_at.elapsed();
                        let mut stats_guard = stats.lock().await;
                        stats_guard.total_pongs_received += 1;
                        stats_guard.consecutive_failures = 0;
                        stats_guard.last_successful_heartbeat = Some(SystemTime::now());

                        if rtt > stats_guard.max_round_trip_time {
                            stats_guard.max_round_trip_time = rtt;
                        }

                        let total = stats_guard.total_pongs_received;
                        let current_avg = stats_guard.avg_round_trip_time;
                        stats_guard.avg_round_trip_time = Duration::from_nanos(
                            ((current_avg.as_nanos() as u64 * (total - 1))
                                + rtt.as_nanos() as u64)
                                / total,
                        );

                        debug!("Heartbeat probe succeeded (RTT: {:?})", rtt);
                    }
                    Err(e) => {
                        error!("Heartbeat probe failed: {}", e);
                        let mut stats_guard = stats.lock().await;
                        stats_guard.consecutive_failures += 1;
                        stats_guard.missed_heartbeats += 1;

                        if stats_guard.consecutive_failures >= config.max_missed {
                            if let Some(callback) = &failure_callback {
                                callback();
                            }
                        }
                    }
                }
            }
        });

        self.heartbeat_handle = Some(handle);
    }

    /// Send a single probe on demand
    async fn send_probe(&self) -> Result<()> {
        let sent_at = Instant::now();
        let probe = BrpRequest {
            method: builtin_methods::RPC_DISCOVER_METHOD.to_string(),
            id: Some(Value::from(Uuid::new_v4().to_string())),
            params: None,
        };

        {
            let mut stats_guard = self.stats.lock().await;
            stats_guard.total_pings_sent += 1;
        }

        match self.transport.send_request(&probe).await {
            Ok(_) => {
                let mut stats_guard = self.stats.lock().await;
                stats_guard.total_pongs_received += 1;
                stats_guard.consecutive_failures = 0;
                stats_guard.last_successful_heartbeat = Some(SystemTime::now());
                let _ = sent_at;
                Ok(())
            }
            Err(e) => {
                let mut stats_guard = self.stats.lock().await;
                stats_guard.consecutive_failures += 1;
                stats_guard.missed_heartbeats += 1;
                Err(e)
            }
        }
    }
}

impl Drop for HeartbeatService {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.heartbeat_handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockTransport {
        fail: bool,
    }

    #[async_trait]
    impl HeartbeatTransport for MockTransport {
        async fn send_request(&self, request: &BrpRequest) -> Result<BrpResponse> {
            if self.fail {
                Err(Error::Connection("mock failure".to_string()))
            } else {
                Ok(BrpResponse::new(request.id.clone(), Ok(Value::Null)))
            }
        }
    }

    #[test]
    fn test_heartbeat_stats() {
        let mut stats = HeartbeatStats::default();
        stats.total_pings_sent = 10;
        stats.total_pongs_received = 8;

        assert_eq!(stats.success_rate(), 80.0);

        stats.consecutive_failures = 2;
        assert!(stats.is_healthy(3));
        assert!(!stats.is_healthy(2));
    }

    #[tokio::test]
    async fn test_heartbeat_service_creation() {
        let transport = Arc::new(MockTransport { fail: false });
        let config = HeartbeatConfig::default();

        let service = HeartbeatService::new(transport, config);
        let stats = service.get_stats().await;

        assert_eq!(stats.total_pings_sent, 0);
        assert_eq!(stats.success_rate(), 100.0);
    }

    #[tokio::test]
    async fn test_heartbeat_manual_probe() {
        let transport = Arc::new(MockTransport { fail: false });
        let config = HeartbeatConfig::default();

        let mut service = HeartbeatService::new(transport, config);
        service.start().await.unwrap();
        service.trigger_heartbeat().await.unwrap();
        service.stop().await;

        let stats = service.get_stats().await;
        assert!(stats.total_pings_sent >= 1);
        assert!(stats.total_pongs_received >= 1);
        assert!(stats.is_healthy(3));
    }
}
