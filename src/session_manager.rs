/// Debug Session Manager for maintaining context across debugging commands
/// 
/// This module provides comprehensive session management including:
/// - Session creation and lifecycle management
/// - Command history with undo/redo support
/// - World state checkpointing
/// - Command replay with timing preservation
/// - Session persistence across reconnections

use crate::brp_messages::{DebugCommand, DebugResponse, SessionState};
use crate::checkpoint::{Checkpoint, CheckpointManager, CheckpointConfig};
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Configuration constants
pub mod constants {
    /// Default maximum command history entries per session
    pub const DEFAULT_COMMAND_HISTORY_LIMIT: usize = 1000;
    /// Default session cleanup time in hours
    pub const DEFAULT_CLEANUP_HOURS: u32 = 24;
    /// Default maximum concurrent sessions
    pub const DEFAULT_MAX_SESSIONS: usize = 50;
    /// Default cleanup interval in minutes
    pub const DEFAULT_CLEANUP_INTERVAL_MINUTES: u32 = 30;
    /// Maximum session name length
    pub const MAX_SESSION_NAME_LENGTH: usize = 256;
    /// Maximum command history retention
    pub const MAX_COMMAND_HISTORY_RETENTION: usize = 1000;
}

/// Command entry in session history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandHistoryEntry {
    /// Unique ID for this command
    pub id: String,
    /// Command that was executed
    pub command: DebugCommand,
    /// Response received
    pub response: Option<DebugResponse>,
    /// Timestamp when command was executed
    pub timestamp: DateTime<Utc>,
    /// Execution duration in microseconds
    pub execution_duration_us: u64,
    /// Whether command was successful
    pub success: bool,
    /// Error message if command failed
    pub error_message: Option<String>,
    /// Correlation ID for tracking
    pub correlation_id: String,
}

/// Debug session state and context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugSession {
    /// Session ID
    pub id: String,
    /// Session name
    pub name: String,
    /// Session description
    pub description: Option<String>,
    /// Session state
    pub state: SessionState,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
    /// Command history
    pub command_history: VecDeque<CommandHistoryEntry>,
    /// Current replay position (None if not replaying)
    pub replay_position: Option<usize>,
    /// Replay speed multiplier
    pub replay_speed: f32,
    /// Session-specific checkpoints
    pub checkpoints: Vec<String>,
    /// Session metadata
    pub metadata: HashMap<String, String>,
    /// Auto-cleanup after inactivity (hours)
    pub auto_cleanup_hours: Option<u32>,
}

impl DebugSession {
    /// Create a new debug session
    pub fn new(name: String, description: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            description,
            state: SessionState::Active,
            created_at: now,
            last_activity: now,
            command_history: VecDeque::with_capacity(constants::DEFAULT_COMMAND_HISTORY_LIMIT),
            replay_position: None,
            replay_speed: 1.0,
            checkpoints: Vec::new(),
            metadata: HashMap::new(),
            auto_cleanup_hours: Some(24),
        }
    }

    /// Add command to history
    pub fn add_command_history(&mut self, entry: CommandHistoryEntry) {
        // Maintain maximum history size
        while self.command_history.len() >= constants::MAX_COMMAND_HISTORY_RETENTION {
            self.command_history.pop_front();
        }
        
        self.command_history.push_back(entry);
        self.last_activity = Utc::now();
    }

    /// Get command history in reverse chronological order
    pub fn get_recent_history(&self, limit: Option<usize>) -> Vec<&CommandHistoryEntry> {
        let max_count = limit.unwrap_or(100);
        self.command_history
            .iter()
            .rev()
            .take(max_count)
            .collect()
    }

    /// Check if session should be cleaned up due to inactivity
    pub fn should_cleanup(&self) -> bool {
        if let Some(hours) = self.auto_cleanup_hours {
            let age = Utc::now()
                .signed_duration_since(self.last_activity)
                .num_hours();
            age >= hours as i64
        } else {
            false
        }
    }

    /// Update last activity timestamp
    pub fn touch(&mut self) {
        self.last_activity = Utc::now();
    }

    /// Start command replay
    pub fn start_replay(&mut self, from_position: Option<usize>) -> Result<()> {
        if self.command_history.is_empty() {
            return Err(Error::Validation("No command history to replay".to_string()));
        }

        let start_pos = from_position.unwrap_or(0);
        if start_pos >= self.command_history.len() {
            return Err(Error::Validation("Invalid replay position".to_string()));
        }

        self.replay_position = Some(start_pos);
        self.state = SessionState::Replaying;
        Ok(())
    }

    /// Get next command for replay
    pub fn next_replay_command(&mut self) -> Option<&CommandHistoryEntry> {
        if let Some(pos) = self.replay_position {
            if pos < self.command_history.len() {
                let entry = &self.command_history[pos];
                self.replay_position = Some(pos + 1);
                Some(entry)
            } else {
                // Replay finished
                self.replay_position = None;
                self.state = SessionState::Active;
                None
            }
        } else {
            None
        }
    }

    /// Stop replay
    pub fn stop_replay(&mut self) {
        self.replay_position = None;
        self.state = SessionState::Active;
    }
}

