use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs;
use tracing::{debug, error, info, warn};

use crate::error::{Error, Result};

/// A checkpoint represents a saved state that can be restored later
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Unique identifier for this checkpoint
    pub id: String,
    /// When this checkpoint was created
    pub timestamp: u64,
    /// Human-readable name for this checkpoint
    pub name: String,
    /// Description of what operation this checkpoint represents
    pub description: String,
    /// The operation type being checkpointed
    pub operation_type: String,
    /// Component that created this checkpoint
    pub component: String,
    /// Serialized state data
    pub state_data: serde_json::Value,
    /// Metadata about the checkpoint
    pub metadata: HashMap<String, String>,
    /// Whether this checkpoint can be automatically restored
    pub auto_restorable: bool,
    /// Expiration time (if any)
    pub expires_at: Option<u64>,
}

impl Checkpoint {
    pub fn new(
        name: &str,
        description: &str,
        operation_type: &str,
        component: &str,
        state_data: serde_json::Value,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp,
            name: name.to_string(),
            description: description.to_string(),
            operation_type: operation_type.to_string(),
            component: component.to_string(),
            state_data,
            metadata: HashMap::new(),
            auto_restorable: true,
            expires_at: None,
        }
    }

    /// Add metadata to the checkpoint
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Set expiration time (in seconds from now)
    pub fn with_expiration(mut self, seconds_from_now: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.expires_at = Some(now + seconds_from_now);
        self
    }

    /// Set whether this checkpoint can be automatically restored
    pub fn set_auto_restorable(mut self, auto_restorable: bool) -> Self {
        self.auto_restorable = auto_restorable;
        self
    }

    /// Check if this checkpoint has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            now > expires_at
        } else {
            false
        }
    }

    /// Get the age of this checkpoint in seconds
    pub fn age_seconds(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.timestamp)
    }
}

/// Configuration for the checkpoint manager
#[derive(Debug, Clone)]
pub struct CheckpointConfig {
    /// Maximum number of checkpoints to keep in memory
    pub max_checkpoints: usize,
    /// Maximum age of checkpoints in seconds before auto-cleanup
    pub max_age_seconds: u64,
    /// Whether to persist checkpoints to disk
    pub persist_to_disk: bool,
    /// Directory for storing checkpoint files
    pub storage_directory: String,
    /// How often to run cleanup (in seconds)
    pub cleanup_interval_seconds: u64,
    /// Maximum size of state data in bytes
    pub max_state_size_bytes: usize,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            max_checkpoints: 100,
            max_age_seconds: 24 * 60 * 60, // 24 hours
            persist_to_disk: true,
            storage_directory: "./checkpoints".to_string(),
            cleanup_interval_seconds: 60 * 60,      // 1 hour
            max_state_size_bytes: 10 * 1024 * 1024, // 10MB
        }
    }
}

/// Manager for creating and restoring checkpoints
#[derive(Debug)]
pub struct CheckpointManager {
    config: CheckpointConfig,
    checkpoints: std::sync::Arc<std::sync::RwLock<HashMap<String, Checkpoint>>>,
    cleanup_handle: Option<tokio::task::JoinHandle<()>>,
    shutdown_tx: Option<tokio::sync::mpsc::Sender<()>>,
}

impl CheckpointManager {
    pub fn new(config: CheckpointConfig) -> Self {
        Self {
            config,
            checkpoints: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
            cleanup_handle: None,
            shutdown_tx: None,
        }
    }

