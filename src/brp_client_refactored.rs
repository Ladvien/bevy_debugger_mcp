//! Refactored BRP client that doesn't require external RwLock wrapping
//!
//! This version uses interior mutability appropriately and can be shared
//! as `Arc<BrpClient>` instead of `Arc<RwLock<BrpClient>>`.
//!
//! Transport is HTTP JSON-RPC (Bevy 0.19 BRP); the state is essentially
//! immutable, so interior mutability is limited to the connected flag and
//! the JSON-RPC id counter.

use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tracing::info;

use crate::brp::{builtin_methods, BrpRequest, BrpResponse, DebugCommand};
use crate::brp_command_handler::{CommandHandlerRegistry, CoreBrpHandler, BrpCommandHandler};
use crate::config::Config;
use crate::debug_command_processor::{DebugCommandRequest, DebugCommandRouter};
use crate::error::{Error, Result};
use crate::resource_manager::ResourceManager;

/// Refactored BRP client with interior mutability
///
/// Can be shared as `Arc<BrpClient>` instead of `Arc<RwLock<BrpClient>>`:
/// the HTTP transport is stateless, and all mutable bits are atomics.
pub struct BrpClient {
    config: Config,
    http: reqwest::Client,
    url: String,
    connected: AtomicBool,
    retry_count: AtomicU64,
    request_id: AtomicU64,
    resource_manager: Option<Arc<ResourceManager>>,
    command_registry: Arc<CommandHandlerRegistry>,
    debug_router: Option<Arc<DebugCommandRouter>>,
}

impl std::fmt::Debug for BrpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrpClient")
            .field("config", &self.config)
            .field("url", &self.url)
            .field("connected", &self.connected.load(Ordering::Relaxed))
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
            http: reqwest::Client::new(),
            url: format!(
                "http://{}:{}",
                config.bevy_brp_host, config.bevy_brp_port
            ),
            connected: AtomicBool::new(false),
            retry_count: AtomicU64::new(0),
            request_id: AtomicU64::new(1),
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

    /// Set resource manager - takes `Arc<ResourceManager>` directly
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
        self.connected.load(Ordering::Relaxed)
    }

    /// Get current retry count
    pub async fn retry_count(&self) -> u32 {
        self.retry_count.load(Ordering::Relaxed) as u32
    }

    /// Probe the BRP server with `rpc.discover` until 2xx, with retries.
    pub async fn connect(&self) -> Result<()> {
        let max_retries = self.config.resilience.retry.max_attempts;

        for attempt in 0..=max_retries {
            let probe = BrpRequest {
                method: builtin_methods::RPC_DISCOVER_METHOD.to_string(),
                id: Some(Value::from(self.request_id.fetch_add(1, Ordering::Relaxed))),
                params: None,
            };

            match self.http.post(&self.url).json(&probe).send().await {
                Ok(response) if response.status().is_success() => {
                    info!("Connected to BRP server at {}", self.url);
                    self.connected.store(true, Ordering::Relaxed);
                    self.retry_count.store(0, Ordering::Relaxed);
                    return Ok(());
                }
                Ok(response) => {
                    self.retry_count
                        .store((attempt + 1) as u64, Ordering::Relaxed);
                    if attempt >= max_retries {
                        return Err(Error::Connection(format!(
                            "BRP health check returned HTTP {}",
                            response.status()
                        )));
                    }
                }
                Err(e) => {
                    self.retry_count
                        .store((attempt + 1) as u64, Ordering::Relaxed);
                    if attempt >= max_retries {
                        return Err(Error::Connection(format!("Connection failed: {e}")));
                    }
                }
            }

            let delay = std::time::Duration::from_millis(
                self.config.resilience.retry.initial_delay.as_millis() as u64
                    * (1u64 << attempt.min(5)),
            );
            tokio::time::sleep(delay).await;
        }

        Err(Error::Connection("Max retries exceeded".to_string()))
    }

    /// Disconnect: marks the client disconnected (HTTP is stateless).
    pub async fn disconnect(&self) -> Result<()> {
        self.connected.store(false, Ordering::Relaxed);
        info!("Disconnected from BRP server");
        Ok(())
    }

    /// Send request (with interior mutability)
    pub async fn send_request(&self, request: BrpRequest) -> Result<BrpResponse> {
        let mut request = request;
        if request.id.is_none() {
            request.id = Some(Value::from(self.request_id.fetch_add(1, Ordering::Relaxed)));
        }

        let response = self
            .http
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                self.connected.store(false, Ordering::Relaxed);
                Error::Connection(format!("BRP HTTP request failed: {e}"))
            })?;

        if !response.status().is_success() {
            self.connected.store(false, Ordering::Relaxed);
            return Err(Error::Connection(format!(
                "BRP server returned HTTP {}",
                response.status()
            )));
        }

        self.connected.store(true, Ordering::Relaxed);
        response.json::<BrpResponse>().await.map_err(Error::from)
    }

    /// Send debug command through the debug router
    pub async fn send_debug_command(&self, command: DebugCommand) -> Result<BrpResponse> {
        if let Some(debug_router) = &self.debug_router {
            let request = DebugCommandRequest::new(command, uuid::Uuid::new_v4().to_string(), None);
            debug_router.queue_command(request).await?;
            match debug_router.process_next().await {
                Some(Ok((id, response))) => {
                    let value = serde_json::to_value(response).map_err(Error::Json)?;
                    Ok(BrpResponse::new(Some(Value::from(id.to_string())), Ok(value)))
                }
                Some(Err(e)) => Err(e),
                None => Err(Error::Brp("No response from debug command processor".to_string())),
            }
        } else {
            Err(Error::Brp("Debug commands not supported without a debug router".to_string()))
        }
    }

    /// Get connection statistics
    pub async fn get_connection_stats(&self) -> ConnectionStats {
        ConnectionStats {
            connected: self.connected.load(Ordering::Relaxed),
            retry_count: self.retry_count.load(Ordering::Relaxed) as u32,
            queue_size: 0,
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
