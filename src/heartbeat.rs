/*
 * Production-Grade Heartbeat Service for BRP Connections
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

use crate::config::HeartbeatConfig;
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, Mutex};
use tokio::time::{interval, timeout};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Heartbeat message types
#[derive(Debug, Clone)]
pub enum HeartbeatMessage {
    Ping { id: Uuid, timestamp: u64 },
    Pong { id: Uuid, timestamp: u64 },
}

impl HeartbeatMessage {
    fn to_json(&self) -> String {
        match self {
            HeartbeatMessage::Ping { id, timestamp } => {
                format!(r#"{{"type":"heartbeat_ping","id":"{}","timestamp":{}}}"#, id, timestamp)
            }
            HeartbeatMessage::Pong { id, timestamp } => {
                format!(r#"{{"type":"heartbeat_pong","id":"{}","timestamp":{}}}"#, id, timestamp)
            }
        }
    }
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

/// Production-grade heartbeat service for maintaining connection health
pub struct HeartbeatService<T>
where
    T: SinkExt<Message> + StreamExt + Unpin + Send + 'static,
    T::Error: std::fmt::Debug + Send,
{
    config: HeartbeatConfig,
    websocket: Arc<Mutex<T>>,
    stats: Arc<Mutex<HeartbeatStats>>,
    is_running: Arc<AtomicBool>,
    pending_pings: Arc<Mutex<std::collections::HashMap<Uuid, Instant>>>,
    heartbeat_handle: Option<tokio::task::JoinHandle<()>>,
    response_handle: Option<tokio::task::JoinHandle<()>>,
    failure_callback: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl<T> HeartbeatService<T>
where
    T: SinkExt<Message> + StreamExt<Item = Result<Message, T::Error>> + Unpin + Send + 'static,
    T::Error: std::fmt::Debug + Send,
{
    pub fn new(websocket: T, config: HeartbeatConfig) -> Self {
        Self {
            config,
            websocket: Arc::new(Mutex::new(websocket)),
            stats: Arc::new(Mutex::new(HeartbeatStats::default())),
            is_running: Arc::new(AtomicBool::new(false)),
            pending_pings: Arc::new(Mutex::new(std::collections::HashMap::new())),
            heartbeat_handle: None,
            response_handle: None,
            failure_callback: None,
        }
    }

    /// Set callback to be called when connection is considered failed
    pub fn set_failure_callback<F>(&mut self, callback: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.failure_callback = Some(Arc::new(callback));
    }

    /// Start the heartbeat service
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.is_running.load(Ordering::Relaxed) {
            return Ok(()); // Already running
        }

        info!("Starting heartbeat service (interval: {:?}, timeout: {:?})", 
              self.config.interval, self.config.timeout);

        self.is_running.store(true, Ordering::Relaxed);

        // Start heartbeat sender task
        self.start_heartbeat_task().await;

        // Start response handler task
        self.start_response_handler().await;

        Ok(())
    }

    /// Stop the heartbeat service
    pub async fn stop(&mut self) {
        info!("Stopping heartbeat service");
        
        self.is_running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.heartbeat_handle.take() {
            handle.abort();
        }

        if let Some(handle) = self.response_handle.take() {
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
    pub async fn trigger_heartbeat(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.is_running.load(Ordering::Relaxed) {
            return Err("Heartbeat service not running".into());
        }

        self.send_ping().await
    }

    /// Start the heartbeat sender task
    async fn start_heartbeat_task(&mut self) {
        let websocket = self.websocket.clone();
        let config = self.config.clone();
        let is_running = self.is_running.clone();
        let stats = self.stats.clone();
        let pending_pings = self.pending_pings.clone();
        let failure_callback = self.failure_callback.clone();

        let handle = tokio::spawn(async move {
            let mut heartbeat_interval = interval(config.interval);
            
            while is_running.load(Ordering::Relaxed) {
                heartbeat_interval.tick().await;

                // Send ping
                let ping_id = Uuid::new_v4();
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                let ping_message = HeartbeatMessage::Ping { id: ping_id, timestamp };
                
                // Record pending ping
                {
                    let mut pending = pending_pings.lock().await;
                    pending.insert(ping_id, Instant::now());
                }

                // Send ping message
                let send_result = {
                    let mut ws = websocket.lock().await;
                    ws.send(Message::Text(ping_message.to_json())).await
                };

                match send_result {
                    Ok(_) => {
                        let mut stats_guard = stats.lock().await;
                        stats_guard.total_pings_sent += 1;
                        debug!("Heartbeat ping sent: {}", ping_id);
                    }
                    Err(e) => {
                        error!("Failed to send heartbeat ping: {:?}", e);
                        let mut stats_guard = stats.lock().await;
                        stats_guard.consecutive_failures += 1;
                        stats_guard.missed_heartbeats += 1;

                        // Check if we should trigger failure callback
                        if stats_guard.consecutive_failures >= config.max_missed {
                            if let Some(callback) = &failure_callback {
                                callback();
                            }
                        }
                    }
                }

                // Clean up expired pending pings
                {
                    let mut pending = pending_pings.lock().await;
                    let timeout_threshold = Instant::now() - config.timeout - Duration::from_secs(1);
                    pending.retain(|_, sent_at| *sent_at > timeout_threshold);
                }
            }
        });

        self.heartbeat_handle = Some(handle);
    }

    /// Start response handler for processing pong messages
    async fn start_response_handler(&mut self) {
        let websocket = self.websocket.clone();
        let is_running = self.is_running.clone();
        let stats = self.stats.clone();
        let pending_pings = self.pending_pings.clone();

        let handle = tokio::spawn(async move {
            while is_running.load(Ordering::Relaxed) {
                let message_result = {
                    let mut ws = websocket.lock().await;
                    
                    // Use timeout to prevent blocking indefinitely
                    timeout(Duration::from_millis(100), ws.next()).await
                };

                match message_result {
                    Ok(Some(Ok(Message::Text(text)))) => {
                        // Try to parse as heartbeat pong
                        if let Ok(pong) = Self::parse_heartbeat_message(&text) {
                            if let HeartbeatMessage::Pong { id, timestamp: _ } = pong {
                                // Check if we have a pending ping for this ID
                                let rtt = {
                                    let mut pending = pending_pings.lock().await;
                                    pending.remove(&id).map(|sent_at| sent_at.elapsed())
                                };

                                if let Some(round_trip_time) = rtt {
                                    let mut stats_guard = stats.lock().await;
                                    stats_guard.total_pongs_received += 1;
                                    stats_guard.consecutive_failures = 0;
                                    stats_guard.last_successful_heartbeat = Some(SystemTime::now());

                                    // Update RTT statistics
                                    if round_trip_time > stats_guard.max_round_trip_time {
                                        stats_guard.max_round_trip_time = round_trip_time;
                                    }

                                    // Calculate moving average RTT
                                    let total_responses = stats_guard.total_pongs_received;
                                    if total_responses == 1 {
                                        stats_guard.avg_round_trip_time = round_trip_time;
                                    } else {
                                        let current_avg = stats_guard.avg_round_trip_time;
                                        stats_guard.avg_round_trip_time = Duration::from_nanos(
                                            ((current_avg.as_nanos() as u64 * (total_responses - 1)) 
                                             + round_trip_time.as_nanos() as u64) / total_responses
                                        );
                                    }

                                    debug!("Heartbeat pong received: {} (RTT: {:?})", id, round_trip_time);
                                }
                            }
                        }
                    }
                    Ok(Some(Ok(_))) => {
                        // Other message types, ignore for heartbeat purposes
                    }
                    Ok(Some(Err(e))) => {
                        error!("WebSocket error in heartbeat response handler: {:?}", e);
                        break;
                    }
                    Ok(None) => {
                        warn!("WebSocket connection closed");
                        break;
                    }
                    Err(_) => {
                        // Timeout, continue
                    }
                }
            }
        });

        self.response_handle = Some(handle);
    }

    /// Send a ping message
    async fn send_ping(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ping_id = Uuid::new_v4();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let ping_message = HeartbeatMessage::Ping { id: ping_id, timestamp };
        
        // Record pending ping
        {
            let mut pending = self.pending_pings.lock().await;
            pending.insert(ping_id, Instant::now());
        }

        // Send ping
        let mut ws = self.websocket.lock().await;
        ws.send(Message::Text(ping_message.to_json())).await
            .map_err(|e| format!("Failed to send ping: {:?}", e))?;

        // Update stats
        let mut stats = self.stats.lock().await;
        stats.total_pings_sent += 1;

        Ok(())
    }

    /// Parse heartbeat message from JSON
    fn parse_heartbeat_message(text: &str) -> Result<HeartbeatMessage, serde_json::Error> {
        let value: serde_json::Value = serde_json::from_str(text)?;
        
        match value.get("type").and_then(|t| t.as_str()) {
            Some("heartbeat_pong") => {
                let id = value.get("id")
                    .and_then(|i| i.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .ok_or_else(|| serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid ping ID")))?;
                
                let timestamp = value.get("timestamp")
                    .and_then(|t| t.as_u64())
                    .unwrap_or(0);

                Ok(HeartbeatMessage::Pong { id, timestamp })
            }
            _ => Err(serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::InvalidData, "Not a heartbeat message")))
        }
    }
}

impl<T> Drop for HeartbeatService<T>
where
    T: SinkExt<Message> + StreamExt + Unpin + Send + 'static,
    T::Error: std::fmt::Debug + Send,
{
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        
        if let Some(handle) = self.heartbeat_handle.take() {
            handle.abort();
        }
        
        if let Some(handle) = self.response_handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{stream, Sink, Stream};
    use std::pin::Pin;
    use std::task::{Context, Poll};
    
    // Mock WebSocket for testing
    struct MockWebSocket {
        messages: Vec<Message>,
        send_error: bool,
    }

    impl MockWebSocket {
        fn new() -> Self {
            Self {
                messages: Vec::new(),
                send_error: false,
            }
        }
    }

    impl Sink<Message> for MockWebSocket {
        type Error = tokio_tungstenite::tungstenite::Error;

        fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
            if self.send_error {
                return Err(tokio_tungstenite::tungstenite::Error::ConnectionClosed);
            }
            self.messages.push(item);
            Ok(())
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    impl Stream for MockWebSocket {
        type Item = Result<Message, tokio_tungstenite::tungstenite::Error>;

        fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            Poll::Pending // No messages to receive in basic test
        }
    }

    impl Unpin for MockWebSocket {}

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

    #[test]
    fn test_heartbeat_message_serialization() {
        let ping = HeartbeatMessage::Ping { 
            id: Uuid::new_v4(), 
            timestamp: 1234567890 
        };
        
        let json = ping.to_json();
        assert!(json.contains("heartbeat_ping"));
        assert!(json.contains("1234567890"));
    }

    #[tokio::test]
    async fn test_heartbeat_service_creation() {
        let mock_ws = MockWebSocket::new();
        let config = HeartbeatConfig::default();
        
        let service = HeartbeatService::new(mock_ws, config);
        let stats = service.get_stats().await;
        
        assert_eq!(stats.total_pings_sent, 0);
        assert_eq!(stats.success_rate(), 100.0);
    }
}