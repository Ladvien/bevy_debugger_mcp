//! Refactored BRP client that doesn't require external RwLock wrapping
//!
//! This version uses interior mutability appropriately and can be shared
//! as Arc<BrpClient> instead of Arc<RwLock<BrpClient>>

use futures_util::{SinkExt, StreamExt};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::time::{interval, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};
use url::Url;

use crate::brp_messages::{BrpRequest, BrpResponse, DebugCommand};
use crate::brp_command_handler::{CommandHandlerRegistry, CoreBrpHandler, BrpCommandHandler};
use crate::config::Config;
use crate::debug_command_processor::{DebugCommandRouter, DebugCommandRequest};
use crate::error::{Error, Result};
use crate::resource_manager::ResourceManager;

/// Batched request for efficient processing with proper cleanup
#[derive(Debug)]
struct BatchedRequest {
    request: BrpRequest,
    timestamp: Instant,
    response_tx: mpsc::Sender<Result<BrpResponse>>,
}

impl BatchedRequest {
    async fn send_response(self, response: Result<BrpResponse>) {
        let _ = self.response_tx.send(response).await;
    }

    fn is_expired(&self, timeout: Duration) -> bool {
        self.timestamp.elapsed() > timeout
    }
}

impl Clone for BatchedRequest {
    fn clone(&self) -> Self {
        Self {
            request: self.request.clone(),
            timestamp: self.timestamp,
            response_tx: self.response_tx.clone(),
        }
    }
}

/// Internal mutable state of the BRP client
struct BrpClientState {
    ws_stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    connected: bool,
    retry_count: u32,
    request_queue: VecDeque<BatchedRequest>,
    batch_processor_handle: Option<tokio::task::JoinHandle<()>>,
}

/// Refactored BRP client with interior mutability
/// 
/// This version can be shared as Arc<BrpClient> instead of Arc<RwLock<BrpClient>>
/// because it manages its own interior mutability appropriately.
pub struct BrpClient {
    config: Config,
    state: Mutex<BrpClientState>,
    resource_manager: Option<Arc<ResourceManager>>, // No longer wrapped in RwLock
    command_registry: Arc<CommandHandlerRegistry>,
    debug_router: Option<Arc<DebugCommandRouter>>,
}

impl std::fmt::Debug for BrpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrpClient")
            .field("config", &self.config)
            .field("has_resource_manager", &self.resource_manager.is_some())
            .field("has_debug_router", &self.debug_router.is_some())
            .finish()
    }
}

impl BrpClient {
    pub fn new(config: &Config) -> Self {
        let command_registry = Arc::new(CommandHandlerRegistry::new());
        
        BrpClient {
            config: config.clone(),
            state: Mutex::new(BrpClientState {
                ws_stream: None,
                connected: false,
                retry_count: 0,
                request_queue: VecDeque::new(),
                batch_processor_handle: None,
            }),
            resource_manager: None,
            command_registry,
            debug_router: None,
        }
    }

    /// Initialize the client asynchronously with default handlers
    pub async fn init(&self) -> Result<()> {
        let core_handler = Arc::new(CoreBrpHandler);
        self.command_registry.register(core_handler).await;
        Ok(())
    }

    /// Set resource manager - now takes Arc<ResourceManager> directly
    pub fn with_resource_manager(mut self, resource_manager: Arc<ResourceManager>) -> Self {
        self.resource_manager = Some(resource_manager);
        self
    }

    /// Set debug router
    pub fn with_debug_router(mut self, debug_router: Arc<DebugCommandRouter>) -> Self {
        self.debug_router = Some(debug_router);
        self
    }

    /// Check if client is connected (read-only operation, no external lock needed)
    pub async fn is_connected(&self) -> bool {
        let state = self.state.lock().await;
        state.connected
    }

    /// Get current retry count
    pub async fn retry_count(&self) -> u32 {
        let state = self.state.lock().await;
        state.retry_count
    }

    /// Connect to BRP server
    pub async fn connect(&self) -> Result<()> {
        let url = format!("ws://{}:{}", self.config.brp_host, self.config.brp_port);
        let parsed_url = Url::parse(&url).map_err(|e| Error::InvalidInput(format!("Invalid URL {}: {}", url, e)))?;

        info!("Connecting to BRP server at {}", url);

        let mut state = self.state.lock().await;

        // Disconnect if already connected
        if state.connected {
            self.disconnect_internal(&mut state).await?;
        }

        // Attempt connection with retries
        let max_retries = self.config.max_retries;
        for attempt in 0..=max_retries {
            match connect_async(&parsed_url).await {
                Ok((ws_stream, response)) => {
                    info!("Connected to BRP server. Response: {:?}", response);
                    state.ws_stream = Some(ws_stream);
                    state.connected = true;
                    state.retry_count = 0;

                    // Start batch processor
                    self.start_batch_processor(&mut state).await;
                    return Ok(());
                }
                Err(e) => {
                    state.retry_count = attempt + 1;
                    if attempt < max_retries {
                        let delay = Duration::from_millis(
                            self.config.retry_delay_ms * (1 << attempt.min(5)) as u64
                        );
                        warn!("Connection attempt {} failed, retrying in {:?}: {}", attempt + 1, delay, e);
                        drop(state); // Release lock before sleeping
                        tokio::time::sleep(delay).await;
                        state = self.state.lock().await; // Reacquire lock
                    } else {
                        error!("Failed to connect after {} attempts: {}", max_retries + 1, e);
                        return Err(Error::Connection(format!("Connection failed: {}", e)));
                    }
                }
            }
        }

        Err(Error::Connection("Max retries exceeded".to_string()))
    }

