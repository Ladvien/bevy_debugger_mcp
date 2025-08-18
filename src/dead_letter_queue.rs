use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::error::{ErrorContext, Result};

/// Failed operation record for dead letter queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedOperation {
    /// Unique ID for this failed operation
    pub id: String,
    /// When the operation was first attempted
    pub original_timestamp: u64,
    /// When it was added to the dead letter queue
    pub failed_timestamp: u64,
    /// The operation that failed
    pub operation: String,
    /// Component that performed the operation
    pub component: String,
    /// Number of retry attempts made
    pub retry_count: u32,
    /// The error context from the final failure
    pub error_context: ErrorContext,
    /// The original request data (serialized JSON)
    pub request_data: serde_json::Value,
    /// Reason for final failure
    pub failure_reason: String,
    /// Whether this operation can still be retried manually
    pub can_retry: bool,
}

impl FailedOperation {
    pub fn new(
        operation: &str,
        component: &str,
        retry_count: u32,
        error_context: ErrorContext,
        request_data: serde_json::Value,
        failure_reason: &str,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| {
                tracing::warn!("Failed to get system time for failed operation, using epoch");
                std::time::Duration::from_secs(0)
            })
            .as_secs();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            original_timestamp: now,
            failed_timestamp: now,
            operation: operation.to_string(),
            component: component.to_string(),
            retry_count,
            error_context,
            request_data,
            failure_reason: failure_reason.to_string(),
            can_retry: true,
        }
    }
}

/// Configuration for the dead letter queue
#[derive(Debug, Clone)]
pub struct DeadLetterConfig {
    /// Maximum number of failed operations to keep in memory
    pub max_size: usize,
    /// How long to keep failed operations before purging (in seconds)
    pub retention_period_secs: u64,
    /// Whether to persist to disk
    pub persist_to_disk: bool,
    /// Path for disk persistence (if enabled)
    pub persistence_path: Option<String>,
    /// How often to run cleanup (in seconds)
    pub cleanup_interval_secs: u64,
}

impl Default for DeadLetterConfig {
    fn default() -> Self {
        Self {
            max_size: 1000,
            retention_period_secs: 24 * 60 * 60, // 24 hours
            persist_to_disk: false,
            persistence_path: None,
            cleanup_interval_secs: 60 * 60, // 1 hour
        }
    }
}

