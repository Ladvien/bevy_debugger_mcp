/// Session Processor for handling debug session management commands
/// 
/// This processor provides session management capabilities through the debug command system,
/// integrating with the SessionManager to offer session lifecycle, command history, and replay functionality.

use crate::brp_messages::{DebugCommand, DebugResponse, SessionOperation, SessionState, CheckpointInfo};
use crate::brp_client::BrpClient;
use crate::debug_command_processor::DebugCommandProcessor;
use crate::session_manager::{SessionManager, SessionManagerConfig};
use crate::error::{Error, Result};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::debug;

/// Session processor for debug commands
pub struct SessionProcessor {
    /// Session manager instance
    session_manager: Arc<SessionManager>,
    /// BRP client for command execution
    brp_client: Arc<RwLock<BrpClient>>,
}

impl SessionProcessor {
    /// Create a new session processor
    pub fn new(brp_client: Arc<RwLock<BrpClient>>) -> Self {
        let config = SessionManagerConfig::default();
        let session_manager = Arc::new(SessionManager::new(config));
        
        Self {
            session_manager,
            brp_client,
        }
    }

    /// Create with custom configuration
    pub fn with_config(
        brp_client: Arc<RwLock<BrpClient>>,
        config: SessionManagerConfig
    ) -> Self {
        let session_manager = Arc::new(SessionManager::new(config));
        
        Self {
            session_manager,
            brp_client,
        }
    }

    /// Start the session processor
    pub async fn start(&self) -> Result<()> {
        self.session_manager.start().await
    }

    /// Handle session control operations
    async fn handle_session_control(&self, operation: SessionOperation, session_id: Option<String>) -> Result<DebugResponse> {
        match operation {
            SessionOperation::Create => self.handle_create_session(session_id).await,
            SessionOperation::Resume => self.handle_resume_session(session_id).await,
            SessionOperation::Checkpoint => self.handle_create_checkpoint(session_id).await,
            SessionOperation::Restore { checkpoint_id } => self.handle_restore_checkpoint(session_id, checkpoint_id).await,
            SessionOperation::End => self.handle_end_session(session_id).await,
        }
    }

    /// Handle session creation
    async fn handle_create_session(&self, session_id: Option<String>) -> Result<DebugResponse> {
        let session_name = session_id.unwrap_or_else(|| format!("Session-{}", chrono::Utc::now().timestamp()));
        let created_session_id = self.session_manager.create_session(session_name.clone(), None).await?;
        
        Ok(DebugResponse::SessionStatus {
            session_id: created_session_id,
            state: SessionState::Active,
            command_count: 0,
            checkpoints: Vec::new(),
        })
    }

    /// Handle session resumption
    async fn handle_resume_session(&self, session_id: Option<String>) -> Result<DebugResponse> {
        let session_id = session_id.ok_or_else(|| Error::Validation("Session ID required for resume operation".to_string()))?;
        self.session_manager.resume_session(&session_id).await?;
        
        let session = self.session_manager.get_session(&session_id).await
            .ok_or_else(|| Error::Validation("Session not found after resume".to_string()))?;
        
        let checkpoints = self.build_checkpoint_info_list(&session.checkpoints);
        
        Ok(DebugResponse::SessionStatus {
            session_id,
            state: session.state,
            command_count: session.command_history.len(),
            checkpoints,
        })
    }

    /// Handle checkpoint creation
    async fn handle_create_checkpoint(&self, session_id: Option<String>) -> Result<DebugResponse> {
        let session_id = session_id.ok_or_else(|| Error::Validation("Session ID required for checkpoint operation".to_string()))?;
        let checkpoint_id = self.session_manager.create_checkpoint(&session_id, "Manual checkpoint").await?;
        
        let session = self.session_manager.get_session(&session_id).await
            .ok_or_else(|| Error::Validation("Session not found after checkpoint".to_string()))?;
        
        let checkpoints = self.build_checkpoint_info_list(&session.checkpoints);
        
        Ok(DebugResponse::Success {
            message: format!("Created checkpoint: {}", checkpoint_id),
            data: Some(serde_json::json!({
                "session_id": session_id,
                "checkpoint_id": checkpoint_id,
                "checkpoints": checkpoints
            })),
        })
    }