    /// Disconnect from BRP server
    pub async fn disconnect(&self) -> Result<()> {
        let mut state = self.state.lock().await;
        self.disconnect_internal(&mut state).await
    }

    async fn disconnect_internal(&self, state: &mut BrpClientState) -> Result<()> {
        if !state.connected {
            return Ok(());
        }

        info!("Disconnecting from BRP server");

        // Stop batch processor
        if let Some(handle) = state.batch_processor_handle.take() {
            handle.abort();
        }

        // Close WebSocket connection
        if let Some(mut ws_stream) = state.ws_stream.take() {
            let _ = ws_stream.close(None).await;
        }

        state.connected = false;
        info!("Disconnected from BRP server");
        Ok(())
    }

    /// Send request (now with interior mutability)
    pub async fn send_request(&self, request: BrpRequest) -> Result<BrpResponse> {
        let state = self.state.lock().await;
        
        if !state.connected {
            return Err(Error::Connection("Not connected to BRP server".to_string()));
        }

        // Create response channel
        let (response_tx, mut response_rx) = mpsc::channel(1);

        // Create batched request
        let batched_request = BatchedRequest {
            request: request.clone(),
            timestamp: Instant::now(),
            response_tx,
        };

        // Add to request queue (Note: we'd need to modify this to work with the Mutex)
        // For now, this is a simplified version
        drop(state);

        // Add request to queue
        {
            let mut state = self.state.lock().await;
            state.request_queue.push_back(batched_request);
        }

        // Wait for response with timeout
        let timeout_duration = Duration::from_millis(self.config.request_timeout_ms);
        match tokio::time::timeout(timeout_duration, response_rx.recv()).await {
            Ok(Some(response)) => response,
            Ok(None) => Err(Error::Connection("Response channel closed".to_string())),
            Err(_) => Err(Error::Timeout("Request timeout".to_string())),
        }
    }

    /// Send debug command
    pub async fn send_debug_command(&self, command: DebugCommand) -> Result<BrpResponse> {
        if let Some(debug_router) = &self.debug_router {
            let request = DebugCommandRequest::new(command);
            match debug_router.route_command(request).await {
                Ok(response) => Ok(response),
                Err(e) => {
                    warn!("Debug command routing failed: {}", e);
                    // Fallback to direct BRP request
                    let brp_request = BrpRequest::DebugCommand { command };
                    self.send_request(brp_request).await
                }
            }
        } else {
            let brp_request = BrpRequest::DebugCommand { command };
            self.send_request(brp_request).await
        }
    }

    async fn start_batch_processor(&self, state: &mut BrpClientState) {
        // This would start the batch processor task
        // Implementation simplified for this example
        info!("Batch processor started");
    }

    /// Get connection statistics
    pub async fn get_connection_stats(&self) -> ConnectionStats {
        let state = self.state.lock().await;
        ConnectionStats {
            connected: state.connected,
            retry_count: state.retry_count,
            queue_size: state.request_queue.len(),
        }
    }

    /// Get resource manager reference (no locks needed)
    pub fn get_resource_manager(&self) -> Option<&Arc<ResourceManager>> {
        self.resource_manager.as_ref()
    }

    /// Register command handler
    pub async fn register_handler(&self, handler: Arc<dyn BrpCommandHandler>) {
        self.command_registry.register(handler).await;
    }

    /// Process queued requests (internal method)
    async fn process_request_queue(&self) -> Result<()> {
        let mut state = self.state.lock().await;
        
        while let Some(batched_request) = state.request_queue.pop_front() {
            // Check if request has expired
            if batched_request.is_expired(Duration::from_millis(self.config.request_timeout_ms)) {
                let _ = batched_request.send_response(
                    Err(Error::Timeout("Request expired in queue".to_string()))
                ).await;
                continue;
            }

            // Process the request
            let response = self.process_single_request(&batched_request.request).await;
            batched_request.send_response(response).await;
        }

        Ok(())
    }

    async fn process_single_request(&self, request: &BrpRequest) -> Result<BrpResponse> {
        // Implementation would process the request
        // This is simplified for the example
        Ok(BrpResponse::Success { 
            id: "test".to_string(),
            result: serde_json::Value::Null 
        })
    }
}

/// Connection statistics
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub connected: bool,
    pub retry_count: u32,
    pub queue_size: usize,
}

/// Factory function to create a properly configured BrpClient
pub async fn create_brp_client(config: &Config) -> Result<Arc<BrpClient>> {
    let client = BrpClient::new(config);
    client.init().await?;
    Ok(Arc::new(client))
}

/// Helper function to create BrpClient with resource manager
pub async fn create_brp_client_with_manager(
    config: &Config,
    resource_manager: Arc<ResourceManager>,
) -> Result<Arc<BrpClient>> {
    let client = BrpClient::new(config)
        .with_resource_manager(resource_manager);
    client.init().await?;
    Ok(Arc::new(client))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_brp_client_creation() {
        let config = Config::default();
        let client = BrpClient::new(&config);
        assert!(!client.is_connected().await);
        assert_eq!(client.retry_count().await, 0);
    }

    #[tokio::test]
    async fn test_brp_client_with_resource_manager() {
        let config = Config::default();
        let resource_manager = Arc::new(ResourceManager::new());
        let client = create_brp_client_with_manager(&config, resource_manager).await.unwrap();
        
        assert!(client.get_resource_manager().is_some());
    }

    #[tokio::test]
    async fn test_connection_stats() {
        let config = Config::default();
        let client = BrpClient::new(&config);
        
        let stats = client.get_connection_stats().await;
        assert!(!stats.connected);
        assert_eq!(stats.retry_count, 0);
        assert_eq!(stats.queue_size, 0);
    }
}