    /// Start the checkpoint manager with automatic cleanup
    pub async fn start(&mut self) -> Result<()> {
        // Create storage directory if it doesn't exist
        if self.config.persist_to_disk {
            fs::create_dir_all(&self.config.storage_directory).await?;
            info!(
                "Checkpoint storage directory: {}",
                self.config.storage_directory
            );
        }

        // Load existing checkpoints from disk
        if self.config.persist_to_disk {
            if let Err(e) = self.load_checkpoints_from_disk().await {
                error!("Failed to load checkpoints from disk: {}", e);
            }
        }

        // Start cleanup task
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        let checkpoints = self.checkpoints.clone();
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(config.cleanup_interval_seconds));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        Self::cleanup_expired_checkpoints(&checkpoints, &config).await;
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Checkpoint manager cleanup shutting down");
                        break;
                    }
                }
            }
        });

        self.cleanup_handle = Some(handle);
        info!("Checkpoint manager started");
        Ok(())
    }

    /// Create a new checkpoint
    pub async fn create_checkpoint(&self, checkpoint: Checkpoint) -> Result<String> {
        // Validate state size
        let state_size = serde_json::to_vec(&checkpoint.state_data)?.len();
        if state_size > self.config.max_state_size_bytes {
            return Err(Error::Validation(format!(
                "Checkpoint state too large: {} bytes (max: {})",
                state_size, self.config.max_state_size_bytes
            )));
        }

        let checkpoint_id = checkpoint.id.clone();

        // Store in memory
        {
            let mut checkpoints = self.checkpoints.write().unwrap();

            // Remove oldest checkpoints if we're at the limit
            while checkpoints.len() >= self.config.max_checkpoints {
                if let Some((oldest_id, _)) = checkpoints
                    .iter()
                    .min_by_key(|(_, cp)| cp.timestamp)
                    .map(|(id, cp)| (id.clone(), cp.clone()))
                {
                    checkpoints.remove(&oldest_id);
                    warn!("Removed oldest checkpoint to make room: {}", oldest_id);
                }
            }

            checkpoints.insert(checkpoint_id.clone(), checkpoint.clone());
        }

        // Persist to disk if configured
        if self.config.persist_to_disk {
            if let Err(e) = self.save_checkpoint_to_disk(&checkpoint).await {
                error!("Failed to save checkpoint to disk: {}", e);
            }
        }

        info!(
            "Created checkpoint: {} ({})",
            checkpoint.name, checkpoint_id
        );
        Ok(checkpoint_id)
    }

    /// Restore a checkpoint by ID
    pub async fn restore_checkpoint(&self, checkpoint_id: &str) -> Result<Checkpoint> {
        let checkpoint = {
            let checkpoints = self.checkpoints.read().unwrap();
            checkpoints.get(checkpoint_id).cloned()
        };

        match checkpoint {
            Some(cp) => {
                if cp.is_expired() {
                    return Err(Error::Validation(format!(
                        "Checkpoint {checkpoint_id} has expired"
                    )));
                }

                info!("Restoring checkpoint: {} ({})", cp.name, checkpoint_id);
                Ok(cp)
            }
            None => {
                // Try loading from disk if not in memory
                if self.config.persist_to_disk {
                    if let Ok(cp) = self.load_checkpoint_from_disk(checkpoint_id).await {
                        if !cp.is_expired() {
                            // Add back to memory
                            let mut checkpoints = self.checkpoints.write().unwrap();
                            checkpoints.insert(checkpoint_id.to_string(), cp.clone());

                            info!(
                                "Restored checkpoint from disk: {} ({})",
                                cp.name, checkpoint_id
                            );
                            return Ok(cp);
                        }
                    }
                }

                Err(Error::Validation(format!(
                    "Checkpoint not found: {checkpoint_id}"
                )))
            }
        }
    }

    /// Get all available checkpoints
    pub async fn list_checkpoints(&self) -> Vec<Checkpoint> {
        let checkpoints = self.checkpoints.read().unwrap();
        checkpoints.values().cloned().collect()
    }

    /// Get checkpoints by operation type
    pub async fn list_checkpoints_by_operation(&self, operation_type: &str) -> Vec<Checkpoint> {
        let checkpoints = self.checkpoints.read().unwrap();
        checkpoints
            .values()
            .filter(|cp| cp.operation_type == operation_type)
            .cloned()
            .collect()
    }

    /// Get checkpoints by component
    pub async fn list_checkpoints_by_component(&self, component: &str) -> Vec<Checkpoint> {
        let checkpoints = self.checkpoints.read().unwrap();
        checkpoints
            .values()
            .filter(|cp| cp.component == component)
            .cloned()
            .collect()
    }

    /// Delete a checkpoint
    pub async fn delete_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        // Remove from memory
        let removed = {
            let mut checkpoints = self.checkpoints.write().unwrap();
            checkpoints.remove(checkpoint_id).is_some()
        };

        // Remove from disk if configured
        if self.config.persist_to_disk {
            let file_path = self.get_checkpoint_file_path(checkpoint_id);
            if fs::metadata(&file_path).await.is_ok() {
                fs::remove_file(&file_path).await?;
            }
        }

        if removed {
            info!("Deleted checkpoint: {}", checkpoint_id);
            Ok(())
        } else {
            Err(Error::Validation(format!(
                "Checkpoint not found: {checkpoint_id}"
            )))
        }
    }

    /// Get checkpoint statistics
    pub async fn get_statistics(&self) -> CheckpointStats {
        let checkpoints = self.checkpoints.read().unwrap();

        let mut stats = CheckpointStats {
            total_count: checkpoints.len(),
            by_operation_type: HashMap::new(),
            by_component: HashMap::new(),
            expired_count: 0,
            auto_restorable_count: 0,
            oldest_timestamp: None,
            newest_timestamp: None,
        };

        for checkpoint in checkpoints.values() {
            // Count by operation type
            *stats
                .by_operation_type
                .entry(checkpoint.operation_type.clone())
                .or_insert(0) += 1;

            // Count by component
            *stats
                .by_component
                .entry(checkpoint.component.clone())
                .or_insert(0) += 1;

            // Count expired and auto-restorable
            if checkpoint.is_expired() {
                stats.expired_count += 1;
            }
            if checkpoint.auto_restorable {
                stats.auto_restorable_count += 1;
            }

            // Track timestamps
            if stats.oldest_timestamp.is_none()
                || Some(checkpoint.timestamp) < stats.oldest_timestamp
            {
                stats.oldest_timestamp = Some(checkpoint.timestamp);
            }
            if stats.newest_timestamp.is_none()
                || Some(checkpoint.timestamp) > stats.newest_timestamp
            {
                stats.newest_timestamp = Some(checkpoint.timestamp);
            }
        }

        stats
    }

    async fn cleanup_expired_checkpoints(
        checkpoints: &std::sync::Arc<std::sync::RwLock<HashMap<String, Checkpoint>>>,
        config: &CheckpointConfig,
    ) {
        let max_age = config.max_age_seconds;
        let mut to_remove = Vec::new();

        {
            let checkpoints_guard = checkpoints.read().unwrap();
            for (id, checkpoint) in checkpoints_guard.iter() {
                if checkpoint.is_expired() || checkpoint.age_seconds() > max_age {
                    to_remove.push(id.clone());
                }
            }
        }

        if !to_remove.is_empty() {
            let mut checkpoints_guard = checkpoints.write().unwrap();
            for id in &to_remove {
                checkpoints_guard.remove(id);
            }
            info!("Cleaned up {} expired checkpoints", to_remove.len());
        }
    }

    async fn save_checkpoint_to_disk(&self, checkpoint: &Checkpoint) -> Result<()> {
        let file_path = self.get_checkpoint_file_path(&checkpoint.id);
        let data = serde_json::to_string_pretty(checkpoint)?;
        fs::write(&file_path, data).await?;
        debug!("Saved checkpoint to disk: {}", file_path.display());
        Ok(())
    }

    async fn load_checkpoint_from_disk(&self, checkpoint_id: &str) -> Result<Checkpoint> {
        let file_path = self.get_checkpoint_file_path(checkpoint_id);
        let data = fs::read_to_string(&file_path).await?;
        let checkpoint: Checkpoint = serde_json::from_str(&data)?;
        Ok(checkpoint)
    }

    async fn load_checkpoints_from_disk(&self) -> Result<()> {
        let storage_dir = Path::new(&self.config.storage_directory);
        if !storage_dir.exists() {
            return Ok(());
        }

        let mut entries = fs::read_dir(storage_dir).await?;
        let mut loaded_count = 0;

        while let Some(entry) = entries.next_entry().await? {
            if let Some(file_name) = entry.file_name().to_str() {
                if file_name.ends_with(".json") {
                    let checkpoint_id = file_name.trim_end_matches(".json");

                    match self.load_checkpoint_from_disk(checkpoint_id).await {
                        Ok(checkpoint) => {
                            if !checkpoint.is_expired() {
                                let mut checkpoints = self.checkpoints.write().unwrap();
                                checkpoints.insert(checkpoint.id.clone(), checkpoint);
                                loaded_count += 1;
                            } else {
                                // Remove expired checkpoint file
                                let _ = fs::remove_file(entry.path()).await;
                            }
                        }
                        Err(e) => {
                            warn!("Failed to load checkpoint {}: {}", checkpoint_id, e);
                        }
                    }
                }
            }
        }

        info!("Loaded {} checkpoints from disk", loaded_count);
        Ok(())
    }

    fn get_checkpoint_file_path(&self, checkpoint_id: &str) -> std::path::PathBuf {
        // Sanitize checkpoint ID to prevent path traversal
        let sanitized_id = checkpoint_id
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect::<String>();

        if sanitized_id.is_empty() {
            panic!("Invalid checkpoint ID: {checkpoint_id}");
        }

        std::path::Path::new(&self.config.storage_directory).join(format!("{sanitized_id}.json"))
    }

    /// Shutdown the checkpoint manager
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()).await;
        }

        if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
        }

        // Save all checkpoints to disk if configured
        if self.config.persist_to_disk {
            let checkpoints = self.checkpoints.read().unwrap();
            for checkpoint in checkpoints.values() {
                if let Err(e) = self.save_checkpoint_to_disk(checkpoint).await {
                    error!("Failed to save checkpoint during shutdown: {}", e);
                }
            }
        }

        info!("Checkpoint manager shutdown complete");
        Ok(())
    }
}

/// Statistics about checkpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointStats {
    pub total_count: usize,
    pub by_operation_type: HashMap<String, usize>,
    pub by_component: HashMap<String, usize>,
    pub expired_count: usize,
    pub auto_restorable_count: usize,
    pub oldest_timestamp: Option<u64>,
    pub newest_timestamp: Option<u64>,
}

impl Drop for CheckpointManager {
    fn drop(&mut self) {
        if self.cleanup_handle.is_some() {
            warn!("CheckpointManager dropped without proper shutdown");
        }
    }
}