    /// Handle checkpoint restoration
    async fn handle_restore_checkpoint(&self, session_id: Option<String>, checkpoint_id: String) -> Result<DebugResponse> {
        let session_id = session_id.ok_or_else(|| Error::Validation("Session ID required for restore operation".to_string()))?;
        self.session_manager.restore_checkpoint(&session_id, &checkpoint_id).await?;
        
        Ok(DebugResponse::Success {
            message: format!("Restored from checkpoint: {}", checkpoint_id),
            data: Some(serde_json::json!({
                "session_id": session_id,
                "checkpoint_id": checkpoint_id
            })),
        })
    }

    /// Handle session termination
    async fn handle_end_session(&self, session_id: Option<String>) -> Result<DebugResponse> {
        let session_id = session_id.ok_or_else(|| Error::Validation("Session ID required for end operation".to_string()))?;
        self.session_manager.end_session(&session_id).await?;
        
        Ok(DebugResponse::Success {
            message: format!("Ended session: {}", session_id),
            data: Some(serde_json::json!({
                "session_id": session_id
            })),
        })
    }

    /// Helper to build checkpoint info list
    fn build_checkpoint_info_list(&self, checkpoint_ids: &[String]) -> Vec<CheckpointInfo> {
        checkpoint_ids.iter().map(|checkpoint_id| {
            CheckpointInfo {
                id: checkpoint_id.clone(),
                timestamp: chrono::Utc::now().timestamp() as u64,
                description: Some("Session checkpoint".to_string()),
                size: 0, // TODO: Calculate actual size
            }
        }).collect()
    }

    /// Get debug system status
    async fn handle_get_status(&self) -> Result<DebugResponse> {
        let stats = self.session_manager.get_statistics().await;
        
        let active_sessions = stats.get("active_sessions")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        
        let command_queue_size = 0; // TODO: Implement actual command queue if needed
        let performance_overhead_percent = 0.5; // TODO: Calculate actual overhead
        
        Ok(DebugResponse::Status {
            version: "1.0.0".to_string(),
            active_sessions,
            command_queue_size,
            performance_overhead_percent,
        })
    }

    /// Record command execution for session tracking
    pub async fn record_command_execution(
        &self,
        session_id: &str,
        command: DebugCommand,
        response: Option<DebugResponse>,
        execution_duration: Duration,
        correlation_id: String,
    ) -> Result<()> {
        self.session_manager.record_command(
            session_id,
            command,
            response,
            execution_duration,
            correlation_id,
        ).await
    }

    /// Get command history for a session
    pub async fn get_command_history(&self, session_id: &str, limit: Option<usize>) -> Result<Vec<serde_json::Value>> {
        let history = self.session_manager.get_command_history(session_id, limit).await?;
        
        let history_json: Vec<serde_json::Value> = history.iter().map(|entry| {
            serde_json::json!({
                "id": entry.id,
                "command": entry.command,
                "timestamp": entry.timestamp.to_rfc3339(),
                "execution_duration_us": entry.execution_duration_us,
                "success": entry.success,
                "error_message": entry.error_message,
                "correlation_id": entry.correlation_id
            })
        }).collect();
        
        Ok(history_json)
    }

    /// Start replay for a session
    pub async fn start_replay(&self, session_id: &str, from_position: Option<usize>) -> Result<()> {
        self.session_manager.start_replay(session_id, from_position).await
    }

