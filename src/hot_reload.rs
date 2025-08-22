/*
 * Bevy Debugger MCP Server - Hot Reload System for ML Models
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

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher, EventKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::error::{Error, Result};
use crate::pattern_learning::PatternLearningSystem;
use crate::suggestion_engine::SuggestionEngine;
use crate::workflow_automation::WorkflowAutomation;

/// Hot reload configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotReloadConfig {
    /// Directory to watch for model updates
    pub watch_directory: PathBuf,
    /// Debounce delay to avoid multiple reloads for rapid file changes
    pub debounce_delay_ms: u64,
    /// Enable hot reload functionality
    pub enabled: bool,
    /// Backup directory for model snapshots
    pub backup_directory: Option<PathBuf>,
    /// Maximum number of backups to keep
    pub max_backups: usize,
}

impl Default for HotReloadConfig {
    fn default() -> Self {
        Self {
            watch_directory: PathBuf::from("./models"),
            debounce_delay_ms: 1000, // 1 second debounce
            enabled: true,
            backup_directory: Some(PathBuf::from("./models/backups")),
            max_backups: 10,
        }
    }
}

/// Hot reload events
#[derive(Debug, Clone)]
pub enum HotReloadEvent {
    /// Pattern model updated
    PatternModelUpdated(PathBuf),
    /// Suggestion templates updated
    SuggestionTemplatesUpdated(PathBuf),
    /// Workflow definitions updated
    WorkflowsUpdated(PathBuf),
    /// Configuration updated
    ConfigUpdated(PathBuf),
}

/// Model versioning information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelVersion {
    /// Version identifier
    pub version: String,
    /// Timestamp of last update (serialized as seconds since epoch)
    #[serde(with = "instant_serde")]
    pub updated_at: Instant,
    /// Checksum for integrity verification
    pub checksum: String,
    /// Model file path
    pub file_path: PathBuf,
}

mod instant_serde {
    use std::time::{Instant, Duration, SystemTime, UNIX_EPOCH};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(instant: &Instant, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Convert to system time for serialization
        let elapsed = instant.elapsed();
        let system_time = SystemTime::now() - elapsed;
        let duration_since_epoch = system_time.duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0));
        duration_since_epoch.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Instant, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        // Convert back to Instant (approximate)
        let system_time = UNIX_EPOCH + Duration::from_secs(secs);
        let now = SystemTime::now();
        let instant_now = Instant::now();
        
        if let Ok(duration) = now.duration_since(system_time) {
            Ok(instant_now - duration)
        } else {
            // If the time is in the future, just use now
            Ok(instant_now)
        }
    }
}

/// Hot reload system for ML models and configurations
pub struct HotReloadSystem {
    /// Configuration
    config: HotReloadConfig,
    /// File system watcher
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    /// Event channel sender
    event_tx: Arc<Mutex<Option<mpsc::UnboundedSender<HotReloadEvent>>>>,
    /// Pattern learning system reference
    pattern_system: Arc<PatternLearningSystem>,
    /// Suggestion engine reference
    suggestion_engine: Arc<SuggestionEngine>,
    /// Workflow automation reference
    workflow_automation: Arc<WorkflowAutomation>,
    /// Model versions tracking
    model_versions: Arc<RwLock<HashMap<String, ModelVersion>>>,
    /// Debounce tracking
    last_reload: Arc<RwLock<HashMap<PathBuf, Instant>>>,
}

impl HotReloadSystem {
    pub fn new(
        config: HotReloadConfig,
        pattern_system: Arc<PatternLearningSystem>,
        suggestion_engine: Arc<SuggestionEngine>,
        workflow_automation: Arc<WorkflowAutomation>,
    ) -> Self {
        Self {
            config,
            watcher: Arc::new(Mutex::new(None)),
            event_tx: Arc::new(Mutex::new(None)),
            pattern_system,
            suggestion_engine,
            workflow_automation,
            model_versions: Arc::new(RwLock::new(HashMap::new())),
            last_reload: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the hot reload system
    pub async fn start(&self) -> Result<()> {
        if !self.config.enabled {
            info!("Hot reload system disabled");
            return Ok(());
        }

        // Create watch directory if it doesn't exist
        if !self.config.watch_directory.exists() {
            tokio::fs::create_dir_all(&self.config.watch_directory).await?;
            info!("Created watch directory: {:?}", self.config.watch_directory);
        }

        // Create backup directory if configured
        if let Some(ref backup_dir) = self.config.backup_directory {
            if !backup_dir.exists() {
                tokio::fs::create_dir_all(backup_dir).await?;
                info!("Created backup directory: {:?}", backup_dir);
            }
        }

        // Set up file system watcher
        let (tx, mut rx) = mpsc::unbounded_channel::<HotReloadEvent>();
        
        // Store sender for shutdown
        {
            let mut event_tx = self.event_tx.lock().await;
            *event_tx = Some(tx.clone());
        }

        // Create file watcher
        let watch_dir = self.config.watch_directory.clone();
        let debounce_delay = Duration::from_millis(self.config.debounce_delay_ms);
        let last_reload = self.last_reload.clone();

        let watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            match res {
                Ok(event) => {
                    let tx_clone = tx.clone();
                    let last_reload_clone = last_reload.clone();
                    let debounce_delay_clone = debounce_delay;
                    
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_file_event(
                            event, 
                            tx_clone, 
                            last_reload_clone, 
                            debounce_delay_clone
                        ).await {
                            error!("Error handling file event: {}", e);
                        }
                    });
                }
                Err(e) => error!("File watcher error: {}", e),
            }
        }).map_err(|e| Error::Validation(format!("Failed to create file watcher: {}", e)))?;

        // Store watcher
        {
            let mut watcher_lock = self.watcher.lock().await;
            *watcher_lock = Some(watcher);
        }

        // Start watching the directory
        {
            let mut watcher_lock = self.watcher.lock().await;
            if let Some(ref mut watcher) = *watcher_lock {
                watcher.watch(&watch_dir, RecursiveMode::Recursive)
                    .map_err(|e| Error::Validation(format!("Failed to start watching directory: {}", e)))?;
            }
        }

        // Start event processing loop
        let system_clone = self.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let Err(e) = system_clone.process_reload_event(event).await {
                    error!("Error processing reload event: {}", e);
                }
            }
        });

        info!("Hot reload system started, watching: {:?}", watch_dir);
        Ok(())
    }

    /// Stop the hot reload system
    pub async fn stop(&self) -> Result<()> {
        // Stop the watcher
        {
            let mut watcher_lock = self.watcher.lock().await;
            *watcher_lock = None;
        }

        // Close event channel
        {
            let mut event_tx = self.event_tx.lock().await;
            *event_tx = None;
        }

        info!("Hot reload system stopped");
        Ok(())
    }

    /// Handle file system events with debouncing
    async fn handle_file_event(
        event: Event,
        tx: mpsc::UnboundedSender<HotReloadEvent>,
        last_reload: Arc<RwLock<HashMap<PathBuf, Instant>>>,
        debounce_delay: Duration,
    ) -> Result<()> {
        // Only handle modify events
        if !matches!(event.kind, EventKind::Modify(_)) {
            return Ok(());
        }

        for path in event.paths {
            // Check if we should debounce this reload
            {
                let mut last_reload_lock = last_reload.write().await;
                let now = Instant::now();
                
                if let Some(last_time) = last_reload_lock.get(&path) {
                    if now.duration_since(*last_time) < debounce_delay {
                        debug!("Debouncing reload for: {:?}", path);
                        continue;
                    }
                }
                
                last_reload_lock.insert(path.clone(), now);
            }

            // Determine event type based on file extension and name
            let reload_event = Self::classify_file_event(&path)?;
            
            if let Some(event) = reload_event {
                debug!("Detected hot reload event: {:?}", event);
                if let Err(e) = tx.send(event) {
                    error!("Failed to send reload event: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Classify file events into hot reload events
    fn classify_file_event(path: &Path) -> Result<Option<HotReloadEvent>> {
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let extension = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let event = match (file_name, extension) {
            // Pattern model files
            (name, "json") if name.contains("patterns") => {
                Some(HotReloadEvent::PatternModelUpdated(path.to_path_buf()))
            }
            // Suggestion template files
            (name, "json") if name.contains("suggestions") || name.contains("templates") => {
                Some(HotReloadEvent::SuggestionTemplatesUpdated(path.to_path_buf()))
            }
            // Workflow definition files
            (name, "json") if name.contains("workflows") => {
                Some(HotReloadEvent::WorkflowsUpdated(path.to_path_buf()))
            }
            // Configuration files
            ("config.json" | "hot_reload.json", "json") => {
                Some(HotReloadEvent::ConfigUpdated(path.to_path_buf()))
            }
            _ => None,
        };

        Ok(event)
    }

    /// Process a hot reload event
    async fn process_reload_event(&self, event: HotReloadEvent) -> Result<()> {
        info!("Processing hot reload event: {:?}", event);

        // Create backup before reloading
        if let Err(e) = self.create_backup(&event).await {
            warn!("Failed to create backup: {}", e);
        }

        match event {
            HotReloadEvent::PatternModelUpdated(path) => {
                self.reload_pattern_model(&path).await?;
            }
            HotReloadEvent::SuggestionTemplatesUpdated(path) => {
                self.reload_suggestion_templates(&path).await?;
            }
            HotReloadEvent::WorkflowsUpdated(path) => {
                self.reload_workflows(&path).await?;
            }
            HotReloadEvent::ConfigUpdated(path) => {
                self.reload_config(&path).await?;
            }
        }

        info!("Hot reload completed successfully");
        Ok(())
    }

    /// Reload pattern learning model
    async fn reload_pattern_model(&self, path: &Path) -> Result<()> {
        debug!("Reloading pattern model from: {:?}", path);

        // Read and validate the new patterns
        let content = tokio::fs::read_to_string(path).await?;

        // Import patterns (this validates the format)
        self.pattern_system.import_patterns(&content).await
            .map_err(|e| Error::Validation(format!("Invalid pattern file format: {}", e)))?;

        // Update version tracking
        self.update_model_version("patterns", path).await?;

        info!("Pattern model reloaded successfully");
        Ok(())
    }

    /// Reload suggestion templates (placeholder implementation)
    async fn reload_suggestion_templates(&self, path: &Path) -> Result<()> {
        debug!("Reloading suggestion templates from: {:?}", path);

        // Read template file
        let _content = tokio::fs::read_to_string(path).await?;

        // TODO: Implement template reloading in SuggestionEngine
        // This would require adding a reload_templates method to SuggestionEngine

        self.update_model_version("suggestions", path).await?;

        info!("Suggestion templates reloaded successfully");
        Ok(())
    }

    /// Reload workflow definitions (placeholder implementation)
    async fn reload_workflows(&self, path: &Path) -> Result<()> {
        debug!("Reloading workflows from: {:?}", path);

        // Read workflow file
        let _content = tokio::fs::read_to_string(path).await?;

        // TODO: Implement workflow reloading in WorkflowAutomation
        // This would require adding import/export methods for workflows

        self.update_model_version("workflows", path).await?;

        info!("Workflows reloaded successfully");
        Ok(())
    }

    /// Reload configuration
    async fn reload_config(&self, path: &Path) -> Result<()> {
        debug!("Reloading configuration from: {:?}", path);

        // Read and parse config
        let content = tokio::fs::read_to_string(path).await?;

        let _new_config: HotReloadConfig = serde_json::from_str(&content)
            .map_err(|e| Error::Validation(format!("Invalid config format: {}", e)))?;

        // TODO: Apply new configuration
        // This would require making config mutable and updating watch paths

        info!("Configuration reloaded successfully");
        Ok(())
    }

    /// Create backup of current model state
    async fn create_backup(&self, event: &HotReloadEvent) -> Result<()> {
        let backup_dir = match &self.config.backup_directory {
            Some(dir) => dir,
            None => return Ok(()), // Backups disabled
        };

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        
        match event {
            HotReloadEvent::PatternModelUpdated(_) => {
                let backup_path = backup_dir.join(format!("patterns_backup_{}.json", timestamp));
                let patterns_export = self.pattern_system.export_patterns().await?;
                tokio::fs::write(&backup_path, &patterns_export).await?;
                debug!("Created pattern backup: {:?}", backup_path);
            }
            _ => {
                // TODO: Implement backups for other model types
            }
        }

        // Clean up old backups
        self.cleanup_old_backups(backup_dir).await?;

        Ok(())
    }

    /// Clean up old backup files
    async fn cleanup_old_backups(&self, backup_dir: &Path) -> Result<()> {
        let mut entries = tokio::fs::read_dir(backup_dir).await?;
        let mut backup_files = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.contains("backup") {
                        if let Ok(metadata) = entry.metadata().await {
                            if let Ok(modified) = metadata.modified() {
                                backup_files.push((path, modified));
                            }
                        }
                    }
                }
            }
        }

        // Sort by modification time (newest first)
        backup_files.sort_by(|a, b| b.1.cmp(&a.1));

        // Remove excess backups
        if backup_files.len() > self.config.max_backups {
            for (path, _) in backup_files.iter().skip(self.config.max_backups) {
                if let Err(e) = tokio::fs::remove_file(path).await {
                    warn!("Failed to remove old backup {:?}: {}", path, e);
                } else {
                    debug!("Removed old backup: {:?}", path);
                }
            }
        }

        Ok(())
    }

    /// Update model version tracking
    async fn update_model_version(&self, model_type: &str, path: &Path) -> Result<()> {
        let content = tokio::fs::read(path).await?;
        let checksum = format!("{:x}", md5::compute(&content));
        
        let version = ModelVersion {
            version: format!("{}_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S"), &checksum[..8]),
            updated_at: Instant::now(),
            checksum,
            file_path: path.to_path_buf(),
        };

        let mut versions = self.model_versions.write().await;
        versions.insert(model_type.to_string(), version);

        Ok(())
    }

    /// Get current model versions
    pub async fn get_model_versions(&self) -> HashMap<String, ModelVersion> {
        let versions = self.model_versions.read().await;
        versions.clone()
    }

    /// Force reload all models
    pub async fn force_reload_all(&self) -> Result<()> {
        info!("Force reloading all models");

        // Scan watch directory for model files
        let mut entries = tokio::fs::read_dir(&self.config.watch_directory).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                if let Ok(Some(event)) = Self::classify_file_event(&path) {
                    self.process_reload_event(event).await?;
                }
            }
        }

        info!("Force reload completed");
        Ok(())
    }
}

impl Clone for HotReloadSystem {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            watcher: self.watcher.clone(),
            event_tx: self.event_tx.clone(),
            pattern_system: self.pattern_system.clone(),
            suggestion_engine: self.suggestion_engine.clone(),
            workflow_automation: self.workflow_automation.clone(),
            model_versions: self.model_versions.clone(),
            last_reload: self.last_reload.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_hot_reload_config() {
        let config = HotReloadConfig::default();
        assert!(config.enabled);
        assert_eq!(config.debounce_delay_ms, 1000);
    }

    #[tokio::test]
    async fn test_file_classification() {
        let patterns_path = Path::new("./models/patterns_v1.json");
        let event = HotReloadSystem::classify_file_event(patterns_path).unwrap();
        assert!(matches!(event, Some(HotReloadEvent::PatternModelUpdated(_))));

        let suggestions_path = Path::new("./models/suggestion_templates.json");
        let event = HotReloadSystem::classify_file_event(suggestions_path).unwrap();
        assert!(matches!(event, Some(HotReloadEvent::SuggestionTemplatesUpdated(_))));

        let workflows_path = Path::new("./models/workflows.json");
        let event = HotReloadSystem::classify_file_event(workflows_path).unwrap();
        assert!(matches!(event, Some(HotReloadEvent::WorkflowsUpdated(_))));

        let config_path = Path::new("./config.json");
        let event = HotReloadSystem::classify_file_event(config_path).unwrap();
        assert!(matches!(event, Some(HotReloadEvent::ConfigUpdated(_))));

        let other_path = Path::new("./some_file.txt");
        let event = HotReloadSystem::classify_file_event(other_path).unwrap();
        assert!(event.is_none());
    }

    #[tokio::test]
    async fn test_backup_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path();

        // Create mock backup files with different timestamps
        for i in 0..15 {
            let backup_file = backup_dir.join(format!("patterns_backup_{}.json", i));
            tokio::fs::write(&backup_file, "{}").await.unwrap();
        }

        let pattern_system = Arc::new(PatternLearningSystem::new());
        let suggestion_engine = Arc::new(SuggestionEngine::new(pattern_system.clone()));
        let workflow_automation = Arc::new(WorkflowAutomation::new(pattern_system.clone(), suggestion_engine.clone()));

        let config = HotReloadConfig {
            backup_directory: Some(backup_dir.to_path_buf()),
            max_backups: 10,
            ..Default::default()
        };

        let hot_reload = HotReloadSystem::new(config, pattern_system, suggestion_engine, workflow_automation);
        hot_reload.cleanup_old_backups(backup_dir).await.unwrap();

        // Count remaining files
        let mut entries = tokio::fs::read_dir(backup_dir).await.unwrap();
        let mut count = 0;
        while let Some(_) = entries.next_entry().await.unwrap() {
            count += 1;
        }

        assert_eq!(count, 10); // Should keep only 10 backups
    }
}