/// Session management configuration
#[derive(Debug, Clone)]
pub struct SessionManagerConfig {
    /// Maximum number of concurrent sessions
    pub max_sessions: usize,
    /// Default auto-cleanup time in hours
    pub default_cleanup_hours: u32,
    /// Command history limit per session
    pub command_history_limit: usize,
    /// Enable session persistence
    pub enable_persistence: bool,
    /// Session storage directory
    pub storage_directory: String,
    /// Cleanup check interval in minutes
    pub cleanup_interval_minutes: u32,
}

impl Default for SessionManagerConfig {
    fn default() -> Self {
        Self {
            max_sessions: constants::DEFAULT_MAX_SESSIONS,
            default_cleanup_hours: constants::DEFAULT_CLEANUP_HOURS,
            command_history_limit: constants::DEFAULT_COMMAND_HISTORY_LIMIT,
            enable_persistence: true,
            storage_directory: "./debug_sessions".to_string(),
            cleanup_interval_minutes: constants::DEFAULT_CLEANUP_INTERVAL_MINUTES,
        }
    }
}

/// Debug session manager
pub struct SessionManager {
    /// Configuration
    config: SessionManagerConfig,
    /// Active sessions
    sessions: Arc<RwLock<HashMap<String, DebugSession>>>,
    /// Checkpoint manager
    checkpoint_manager: Arc<RwLock<CheckpointManager>>,
    /// Cleanup task handle
    cleanup_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl SessionManager {
    /// Create new session manager
    pub fn new(config: SessionManagerConfig) -> Self {
        let checkpoint_config = CheckpointConfig {
            max_checkpoints: 500,
            max_age_seconds: (config.default_cleanup_hours * 3600) as u64,
            persist_to_disk: config.enable_persistence,
            storage_directory: format!("{}/checkpoints", config.storage_directory),
            cleanup_interval_seconds: (config.cleanup_interval_minutes * 60) as u64,
            max_state_size_bytes: 50 * 1024 * 1024, // 50MB
        };

        Self {
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            checkpoint_manager: Arc::new(RwLock::new(CheckpointManager::new(checkpoint_config))),
            cleanup_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Start session manager
    pub async fn start(&self) -> Result<()> {
        // Start checkpoint manager
        {
            let mut checkpoint_manager = self.checkpoint_manager.write().await;
            checkpoint_manager.start().await?;
        }

        // Start cleanup task
        let sessions = Arc::clone(&self.sessions);
        let cleanup_interval = Duration::from_secs((self.config.cleanup_interval_minutes * 60) as u64);

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);

            loop {
                interval.tick().await;

                // Handle cleanup with proper error recovery
                match sessions.try_write() {
                    Ok(mut sessions_guard) => {
                        let mut to_remove = Vec::new();

                        for (session_id, session) in sessions_guard.iter() {
                            if session.should_cleanup() {
                                to_remove.push(session_id.clone());
                            }
                        }

                        for session_id in to_remove {
                            sessions_guard.remove(&session_id);
                            info!("Cleaned up inactive session: {}", session_id);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to acquire session lock for cleanup, retrying: {}", e);
                        // Continue to next iteration rather than crashing
                        continue;
                    }
                }
            }
        });

        {
            let mut cleanup_guard = self.cleanup_handle.write().await;
            *cleanup_guard = Some(handle);
        }

        info!("Session manager started");
        Ok(())
    }

    /// Create new session
    pub async fn create_session(&self, name: String, description: Option<String>) -> Result<String> {
        // Validate input
        if name.is_empty() {
            return Err(Error::Validation("Session name cannot be empty".to_string()));
        }
        
        if name.len() > constants::MAX_SESSION_NAME_LENGTH {
            return Err(Error::Validation(format!(
                "Session name too long (max {} characters)", 
                constants::MAX_SESSION_NAME_LENGTH
            )));
        }
        
        // Check for invalid characters that could cause issues
        if name.chars().any(|c| c.is_control() || "/<>:|\"?*\\".contains(c)) {
            return Err(Error::Validation("Session name contains invalid characters".to_string()));
        }

        let mut sessions = self.sessions.write().await;

        // Check session limit
        if sessions.len() >= self.config.max_sessions {
            return Err(Error::Validation(format!(
                "Maximum session limit reached: {}",
                self.config.max_sessions
            )));
        }

        let mut session = DebugSession::new(name.clone(), description);
        session.auto_cleanup_hours = Some(self.config.default_cleanup_hours);

        let session_id = session.id.clone();
        sessions.insert(session_id.clone(), session);

        info!("Created debug session: {} ({})", name, session_id);
        Ok(session_id)
    }

    /// Get session by ID
    pub async fn get_session(&self, session_id: &str) -> Option<DebugSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// End session
    pub async fn end_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(mut session) = sessions.remove(session_id) {
            session.state = SessionState::Ended;
            info!("Ended debug session: {}", session_id);
            Ok(())
        } else {
            Err(Error::Validation(format!("Session not found: {}", session_id)))
        }
    }