    /// Execute next command in replay sequence
    pub async fn replay_next(&self, session_id: &str) -> Result<Option<DebugResponse>> {
        if let Some(command) = self.session_manager.get_next_replay_command(session_id).await? {
            // Execute the command through the appropriate processor
            // For now, return a placeholder response
            Ok(Some(DebugResponse::Success {
                message: "Replayed command".to_string(),
                data: Some(serde_json::json!({
                    "command": command,
                    "replayed": true
                }))
            }))
        } else {
            Ok(None)
        }
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Result<Vec<serde_json::Value>> {
        let sessions = self.session_manager.list_sessions().await;
        
        let sessions_json: Vec<serde_json::Value> = sessions.iter().map(|session| {
            serde_json::json!({
                "id": session.id,
                "name": session.name,
                "description": session.description,
                "state": session.state,
                "created_at": session.created_at.to_rfc3339(),
                "last_activity": session.last_activity.to_rfc3339(),
                "command_count": session.command_history.len(),
                "checkpoint_count": session.checkpoints.len(),
                "replay_position": session.replay_position
            })
        }).collect();
        
        Ok(sessions_json)
    }

    /// Get session statistics
    pub async fn get_session_statistics(&self) -> Result<serde_json::Value> {
        let stats = self.session_manager.get_statistics().await;
        Ok(serde_json::Value::Object(
            stats.into_iter().collect()
        ))
    }
}

#[async_trait]
impl DebugCommandProcessor for SessionProcessor {
    async fn process(&self, command: DebugCommand) -> Result<DebugResponse> {
        match command {
            DebugCommand::SessionControl { operation, session_id } => {
                debug!("Handling session control: {:?}", operation);
                self.handle_session_control(operation, session_id).await
            }
            
            DebugCommand::GetStatus => {
                debug!("Handling get status request");
                self.handle_get_status().await
            }
            
            _ => Err(Error::DebugError(
                "Unsupported command for session processor".to_string(),
            )),
        }
    }

    async fn validate(&self, command: &DebugCommand) -> Result<()> {
        match command {
            DebugCommand::SessionControl { operation, session_id } => {
                match operation {
                    SessionOperation::Create => {
                        // Create operation can work with or without session_id
                        Ok(())
                    }
                    SessionOperation::Resume | SessionOperation::Checkpoint | SessionOperation::End => {
                        if session_id.is_none() {
                            return Err(Error::Validation(
                                format!("Session ID required for {:?} operation", operation)
                            ));
                        }
                        Ok(())
                    }
                    SessionOperation::Restore { .. } => {
                        if session_id.is_none() {
                            return Err(Error::Validation(
                                "Session ID required for restore operation".to_string()
                            ));
                        }
                        Ok(())
                    }
                }
            }
            
            DebugCommand::GetStatus => Ok(()),
            
            _ => Err(Error::DebugError(
                "Command not supported by session processor".to_string()
            )),
        }
    }

    fn estimate_processing_time(&self, command: &DebugCommand) -> Duration {
        match command {
            DebugCommand::SessionControl { operation, .. } => {
                match operation {
                    SessionOperation::Create => Duration::from_millis(50),
                    SessionOperation::Resume => Duration::from_millis(20),
                    SessionOperation::Checkpoint => Duration::from_millis(200),
                    SessionOperation::Restore { .. } => Duration::from_millis(300),
                    SessionOperation::End => Duration::from_millis(30),
                }
            }
            DebugCommand::GetStatus => Duration::from_millis(10),
            _ => Duration::from_millis(1),
        }
    }

    fn supports_command(&self, command: &DebugCommand) -> bool {
        matches!(
            command,
            DebugCommand::SessionControl { .. } | DebugCommand::GetStatus
        )
    }
}

impl Drop for SessionProcessor {
    fn drop(&mut self) {
        // The session manager will handle its own cleanup
        // Could add explicit cleanup here if needed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    async fn create_test_session_processor() -> SessionProcessor {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };
        let brp_client = Arc::new(RwLock::new(crate::brp_client::BrpClient::new(&config)));
        let processor = SessionProcessor::new(brp_client);
        processor.start().await.unwrap();
        processor
    }

    #[tokio::test]
    async fn test_supports_session_commands() {
        let processor = create_test_session_processor().await;
        
        let create_cmd = DebugCommand::SessionControl {
            operation: SessionOperation::Create,
            session_id: Some("test-session".to_string()),
        };
        
        let status_cmd = DebugCommand::GetStatus;
        
        assert!(processor.supports_command(&create_cmd));
        assert!(processor.supports_command(&status_cmd));
    }

    #[tokio::test]
    async fn test_create_session() {
        let processor = create_test_session_processor().await;
        
        let create_cmd = DebugCommand::SessionControl {
            operation: SessionOperation::Create,
            session_id: Some("test-session".to_string()),
        };
        
        let result = processor.process(create_cmd).await;
        assert!(result.is_ok());
        
        match result.unwrap() {
            DebugResponse::SessionStatus { session_id, state, .. } => {
                assert!(!session_id.is_empty());
                assert!(matches!(state, SessionState::Active));
            }
            _ => panic!("Expected SessionStatus response"),
        }
    }

