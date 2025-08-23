/*
 * Bevy Debugger MCP Server - BRP Command Handler Interface
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

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::brp_messages::{BrpRequest, BrpResponse};
use crate::brp_validation::BrpValidator;
use crate::error::Result;

/// Version information for command handlers
#[derive(Debug, Clone)]
pub struct CommandVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl CommandVersion {
    pub fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self { major, minor, patch }
    }

    pub fn is_compatible_with(&self, other: &CommandVersion) -> bool {
        // Major version must match, minor/patch can differ
        self.major == other.major
    }
}

impl Default for CommandVersion {
    fn default() -> Self {
        Self::new(1, 0, 0)
    }
}

/// Metadata for command handlers
#[derive(Debug, Clone)]
pub struct CommandHandlerMetadata {
    pub name: String,
    pub version: CommandVersion,
    pub description: String,
    pub supported_commands: Vec<String>,
}

/// Trait for handling BRP commands in an extensible way.
/// 
/// Implementors of this trait can process specific types of BRP requests
/// and return appropriate responses. Handlers are registered with a
/// `CommandHandlerRegistry` and selected based on their ability to handle
/// a request and their priority.
/// 
/// # Example
/// ```ignore
/// struct MyHandler;
/// 
/// #[async_trait]
/// impl BrpCommandHandler for MyHandler {
///     fn metadata(&self) -> CommandHandlerMetadata { ... }
///     fn can_handle(&self, request: &BrpRequest) -> bool { ... }
///     async fn handle(&self, request: BrpRequest) -> Result<BrpResponse> { ... }
/// }
/// ```
#[async_trait]
pub trait BrpCommandHandler: Send + Sync {
    /// Get metadata about this handler including name, version, and supported commands
    fn metadata(&self) -> CommandHandlerMetadata;

    /// Check if this handler can process the given request.
    /// Returns true if the handler supports the request type.
    fn can_handle(&self, request: &BrpRequest) -> bool;

    /// Process a BRP request and return a response.
    /// This is called after validation passes.
    async fn handle(&self, request: BrpRequest) -> Result<BrpResponse>;

    /// Validate a request before processing.
    /// Override this to add custom validation logic.
    /// Default implementation uses comprehensive BRP validation.
    async fn validate(&self, request: &BrpRequest) -> Result<()> {
        // Use basic validation from brp_messages module as fallback
        crate::brp_messages::validation::validate_request(request)
            .map_err(|e| crate::error::Error::Validation(e))
    }

    /// Get the handler's priority (higher = processed first).
    /// Handlers with higher priority are checked first when finding
    /// a handler for a request. Default priority is 0.
    fn priority(&self) -> i32 {
        0
    }
}

/// Registry for command handlers with versioning support
pub struct CommandHandlerRegistry {
    handlers: Arc<RwLock<Vec<Arc<dyn BrpCommandHandler>>>>,
    version_map: Arc<RwLock<HashMap<String, CommandVersion>>>,
    validator: Arc<BrpValidator>,
}

impl CommandHandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(Vec::new())),
            version_map: Arc::new(RwLock::new(HashMap::new())),
            validator: Arc::new(BrpValidator::new()),
        }
    }
    
    /// Create a new registry with custom validation configuration
    pub fn with_validator(validator: BrpValidator) -> Self {
        Self {
            handlers: Arc::new(RwLock::new(Vec::new())),
            version_map: Arc::new(RwLock::new(HashMap::new())),
            validator: Arc::new(validator),
        }
    }

    /// Register a new command handler
    pub async fn register(&self, handler: Arc<dyn BrpCommandHandler>) {
        let metadata = handler.metadata();
        
        // Update version map
        let mut version_map = self.version_map.write().await;
        version_map.insert(metadata.name.clone(), metadata.version.clone());
        
        // Add handler sorted by priority
        let mut handlers = self.handlers.write().await;
        handlers.push(handler);
        handlers.sort_by_key(|h| -h.priority()); // Sort descending by priority
    }

    /// Find a handler for the given request
    pub async fn find_handler(&self, request: &BrpRequest) -> Option<Arc<dyn BrpCommandHandler>> {
        let handlers = self.handlers.read().await;
        
        for handler in handlers.iter() {
            if handler.can_handle(request) {
                return Some(handler.clone());
            }
        }
        
        None
    }

    /// Process a request using the appropriate handler with comprehensive validation
    pub async fn process(&self, request: BrpRequest) -> Result<BrpResponse> {
        self.process_with_session(request, "default_session").await
    }
    
    /// Process a request with session-specific validation
    pub async fn process_with_session(&self, request: BrpRequest, session_id: &str) -> Result<BrpResponse> {
        if let Some(handler) = self.find_handler(&request).await {
            // First run comprehensive validation
            let request_size = serde_json::to_vec(&request)
                .map_err(|e| crate::error::Error::Json(e))?
                .len();
            
            self.validator.validate_request(&request, session_id, request_size).await?;
            
            // Then run handler-specific validation
            handler.validate(&request).await?;
            
            // Finally handle the request
            handler.handle(request).await
        } else {
            Err(crate::error::Error::Validation(format!(
                "No handler found for request type: {:?}",
                request
            )))
        }
    }

    /// Get version information for all registered handlers
    pub async fn get_versions(&self) -> HashMap<String, CommandVersion> {
        self.version_map.read().await.clone()
    }

    /// Check if a specific version is supported
    pub async fn is_version_supported(&self, handler_name: &str, version: &CommandVersion) -> bool {
        let version_map = self.version_map.read().await;
        
        if let Some(registered_version) = version_map.get(handler_name) {
            registered_version.is_compatible_with(version)
        } else {
            false
        }
    }
    
    /// Get the validator for configuration
    pub fn get_validator(&self) -> Arc<BrpValidator> {
        self.validator.clone()
    }
}

/// Default handler for core BRP commands
pub struct CoreBrpHandler;

#[async_trait]
impl BrpCommandHandler for CoreBrpHandler {
    fn metadata(&self) -> CommandHandlerMetadata {
        CommandHandlerMetadata {
            name: "core".to_string(),
            version: CommandVersion::new(1, 0, 0),
            description: "Handler for core BRP commands".to_string(),
            supported_commands: vec![
                "Query".to_string(),
                "Get".to_string(),
                "Set".to_string(),
                "ListEntities".to_string(),
                "ListComponents".to_string(),
                "SpawnEntity".to_string(),
                "DestroyEntity".to_string(),
            ],
        }
    }

    fn can_handle(&self, request: &BrpRequest) -> bool {
        matches!(
            request,
            BrpRequest::Query { .. }
                | BrpRequest::Get { .. }
                | BrpRequest::Set { .. }
                | BrpRequest::ListEntities { .. }
                | BrpRequest::ListComponents { .. }
                | BrpRequest::Spawn { .. }
                | BrpRequest::Destroy { .. }
        )
    }

    async fn handle(&self, request: BrpRequest) -> Result<BrpResponse> {
        // This will be handled by the actual BRP WebSocket connection
        // For now, return a placeholder
        Ok(BrpResponse::Success(Box::new(crate::brp_messages::BrpResult::Success)))
    }

    fn priority(&self) -> i32 {
        -100 // Low priority, let specialized handlers go first
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_compatibility() {
        let v1 = CommandVersion::new(1, 0, 0);
        let v2 = CommandVersion::new(1, 1, 0);
        let v3 = CommandVersion::new(2, 0, 0);

        assert!(v1.is_compatible_with(&v2));
        assert!(!v1.is_compatible_with(&v3));
    }

    #[tokio::test]
    async fn test_handler_registry() {
        let registry = CommandHandlerRegistry::new();
        let handler = Arc::new(CoreBrpHandler);
        
        registry.register(handler.clone()).await;
        
        let request = BrpRequest::ListEntities { filter: None };
        let found = registry.find_handler(&request).await;
        
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn test_handler_priority() {
        struct HighPriorityHandler;
        
        #[async_trait]
        impl BrpCommandHandler for HighPriorityHandler {
            fn metadata(&self) -> CommandHandlerMetadata {
                CommandHandlerMetadata {
                    name: "high_priority".to_string(),
                    version: CommandVersion::default(),
                    description: "High priority handler".to_string(),
                    supported_commands: vec!["ListEntities".to_string()],
                }
            }

            fn can_handle(&self, request: &BrpRequest) -> bool {
                matches!(request, BrpRequest::ListEntities { .. })
            }

            async fn handle(&self, _request: BrpRequest) -> Result<BrpResponse> {
                Ok(BrpResponse::Success(Box::new(crate::brp_messages::BrpResult::Success)))
            }

            fn priority(&self) -> i32 {
                100 // High priority
            }
        }

        let registry = CommandHandlerRegistry::new();
        
        // Register low priority first
        registry.register(Arc::new(CoreBrpHandler)).await;
        
        // Then high priority
        registry.register(Arc::new(HighPriorityHandler)).await;
        
        let request = BrpRequest::ListEntities { filter: None };
        let handler = registry.find_handler(&request).await.unwrap();
        
        // Should get high priority handler
        assert_eq!(handler.priority(), 100);
    }
}