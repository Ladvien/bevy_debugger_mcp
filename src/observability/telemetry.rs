/*
 * Bevy Debugger MCP - Telemetry Service
 * Copyright (C) 2025 ladvien
 *
 * Custom telemetry and data collection for advanced monitoring
 */

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, mpsc};
use tracing::{error, info, warn};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::{Result, Error};

/// Telemetry event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TelemetryEvent {
    /// MCP operation completed
    McpOperation {
        tool_name: String,
        duration_ms: u64,
        success: bool,
        error_message: Option<String>,
    },
    /// BRP connection event
    BrpConnection {
        event_type: String, // "connected", "disconnected", "error"
        details: String,
    },
    /// System performance metrics
    SystemMetrics {
        cpu_usage: f64,
        memory_usage_mb: u64,
        disk_usage_mb: u64,
        network_connections: u32,
    },
    /// Custom user-defined event
    Custom {
        event_name: String,
        data: serde_json::Value,
    },
}

/// Telemetry data point with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryDataPoint {
    pub timestamp: u64,
    pub event: TelemetryEvent,
    pub session_id: String,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl TelemetryDataPoint {
    pub fn new(event: TelemetryEvent, session_id: String) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            event,
            session_id,
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: &str, value: serde_json::Value) -> Self {
        self.metadata.insert(key.to_string(), value);
        self
    }
}

/// Telemetry aggregation window
#[derive(Debug, Clone)]
pub struct TelemetryWindow {
    pub start_time: Instant,
    pub duration: Duration,
    pub events: Vec<TelemetryDataPoint>,
}

impl TelemetryWindow {
    pub fn new(duration: Duration) -> Self {
        Self {
            start_time: Instant::now(),
            duration,
            events: Vec::new(),
        }
    }

    pub fn add_event(&mut self, event: TelemetryDataPoint) {
        self.events.push(event);
    }

    pub fn is_expired(&self) -> bool {
        self.start_time.elapsed() > self.duration
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}

/// Main telemetry service
pub struct TelemetryService {
    config: Config,
    session_id: String,
    event_sender: mpsc::UnboundedSender<TelemetryDataPoint>,
    current_window: Arc<RwLock<TelemetryWindow>>,
    historical_windows: Arc<RwLock<Vec<TelemetryWindow>>>,
    window_duration: Duration,
    max_history_windows: usize,
}

impl TelemetryService {
    /// Create a new telemetry service
    pub async fn new(config: &Config) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let session_id = uuid::Uuid::new_v4().to_string();
        let window_duration = Duration::from_secs(60); // 1-minute windows
        let max_history_windows = 1440; // 24 hours of 1-minute windows

        let current_window = Arc::new(RwLock::new(TelemetryWindow::new(window_duration)));
        let historical_windows = Arc::new(RwLock::new(Vec::new()));

        // Start event processing task
        let current_window_clone = current_window.clone();
        let historical_windows_clone = historical_windows.clone();
        let window_duration_clone = window_duration;
        let max_history_clone = max_history_windows;

        tokio::spawn(async move {
            Self::process_events(
                event_receiver,
                current_window_clone,
                historical_windows_clone,
                window_duration_clone,
                max_history_clone,
            ).await;
        });

        Ok(Self {
            config: config.clone(),
            session_id,
            event_sender,
            current_window,
            historical_windows,
            window_duration,
            max_history_windows,
        })
    }