    #[tokio::test]
    async fn test_get_status() {
        let processor = create_test_session_processor().await;
        
        let status_cmd = DebugCommand::GetStatus;
        let result = processor.process(status_cmd).await;
        assert!(result.is_ok());
        
        match result.unwrap() {
            DebugResponse::Status { version, active_sessions, .. } => {
                assert_eq!(version, "1.0.0");
                assert!(active_sessions >= 0);
            }
            _ => panic!("Expected Status response"),
        }
    }

    #[tokio::test]
    async fn test_session_checkpoint() {
        let processor = create_test_session_processor().await;
        
        // First create a session
        let create_cmd = DebugCommand::SessionControl {
            operation: SessionOperation::Create,
            session_id: Some("test-session".to_string()),
        };
        
        let create_result = processor.process(create_cmd).await.unwrap();
        let session_id = match create_result {
            DebugResponse::SessionStatus { session_id, .. } => session_id,
            _ => panic!("Expected SessionStatus response"),
        };
        
        // Create a checkpoint
        let checkpoint_cmd = DebugCommand::SessionControl {
            operation: SessionOperation::Checkpoint,
            session_id: Some(session_id),
        };
        
        let result = processor.process(checkpoint_cmd).await;
        assert!(result.is_ok());
        
        match result.unwrap() {
            DebugResponse::Success { message, data } => {
                assert!(message.contains("Created checkpoint"));
                assert!(data.is_some());
            }
            _ => panic!("Expected Success response"),
        }
    }

    #[tokio::test]
    async fn test_command_validation() {
        let processor = create_test_session_processor().await;
        
        // Valid command
        let valid_cmd = DebugCommand::SessionControl {
            operation: SessionOperation::Create,
            session_id: None, // Create can work without session_id
        };
        
        assert!(processor.validate(&valid_cmd).await.is_ok());
        
        // Invalid command - Resume without session_id
        let invalid_cmd = DebugCommand::SessionControl {
            operation: SessionOperation::Resume,
            session_id: None,
        };
        
        assert!(processor.validate(&invalid_cmd).await.is_err());
    }

    #[tokio::test]
    async fn test_processing_time_estimates() {
        let processor = create_test_session_processor().await;
        
        let commands = vec![
            (DebugCommand::SessionControl {
                operation: SessionOperation::Create,
                session_id: Some("test".to_string()),
            }, Duration::from_millis(50)),
            (DebugCommand::SessionControl {
                operation: SessionOperation::Checkpoint,
                session_id: Some("test".to_string()),
            }, Duration::from_millis(200)),
            (DebugCommand::GetStatus, Duration::from_millis(10)),
        ];
        
        for (cmd, expected_duration) in commands {
            let estimated = processor.estimate_processing_time(&cmd);
            assert_eq!(estimated, expected_duration);
        }
    }

    #[tokio::test]
    async fn test_command_history_recording() {
        let processor = create_test_session_processor().await;
        
        // Create a session first
        let create_cmd = DebugCommand::SessionControl {
            operation: SessionOperation::Create,
            session_id: Some("test-session".to_string()),
        };
        
        let create_result = processor.process(create_cmd.clone()).await.unwrap();
        let session_id = match create_result {
            DebugResponse::SessionStatus { session_id, .. } => session_id,
            _ => panic!("Expected SessionStatus response"),
        };
        
        // Record a command execution
        let test_response = DebugResponse::Success {
            message: "Test command executed".to_string(),
            data: None,
        };
        
        processor.record_command_execution(
            &session_id,
            create_cmd,
            Some(test_response),
            Duration::from_millis(50),
            "test-correlation".to_string(),
        ).await.unwrap();
        
        // Get command history
        let history = processor.get_command_history(&session_id, Some(10)).await.unwrap();
        assert_eq!(history.len(), 1);
        
        let history_entry = &history[0];
        assert_eq!(history_entry["correlation_id"], "test-correlation");
        assert_eq!(history_entry["success"], true);
    }
}