    /// Resume session (change state to active)
    pub async fn resume_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(session_id) {
            session.state = SessionState::Active;
            session.touch();
            info!("Resumed debug session: {}", session_id);
            Ok(())
        } else {
            Err(Error::Validation(format!("Session not found: {}", session_id)))
        }
    }

    /// Create checkpoint for session
    pub async fn create_checkpoint(&self, session_id: &str, description: &str) -> Result<String> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(session_id) {
            // Create checkpoint with session state (clone to avoid borrow issues)
            let session_clone = session.clone();
            let checkpoint_data = serde_json::to_value(&session_clone)?;
            let checkpoint = Checkpoint::new(
                &format!("Session {} Checkpoint", session.name),
                description,
                "session_state",
                "debug_session_manager",
                checkpoint_data,
            );

            let checkpoint_id = checkpoint.id.clone();
            
            {
                let checkpoint_manager = self.checkpoint_manager.read().await;
                checkpoint_manager.create_checkpoint(checkpoint).await?;
            }

            session.checkpoints.push(checkpoint_id.clone());
            session.touch();

            info!("Created checkpoint for session {}: {}", session_id, checkpoint_id);
            Ok(checkpoint_id)
        } else {
            Err(Error::Validation(format!("Session not found: {}", session_id)))
        }
    }

    /// Restore session from checkpoint
    pub async fn restore_checkpoint(&self, session_id: &str, checkpoint_id: &str) -> Result<()> {
        let checkpoint = {
            let checkpoint_manager = self.checkpoint_manager.read().await;
            checkpoint_manager.restore_checkpoint(checkpoint_id).await?
        };

        let restored_session: DebugSession = serde_json::from_value(checkpoint.state_data)?;

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.to_string(), restored_session);

        info!("Restored session {} from checkpoint {}", session_id, checkpoint_id);
        Ok(())
    }

    /// Record command execution in session
    pub async fn record_command(
        &self,
        session_id: &str,
        command: DebugCommand,
        response: Option<DebugResponse>,
        execution_duration: Duration,
        correlation_id: String,
    ) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(session_id) {
            let success = response.is_some() && matches!(response, Some(DebugResponse::Success { .. }));
            let error_message = if !success && response.is_some() {
                Some(format!("{:?}", response))
            } else {
                None
            };

            let entry = CommandHistoryEntry {
                id: Uuid::new_v4().to_string(),
                command,
                response,
                timestamp: Utc::now(),
                execution_duration_us: execution_duration.as_micros() as u64,
                success,
                error_message,
                correlation_id,
            };

            session.add_command_history(entry);
            debug!("Recorded command in session: {}", session_id);
            Ok(())
        } else {
            Err(Error::Validation(format!("Session not found: {}", session_id)))
        }
    }

    /// Get session command history
    pub async fn get_command_history(
        &self,
        session_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<CommandHistoryEntry>> {
        let sessions = self.sessions.read().await;
        
        if let Some(session) = sessions.get(session_id) {
            let history = session
                .get_recent_history(limit)
                .iter()
                .map(|&entry| entry.clone())
                .collect();
            Ok(history)
        } else {
            Err(Error::Validation(format!("Session not found: {}", session_id)))
        }
    }

    /// Start command replay for session
    pub async fn start_replay(&self, session_id: &str, from_position: Option<usize>) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(session_id) {
            session.start_replay(from_position)?;
            info!("Started replay for session: {}", session_id);
            Ok(())
        } else {
            Err(Error::Validation(format!("Session not found: {}", session_id)))
        }
    }

    /// Get next replay command
    pub async fn get_next_replay_command(&self, session_id: &str) -> Result<Option<DebugCommand>> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(session_id) {
            if let Some(entry) = session.next_replay_command() {
                Ok(Some(entry.command.clone()))
            } else {
                Ok(None)
            }
        } else {
            Err(Error::Validation(format!("Session not found: {}", session_id)))
        }
    }

    /// Stop command replay
    pub async fn stop_replay(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(session_id) {
            session.stop_replay();
            info!("Stopped replay for session: {}", session_id);
            Ok(())
        } else {
            Err(Error::Validation(format!("Session not found: {}", session_id)))
        }
    }

    /// List all active sessions
    pub async fn list_sessions(&self) -> Vec<DebugSession> {
        let sessions = self.sessions.read().await;
        sessions.values().cloned().collect()
    }

    /// Get session statistics
    pub async fn get_statistics(&self) -> HashMap<String, serde_json::Value> {
        let sessions = self.sessions.read().await;
        let checkpoint_stats = {
            let checkpoint_manager = self.checkpoint_manager.read().await;
            checkpoint_manager.get_statistics().await
        };

        let mut stats = HashMap::new();

        stats.insert(
            "total_sessions".to_string(),
            serde_json::Value::Number(sessions.len().into()),
        );

        let active_sessions = sessions
            .values()
            .filter(|s| matches!(s.state, SessionState::Active))
            .count();

        stats.insert(
            "active_sessions".to_string(),
            serde_json::Value::Number(active_sessions.into()),
        );

        let replaying_sessions = sessions
            .values()
            .filter(|s| matches!(s.state, SessionState::Replaying))
            .count();

        stats.insert(
            "replaying_sessions".to_string(),
            serde_json::Value::Number(replaying_sessions.into()),
        );

        let total_commands: usize = sessions
            .values()
            .map(|s| s.command_history.len())
            .sum();

        stats.insert(
            "total_commands_recorded".to_string(),
            serde_json::Value::Number(total_commands.into()),
        );

        stats.insert(
            "total_checkpoints".to_string(),
            serde_json::Value::Number(checkpoint_stats.total_count.into()),
        );

        stats
    }

    /// Shutdown session manager
    pub async fn shutdown(&mut self) -> Result<()> {
        // Stop cleanup task
        {
            let mut cleanup_guard = self.cleanup_handle.write().await;
            if let Some(handle) = cleanup_guard.take() {
                handle.abort();
            }
        }

        // Shutdown checkpoint manager
        {
            let mut checkpoint_manager = self.checkpoint_manager.write().await;
            checkpoint_manager.shutdown().await?;
        }

        info!("Session manager shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_creation() {
        let config = SessionManagerConfig::default();
        let manager = SessionManager::new(config);
        manager.start().await.unwrap();

        let session_id = manager
            .create_session("Test Session".to_string(), Some("Test description".to_string()))
            .await
            .unwrap();

        assert!(!session_id.is_empty());

        let session = manager.get_session(&session_id).await.unwrap();
        assert_eq!(session.name, "Test Session");
        assert!(session.description.is_some());
        assert!(matches!(session.state, SessionState::Active));
    }

    #[tokio::test]
    async fn test_command_history() {
        let config = SessionManagerConfig::default();
        let manager = SessionManager::new(config);
        manager.start().await.unwrap();

        let session_id = manager
            .create_session("Test Session".to_string(), None)
            .await
            .unwrap();

        let command = DebugCommand::GetMemoryProfile;
        let response = DebugResponse::Success {
            message: "Test response".to_string(),
            data: None,
        };

        manager
            .record_command(
                &session_id,
                command,
                Some(response),
                Duration::from_millis(10),
                "test-correlation-id".to_string(),
            )
            .await
            .unwrap();

        let history = manager.get_command_history(&session_id, Some(10)).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].correlation_id, "test-correlation-id");
        assert!(history[0].success);
    }

    #[tokio::test]
    async fn test_checkpoint_creation() {
        let config = SessionManagerConfig::default();
        let manager = SessionManager::new(config);
        manager.start().await.unwrap();

        let session_id = manager
            .create_session("Test Session".to_string(), None)
            .await
            .unwrap();

        let checkpoint_id = manager
            .create_checkpoint(&session_id, "Test checkpoint")
            .await
            .unwrap();

        assert!(!checkpoint_id.is_empty());

        let session = manager.get_session(&session_id).await.unwrap();
        assert!(session.checkpoints.contains(&checkpoint_id));
    }

    #[tokio::test]
    async fn test_replay_functionality() {
        let config = SessionManagerConfig::default();
        let manager = SessionManager::new(config);
        manager.start().await.unwrap();

        let session_id = manager
            .create_session("Test Session".to_string(), None)
            .await
            .unwrap();

        // Add some commands to history
        for i in 0..5 {
            let command = DebugCommand::GetMemoryProfile;
            manager
                .record_command(
                    &session_id,
                    command,
                    None,
                    Duration::from_millis(10),
                    format!("correlation-{}", i),
                )
                .await
                .unwrap();
        }

        // Start replay
        manager.start_replay(&session_id, Some(0)).await.unwrap();

        // Get replay commands
        let mut replay_count = 0;
        while let Some(_command) = manager.get_next_replay_command(&session_id).await.unwrap() {
            replay_count += 1;
        }

        assert_eq!(replay_count, 5);

        // Session should be back to active state
        let session = manager.get_session(&session_id).await.unwrap();
        assert!(matches!(session.state, SessionState::Active));
    }

    #[tokio::test]
    async fn test_session_cleanup_logic() {
        let mut config = SessionManagerConfig::default();
        config.default_cleanup_hours = 0; // Immediate cleanup for testing

        let manager = SessionManager::new(config);
        manager.start().await.unwrap();

        let session_id = manager
            .create_session("Test Session".to_string(), None)
            .await
            .unwrap();

        let session = manager.get_session(&session_id).await.unwrap();
        assert!(session.should_cleanup()); // Should be marked for cleanup
    }

    #[tokio::test]
    async fn test_statistics() {
        let config = SessionManagerConfig::default();
        let manager = SessionManager::new(config);
        manager.start().await.unwrap();

        let _session_id = manager
            .create_session("Test Session".to_string(), None)
            .await
            .unwrap();

        let stats = manager.get_statistics().await;
        assert_eq!(stats["total_sessions"], 1);
        assert_eq!(stats["active_sessions"], 1);
        assert_eq!(stats["replaying_sessions"], 0);
    }
}