    /// Start telemetry service
    pub async fn start(&self) -> Result<()> {
        info!("Starting telemetry service with session ID: {}", self.session_id);

        // Record service start event
        self.record_custom_event("service_start", serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "session_id": self.session_id
        })).await?;

        info!("Telemetry service started");
        Ok(())
    }

    /// Shutdown telemetry service
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down telemetry service");

        // Record service shutdown event
        if let Err(e) = self.record_custom_event("service_shutdown", serde_json::json!({})).await {
            warn!("Failed to record service shutdown event: {}", e);
        }

        // Final window rotation
        self.rotate_window().await?;

        info!("Telemetry service shutdown complete");
        Ok(())
    }

    /// Record MCP operation event
    pub async fn record_mcp_operation(
        &self,
        tool_name: &str,
        duration: Duration,
        success: bool,
        error_message: Option<String>,
    ) -> Result<()> {
        let event = TelemetryEvent::McpOperation {
            tool_name: tool_name.to_string(),
            duration_ms: duration.as_millis() as u64,
            success,
            error_message,
        };

        self.send_event(event).await
    }

    /// Record BRP connection event
    pub async fn record_brp_event(&self, event_type: &str, details: &str) -> Result<()> {
        let event = TelemetryEvent::BrpConnection {
            event_type: event_type.to_string(),
            details: details.to_string(),
        };

        self.send_event(event).await
    }

    /// Record system metrics
    pub async fn record_system_metrics(
        &self,
        cpu_usage: f64,
        memory_usage_mb: u64,
        disk_usage_mb: u64,
        network_connections: u32,
    ) -> Result<()> {
        let event = TelemetryEvent::SystemMetrics {
            cpu_usage,
            memory_usage_mb,
            disk_usage_mb,
            network_connections,
        };

        self.send_event(event).await
    }

    /// Record custom event
    pub async fn record_custom_event(
        &self,
        event_name: &str,
        data: serde_json::Value,
    ) -> Result<()> {
        let event = TelemetryEvent::Custom {
            event_name: event_name.to_string(),
            data,
        };

        self.send_event(event).await
    }

    /// Send telemetry event to processing queue
    async fn send_event(&self, event: TelemetryEvent) -> Result<()> {
        let data_point = TelemetryDataPoint::new(event, self.session_id.clone());

        self.event_sender.send(data_point)
            .map_err(|e| Error::Config(format!("Failed to send telemetry event: {}", e)))?;

        Ok(())
    }

    /// Get current telemetry statistics
    pub async fn get_statistics(&self) -> TelemetryStatistics {
        let current_window = self.current_window.read().await;
        let historical_windows = self.historical_windows.read().await;

        let total_events: usize = historical_windows.iter()
            .map(|w| w.event_count())
            .sum::<usize>() + current_window.event_count();

        let total_mcp_operations = self.count_events_by_type(&current_window, &historical_windows, "mcp_operation").await;
        let total_brp_events = self.count_events_by_type(&current_window, &historical_windows, "brp_connection").await;

        TelemetryStatistics {
            session_id: self.session_id.clone(),
            total_events,
            total_mcp_operations,
            total_brp_events,
            current_window_events: current_window.event_count(),
            historical_windows_count: historical_windows.len(),
            window_duration_seconds: self.window_duration.as_secs(),
        }
    }

    /// Get recent events for analysis
    pub async fn get_recent_events(&self, limit: usize) -> Vec<TelemetryDataPoint> {
        let current_window = self.current_window.read().await;
        let historical_windows = self.historical_windows.read().await;

        let mut all_events = Vec::new();
        
        // Add current window events
        all_events.extend(current_window.events.clone());

        // Add events from recent windows
        for window in historical_windows.iter().rev() {
            all_events.extend(window.events.clone());
            if all_events.len() >= limit {
                break;
            }
        }

        // Sort by timestamp and limit
        all_events.sort_by_key(|e| e.timestamp);
        all_events.into_iter().rev().take(limit).collect()
    }

    /// Process events in background task
    async fn process_events(
        mut event_receiver: mpsc::UnboundedReceiver<TelemetryDataPoint>,
        current_window: Arc<RwLock<TelemetryWindow>>,
        historical_windows: Arc<RwLock<Vec<TelemetryWindow>>>,
        window_duration: Duration,
        max_history_windows: usize,
    ) {
        let mut window_check_interval = tokio::time::interval(Duration::from_secs(5));

        loop {
            tokio::select! {
                // Process incoming events
                event = event_receiver.recv() => {
                    if let Some(event) = event {
                        let mut current = current_window.write().await;
                        current.add_event(event);
                    } else {
                        // Channel closed, exit
                        break;
                    }
                }
                
                // Check for window rotation
                _ = window_check_interval.tick() => {
                    let should_rotate = {
                        let current = current_window.read().await;
                        current.is_expired()
                    };
                    
                    if should_rotate {
                        Self::rotate_window_internal(
                            &current_window,
                            &historical_windows,
                            window_duration,
                            max_history_windows,
                        ).await;
                    }
                }
            }
        }

        info!("Telemetry event processing task ended");
    }

    /// Rotate the current window to historical storage
    async fn rotate_window(&self) -> Result<()> {
        Self::rotate_window_internal(
            &self.current_window,
            &self.historical_windows,
            self.window_duration,
            self.max_history_windows,
        ).await;
        Ok(())
    }

    /// Internal window rotation logic
    async fn rotate_window_internal(
        current_window: &Arc<RwLock<TelemetryWindow>>,
        historical_windows: &Arc<RwLock<Vec<TelemetryWindow>>>,
        window_duration: Duration,
        max_history_windows: usize,
    ) {
        // Move current window to history
        let old_window = {
            let mut current = current_window.write().await;
            let old = std::mem::replace(&mut *current, TelemetryWindow::new(window_duration));
            old
        };

        // Only store windows that have events
        if old_window.event_count() > 0 {
            let mut history = historical_windows.write().await;
            history.push(old_window);

            // Limit history size
            while history.len() > max_history_windows {
                history.remove(0);
            }

            info!("Rotated telemetry window, {} historical windows stored", history.len());
        }
    }

    /// Count events of specific type across all windows
    async fn count_events_by_type(
        &self,
        current_window: &TelemetryWindow,
        historical_windows: &[TelemetryWindow],
        event_type: &str,
    ) -> usize {
        let mut count = 0;

        // Count in current window
        count += current_window.events.iter()
            .filter(|event| self.event_matches_type(&event.event, event_type))
            .count();

        // Count in historical windows
        for window in historical_windows {
            count += window.events.iter()
                .filter(|event| self.event_matches_type(&event.event, event_type))
                .count();
        }

        count
    }

    /// Check if an event matches a specific type
    fn event_matches_type(&self, event: &TelemetryEvent, event_type: &str) -> bool {
        match (event, event_type) {
            (TelemetryEvent::McpOperation { .. }, "mcp_operation") => true,
            (TelemetryEvent::BrpConnection { .. }, "brp_connection") => true,
            (TelemetryEvent::SystemMetrics { .. }, "system_metrics") => true,
            (TelemetryEvent::Custom { .. }, "custom") => true,
            _ => false,
        }
    }
}