/// Dead letter queue for managing permanently failed operations
#[derive(Debug)]
pub struct DeadLetterQueue {
    config: DeadLetterConfig,
    queue: Arc<RwLock<VecDeque<FailedOperation>>>,
    cleanup_handle: Option<tokio::task::JoinHandle<()>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl DeadLetterQueue {
    pub fn new(config: DeadLetterConfig) -> Self {
        Self {
            config,
            queue: Arc::new(RwLock::new(VecDeque::new())),
            cleanup_handle: None,
            shutdown_tx: None,
        }
    }

    /// Start the dead letter queue with automatic cleanup
    pub async fn start(&mut self) -> Result<()> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        let queue = self.queue.clone();
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(config.cleanup_interval_secs));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        Self::cleanup_expired(&queue, &config).await;
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Dead letter queue cleanup shutting down");
                        break;
                    }
                }
            }
        });

        self.cleanup_handle = Some(handle);
        info!(
            "Dead letter queue started with cleanup interval: {}s",
            self.config.cleanup_interval_secs
        );
        Ok(())
    }

    /// Add a failed operation to the dead letter queue
    pub async fn add_failed_operation(&self, failed_operation: FailedOperation) -> Result<()> {
        let mut queue = self.queue.write().await;

        // Check if we need to make room
        while queue.len() >= self.config.max_size {
            if let Some(oldest) = queue.pop_front() {
                warn!(
                    "Dead letter queue full, dropping oldest operation: {}",
                    oldest.id
                );
            }
        }

        info!(
            "Adding failed operation to dead letter queue: {} (retry count: {})",
            failed_operation.operation, failed_operation.retry_count
        );

        queue.push_back(failed_operation);

        // Persist to disk if configured
        if self.config.persist_to_disk {
            if let Err(e) = self.persist_to_disk().await {
                error!("Failed to persist dead letter queue to disk: {}", e);
            }
        }

        Ok(())
    }

    /// Get all failed operations
    pub async fn get_failed_operations(&self) -> Vec<FailedOperation> {
        self.queue.read().await.iter().cloned().collect()
    }

    /// Get failed operations by component
    pub async fn get_failed_operations_by_component(
        &self,
        component: &str,
    ) -> Vec<FailedOperation> {
        self.queue
            .read()
            .await
            .iter()
            .filter(|op| op.component == component)
            .cloned()
            .collect()
    }

    /// Get failed operations by operation type
    pub async fn get_failed_operations_by_type(&self, operation: &str) -> Vec<FailedOperation> {
        self.queue
            .read()
            .await
            .iter()
            .filter(|op| op.operation == operation)
            .cloned()
            .collect()
    }

    /// Remove a failed operation by ID (for manual retry or dismissal)
    pub async fn remove_failed_operation(&self, id: &str) -> Result<Option<FailedOperation>> {
        let mut queue = self.queue.write().await;

        if let Some(pos) = queue.iter().position(|op| op.id == id) {
            let operation = queue.remove(pos).unwrap();
            info!("Removed failed operation from dead letter queue: {}", id);
            Ok(Some(operation))
        } else {
            Ok(None)
        }
    }

    /// Get statistics about the dead letter queue
    pub async fn get_statistics(&self) -> DeadLetterStats {
        let queue = self.queue.read().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| {
                tracing::warn!("Failed to get system time for failed operation, using epoch");
                std::time::Duration::from_secs(0)
            })
            .as_secs();

        let mut stats = DeadLetterStats {
            total_count: queue.len(),
            by_component: std::collections::HashMap::new(),
            by_operation: std::collections::HashMap::new(),
            oldest_timestamp: None,
            newest_timestamp: None,
            total_retry_attempts: 0,
        };

        for operation in queue.iter() {
            // Count by component
            *stats
                .by_component
                .entry(operation.component.clone())
                .or_insert(0) += 1;

            // Count by operation type
            *stats
                .by_operation
                .entry(operation.operation.clone())
                .or_insert(0) += 1;

            // Track timestamps
            if stats.oldest_timestamp.is_none()
                || Some(operation.failed_timestamp) < stats.oldest_timestamp
            {
                stats.oldest_timestamp = Some(operation.failed_timestamp);
            }
            if stats.newest_timestamp.is_none()
                || Some(operation.failed_timestamp) > stats.newest_timestamp
            {
                stats.newest_timestamp = Some(operation.failed_timestamp);
            }

            stats.total_retry_attempts += operation.retry_count;
        }

        stats
    }

    /// Clean up expired operations
    async fn cleanup_expired(
        queue: &Arc<RwLock<VecDeque<FailedOperation>>>,
        config: &DeadLetterConfig,
    ) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| {
                tracing::warn!("Failed to get system time for failed operation, using epoch");
                std::time::Duration::from_secs(0)
            })
            .as_secs();

        let cutoff = now.saturating_sub(config.retention_period_secs);

        let mut queue_guard = queue.write().await;
        let original_size = queue_guard.len();

        // Remove expired operations (those older than retention period)
        queue_guard.retain(|op| op.failed_timestamp > cutoff);

        let removed = original_size - queue_guard.len();
        if removed > 0 {
            info!(
                "Cleaned up {} expired operations from dead letter queue",
                removed
            );
        }

        debug!(
            "Dead letter queue cleanup complete. Current size: {}",
            queue_guard.len()
        );
    }

    /// Persist the queue to disk (if configured)
    async fn persist_to_disk(&self) -> Result<()> {
        if let Some(ref path) = self.config.persistence_path {
            let queue = self.queue.read().await;
            let data = serde_json::to_string_pretty(&*queue)?;
            tokio::fs::write(path, data).await?;
            debug!("Persisted dead letter queue to disk: {}", path);
        }
        Ok(())
    }

    /// Load the queue from disk (if configured and file exists)
    pub async fn load_from_disk(&self) -> Result<()> {
        if let Some(ref path) = self.config.persistence_path {
            if tokio::fs::metadata(path).await.is_ok() {
                let data = tokio::fs::read_to_string(path).await?;
                let operations: VecDeque<FailedOperation> = serde_json::from_str(&data)?;

                let mut queue = self.queue.write().await;
                *queue = operations;

                info!(
                    "Loaded {} operations from dead letter queue disk file: {}",
                    queue.len(),
                    path
                );
            }
        }
        Ok(())
    }

    /// Shutdown the dead letter queue
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()).await;
        }

        if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
        }

        // Final persist to disk if configured
        if self.config.persist_to_disk {
            if let Err(e) = self.persist_to_disk().await {
                error!("Failed to persist dead letter queue during shutdown: {}", e);
            }
        }

        info!("Dead letter queue shutdown complete");
        Ok(())
    }
}

/// Statistics about the dead letter queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterStats {
    pub total_count: usize,
    pub by_component: std::collections::HashMap<String, usize>,
    pub by_operation: std::collections::HashMap<String, usize>,
    pub oldest_timestamp: Option<u64>,
    pub newest_timestamp: Option<u64>,
    pub total_retry_attempts: u32,
}

impl Drop for DeadLetterQueue {
    fn drop(&mut self) {
        if self.cleanup_handle.is_some() {
            warn!("DeadLetterQueue dropped without proper shutdown");
        }
    }
}