/// Telemetry statistics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryStatistics {
    pub session_id: String,
    pub total_events: usize,
    pub total_mcp_operations: usize,
    pub total_brp_events: usize,
    pub current_window_events: usize,
    pub historical_windows_count: usize,
    pub window_duration_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_telemetry_service_creation() {
        let config = Config::default();
        let service = TelemetryService::new(&config).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_telemetry_event_recording() {
        let config = Config::default();
        let service = TelemetryService::new(&config).await.unwrap();
        
        // Record different types of events
        let result = service.record_mcp_operation("observe", Duration::from_millis(100), true, None).await;
        assert!(result.is_ok());

        let result = service.record_brp_event("connected", "Successfully connected to BRP").await;
        assert!(result.is_ok());

        let result = service.record_system_metrics(25.5, 512, 1024, 5).await;
        assert!(result.is_ok());

        let result = service.record_custom_event("test_event", serde_json::json!({"key": "value"})).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_telemetry_window_rotation() {
        let window = TelemetryWindow::new(Duration::from_millis(100));
        
        // Window should not be expired immediately
        assert!(!window.is_expired());
        
        // Wait and check expiration
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(window.is_expired());
    }

    #[tokio::test]
    async fn test_telemetry_statistics() {
        let config = Config::default();
        let service = TelemetryService::new(&config).await.unwrap();
        
        // Record some events
        service.record_mcp_operation("observe", Duration::from_millis(50), true, None).await.unwrap();
        service.record_brp_event("connected", "test").await.unwrap();
        
        // Give some time for events to be processed
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        let stats = service.get_statistics().await;
        assert!(stats.total_events >= 2);
        assert!(stats.total_mcp_operations >= 1);
        assert!(stats.total_brp_events >= 1);
    }

    #[test]
    fn test_telemetry_data_point_creation() {
        let event = TelemetryEvent::Custom {
            event_name: "test".to_string(),
            data: serde_json::json!({"test": true}),
        };
        
        let data_point = TelemetryDataPoint::new(event, "test_session".to_string())
            .with_metadata("test_key", serde_json::Value::from("test_value"));
        
        assert_eq!(data_point.session_id, "test_session");
        assert_eq!(data_point.metadata.get("test_key").unwrap(), "test_value");
    }
}